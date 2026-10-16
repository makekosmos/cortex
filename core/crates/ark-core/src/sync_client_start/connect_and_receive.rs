    {
        match result {
            Some((ws_stream, winning_addr)) => {
                eprintln!(
                    "{TAG} Connected to {} via {winning_addr}",
                    peer_record.device_name
                );

                // Update last_address
                {
                    let mut p = peer.write().await;
                    p.last_address = Some(winning_addr.clone());
                }

                reconnect_delay = RECONNECT_BASE_MS;
                // An unacknowledged cursor remains pending in durable peer
                // progress. Releasing the connection-local claim replays it.
                *feed_in_flight.lock().await = None;

                let (mut ws_sink, mut ws_stream_rx) = ws_stream.split();

                let hello_json = client_hello::serialize_client_hello(
                    &device_id,
                    &device_name,
                    &space_id,
                    &own_addresses,
                    auth_secret.as_deref(),
                );
                if ws_sink
                    .send(Message::Text(hello_json.into()))
                    .await
                    .is_err()
                {
                    continue;
                }

                let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

                // Writer task. Re-check signed integration authority at the
                // actual socket write boundary after queued revocation races.
                let storage_writer = storage.clone();
                let space_id_writer = space_id.clone();
                let device_id_writer = device_id.clone();
                let writer = tokio::spawn(async move {
                    while let Some(msg) = rx.recv().await {
                        if let Message::Text(text) = &msg {
                            if let Some(LanSyncMessage::SignedIntegrationFrame { frame }) =
                                deserialize_message(text)
                            {
                                if storage_writer
                                    .validate_outbound_signed_integration_frame(
                                        &frame,
                                        &space_id_writer,
                                        &device_id_writer,
                                    )
                                    .await
                                    .is_err()
                                {
                                    continue;
                                }
                            }
                        }
                        if ws_sink.send(msg).await.is_err() {
                            break;
                        }
                    }
                });
                let writer_abort = writer.abort_handle();
                track_task(&writer_tasks, writer);

                // Read messages
                let mut authenticated = false;
                let mut sync_complete = false;
                let mut pending_initial_snapshot_ack: Option<String> = None;
                let mut peer_device_id_actual = String::new();
                let mut feed_recovery = tokio::time::interval(Duration::from_secs(1));
                // `interval` ticks immediately; start the bounded recovery
                // period after the connection has had a chance to authenticate.
                feed_recovery.tick().await;

                loop {
                    tokio::select! {
                        _ = feed_recovery.tick() => {
                            if let Err(error) = feed_recovery::pump(
                                &storage, &authenticated_tx, &feed_in_flight,
                            ).await {
                                eprintln!("{TAG} periodic committed feed drain failed: {error}");
                            }
                        }
                        maybe_msg = ws_stream_rx.next() => {
                            let Some(Ok(msg)) = maybe_msg else { break; };
                            match msg {
                        Message::Text(text) => {
                            if let Some(sync_msg) = deserialize_message(&text) {
                                match sync_msg {
                                    LanSyncMessage::Hello {
                                        device_id: server_device_id,
                                        device_name: server_device_name,
                                        space_id: server_space_id,
                                        auth_nonce,
                                        auth_hmac,
                                        platform: server_platform,
                                        app_version: server_app_version,
                                        ..
                                    } => {
                                        if authenticated {
                                            eprintln!(
                                                "{TAG} Rejecting repeated hello on an \
                                                    authenticated connection"
                                            );
                                            break;
                                        }
                                        if server_space_id != space_id {
                                            eprintln!(
                                                "{TAG} Rejecting peer hello from a different space"
                                            );
                                            break;
                                        }
                                        if let Some(secret) = auth_secret.as_ref() {
                                            let valid =
                                                match (auth_nonce.as_deref(), auth_hmac.as_deref())
                                                {
                                                    (Some(nonce), Some(hmac)) => {
                                                        verify_hello_auth_hmac(
                                                            secret,
                                                            &server_space_id,
                                                            &server_device_id,
                                                            nonce,
                                                            hmac,
                                                        )
                                                    }
                                                    _ => false,
                                                };
                                            if !valid {
                                                eprintln!(
                                                    "{TAG} Rejecting peer hello with invalid HMAC"
                                                );
                                                break;
                                            }
                                        }

                                        // Reject self-connects and evict pure self-references.
                                        if !server_device_id.is_empty()
                                            && server_device_id == device_id
                                        {
                                            eprintln!(
                                                "{TAG} Rejecting self-connect to \
                                                     {server_device_name} ({server_device_id})"
                                            );
                                            // Drop only this address; stop if it was the last one.
                                            let remaining = {
                                                let mut p = peer.write().await;
                                                p.addresses.retain(|a| a != &winning_addr);
                                                p.last_address = None;
                                                p.addresses.len()
                                            };
                                            if remaining == 0 {
                                                stopped.store(
                                                    true,
                                                    std::sync::atomic::Ordering::Relaxed,
                                                );
                                            }
                                            break;
                                        }

                                        let generation =
                                                storage.selective_sync_policy_generation();
                                        if let Err(error) = storage
                                            .begin_outbound_policy(&server_device_id, generation)
                                            .await
                                        {
                                            eprintln!(
                                                "{TAG} outbound snapshot baseline failed: {error}"
                                            );
                                            break;
                                        }
                                        authenticated = true;
                                        peer_device_id_actual = server_device_id.clone();
                                        *authenticated_tx.lock().await =
                                            Some((server_device_id.clone(), tx.clone(), false));
                                        // Adopt the authenticated
                                        // identity — bootstrap records
                                        // carry a "seed-*" placeholder
                                        // device_id, and DisconnectPeer /
                                        // peer listings key off the
                                        // stored record.
                                        let connected_record = {
                                            let mut p = peer.write().await;
                                            p.device_id = server_device_id.clone();
                                            p.device_name = server_device_name.clone();
                                            p.platform = server_platform.clone();
                                            p.app_version = server_app_version.clone();
                                            p.clone()
                                        };
                                        eprintln!(
                                            "{TAG} Authenticated with {server_device_name} \
                                                 ({server_device_id})"
                                        );
                                        if let Some(handler) = on_connected.lock().await.as_ref() {
                                            handler(connected_record);
                                        }

                                        // Send version vector
                                        let vector = match load_version_vector(&storage).await {
                                            Ok(vector) => vector,
                                            Err(error) => {
                                                eprintln!(
                                            "{TAG} failed to load sync version vector: {error}"
                                        );
                                                break;
                                            }
                                        };
                                        send_msg(
                                            &tx,
                                            &LanSyncMessage::VersionVector {
                                                vector,
                                                origin_device_id: None,
                                            },
                                        );
                                    }

                                    LanSyncMessage::VersionVector {
                                        vector: remote_vector,
                                        ..
                                    } => {
                                        if !authenticated {
                                            continue;
                                        }

                                        let batch_id = match send_missing_entities::send(
                                            &storage,
                                            &tx,
                                            &peer_device_id_actual,
                                            &remote_vector,
                                            sync_complete,
                                        )
                                        .await
                                        {
                                            Ok(batch_id) => batch_id,
                                            Err(error) => {
                                                eprintln!("{TAG} outbound sync failed: {error}");
                                                break;
                                            }
                                        };

                                        sync_complete = true;
                                        pending_initial_snapshot_ack = Some(batch_id);
                                        // Broadcasts are wakeups only; drain the
                                        // durable feed so reconnect can replay the
                                        // same cursor-derived change ID.
                                    }

                                    LanSyncMessage::SyncChanges {
                                        batch_id, entities, is_last, usage_complete_through, ..
                                    } => {
                                        if !authenticated { continue; }
                                        let context = sync_changes::Context {
                                            storage: &storage,
                                            tx: &tx,
                                            peer_device_id: &peer_device_id_actual,
                                            on_change: &on_change,
                                        };
                                        if let Err(error) = sync_changes::apply(
                                            context, batch_id, entities, is_last,
                                            usage_complete_through,
                                        ).await {
                                            eprintln!("{TAG} inbound sync batch failed: {error}");
                                            break;
                                        }
                                    }

                                    LanSyncMessage::SyncAck { batch_id, .. } => {
                                        if pending_initial_snapshot_ack.as_deref()
                                            == Some(batch_id.as_str())
                                        {
                                            let generation =
                                                storage.selective_sync_policy_generation();
                                            if let Err(error) = storage
                                                .complete_outbound_snapshot(
                                                    &peer_device_id_actual,
                                                    generation,
                                                )
                                                .await
                                            {
                                                eprintln!(
                                                    "{TAG} outbound snapshot completion failed: \
                                                     {error}"
                                                );
                                                break;
                                            }
                                            pending_initial_snapshot_ack = None;
                                            if let Some((_, _, complete)) =
                                                authenticated_tx.lock().await.as_mut()
                                            {
                                                *complete = true;
                                            }
                                            if let Err(error) = super::pump_client_committed_feed(
                                                &storage,
                                                &peer_device_id_actual,
                                                &tx,
                                                &feed_in_flight,
                                            )
                                            .await
                                            {
                                                eprintln!(
                                                    "{TAG} committed feed drain failed: {error}"
                                                );
                                            }
                                        }
                                    }

                                    LanSyncMessage::SignedIntegrationFrame { frame } => {
                                        if !authenticated {
                                            continue;
                                        }
                                        let accepted = if frame.space_id != space_id
                                            || frame.origin_node_id != peer_device_id_actual
                                            || frame.recipient_node_id != device_id
                                        {
                                            false
                                        } else {
                                            storage
                                                .apply_signed_integration_frame(
                                                    &frame,
                                                    &space_id,
                                                    &peer_device_id_actual,
                                                    &device_id,
                                                )
                                                .await
                                                .is_ok()
                                        };
                                        send_msg(
                                            &tx,
                                            &LanSyncMessage::SignedIntegrationAck {
                                                message_id: frame.message_id,
                                                accepted,
                                            },
                                        );
                                    }

                                    LanSyncMessage::SignedIntegrationAck { .. } => {}

                                    LanSyncMessage::LiveChange {
                                        change_id, entity, ..
                                    } => {
                                        if !authenticated {
                                            continue;
                                        }

                                        let mut local_vector =
                                            match load_version_vector(&storage).await {
                                                Ok(vector) => vector,
                                                Err(error) => {
                                                    eprintln!(
                                                    "{TAG} failed to load version vector: {error}"
                                                );
                                                    break;
                                                }
                                            };
                                        let should_apply = is_usage_entity(&entity)
                                            || local_vector
                                                .get(&entity.id)
                                                .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));

                                        match storage
                                            .apply_authenticated_entity_with_id(
                                                &entity,
                                                &peer_device_id_actual,
                                                &change_id,
                                            )
                                            .await
                                        {
                                                Ok(()) => {
                                                    if should_apply && observe_non_usage_entity(
                                                        &mut local_vector,
                                                        &entity,
                                                    ) {
                                                        let stored_vector =
                                                            match load_version_vector(&storage)
                                                                .await
                                                            {
                                                                Ok(vector) => vector,
                                                                Err(error) => {
                                                                    eprintln!(
                                                                    "{TAG} reload failed: {error}"
                                                                );
                                                                    break;
                                                                }
                                                            };
                                                        merge_usage_cursors(
                                                            &mut local_vector,
                                                            &stored_vector,
                                                        );
                                                        if let Err(error) = save_version_vector(
                                                            &storage,
                                                            &local_vector,
                                                        )
                                                        .await
                                                        {
                                                            eprintln!("{TAG} save failed: {error}");
                                                            break;
                                                        }
                                                    }

                                                    if should_apply {
                                                        if let Some(handler) =
                                                            on_change.lock().await.as_ref()
                                                        {
                                                            handler(entity);
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    eprintln!(
                                                        "{TAG} Failed to apply live sync entity \
                                                             {}:{}: {e}",
                                                        entity.entity_type, entity.id
                                                    );
                                                    // Do not acknowledge a change that was not
                                                    // committed locally; the sender must retry it.
                                                    break;
                                                }
                                        }

                                        send_msg(&tx, &LanSyncMessage::LiveAck { change_id });
                                    }

                                    LanSyncMessage::LiveAck { change_id } => {
                                        if sync_complete {
                                            if let Err(error) = super::acknowledge_client_feed(
                                                &storage,
                                                &peer_device_id_actual,
                                                &change_id,
                                                &tx,
                                                &feed_in_flight,
                                            )
                                            .await
                                            {
                                                eprintln!(
                                                    "{TAG} committed feed ACK failed: {error}"
                                                );
                                                break;
                                            }
                                        }
                                    }

                                    LanSyncMessage::PeerList { peers: peer_list } => {
                                        if !authenticated {
                                            continue;
                                        }
                                        if let Some(handler) = on_peer_list.lock().await.as_ref() {
                                            handler(peer_list);
                                        }
                                    }

                                    LanSyncMessage::Ping { ts } => {
                                        send_msg(&tx, &LanSyncMessage::Pong { ts });
                                    }

                                    LanSyncMessage::Pong { .. } => {}

                                    // Pairing consent lives on the iroh
                                    // transport — meaningless over WS.
                                    LanSyncMessage::PairingRejected { .. } => {}
                                }
                            }
                        }
                        Message::Close(_) => break,
                        _ => {}
                            }
                        }
                    }
                }

                // Connection closed
                writer_abort.abort();
                *authenticated_tx.lock().await = None;

                if authenticated {
                    eprintln!("{TAG} Disconnected from peer");
                    if let Some(handler) = on_disconnected.lock().await.as_ref() {
                        handler(peer_device_id_actual);
                    }
                }
            }
            None => {
                eprintln!("{TAG} All addresses failed for {}", peer_record.device_name);
            }
        }

        if stopped.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
    }
