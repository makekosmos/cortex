use super::*;

#[path = "sync_server_messages/live_messages.rs"]
mod live_messages;
use live_messages::handle_live_message;
#[path = "sync_server_messages/admission.rs"]
mod admission;
#[path = "sync_server_messages/hello.rs"]
mod hello;
#[cfg(test)]
#[path = "sync_server_messages/live_message_tests.rs"]
mod live_message_tests;

pub(super) async fn handle_message(
    ctx: &MessageContext,
    peer_id: usize,
    session: &mut Option<ark_sync::server_session::DirectServerSession>,
    msg: LanSyncMessage,
) {
    let MessageContext {
        peers,
        storage,
        on_change,
        ..
    } = ctx;
    let ready_peer = if ark_sync::protocol::requires_registry_admission(&msg) {
        let Some(ready_peer) = current_ready_peer(storage.as_ref(), peers, peer_id).await else {
            return;
        };
        Some(ready_peer)
    } else {
        None
    };
    let Some(msg) =
        hello::route_session_message(ctx, peer_id, session, msg, ready_peer.as_ref()).await
    else {
        return;
    };
    let Some(session) = session.as_mut() else {
        return;
    };
    match msg {
        ark_sync::server_session::DirectServerMessage::SyncPolicy(update) => {
            let Some(ready_peer) = ready_peer.as_ref() else {
                return;
            };
            let (tx, protocol) = {
                let peers = peers.lock().await;
                let Some(peer) = peers.get(&peer_id) else {
                    return;
                };
                (peer.tx.clone(), peer.negotiated_protocol.clone())
            };
            let Some(protocol) = protocol else {
                admission::reject_peer(ctx, peer_id, &tx).await;
                return;
            };
            if let Err(error) = ark_sync::client_session::accept_sync_policy(
                storage.as_ref(),
                &ready_peer.device_id,
                &protocol,
                true,
                update,
            )
            .await
            {
                eprintln!(
                    "{TAG} invalid or unpersisted sync policy from {}: {error}",
                    ready_peer.device_id
                );
                admission::reject_peer(ctx, peer_id, &tx).await;
                return;
            }
            if let Err(error) =
                admission::start_sync_after_policy(ctx, peer_id, session, ready_peer).await
            {
                eprintln!(
                    "{TAG} policy-gated sync start failed for {}: {error}",
                    ready_peer.device_id
                );
                admission::reject_peer(ctx, peer_id, &tx).await;
            }
        }

        ark_sync::server_session::DirectServerMessage::VersionVector(remote_vector) => {
            let (tx, peer_device_id, peer_sync_complete, policy_bound) = {
                let peers_guard = peers.lock().await;
                match peers_guard.get(&peer_id) {
                    Some(peer) if peer.ready_peer.is_some() => (
                        peer.tx.clone(),
                        peer.device_id.clone(),
                        peer.outbound_snapshot.has_been_acknowledged(),
                        peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                            protocol.capabilities.contains(
                                &ark_sync::protocol::ProtocolCapability::PolicyBoundDeliveryV1,
                            )
                        }),
                    ),
                    _ => return,
                }
            };
            let mut snapshot = match ark_sync::outbound_snapshot::OutboundSnapshot::begin(
                storage.as_ref(),
                peer_device_id.clone(),
                remote_vector,
                peer_sync_complete,
                policy_bound,
            )
            .await
            {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    eprintln!("{TAG} outbound snapshot setup failed for {peer_device_id}: {error}");
                    return;
                }
            };

            if let Err(error) = snapshot
                .drive(storage.as_ref(), |emission| {
                    let peers = Arc::clone(peers);
                    let tx = tx.clone();
                    async move {
                        match emission {
                            ark_sync::outbound_snapshot::SnapshotEmission::Page(messages) => {
                                for message in messages {
                                    send_msg(&tx, &message).await;
                                }
                            }
                            ark_sync::outbound_snapshot::SnapshotEmission::Terminal {
                                ack,
                                message,
                            } => {
                                let ticket_stored = {
                                    let mut peers_guard = peers.lock().await;
                                    peers_guard.get_mut(&peer_id).is_some_and(|peer| {
                                        if peer.ready_peer.is_none() {
                                            return false;
                                        }
                                        peer.outbound_snapshot.record_sent(ack);
                                        true
                                    })
                                };
                                if ticket_stored {
                                    send_msg(&tx, &message).await;
                                }
                            }
                        }
                        Ok(())
                    }
                })
                .await
            {
                eprintln!("{TAG} outbound snapshot failed for {peer_device_id}: {error}");
            }
        }

        ark_sync::server_session::DirectServerMessage::SyncChanges(
            ark_sync::server_session::DirectServerSyncChanges {
                batch_id,
                entities,
                is_last,
                policy_identity,
                usage_complete_through,
            },
        ) => {
            let (tx, peer_device_id, protocol) = {
                let peers_guard = peers.lock().await;
                match peers_guard.get(&peer_id) {
                    Some(peer) if peer.ready_peer.is_some() => (
                        peer.tx.clone(),
                        peer.device_id.clone(),
                        peer.negotiated_protocol.clone(),
                    ),
                    _ => return,
                }
            };

            let local_device_id = ctx.device_id.read().await.clone();
            let local_profile = match storage.load_local_sync_profile(&local_device_id).await {
                Ok(profile) => profile,
                Err(error) => {
                    eprintln!("{TAG} local policy identity read failed: {error}");
                    return;
                }
            };
            let mut snapshot = match ark_sync::inbound_snapshot::InboundSnapshotBatch::begin(
                storage.as_ref(),
                local_device_id.clone(),
                peer_device_id.clone(),
                protocol.as_ref(),
                local_profile.as_ref(),
                policy_identity.clone(),
            )
            .await
            {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    eprintln!("{TAG} rejected snapshot page: {error}");
                    return;
                }
            };

            for entity in &entities {
                let message_id = crate::db::entity_inbox_message_id(
                    &peer_device_id,
                    &entity.entity_type,
                    &entity.id,
                    &entity.hlc,
                );
                match snapshot
                    .apply_entity(storage.as_ref(), entity, &message_id)
                    .await
                {
                    Ok(ark_sync::inbound_snapshot::SnapshotEntityResult::Accepted) => {
                        if let Some(handler) = on_change.lock().await.as_ref() {
                            handler(entity.clone());
                        }
                        broadcast_to_others(storage, peers, Some(&peer_device_id)).await;
                    }
                    Ok(
                        ark_sync::inbound_snapshot::SnapshotEntityResult::Skipped
                        | ark_sync::inbound_snapshot::SnapshotEntityResult::Pending,
                    ) => {}
                    Err(error) => {
                        eprintln!(
                            "{TAG} Failed to apply sync entity {}:{}: {error}",
                            entity.entity_type, entity.id
                        );
                    }
                }
            }

            if let Err(error) = snapshot
                .finish(storage.as_ref(), is_last, usage_complete_through.as_ref())
                .await
            {
                eprintln!("{TAG} failed to persist pulled sync progress: {error}");
                return;
            }
            send_msg(
                &tx,
                &LanSyncMessage::SyncAck {
                    batch_id,
                    accepted: snapshot.accepted_count(),
                    policy_identity,
                },
            )
            .await;

            if is_last {
                eprintln!("{TAG} Received all sync batches from peer {peer_id}");
            }
        }

        ark_sync::server_session::DirectServerMessage::RegistryAdmission(message) => {
            admission::handle_registry_admission(ctx, peer_id, session, message).await;
        }

        ark_sync::server_session::DirectServerMessage::Live(message) => {
            handle_live_message(ctx, peer_id, message).await;
        }
    }
}

pub(super) struct ConnectedSessionHandler<'a> {
    pub(super) ctx: &'a MessageContext,
    pub(super) peer_id: usize,
    pub(super) session: Option<ark_sync::server_session::DirectServerSession>,
}

#[async_trait::async_trait]
impl ark_sync::server_session::DirectServerSessionHandler for ConnectedSessionHandler<'_> {
    async fn on_message(&mut self, message: Box<LanSyncMessage>) {
        handle_message(self.ctx, self.peer_id, &mut self.session, *message).await;
    }

    async fn on_malformed_message(&mut self) {
        eprintln!("{TAG} ignoring malformed peer sync message");
    }
}
