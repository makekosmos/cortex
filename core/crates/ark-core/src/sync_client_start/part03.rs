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

                let (mut ws_sink, mut ws_stream_rx) = ws_stream.split();

                // Send hello
                let (auth_nonce, auth_hmac) = match auth_secret.as_ref() {
                    Some(secret) => {
                        let nonce = generate_auth_nonce();
                        let hmac = compute_hello_auth_hmac(secret, &space_id, &device_id, &nonce);
                        (Some(nonce), Some(hmac))
                    }
                    None => (None, None),
                };
                let hello = LanSyncMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    device_id: device_id.clone(),
                    device_name: device_name.clone(),
                    space_id: space_id.clone(),
                    addresses: Some(own_addresses.clone()),
                    auth_nonce,
                    auth_hmac,
                    platform: Some(crate::host::local_platform()),
                    app_version: crate::host::app_version(),
                };
                let hello_json = serialize_message(&hello);
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

                // Read messages
                let mut authenticated = false;
                let mut _sync_complete = false;
                let mut queued_live_changes: Vec<SyncEntity> = Vec::new();
                let mut peer_device_id_actual = String::new();

                while let Some(Ok(msg)) = ws_stream_rx.next().await {
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

                                        // Reject self-connect: if the server's hello
                                        // claims our own device_id, we accidentally
                                        // connected to our own SyncServer (stale phantom
                                        // peer record pointing at our own LAN IP). Close;
                                        // retrying stops only when no other addresses
                                        // remain — a pure self-reference is evicted.
                                        if !server_device_id.is_empty()
                                            && server_device_id == device_id
                                        {
                                            eprintln!(
                                                "{TAG} Rejecting self-connect to \
                                                     {server_device_name} ({server_device_id})"
                                            );
                                            // The winning address
                                            // looped back to us — drop
                                            // just that address and
                                            // only give up when no
                                            // others remain (a pure
                                            // self-record).
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

                                        authenticated = true;
                                        peer_device_id_actual = server_device_id.clone();
                                        *authenticated_tx.lock().await =
                                            Some((server_device_id.clone(), tx.clone()));
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
                                        let vector = load_version_vector(&storage).await;
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

                                        let mut local_vector = load_version_vector(&storage).await;
                                        // KOS-302: snapshot the
                                        // completeness claims BEFORE
                                        // the first page — computed
                                        // later they could include a
                                        // seq allocated mid-pull,
                                        // moving the peer's cursor
                                        // past an entry it never got.
                                        let complete_through =
                                            storage.usage_complete_through().await;
                                        let mut offset = 0;
                                        loop {
                                            let mut load_vector = local_vector.clone();
                                            merge_usage_cursors(&mut load_vector, &remote_vector);
                                            let entities = storage
                                                .load_entities_page(
                                                    &load_vector,
                                                    offset,
                                                    SYNC_LOAD_PAGE_SIZE,
                                                )
                                                .await;
                                            if entities.is_empty() {
                                                break;
                                            }
                                            offset += entities.len();
                                            let mut to_send = Vec::new();
                                            for entity in entities {
                                                if !is_usage_entity(&entity)
                                                    && !local_vector.contains_key(&entity.id)
                                                {
                                                    local_vector.insert(
                                                        entity.id.clone(),
                                                        entity.hlc.clone(),
                                                    );
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
                                        save_version_vector(&storage, &local_vector).await;
                                        send_msg(
                                            &tx,
                                            &LanSyncMessage::SyncChanges {
                                                batch_id: generate_id(),
                                                entities: vec![],
                                                is_last: true,
                                                origin_device_id: None,
                                                usage_complete_through: (!complete_through
                                                    .is_empty())
                                                .then_some(complete_through),
                                            },
                                        );

                                        _sync_complete = true;
                                        // Flush queued live changes
                                        for entity in queued_live_changes.drain(..) {
                                            let change_id = generate_id();
                                            send_msg(
                                                &tx,
                                                &LanSyncMessage::LiveChange {
                                                    change_id,
                                                    entity,
                                                    origin_device_id: None,
                                                },
                                            );
                                        }
                                    }

                                    LanSyncMessage::SyncChanges {
                                        batch_id,
                                        entities,
                                        is_last,
                                        usage_complete_through,
                                        ..
                                    } => {
                                        if !authenticated {
                                            continue;
                                        }

                                        let mut local_vector = load_version_vector(&storage).await;
                                        let mut accepted = 0;
                                        let mut vector_updated = false;

                                        for entity in &entities {
                                            let should_apply = is_usage_entity(entity)
                                                || local_vector.get(&entity.id).is_none_or(|hlc| {
                                                    HLC::is_newer(&entity.hlc, hlc)
                                                });
                                            if should_apply {
                                                match storage.apply_entity(entity).await {
                                                    Ok(()) => {
                                                        vector_updated |= observe_non_usage_entity(
                                                            &mut local_vector,
                                                            entity,
                                                        );
                                                        accepted += 1;
                                                        if let Some(handler) =
                                                            on_change.lock().await.as_ref()
                                                        {
                                                            handler(entity.clone());
                                                        }
                                                    }
                                                    Err(e) => {
                                                        eprintln!(
                                                            "{TAG} Failed to apply sync entity \
                                                                 {}:{}: {e}",
                                                            entity.entity_type, entity.id
                                                        );
                                                    }
                                                }
                                            }
                                        }

                                        crate::sync_server::persist_pull_vector(
                                            &storage,
                                            &mut local_vector,
                                            vector_updated,
                                            is_last,
                                            usage_complete_through.as_ref(),
                                        )
                                        .await;
                                        send_msg(
                                            &tx,
                                            &LanSyncMessage::SyncAck { batch_id, accepted },
                                        );

                                        if is_last {
                                            eprintln!(
                                                "{TAG} Received all sync batches from server"
                                            );
                                        }
                                    }

                                    LanSyncMessage::SyncAck { .. } => {}

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

                                        let mut local_vector = load_version_vector(&storage).await;
                                        let should_apply = is_usage_entity(&entity)
                                            || local_vector
                                                .get(&entity.id)
                                                .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));

                                        if should_apply {
                                            match storage.apply_entity(&entity).await {
                                                Ok(()) => {
                                                    if observe_non_usage_entity(
                                                        &mut local_vector,
                                                        &entity,
                                                    ) {
                                                        merge_usage_cursors(
                                                            &mut local_vector,
                                                            &load_version_vector(&storage).await,
                                                        );
                                                        save_version_vector(
                                                            &storage,
                                                            &local_vector,
                                                        )
                                                        .await;
                                                    }

                                                    if let Some(handler) =
                                                        on_change.lock().await.as_ref()
                                                    {
                                                        handler(entity);
                                                    }
                                                }
                                                Err(e) => {
                                                    eprintln!(
                                                        "{TAG} Failed to apply live sync entity \
                                                             {}:{}: {e}",
                                                        entity.entity_type, entity.id
                                                    );
                                                }
                                            }
                                        }

                                        send_msg(&tx, &LanSyncMessage::LiveAck { change_id });
                                    }

                                    LanSyncMessage::LiveAck { .. } => {}

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
                                }
                            }
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }

                // Connection closed
                writer.abort();
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
