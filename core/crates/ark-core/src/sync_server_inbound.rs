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
    let (tx, authenticated) = {
        let peers_guard = peers.lock().await;
        match peers_guard.get(&peer_id) {
            Some(p) if p.authenticated => (p.tx.clone(), true),
            _ => return,
        }
    };
    if !authenticated {
        return;
    }

    let mut local_vector = load_version_vector(storage).await;
    let should_apply = is_usage_entity(&entity)
        || local_vector
            .get(&entity.id)
            .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));

    if should_apply {
        match storage.apply_entity(&entity).await {
            Ok(()) => {
                if observe_non_usage_entity(&mut local_vector, &entity) {
                    merge_usage_cursors(&mut local_vector, &load_version_vector(storage).await);
                    save_version_vector(storage, &local_vector).await;
                }

                if let Some(handler) = on_change.lock().await.as_ref() {
                    handler(entity.clone());
                }

                let peer_device_id = {
                    let peers_guard = peers.lock().await;
                    peers_guard
                        .get(&peer_id)
                        .map(|p| p.device_id.clone())
                        .unwrap_or_default()
                };
                broadcast_to_others(peers, &entity, Some(&peer_device_id)).await;
            }
            Err(e) => {
                eprintln!(
                    "{TAG} Failed to apply live sync entity {}:{}: {e}",
                    entity.entity_type, entity.id
                );
            }
        }
    }

    send_msg(&tx, &LanSyncMessage::LiveAck { change_id });
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
    let removed = load_removed_peer_ids(storage).await;
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
    save_known_peers(storage, &known).await;

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
