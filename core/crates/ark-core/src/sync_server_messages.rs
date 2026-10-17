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

pub(super) async fn handle_message(ctx: &MessageContext, peer_id: usize, msg: LanSyncMessage) {
    let MessageContext {
        peers,
        storage,
        on_change,
        ..
    } = ctx;
    if ark_sync::protocol::requires_registry_admission(&msg)
        && current_ready_peer(storage.as_ref(), peers, peer_id)
            .await
            .is_none()
    {
        return;
    }
    match msg {
        message @ LanSyncMessage::Hello { .. } => {
            hello::handle_hello(ctx, peer_id, message).await;
        }

        LanSyncMessage::SyncPolicy {
            revision,
            digest,
            active,
            rules,
        } => {
            let Some(ready_peer) = current_ready_peer(storage.as_ref(), peers, peer_id).await
            else {
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
            let update = ark_sync::client_session::SyncPolicyUpdate {
                revision,
                digest,
                active,
                rules,
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
            if let Err(error) = admission::start_sync_after_policy(ctx, peer_id, &ready_peer).await
            {
                eprintln!(
                    "{TAG} policy-gated sync start failed for {}: {error}",
                    ready_peer.device_id
                );
                admission::reject_peer(ctx, peer_id, &tx).await;
            }
        }

        LanSyncMessage::VersionVector {
            vector: remote_vector,
            ..
        } => {
            let (tx, peer_device_id, peer_sync_complete, policy_bound) = {
                let peers_guard = peers.lock().await;
                match peers_guard.get(&peer_id) {
                    Some(p) if p.ready_peer.is_some() => (
                        p.tx.clone(),
                        p.device_id.clone(),
                        p.sync_complete,
                        p.negotiated_protocol.as_ref().is_some_and(|protocol| {
                            protocol.capabilities.contains(
                                &ark_sync::protocol::ProtocolCapability::PolicyBoundDeliveryV1,
                            )
                        }),
                    ),
                    _ => return,
                }
            };

            let mut policy_identity = if policy_bound {
                match storage
                    .accepted_policy_identity_for_peer(&peer_device_id)
                    .await
                {
                    Ok(Some(identity)) => Some(identity),
                    Ok(None) => {
                        eprintln!("{TAG} negotiated peer has no accepted policy identity");
                        return;
                    }
                    Err(error) => {
                        eprintln!("{TAG} accepted policy identity read failed: {error}");
                        return;
                    }
                }
            } else {
                None
            };

            let mut local_vector = match load_version_vector(storage).await {
                Ok(vector) => vector,
                Err(error) => {
                    eprintln!("{TAG} failed to load sync version vector: {error}");
                    return;
                }
            };

            // Hole-tolerant cursors (KOS-302): snapshot the completeness
            // claims BEFORE the first page is read. Computed afterwards
            // they could include a seq allocated mid-pull — the peer would
            // advance its cursor past an entry it never received.
            let read_class =
                ark_application::sync_storage::SyncReadClass::for_sync_state(peer_sync_complete);
            let complete_through = match storage
                .usage_complete_through_for_peer(&peer_device_id, read_class)
                .await
            {
                Ok(through) => through,
                Err(error) => {
                    eprintln!(
                        "{TAG} outbound usage progress read failed for {peer_device_id}: {error}"
                    );
                    return;
                }
            };

            let initial_local_vector = local_vector.clone();
            let mut vector_updated = false;
            let mut offset = 0;
            'snapshot: loop {
                if policy_bound {
                    let current_identity = match storage
                        .accepted_policy_identity_for_peer(&peer_device_id)
                        .await
                    {
                        Ok(Some(identity)) => Some(identity),
                        Ok(None) => {
                            eprintln!("{TAG} accepted policy identity is missing");
                            return;
                        }
                        Err(error) => {
                            eprintln!("{TAG} accepted policy identity read failed: {error}");
                            return;
                        }
                    };
                    if current_identity != policy_identity {
                        policy_identity = current_identity;
                        local_vector.clone_from(&initial_local_vector);
                        vector_updated = false;
                        offset = 0;
                    }
                }
                let mut load_vector = local_vector.clone();
                merge_usage_cursors(&mut load_vector, &remote_vector);
                let entities = match policy_identity.as_ref() {
                    Some(identity) => match storage
                        .load_policy_bound_entities_page_for_peer(
                            &peer_device_id,
                            read_class,
                            &load_vector,
                            offset,
                            SYNC_LOAD_PAGE_SIZE,
                            identity,
                        )
                        .await
                    {
                        Ok(page) => {
                            debug_assert_eq!(page.identity, *identity);
                            page.entities
                        }
                        Err(error) if error.contains("stale_or_unnegotiated_sync_policy") => {
                            continue 'snapshot;
                        }
                        Err(error) => {
                            eprintln!(
                                "{TAG} outbound sync page read failed for {peer_device_id}: {error}"
                            );
                            return;
                        }
                    },
                    None => match storage
                        .load_entities_page_for_peer(
                            &peer_device_id,
                            read_class,
                            &load_vector,
                            offset,
                            SYNC_LOAD_PAGE_SIZE,
                        )
                        .await
                    {
                        Ok(entities) => entities,
                        Err(error) => {
                            eprintln!(
                                "{TAG} outbound sync page read failed for {peer_device_id}: {error}"
                            );
                            return;
                        }
                    },
                };
                if entities.is_empty() {
                    if policy_bound {
                        let current_identity = match storage
                            .accepted_policy_identity_for_peer(&peer_device_id)
                            .await
                        {
                            Ok(identity) => identity,
                            Err(error) => {
                                eprintln!("{TAG} accepted policy identity read failed: {error}");
                                return;
                            }
                        };
                        if current_identity != policy_identity {
                            policy_identity = current_identity;
                            local_vector.clone_from(&initial_local_vector);
                            vector_updated = false;
                            offset = 0;
                            continue 'snapshot;
                        }
                    }
                    break;
                }
                offset += entities.len();
                let mut to_send = Vec::new();
                for entity in entities {
                    if !is_usage_entity(&entity) && !local_vector.contains_key(&entity.id) {
                        local_vector.insert(entity.id.clone(), entity.hlc.clone());
                        vector_updated = true;
                    }
                    if should_send_entity(&remote_vector, &entity) {
                        to_send.push(entity);
                    }
                }
                if policy_bound {
                    let current_identity = match storage
                        .accepted_policy_identity_for_peer(&peer_device_id)
                        .await
                    {
                        Ok(identity) => identity,
                        Err(error) => {
                            eprintln!("{TAG} accepted policy identity read failed: {error}");
                            return;
                        }
                    };
                    if current_identity != policy_identity {
                        policy_identity = current_identity;
                        local_vector.clone_from(&initial_local_vector);
                        vector_updated = false;
                        offset = 0;
                        continue 'snapshot;
                    }
                }
                for batch in split_into_batches(&to_send) {
                    send_msg(
                        &tx,
                        &LanSyncMessage::SyncChanges {
                            policy_identity: policy_identity.clone(),
                            batch_id: generate_id(),
                            entities: batch,
                            is_last: false,
                            origin_device_id: None,
                            usage_complete_through: None,
                        },
                    );
                }
            }
            if vector_updated {
                if let Err(error) = save_version_vector(storage, &local_vector).await {
                    eprintln!("{TAG} failed to save sync version vector: {error}");
                    return;
                }
            }
            let terminal_batch_id = generate_id();
            {
                let mut peers_guard = peers.lock().await;
                if let Some(peer) = peers_guard.get_mut(&peer_id) {
                    peer.pending_initial_snapshot_ack = Some(terminal_batch_id.clone());
                    peer.pending_initial_snapshot_policy_identity = policy_identity.clone();
                }
            }
            send_msg(
                &tx,
                &LanSyncMessage::SyncChanges {
                    policy_identity,
                    batch_id: terminal_batch_id,
                    entities: vec![],
                    is_last: true,
                    origin_device_id: None,
                    usage_complete_through: (!complete_through.is_empty())
                        .then_some(complete_through),
                },
            );
        }

        LanSyncMessage::SyncChanges {
            batch_id,
            entities,
            is_last,
            policy_identity,
            usage_complete_through,
            ..
        } => {
            let (tx, peer_device_id, policy_bound) = {
                let peers_guard = peers.lock().await;
                match peers_guard.get(&peer_id) {
                    Some(p) if p.ready_peer.is_some() => (
                        p.tx.clone(),
                        p.device_id.clone(),
                        p.negotiated_protocol.as_ref().is_some_and(|protocol| {
                            protocol.capabilities.contains(
                                &ark_sync::protocol::ProtocolCapability::PolicyBoundDeliveryV1,
                            )
                        }),
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
            let expected_identity = policy_bound
                .then(|| ark_sync::protocol::sync_policy_identity(local_profile.as_ref()));
            if (policy_bound && policy_identity != expected_identity)
                || (!policy_bound && policy_identity.is_some())
            {
                eprintln!("{TAG} rejected snapshot page for stale or unnegotiated policy");
                return;
            }

            let mut local_vector = match load_version_vector(storage).await {
                Ok(vector) => vector,
                Err(error) => {
                    eprintln!("{TAG} failed to load sync version vector: {error}");
                    return;
                }
            };
            let mut accepted = 0;
            let mut vector_updated = false;

            for entity in &entities {
                let should_apply = is_usage_entity(entity)
                    || local_vector
                        .get(&entity.id)
                        .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));

                if should_apply {
                    match storage
                        .apply_authenticated_entity_with_policy(
                            entity,
                            &peer_device_id,
                            &crate::db::entity_inbox_message_id(
                                &peer_device_id,
                                &entity.entity_type,
                                &entity.id,
                                &entity.hlc,
                            ),
                            &local_device_id,
                            policy_identity.clone(),
                        )
                        .await
                    {
                        Ok(ark_application::sync_storage::InboundApplyOutcome::Applied)
                        | Ok(ark_application::sync_storage::InboundApplyOutcome::Stale) => {
                            vector_updated |= observe_non_usage_entity(&mut local_vector, entity);
                            accepted += 1;

                            if let Some(handler) = on_change.lock().await.as_ref() {
                                handler(entity.clone());
                            }

                            // Broadcast to other peers
                            broadcast_to_others(storage, peers, Some(&peer_device_id)).await;
                        }
                        Ok(ark_application::sync_storage::InboundApplyOutcome::Pending) => {}
                        Err(e) => {
                            eprintln!(
                                "{TAG} Failed to apply sync entity {}:{}: {e}",
                                entity.entity_type, entity.id
                            );
                        }
                    }
                }
            }

            let persist_result = match policy_identity.as_ref() {
                Some(identity) => {
                    persist_pull_vector_with_policy(
                        storage,
                        &local_device_id,
                        identity,
                        &mut local_vector,
                        vector_updated,
                        is_last,
                        usage_complete_through.as_ref(),
                    )
                    .await
                }
                None => {
                    persist_pull_vector(
                        storage,
                        &mut local_vector,
                        vector_updated,
                        is_last,
                        usage_complete_through.as_ref(),
                    )
                    .await
                }
            };
            if let Err(error) = persist_result {
                eprintln!("{TAG} failed to persist pulled sync progress: {error}");
                return;
            }
            send_msg(
                &tx,
                &LanSyncMessage::SyncAck {
                    batch_id,
                    accepted,
                    policy_identity,
                },
            );

            if is_last {
                eprintln!("{TAG} Received all sync batches from peer {peer_id}");
            }
        }

        message @ (LanSyncMessage::RegistryAdmissionStart { .. }
        | LanSyncMessage::RegistryAdmissionProof { .. }
        | LanSyncMessage::RegistryAdmissionConfirm { .. }
        | LanSyncMessage::RegistrySyncStart { .. }
        | LanSyncMessage::RegistryEventsPage { .. }
        | LanSyncMessage::RegistrySyncReady { .. }
        | LanSyncMessage::RegistryEventUpdate { .. }) => {
            admission::handle_registry_admission(ctx, peer_id, message).await;
        }

        message @ (LanSyncMessage::SyncAck { .. }
        | LanSyncMessage::SignedIntegrationFrame { .. }
        | LanSyncMessage::SignedIntegrationAck { .. }
        | LanSyncMessage::LiveChange { .. }
        | LanSyncMessage::LiveAck { .. }
        | LanSyncMessage::LiveReceiptAck { .. }
        | LanSyncMessage::TaskMergeBatch { .. }
        | LanSyncMessage::TaskMergeBatchAck { .. }
        | LanSyncMessage::ProtocolRejected { .. }
        | LanSyncMessage::PeerList { .. }
        | LanSyncMessage::Ping { .. }
        | LanSyncMessage::Pong { .. }
        | LanSyncMessage::PairingRejected { .. }
        | LanSyncMessage::PairingAccepted { .. }) => {
            handle_live_message(ctx, peer_id, message).await;
        }
    }
}
