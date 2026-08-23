use super::*;

impl SyncClient {
    /// Start connecting to the peer. Spawns a background task.
    pub fn start(&self) {
        self.stopped
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let storage = self.storage.clone();
        let peer = self.peer.clone();
        let device_id = self.device_id.clone();
        let device_name = self.device_name.clone();
        let space_id = self.space_id.clone();
        let own_addresses = self.own_addresses.clone();
        let auth_secret = self.auth_secret.clone();
        let stopped = self.stopped.clone();
        let authenticated_tx = self.authenticated_tx.clone();
        let on_change = self.on_change.clone();
        let on_connected = self.on_connected.clone();
        let on_disconnected = self.on_disconnected.clone();
        let on_peer_list = self.on_peer_list.clone();

        tokio::spawn(async move {
            let mut reconnect_delay = RECONNECT_BASE_MS;

            loop {
                if stopped.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                let peer_record = peer.read().await.clone();
                let addresses = peer_record.addresses.clone();

                if addresses.is_empty() {
                    eprintln!(
                        "{TAG} No addresses for peer {}, scheduling reconnect",
                        peer_record.device_name
                    );
                    tokio::time::sleep(Duration::from_millis(reconnect_delay)).await;
                    reconnect_delay =
                        (reconnect_delay as f64 * 1.5).min(RECONNECT_MAX_MS as f64) as u64;
                    continue;
                }

                // Race connections to all addresses
                let result = race_connect(&addresses).await;

                match result {
                    Some((ws_stream, winning_addr)) => {
                        eprintln!(
                            "{TAG} Connected to {} via {winning_addr}",
                            peer_record.device_name
                        );

                        // Update last_address
                        {
                            let mut p = peer.write().await;
                            p.last_address = Some(winning_addr);
                        }

                        reconnect_delay = RECONNECT_BASE_MS;

                        let (mut ws_sink, mut ws_stream_rx) = ws_stream.split();

                        // Send hello
                        let (auth_nonce, auth_hmac) = match auth_secret.as_ref() {
                            Some(secret) => {
                                let nonce = generate_auth_nonce();
                                let hmac =
                                    compute_hello_auth_hmac(secret, &space_id, &device_id, &nonce);
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
                        };
                        let hello_json = serialize_message(&hello);
                        if ws_sink.send(Message::Text(hello_json)).await.is_err() {
                            continue;
                        }

                        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

                        // Writer task
                        let writer = tokio::spawn(async move {
                            while let Some(msg) = rx.recv().await {
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
                                                ..
                                            } => {
                                                if let Some(secret) = auth_secret.as_ref() {
                                                    let valid = match (
                                                        auth_nonce.as_deref(),
                                                        auth_hmac.as_deref(),
                                                    ) {
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
                                                        eprintln!("{TAG} Rejecting peer hello with invalid HMAC");
                                                        break;
                                                    }
                                                }

                                                // Reject self-connect: if the server's hello
                                                // claims our own device_id, we accidentally
                                                // connected to our own SyncServer (stale phantom
                                                // peer record pointing at our own LAN IP). Close
                                                // and stop retrying — this peer record is a
                                                // self-reference and should be evicted.
                                                if !server_device_id.is_empty()
                                                    && server_device_id == device_id
                                                {
                                                    eprintln!("{TAG} Rejecting self-connect to {server_device_name} ({server_device_id})");
                                                    stopped.store(
                                                        true,
                                                        std::sync::atomic::Ordering::Relaxed,
                                                    );
                                                    break;
                                                }

                                                authenticated = true;
                                                peer_device_id_actual = server_device_id.clone();
                                                *authenticated_tx.lock().await = Some(tx.clone());
                                                eprintln!("{TAG} Authenticated with {server_device_name} ({server_device_id})");
                                                if let Some(handler) =
                                                    on_connected.lock().await.as_ref()
                                                {
                                                    handler(server_device_id, server_device_name);
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

                                                let mut local_vector =
                                                    load_version_vector(&storage).await;
                                                let mut offset = 0;
                                                loop {
                                                    let mut load_vector = local_vector.clone();
                                                    merge_usage_cursors(
                                                        &mut load_vector,
                                                        &remote_vector,
                                                    );
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
                                                            && !local_vector
                                                                .contains_key(&entity.id)
                                                        {
                                                            local_vector.insert(
                                                                entity.id.clone(),
                                                                entity.hlc.clone(),
                                                            );
                                                        }
                                                        if should_send_entity(
                                                            &remote_vector,
                                                            &entity,
                                                        ) {
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
                                                ..
                                            } => {
                                                if !authenticated {
                                                    continue;
                                                }

                                                let mut local_vector =
                                                    load_version_vector(&storage).await;
                                                let mut accepted = 0;
                                                let mut vector_updated = false;

                                                for entity in &entities {
                                                    let should_apply = is_usage_entity(entity)
                                                        || local_vector.get(&entity.id).is_none_or(
                                                            |hlc| HLC::is_newer(&entity.hlc, hlc),
                                                        );
                                                    if should_apply {
                                                        match storage.apply_entity(entity).await {
                                                            Ok(()) => {
                                                                vector_updated |=
                                                                    observe_non_usage_entity(
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
                                                                    "{TAG} Failed to apply sync entity {}:{}: {e}",
                                                                    entity.entity_type, entity.id
                                                                );
                                                            }
                                                        }
                                                    }
                                                }

                                                if vector_updated {
                                                    merge_usage_cursors(
                                                        &mut local_vector,
                                                        &load_version_vector(&storage).await,
                                                    );
                                                    save_version_vector(&storage, &local_vector)
                                                        .await;
                                                }
                                                send_msg(
                                                    &tx,
                                                    &LanSyncMessage::SyncAck { batch_id, accepted },
                                                );

                                                if is_last {
                                                    eprintln!("{TAG} Received all sync batches from server");
                                                }
                                            }

                                            LanSyncMessage::SyncAck { .. } => {}

                                            LanSyncMessage::LiveChange {
                                                change_id,
                                                entity,
                                                ..
                                            } => {
                                                if !authenticated {
                                                    continue;
                                                }

                                                let mut local_vector =
                                                    load_version_vector(&storage).await;
                                                let should_apply = is_usage_entity(&entity)
                                                    || local_vector.get(&entity.id).is_none_or(
                                                        |hlc| HLC::is_newer(&entity.hlc, hlc),
                                                    );

                                                if should_apply {
                                                    match storage.apply_entity(&entity).await {
                                                        Ok(()) => {
                                                            if observe_non_usage_entity(
                                                                &mut local_vector,
                                                                &entity,
                                                            ) {
                                                                merge_usage_cursors(
                                                                    &mut local_vector,
                                                                    &load_version_vector(&storage)
                                                                        .await,
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
                                                                "{TAG} Failed to apply live sync entity {}:{}: {e}",
                                                                entity.entity_type, entity.id
                                                            );
                                                        }
                                                    }
                                                }

                                                send_msg(
                                                    &tx,
                                                    &LanSyncMessage::LiveAck { change_id },
                                                );
                                            }

                                            LanSyncMessage::LiveAck { .. } => {}

                                            LanSyncMessage::PeerList { peers: peer_list } => {
                                                if !authenticated {
                                                    continue;
                                                }
                                                if let Some(handler) =
                                                    on_peer_list.lock().await.as_ref()
                                                {
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

                // Reconnect with backoff
                let jitter = rand::random::<f64>() * 1000.0;
                let delay = (reconnect_delay as f64 + jitter).min(RECONNECT_MAX_MS as f64);
                tokio::time::sleep(Duration::from_millis(delay as u64)).await;
                reconnect_delay =
                    ((reconnect_delay as f64) * 1.5).min(RECONNECT_MAX_MS as f64) as u64;
            }
        });
    }
}
