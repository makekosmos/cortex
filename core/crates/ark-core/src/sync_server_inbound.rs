use super::*;

// Inbound replication handlers moved out of `handle_message`'s match — the
// arm bodies are unchanged, they just run as functions taking the shared
// MessageContext.

pub(super) async fn handle_signed_integration_frame(
    ctx: &MessageContext,
    peer_id: usize,
    frame: SignedSyncEnvelope,
) {
    let MessageContext {
        peers,
        storage,
        space_id,
        device_id,
        ..
    } = ctx;
    let (tx, authenticated_peer_id) = {
        let peers_guard = peers.lock().await;
        match peers_guard.get(&peer_id) {
            Some(peer) if peer.authenticated => (peer.tx.clone(), peer.device_id.clone()),
            _ => return,
        }
    };
    let expected_space_id = space_id.read().await.clone();
    let expected_recipient_node_id = device_id.read().await.clone();
    if frame.space_id != expected_space_id
        || frame.origin_node_id != authenticated_peer_id
        || frame.recipient_node_id != expected_recipient_node_id
    {
        return;
    }
    if storage
        .apply_signed_integration_frame(
            &frame,
            &expected_space_id,
            &authenticated_peer_id,
            &expected_recipient_node_id,
        )
        .await
        .is_ok()
    {
        send_msg(
            &tx,
            &LanSyncMessage::SignedIntegrationAck {
                message_id: frame.message_id,
                accepted: true,
            },
        );
    }
}

pub(super) async fn handle_live_change(
    ctx: &MessageContext,
    peer_id: usize,
    change_id: String,
    entity: SyncEntity,
) {
    let MessageContext {
        peers,
        storage,
        on_change,
        ..
    } = ctx;
    let (tx, authenticated_peer_id, receipt_ack) = {
        let peers_guard = peers.lock().await;
        match peers_guard.get(&peer_id) {
            Some(p) if p.authenticated => (
                p.tx.clone(),
                p.device_id.clone(),
                p.negotiated_protocol.as_ref().is_some_and(|protocol| {
                    protocol
                        .capabilities
                        .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1)
                }),
            ),
            _ => return,
        }
    };
    if authenticated_peer_id.is_empty() {
        return;
    }

    if receipt_ack {
        match storage
            .receive_authenticated_entity_with_id(&entity, &authenticated_peer_id, &change_id)
            .await
        {
            Ok(true) => send_msg(
                &tx,
                &LanSyncMessage::LiveReceiptAck {
                    change_id: change_id.clone(),
                },
            ),
            Ok(false) => {}
            Err(error) => {
                eprintln!("{TAG} failed to persist live receipt: {error}");
                return;
            }
        }
    }

    let mut local_vector = match load_version_vector(storage).await {
        Ok(vector) => vector,
        Err(error) => {
            eprintln!("{TAG} failed to load sync version vector: {error}");
            return;
        }
    };
    let should_apply = is_usage_entity(&entity)
        || local_vector
            .get(&entity.id)
            .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));

    match storage
        .apply_authenticated_entity_with_id(&entity, &authenticated_peer_id, &change_id)
        .await
    {
        Ok(crate::application::sync_storage::InboundApplyOutcome::Applied) => {
            if should_apply && observe_non_usage_entity(&mut local_vector, &entity) {
                let stored_vector = match load_version_vector(storage).await {
                    Ok(vector) => vector,
                    Err(error) => {
                        eprintln!("{TAG} failed to reload sync progress: {error}");
                        return;
                    }
                };
                merge_usage_cursors(&mut local_vector, &stored_vector);
                if let Err(error) = save_version_vector(storage, &local_vector).await {
                    eprintln!("{TAG} failed to save sync progress: {error}");
                    return;
                }
            }

            if should_apply {
                if let Some(handler) = on_change.lock().await.as_ref() {
                    handler(entity.clone());
                }
            }

            let peer_device_id = {
                let peers_guard = peers.lock().await;
                peers_guard
                    .get(&peer_id)
                    .map(|p| p.device_id.clone())
                    .unwrap_or_default()
            };
            broadcast_to_others(storage, peers, Some(&peer_device_id)).await;
            send_msg(&tx, &LanSyncMessage::LiveAck { change_id });
        }
        Ok(crate::application::sync_storage::InboundApplyOutcome::Stale) => {
            send_msg(&tx, &LanSyncMessage::LiveAck { change_id });
        }
        Ok(crate::application::sync_storage::InboundApplyOutcome::Pending) => {}
        Err(e) => {
            eprintln!(
                "{TAG} Failed to apply live sync entity {}:{}: {e}",
                entity.entity_type, entity.id
            );
        }
    }
}

pub(super) async fn handle_peer_list(
    ctx: &MessageContext,
    peer_id: usize,
    incoming_peers: Vec<PeerRecord>,
) {
    let MessageContext {
        peers,
        storage,
        device_id,
        own_addresses,
        known_peer_records,
        on_new_peer_discovered,
        ..
    } = ctx;
    let authenticated = {
        let peers_guard = peers.lock().await;
        peers_guard
            .get(&peer_id)
            .map(|p| p.authenticated)
            .unwrap_or(false)
    };
    if !authenticated {
        return;
    }

    let my_device_id = device_id.read().await.clone();
    let my_addresses = own_addresses.read().await.clone();
    // Blocked peers must not re-enter through gossiped peer lists —
    // otherwise a removed device resurrects in known_peers and gets
    // re-gossiped to the rest of the mesh.
    let removed = match load_removed_peer_ids(storage).await {
        Ok(removed) => removed,
        Err(error) => {
            eprintln!("{TAG} failed to load blocked peer list: {error}");
            return;
        }
    };
    // Filter self-references: our own device_id, or any record whose
    // addresses are all ours (stale phantom from prior runs).
    let filtered: Vec<PeerRecord> = incoming_peers
        .into_iter()
        .filter(|p| {
            if p.device_id == my_device_id {
                return false;
            }
            if removed.iter().any(|id| id == &p.device_id) {
                return false;
            }
            if !p.addresses.is_empty() && p.addresses.iter().all(|a| my_addresses.contains(a)) {
                return false;
            }
            true
        })
        .collect();

    let mut known = known_peer_records.lock().await;
    let before_count = known.len();
    *known = merge_peer_records(&known, &filtered);
    if let Err(error) = save_known_peers(storage, &known).await {
        eprintln!("{TAG} failed to persist known peers: {error}");
        return;
    }

    // Notify about new peers
    if let Some(handler) = on_new_peer_discovered.lock().await.as_ref() {
        for peer_rec in &*known {
            if peer_rec.device_id != my_device_id {
                handler(peer_rec.clone());
            }
        }
    }

    if known.len() > before_count {
        eprintln!(
            "{TAG} Peer list updated: {before_count} -> {} known peers",
            known.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_server::sync_server_tests::tests::MemBackend;

    #[tokio::test]
    async fn failed_live_entity_apply_does_not_ack() {
        let backend = Arc::new(MemBackend::new());
        backend.fail_authenticated_apply("injected write failure");
        let storage = backend as Arc<dyn SyncRuntimeStorage>;
        let server = SyncServer::new(storage.clone());
        let (tx, mut rx) = mpsc::unbounded_channel();
        server.peers.lock().await.insert(
            7,
            PeerState {
                device_id: "peer".into(),
                device_name: "Peer".into(),
                addresses: Vec::new(),
                platform: None,
                app_version: None,
                authenticated: true,
                sync_complete: true,
                negotiated_protocol: None,
                in_flight_feed_position: None,
                pending_initial_snapshot_ack: None,
                tx,
            },
        );
        let ctx = MessageContext {
            peers: server.peers.clone(),
            storage,
            space_id: server.space_id.clone(),
            device_id: server.device_id.clone(),
            device_name: server.device_name.clone(),
            own_addresses: server.own_addresses.clone(),
            auth_secret: server.auth_secret.clone(),
            known_peer_records: server.known_peer_records.clone(),
            on_change: server.on_change.clone(),
            on_peer_connect: server.on_peer_connect.clone(),
            on_new_peer_discovered: server.on_new_peer_discovered.clone(),
        };

        sync_server_inbound::handle_live_change(
            &ctx,
            7,
            "change-1".into(),
            SyncEntity {
                entity_type: "object".into(),
                id: "object-1".into(),
                data: serde_json::json!({"name": "object"})
                    .as_object()
                    .unwrap()
                    .clone(),
                hlc: "2026-10-16T00:00:00.000Z:000001:peer".into(),
                deleted: None,
                origin_device_id: None,
                origin_seq: None,
            },
        )
        .await;

        assert!(
            rx.try_recv().is_err(),
            "failed entity apply must not emit LiveAck"
        );
    }
}
