use super::*;

#[path = "sync_server_messages/live_messages.rs"]
mod live_messages;
use live_messages::handle_live_message;

pub(super) async fn handle_message(ctx: &MessageContext, peer_id: usize, msg: LanSyncMessage) {
    let MessageContext {
        peers,
        storage,
        space_id,
        device_id,
        device_name,
        own_addresses,
        auth_secret,
        known_peer_records,
        on_change,
        on_peer_connect,
        ..
    } = ctx;
    match msg {
        LanSyncMessage::Hello {
            protocol_version,
            device_id: peer_device_id,
            device_name: peer_device_name,
            space_id: peer_space_id,
            addresses,
            auth_nonce,
            auth_hmac,
            platform: peer_platform,
            app_version: peer_app_version,
        } => {
            let repeated = peers
                .lock()
                .await
                .get(&peer_id)
                .is_some_and(|peer| peer.authenticated);
            if repeated {
                eprintln!("{TAG} Rejecting repeated hello on an authenticated connection");
                reject_hello(peers, peer_id).await;
                return;
            }
            if protocol_version != PROTOCOL_VERSION {
                eprintln!(
                    "{TAG} Protocol version mismatch: {protocol_version} vs {PROTOCOL_VERSION}"
                );
                reject_hello(peers, peer_id).await;
                return;
            }

            let my_device_id = device_id.read().await.clone();
            let my_device_name = device_name.read().await.clone();
            let my_space_id = space_id.read().await.clone();
            let my_addresses = own_addresses.read().await.clone();

            if peer_space_id != my_space_id {
                eprintln!("{TAG} Rejecting peer hello from a different space");
                reject_hello(peers, peer_id).await;
                return;
            }

            if let Some(secret) = auth_secret.read().await.clone() {
                let valid = match (auth_nonce.as_deref(), auth_hmac.as_deref()) {
                    (Some(nonce), Some(hmac)) => {
                        verify_hello_auth_hmac(&secret, &my_space_id, &peer_device_id, nonce, hmac)
                    }
                    _ => false,
                };
                if !valid {
                    eprintln!("{TAG} Rejecting peer hello with invalid HMAC");
                    reject_hello(peers, peer_id).await;
                    return;
                }
            }

            // Reject self-connect: a client claiming our own device_id is us
            // (stale phantom peer in storage or address looping back to us).
            if !peer_device_id.is_empty() && peer_device_id == my_device_id {
                eprintln!(
                    "{TAG} Rejecting self-connect: client claims our device_id {peer_device_id}"
                );
                reject_hello(peers, peer_id).await;
                return;
            }
            let removed = match load_removed_peer_ids(storage).await {
                Ok(removed) => removed,
                Err(error) => {
                    eprintln!("{TAG} failed to load blocked peer list: {error}");
                    reject_hello(peers, peer_id).await;
                    return;
                }
            };
            if !peer_device_id.is_empty() && removed.iter().any(|id| id == &peer_device_id) {
                eprintln!("{TAG} Rejecting blocked peer: {peer_device_id}");
                reject_hello(peers, peer_id).await;
                return;
            }

            let generation = storage.selective_sync_policy_generation();
            if let Err(error) = storage
                .begin_outbound_policy(&peer_device_id, generation)
                .await
            {
                eprintln!("{TAG} outbound snapshot baseline failed: {error}");
                reject_hello(peers, peer_id).await;
                return;
            }

            // Evict any prior authenticated sessions from the same device — a
            // new hello means a fresh connection, and the old session is stale.
            // Mark stale sessions unauthenticated so their close handler
            // doesn't fire an extra on_peer_disconnected.
            {
                let mut peers_guard = peers.lock().await;
                let stale_ids: Vec<usize> = peers_guard
                    .iter()
                    .filter_map(|(id, p)| {
                        if *id != peer_id && p.authenticated && p.device_id == peer_device_id {
                            Some(*id)
                        } else {
                            None
                        }
                    })
                    .collect();
                for stale_id in stale_ids {
                    if let Some(stale) = peers_guard.get_mut(&stale_id) {
                        eprintln!(
                            "{TAG} Evicting stale session for {} (superseded by new hello)",
                            stale.device_name
                        );
                        stale.authenticated = false;
                        let _ = stale.tx.send(Message::Close(None));
                    }
                    peers_guard.remove(&stale_id);
                }
            }

            let tx = {
                let mut peers_guard = peers.lock().await;
                if let Some(peer) = peers_guard.get_mut(&peer_id) {
                    peer.device_id = peer_device_id.clone();
                    peer.device_name = peer_device_name.clone();
                    peer.addresses = addresses.clone().unwrap_or_default();
                    peer.platform = peer_platform.clone();
                    peer.app_version = peer_app_version.clone();
                    peer.authenticated = true;
                    peer.tx.clone()
                } else {
                    return;
                }
            };

            eprintln!("{TAG} Peer authenticated: {peer_device_name}");

            // Update known peer record — skip if it's a self-reference
            // (device_id match or all-self address list).
            {
                let peer_addresses = addresses.clone().unwrap_or_default();
                let is_self = peer_device_id == my_device_id
                    || (!peer_addresses.is_empty()
                        && peer_addresses.iter().all(|a| my_addresses.contains(a)));
                if !is_self {
                    let new_record = PeerRecord {
                        device_id: peer_device_id.clone(),
                        device_name: peer_device_name,
                        addresses: peer_addresses,
                        last_seen: chrono::Utc::now()
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                        last_address: None,
                        platform: peer_platform,
                        app_version: peer_app_version,
                    };
                    let removed = match load_removed_peer_ids(storage).await {
                        Ok(removed) => removed,
                        Err(error) => {
                            eprintln!("{TAG} failed to load blocked peer list: {error}");
                            reject_hello(peers, peer_id).await;
                            return;
                        }
                    };
                    if !removed.iter().any(|id| id == &new_record.device_id) {
                        let mut known = known_peer_records.lock().await;
                        *known = merge_peer_records(&known, &[new_record]);
                        if let Err(error) = save_known_peers(storage, &known).await {
                            eprintln!("{TAG} failed to persist known peers: {error}");
                            return;
                        }
                    }
                }
            }

            // Reply hello
            let (reply_auth_nonce, reply_auth_hmac) = match auth_secret.read().await.clone() {
                Some(secret) => {
                    let nonce = generate_auth_nonce();
                    let hmac =
                        compute_hello_auth_hmac(&secret, &my_space_id, &my_device_id, &nonce);
                    (Some(nonce), Some(hmac))
                }
                None => (None, None),
            };
            send_msg(
                &tx,
                &LanSyncMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    device_id: my_device_id,
                    device_name: my_device_name,
                    space_id: my_space_id,
                    addresses: Some(my_addresses),
                    auth_nonce: reply_auth_nonce,
                    auth_hmac: reply_auth_hmac,
                    platform: Some(crate::host::local_platform()),
                    app_version: crate::host::app_version(),
                },
            );

            if let Some(handler) = on_peer_connect.lock().await.as_ref() {
                handler(peer_device_id);
            }

            // Send version vector
            let vector = match load_version_vector(storage).await {
                Ok(vector) => vector,
                Err(error) => {
                    eprintln!("{TAG} failed to load sync version vector: {error}");
                    return;
                }
            };
            send_msg(
                &tx,
                &LanSyncMessage::VersionVector {
                    vector,
                    origin_device_id: None,
                },
            );

            // Send peer list after short delay
            let known = known_peer_records.lock().await.clone();
            let tx_delayed = tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                send_msg(&tx_delayed, &LanSyncMessage::PeerList { peers: known });
            });
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
        | LanSyncMessage::PeerList { .. }
        | LanSyncMessage::Ping { .. }
        | LanSyncMessage::Pong { .. }
        | LanSyncMessage::PairingRejected { .. }) => {
            handle_live_message(ctx, peer_id, message).await;
        }
    }
}
