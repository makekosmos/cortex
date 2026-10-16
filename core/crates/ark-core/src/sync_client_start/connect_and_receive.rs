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

                let client_hello = client_hello::build_client_hello(
                    &device_id,
                    &device_name,
                    &space_id,
                    &own_addresses,
                    auth_secret.as_deref(),
                );
                if ws_sink
                    .send(Message::Text(client_hello.message.clone().into()))
                    .await
                    .is_err()
                {
                    continue;
                }

                let (tx, rx) = mpsc::unbounded_channel::<Message>();

                let writer = writer::spawn(
                    ws_sink,
                    rx,
                    storage.clone(),
                    space_id.clone(),
                    device_id.clone(),
                );
                let writer_abort = writer.abort_handle();
                track_task(&writer_tasks, writer);

                // Read messages
                let mut authenticated = false;
                let mut sync_complete = false;
                let mut pending_initial_snapshot_ack: Option<String> = None;
                let mut peer_device_id_actual = String::new();
                let mut pending_registry_admission: Option<
                    registry_admission::PendingRegistryAdmission,
                > = None;
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
                                match direct_ws_control::handle(
                                    &sync_msg,
                                    &tx,
                                    authenticated,
                                    &on_peer_list,
                                    &device_id,
                                    &peer_device_id_actual,
                                )
                                .await
                                {
                                    direct_ws_control::DirectWsControl::Handled => continue,
                                    direct_ws_control::DirectWsControl::CloseConnection => break,
                                    direct_ws_control::DirectWsControl::NotControl => {}
                                }
                                if matches!(
                                    &sync_msg,
                                    LanSyncMessage::RegistryAdmissionStart { .. }
                                        | LanSyncMessage::RegistryAdmissionProof { .. }
                                        | LanSyncMessage::RegistryAdmissionConfirm { .. }
                                        | LanSyncMessage::RegistrySyncStart { .. }
                                        | LanSyncMessage::RegistryEventsPage { .. }
                                        | LanSyncMessage::RegistrySyncReady { .. }
                                        | LanSyncMessage::RegistryEventUpdate { .. }
                                ) {
                                    let Some(admission) = pending_registry_admission.as_mut() else {
                                        eprintln!("{TAG} registry control arrived before Hello");
                                        break;
                                    };
                                    let became_ready = match admission
                                        .handle(
                                            sync_msg,
                                            &storage,
                                            &tx,
                                            &authenticated_tx,
                                            &peer,
                                            &on_connected,
                                        )
                                        .await
                                    {
                                        Ok(became_ready) => became_ready,
                                        Err(error) => {
                                            if error == "registry_admission_session_revoked" {
                                                stopped.store(
                                                    true,
                                                    std::sync::atomic::Ordering::Relaxed,
                                                );
                                            }
                                            eprintln!(
                                                "{TAG} direct registry admission failed: {error}"
                                            );
                                            break;
                                        }
                                    };
                                    if became_ready {
                                        authenticated = true;
                                    }
                                    continue;
                                }
                                match sync_msg {
                                    LanSyncMessage::Hello {
                                        protocol_version,
                                        device_id: server_device_id,
                                        device_name: server_device_name,
                                        space_id: server_space_id,
                                        auth_nonce,
                                        auth_hmac,
                                        platform: server_platform,
                                        app_version: server_app_version,
                                        addresses: _,
                                        supported_protocol_versions,
                                        supported_capabilities,
                                        negotiated_capabilities,
                                        negotiation_hmac,
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

                                        let negotiated_protocol = match
                                            crate::protocol::validate_protocol_selection(
                                            &client_hello.offer,
                                            protocol_version,
                                            supported_protocol_versions.as_deref(),
                                            supported_capabilities.as_deref(),
                                            negotiated_capabilities.as_deref(),
                                            auth_secret.as_deref(),
                                            &space_id,
                                            &device_id,
                                            &server_device_id,
                                            client_hello.auth_nonce.as_deref(),
                                            auth_nonce.as_deref(),
                                            negotiation_hmac.as_deref(),
                                        ) {
                                            Ok(protocol) => protocol,
                                            Err(error) => {
                                                eprintln!(
                                                    "{TAG} invalid protocol selection: {}",
                                                    error.code()
                                                );
                                                break;
                                            }
                                        };
                                        if !negotiated_protocol
                                            .capabilities
                                            .contains(&ProtocolCapability::RegistryHistoryV1)
                                        {
                                            eprintln!(
                                                "{TAG} peer requires registry-history upgrade"
                                            );
                                            break;
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

                                        peer_device_id_actual = server_device_id.clone();
                                        let pending_peer = PeerRecord {
                                            device_id: server_device_id.clone(),
                                            device_name: server_device_name,
                                            addresses: Vec::new(),
                                            last_seen: chrono::Utc::now().to_rfc3339_opts(
                                                chrono::SecondsFormat::Millis,
                                                true,
                                            ),
                                            last_address: Some(winning_addr.clone()),
                                            platform: server_platform,
                                            app_version: server_app_version,
                                        };
                                        let (admission, start) =
                                            match registry_admission::PendingRegistryAdmission::
                                                begin(
                                                &device_id,
                                                &server_device_id,
                                                &space_id,
                                                negotiated_protocol.clone(),
                                                pending_peer,
                                                on_registry_events.clone(),
                                            )
                                            {
                                                Ok(admission) => admission,
                                                Err(error) => {
                                                    eprintln!(
                                                        "{TAG} admission start failed: {error}"
                                                    );
                                                    break;
                                                }
                                            };
                                        pending_registry_admission = Some(admission);
                                        send_msg(&tx, &start);
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
                                            let receipt_ack = if let Some(session) =
                                                authenticated_tx.lock().await.as_mut() {
                                                session.sync_complete = true;
                                                session.protocol.capabilities.contains(
                                                &crate::protocol::ProtocolCapability::
                                                    DurableReceiptAckV1,
                                                )
                                            } else {
                                                false
                                            };
                                            if let Err(error) = super::pump_client_committed_feed(
                                                &storage,
                                                &peer_device_id_actual,
                                                &tx,
                                                &feed_in_flight,
                                                receipt_ack,
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
                                        live_messages::handle_signed_integration_frame(
                                            &storage,
                                            &peer_device_id_actual,
                                            &device_id,
                                            &space_id,
                                            &tx,
                                            frame,
                                        )
                                        .await;
                                    }

                                    LanSyncMessage::SignedIntegrationAck { .. } => {}

                                    LanSyncMessage::LiveChange { change_id, entity, .. } => {
                                        if !authenticated {
                                            continue;
                                        }
                                        if let Err(error) = live_messages::handle_live_change(
                                            &storage,
                                            &authenticated_tx,
                                            &peer_device_id_actual,
                                            &tx,
                                            &on_change,
                                            change_id,
                                            entity,
                                        )
                                        .await
                                        {
                                            eprintln!("{TAG} live change handling failed: {error}");
                                            break;
                                        }
                                    }

                                    LanSyncMessage::LiveAck { change_id } => {
                                        if sync_complete {
                                            if let Err(error) = live_messages::handle_applied_ack(
                                                &storage,
                                                &peer_device_id_actual,
                                                &tx,
                                                &feed_in_flight,
                                                &authenticated_tx,
                                                &change_id,
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

                                    LanSyncMessage::LiveReceiptAck { change_id } => {
                                        if sync_complete {
                                            if let Err(error) = live_messages::handle_receipt_ack(
                                                &storage,
                                                &peer_device_id_actual,
                                                &tx,
                                                &feed_in_flight,
                                                &change_id,
                                            )
                                            .await
                                            {
                                                eprintln!(
                                                    "{TAG} receipt ACK failed: {error}"
                                                );
                                                break;
                                            }
                                        }
                                    }

                                    LanSyncMessage::Ping { .. }
                                    | LanSyncMessage::Pong { .. }
                                    | LanSyncMessage::PairingRejected { .. }
                                    | LanSyncMessage::PairingAccepted { .. }
                                    | LanSyncMessage::RegistryAdmissionStart { .. }
                                    | LanSyncMessage::RegistryAdmissionProof { .. }
                                    | LanSyncMessage::RegistryAdmissionConfirm { .. }
                                    | LanSyncMessage::RegistrySyncStart { .. }
                                    | LanSyncMessage::RegistryEventsPage { .. }
                                    | LanSyncMessage::RegistrySyncReady { .. }
                                    | LanSyncMessage::RegistryEventUpdate { .. }
                                    | LanSyncMessage::ProtocolRejected { .. }
                                    | LanSyncMessage::PeerList { .. } => {
                                        eprintln!(
                                            "{TAG} unsupported direct WebSocket control frame"
                                        );
                                        break;
                                    }

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
