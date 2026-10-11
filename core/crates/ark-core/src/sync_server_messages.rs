use super::*;

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
            let removed = load_removed_peer_ids(storage).await;
            if !peer_device_id.is_empty() && removed.iter().any(|id| id == &peer_device_id) {
                eprintln!("{TAG} Rejecting blocked peer: {peer_device_id}");
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
                    let removed = load_removed_peer_ids(storage).await;
                    if !removed.iter().any(|id| id == &new_record.device_id) {
                        let mut known = known_peer_records.lock().await;
                        *known = merge_peer_records(&known, &[new_record]);
                        save_known_peers(storage, &known).await;
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
            let vector = load_version_vector(storage).await;
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

            // Hole-tolerant cursors (KOS-302): snapshot the completeness
            // claims BEFORE the first page is read. Computed afterwards
            // they could include a seq allocated mid-pull — the peer would
            // advance its cursor past an entry it never received.
            let complete_through = storage.usage_complete_through().await;

            let mut vector_updated = false;
            let mut offset = 0;
            loop {
                let mut load_vector = local_vector.clone();
                merge_usage_cursors(&mut load_vector, &remote_vector);
                let entities = storage
                    .load_entities_page(&load_vector, offset, SYNC_LOAD_PAGE_SIZE)
                    .await;
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
                save_version_vector(storage, &local_vector).await;
            }
            send_msg(
                &tx,
                &LanSyncMessage::SyncChanges {
                    batch_id: generate_id(),
                    entities: vec![],
                    is_last: true,
                    origin_device_id: None,
                    usage_complete_through: (!complete_through.is_empty())
                        .then_some(complete_through),
                },
            );

            // Mark sync complete
            {
                let mut peers_guard = peers.lock().await;
                if let Some(peer) = peers_guard.get_mut(&peer_id) {
                    peer.sync_complete = true;
                    // Flush queued live changes
                    let queued = std::mem::take(&mut peer.queued_live_changes);
                    for entity in queued {
                        let change_id = generate_id();
                        send_msg(
                            &peer.tx,
                            &LanSyncMessage::LiveChange {
                                change_id,
                                entity,
                                origin_device_id: None,
                            },
                        );
                    }
                }
            }
        }

        LanSyncMessage::SyncChanges {
            batch_id,
            entities,
            is_last,
            usage_complete_through,
            ..
        } => {
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
            let mut accepted = 0;
            let mut vector_updated = false;

            let peer_device_id = {
                let peers_guard = peers.lock().await;
                peers_guard
                    .get(&peer_id)
                    .map(|p| p.device_id.clone())
                    .unwrap_or_default()
            };

            for entity in &entities {
                let should_apply = is_usage_entity(entity)
                    || local_vector
                        .get(&entity.id)
                        .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));

                if should_apply {
                    match storage.apply_entity(entity).await {
                        Ok(()) => {
                            vector_updated |= observe_non_usage_entity(&mut local_vector, entity);
                            accepted += 1;

                            if let Some(handler) = on_change.lock().await.as_ref() {
                                handler(entity.clone());
                            }

                            // Broadcast to other peers
                            broadcast_to_others(peers, &entity.clone(), Some(&peer_device_id))
                                .await;
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

            persist_pull_vector(
                storage,
                &mut local_vector,
                vector_updated,
                is_last,
                usage_complete_through.as_ref(),
            )
            .await;
            send_msg(&tx, &LanSyncMessage::SyncAck { batch_id, accepted });

            if is_last {
                eprintln!("{TAG} Received all sync batches from peer {peer_id}");
            }
        }

        LanSyncMessage::SyncAck { .. } => {
            // ACK received -- in a full implementation, resolve pending ACK timers
        }

        LanSyncMessage::SignedIntegrationFrame { frame } => {
            sync_server_inbound::handle_signed_integration_frame(ctx, peer_id, frame).await
        }

        LanSyncMessage::SignedIntegrationAck { .. } => {}

        LanSyncMessage::LiveChange {
            change_id, entity, ..
        } => sync_server_inbound::handle_live_change(ctx, peer_id, change_id, entity).await,

        LanSyncMessage::LiveAck { .. } => {}

        LanSyncMessage::PeerList {
            peers: incoming_peers,
        } => sync_server_inbound::handle_peer_list(ctx, peer_id, incoming_peers).await,

        LanSyncMessage::Ping { ts } => {
            let tx = {
                let peers_guard = peers.lock().await;
                peers_guard.get(&peer_id).map(|p| p.tx.clone())
            };
            if let Some(tx) = tx {
                send_msg(&tx, &LanSyncMessage::Pong { ts });
            }
        }

        LanSyncMessage::Pong { .. } => {}

        // Pairing consent lives on the addressed (iroh) transport; a
        // rejection over the WS path is meaningless — drop it.
        LanSyncMessage::PairingRejected { .. } => {}
    }
}
