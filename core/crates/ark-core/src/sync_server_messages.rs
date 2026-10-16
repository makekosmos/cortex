use super::*;

#[path = "sync_server_messages/live_messages.rs"]
mod live_messages;
use live_messages::handle_live_message;
#[path = "sync_server_messages/hello.rs"]
mod hello;

pub(super) async fn handle_message(ctx: &MessageContext, peer_id: usize, msg: LanSyncMessage) {
    let MessageContext {
        peers,
        storage,
        on_change,
        ..
    } = ctx;
    match msg {
        message @ LanSyncMessage::Hello { .. } => {
            hello::handle_hello(ctx, peer_id, message).await;
        }

        LanSyncMessage::VersionVector {
            vector: remote_vector,
            ..
        } => {
            let (tx, peer_device_id, peer_sync_complete) = {
                let peers_guard = peers.lock().await;
                match peers_guard.get(&peer_id) {
                    Some(p) if p.authenticated => {
                        (p.tx.clone(), p.device_id.clone(), p.sync_complete)
                    }
                    _ => return,
                }
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
                crate::application::sync_storage::SyncReadClass::for_sync_state(peer_sync_complete);
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

            let mut vector_updated = false;
            let mut offset = 0;
            loop {
                let mut load_vector = local_vector.clone();
                merge_usage_cursors(&mut load_vector, &remote_vector);
                let entities = match storage
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
                };
                if entities.is_empty() {
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
                for batch in split_into_batches(&to_send) {
                    send_msg(
                        &tx,
                        &LanSyncMessage::SyncChanges {
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
                }
            }
            send_msg(
                &tx,
                &LanSyncMessage::SyncChanges {
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
            usage_complete_through,
            ..
        } => {
            let (tx, peer_device_id) = {
                let peers_guard = peers.lock().await;
                match peers_guard.get(&peer_id) {
                    Some(p) if p.authenticated => (p.tx.clone(), p.device_id.clone()),
                    _ => return,
                }
            };

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
                        .apply_authenticated_entity(entity, &peer_device_id)
                        .await
                    {
                        Ok(()) => {
                            vector_updated |= observe_non_usage_entity(&mut local_vector, entity);
                            accepted += 1;

                            if let Some(handler) = on_change.lock().await.as_ref() {
                                handler(entity.clone());
                            }

                            // Broadcast to other peers
                            broadcast_to_others(storage, peers, Some(&peer_device_id)).await;
                        }
                        Err(e) => {
                            eprintln!(
                                "{TAG} Failed to apply sync entity {}:{}: {e}",
                                entity.entity_type, entity.id
                            );
                        }
                    }
                }
            }

            if let Err(error) = persist_pull_vector(
                storage,
                &mut local_vector,
                vector_updated,
                is_last,
                usage_complete_through.as_ref(),
            )
            .await
            {
                eprintln!("{TAG} failed to persist pulled sync progress: {error}");
                return;
            }
            send_msg(&tx, &LanSyncMessage::SyncAck { batch_id, accepted });

            if is_last {
                eprintln!("{TAG} Received all sync batches from peer {peer_id}");
            }
        }

        message @ (LanSyncMessage::SyncAck { .. }
        | LanSyncMessage::SignedIntegrationFrame { .. }
        | LanSyncMessage::SignedIntegrationAck { .. }
        | LanSyncMessage::LiveChange { .. }
        | LanSyncMessage::LiveAck { .. }
        | LanSyncMessage::LiveReceiptAck { .. }
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
