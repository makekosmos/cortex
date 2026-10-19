    {
        match result {
            Ok(Some(transport)) => {
                let ark_sync::client_session::ConnectedClientTransport {
                    io: mut session_io,
                    selected_address: winning_addr,
                } = transport;
                eprintln!(
                    "{TAG} Connected to {} via {winning_addr}",
                    peer_record.device_name
                );

                {
                    let mut p = peer.write().await;
                    p.last_address = Some(winning_addr.clone());
                }

                reconnect_delay = RECONNECT_BASE_MS;
                *feed_in_flight.lock().await = None; // Replay the durable cursor after reconnect.

                let client_hello = ark_sync::client_session::build_client_hello(
                    ark_sync::client_session::ClientHelloRequest {
                        device_id: &device_id,
                        device_name: &device_name,
                        space_id: &space_id,
                        addresses: &own_addresses,
                        auth_secret: auth_secret.as_deref(),
                        task_merge_bootstrap_v2,
                        platform: Some(ark_net::host::local_platform()),
                        app_version: ark_net::host::app_version(),
                    },
                );
                let tx = ClientMessageSender::new(
                    session_io.sender(),
                    storage.clone(),
                    peer.clone(),
                    space_id.clone(),
                    device_id.clone(),
                );
                if tx.send(client_hello.message).await.is_err() {
                    let _ = session_io.finish().await;
                    continue;
                }

                let mut client_session = ark_sync::client_session::DirectClientSession::new();
                use ark_sync::client_session::DirectClientSessionRejection as SessionRejection;
                let mut pending_registry_admission: Option<
                    registry_admission::PendingRegistryAdmission,
                > = None;
                let mut feed_recovery = tokio::time::interval(Duration::from_secs(1));
                feed_recovery.tick().await;
                let mut snapshot_drive = None;
                let mut pending_remote_vector = None;

                loop {
                    tokio::select! {
                        result = async {
                            match snapshot_drive.as_mut() {
                                Some(task) => task.await,
                                None => std::future::pending().await,
                            }
                        } => {
                            snapshot_drive = None;
                            match result {
                                Ok(Ok(())) => {}
                                Ok(Err(error)) => {
                                    eprintln!("{TAG} outbound snapshot failed: {error}");
                                    break;
                                }
                                Err(error) => {
                                    eprintln!("{TAG} outbound snapshot task failed: {error}");
                                    break;
                                }
                            }
                            if let Some(remote_vector) = pending_remote_vector.take() {
                                let policy_bound =
                                    sync_policy::is_policy_bound(&authenticated_tx).await;
                                let snapshot_already_sent =
                                    snapshot_ack::was_sent(&authenticated_tx).await;
                                snapshot_drive = Some(snapshot_drive::spawn(
                                    storage.clone(),
                                    client_session.peer_device_id().to_owned(),
                                    remote_vector,
                                    snapshot_already_sent,
                                    policy_bound,
                                    tx.clone(),
                                    authenticated_tx.clone(),
                                ));
                            }
                        }
                        _ = feed_recovery.tick() => {
                            if let Err(error) = feed_recovery::pump(
                                &storage, &authenticated_tx, &feed_in_flight,
                            ).await {
                                eprintln!("{TAG} periodic committed feed drain failed: {error}");
                            }
                        }
                        session_input = client_session.receive(&mut session_io) => {
                            use ark_sync::client_session::DirectClientSessionInput as SessionInput;
                            let message = match session_input {
                                SessionInput::Message(message) => *message,
                                SessionInput::Ignore => continue,
                                SessionInput::Malformed => {
                                    eprintln!("{TAG} ignoring malformed peer sync message");
                                    continue;
                                }
                                SessionInput::Failed(error) => {
                                    eprintln!("{TAG} peer session transport failed: {error}");
                                    break;
                                }
                                SessionInput::Closed => break,
                                SessionInput::Rejected(reason) => {
                                    eprintln!(
                                        "{TAG} closing invalid client session message: {reason:?}"
                                    );
                                    break;
                                }
                            };
                            use ark_sync::client_session::{
                                DirectClientMessage as ClientMessage,
                            };
                            match message {
                                    ClientMessage::Hello(hello) => {
                                        let accepted = match
                                            ark_sync::client_session::accept_server_hello(
                                            hello,
                                            ark_sync::client_session::ServerHelloValidationRequest {
                                                local_device_id: &device_id,
                                                local_space_id: &space_id,
                                                auth_secret: auth_secret.as_deref(),
                                                local_offer: &client_hello.offer,
                                                local_auth_nonce: client_hello
                                                    .auth_nonce
                                                    .as_deref(),
                                            },
                                        ) {
                                            Ok(accepted) => accepted,
                                            Err(error) => {
                                                eprintln!("{TAG} {error}");
                                                break;
                                            }
                                        };
                                        let ark_sync::client_session::AcceptedServerHello {
                                            device_id: server_device_id,
                                            device_name: server_device_name,
                                            platform: server_platform,
                                            app_version: server_app_version,
                                            protocol: negotiated_protocol,
                                        } = accepted;
                                        match client_session
                                            .accept_server_hello(&server_device_id, &device_id)
                                        {
                                            Ok(()) => {}
                                            Err(SessionRejection::SelfConnection) => {
                                                eprintln!(
                                                    "{TAG} Rejecting self-connect to \
                                                         {server_device_name} ({server_device_id})"
                                                );
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
                                            Err(SessionRejection::RepeatedHello) => {
                                                eprintln!("{TAG} rejected Hello: repeated session");
                                                break;
                                            }
                                            Err(SessionRejection::RegistryControlBeforeHello) => {
                                                eprintln!("{TAG} invalid Hello session phase");
                                                break;
                                            }
                                        }
                                        tx.set_authenticated_peer_id(server_device_id.clone())
                                            .await;

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
                                        if let Err(error) = send_msg(&tx, start).await {
                                            eprintln!(
                                                "{TAG} registry admission send failed: {error}"
                                            );
                                            break;
                                        }
                                    }

                                    ClientMessage::SyncPolicy(update) => {
                                        let ark_sync::client_session::SyncPolicyUpdate {
                                            revision,
                                            digest,
                                            active,
                                            rules,
                                        } = update;
                                        if let Err(error) = sync_policy::accept_and_start(
                                            &storage,
                                            &authenticated_tx,
                                            client_session.is_ready(),
                                            revision,
                                            &digest,
                                            active,
                                            rules,
                                        )
                                        .await
                                        {
                                            eprintln!("{TAG} peer sync policy rejected: {error}");
                                            break;
                                        }
                                    }

                                    ClientMessage::VersionVector(remote_vector) => {
                                        if snapshot_drive.is_some() {
                                            pending_remote_vector = Some(remote_vector);
                                            continue;
                                        }

                                        let policy_bound =
                                            sync_policy::is_policy_bound(&authenticated_tx).await;
                                        let snapshot_already_sent =
                                            snapshot_ack::was_sent(&authenticated_tx).await;
                                        snapshot_drive = Some(snapshot_drive::spawn(
                                            storage.clone(),
                                            client_session.peer_device_id().to_owned(),
                                            remote_vector,
                                            snapshot_already_sent,
                                            policy_bound,
                                            tx.clone(),
                                            authenticated_tx.clone(),
                                        ));
                                    }

                                    ClientMessage::SyncChanges {
                                        batch_id,
                                        entities,
                                        is_last,
                                        policy_identity,
                                        usage_complete_through,
                                    } => {
                                        let context = sync_changes::Context {
                                            storage: &storage,
                                            tx: &tx,
                                            peer_device_id: client_session.peer_device_id(),
                                            local_device_id: &device_id,
                                            authenticated_tx: &authenticated_tx,
                                            on_change: &on_change,
                                        };
                                        if let Err(error) = sync_changes::apply(
                                            context, batch_id, entities, is_last,
                                            usage_complete_through,
                                            policy_identity,
                                        ).await {
                                            eprintln!("{TAG} inbound sync batch failed: {error}");
                                            break;
                                        }
                                    }

                                    ClientMessage::SyncAck { batch_id, policy_identity } => {
                                        if !snapshot_ack::handle(
                                            &storage,
                                            client_session.peer_device_id(),
                                            &batch_id,
                                            policy_identity,
                                            &authenticated_tx,
                                            &tx,
                                            &feed_in_flight,
                                        )
                                        .await
                                        {
                                            break;
                                        }
                                    }

                                    ClientMessage::SignedIntegrationFrame(frame) => {
                                        live_messages::handle_signed_integration_message(
                                            client_session.is_ready(),
                                            &storage,
                                            client_session.peer_device_id(),
                                            &device_id,
                                            &space_id,
                                            &tx,
                                            frame,
                                        )
                                        .await;
                                    }

                                    ClientMessage::SignedIntegrationAck => {}

                                    ClientMessage::LiveChange {
                                        change_id,
                                        entity,
                                        policy_identity,
                                    } => {
                                        if let Err(error) = live_messages::handle_live_change(
                                            &storage,
                                            &authenticated_tx,
                                            &device_id,
                                            client_session.peer_device_id(),
                                            &tx,
                                            &on_change,
                                            change_id,
                                            entity,
                                            policy_identity,
                                        )
                                        .await
                                        {
                                            eprintln!("{TAG} live change handling failed: {error}");
                                            break;
                                        }
                                    }

                                    ClientMessage::LiveAck { change_id, policy_identity } => {
                                        if !live_messages::handle_feed_ack_message(
                                            &storage,
                                            client_session.peer_device_id(),
                                            &tx,
                                            &feed_in_flight,
                                            &authenticated_tx,
                                            &change_id,
                                            policy_identity,
                                            false,
                                        )
                                        .await {
                                            break;
                                        }
                                    }

                                    ClientMessage::LiveReceiptAck {
                                        change_id,
                                        policy_identity,
                                    } => {
                                        if !live_messages::handle_feed_ack_message(
                                            &storage,
                                            client_session.peer_device_id(),
                                            &tx,
                                            &feed_in_flight,
                                            &authenticated_tx,
                                            &change_id,
                                            policy_identity,
                                            true,
                                        )
                                        .await {
                                            break;
                                        }
                                    }

                                    ClientMessage::TaskMergeBatch(batch) => {
                                        if let Err(error) = live_messages::handle_task_merge_batch(
                                            &storage,
                                            client_session.peer_device_id(),
                                            &device_id,
                                            &tx,
                                            &authenticated_tx,
                                            batch,
                                        )
                                        .await
                                        {
                                            eprintln!("{TAG} task merge batch rejected: {error}");
                                        }
                                    }

                                    ClientMessage::TaskMergeBatchAck => {}

                                    ClientMessage::Ping { ts } => {
                                        let _ = send_msg(&tx, LanSyncMessage::Pong { ts }).await;
                                    }
                                    ClientMessage::Pong
                                    | ClientMessage::PairingRejected
                                    | ClientMessage::PairingAccepted => {}
                                    ClientMessage::PeerList(peers) => {
                                        if client_session.is_ready() {
                                            if let Some(handler) =
                                                on_peer_list.lock().await.as_ref()
                                            {
                                                handler(peers);
                                            }
                                        }
                                    }
                                    ClientMessage::ProtocolRejected {
                                        origin_device_id,
                                        target_device_id,
                                        reason,
                                    } => {
                                        let authenticated_peer_id = client_session.peer_device_id();
                                        if target_device_id == device_id
                                            && (authenticated_peer_id.is_empty()
                                                || origin_device_id == authenticated_peer_id)
                                        {
                                            eprintln!("{TAG} Peer rejected protocol: {reason}");
                                            break;
                                        }
                                    }
                                    ClientMessage::RegistryControl(sync_msg) => {
                                        include!("connect_and_receive/registry_control.rs");
                                    }
                            }
                        }
                    }
                }

                if let Some(task) = snapshot_drive.take() {
                    task.abort();
                    let _ = task.await;
                }
                if let Err(error) = session_io.finish().await {
                    eprintln!("{TAG} peer session transport shutdown failed: {error:?}");
                }
                *authenticated_tx.lock().await = None;

                if client_session.is_ready() {
                    eprintln!("{TAG} Disconnected from peer");
                    if let Some(handler) = on_disconnected.lock().await.as_ref() {
                        handler(client_session.peer_device_id().to_owned());
                    }
                }
            }
            Ok(None) => {
                eprintln!("{TAG} All addresses failed for {}", peer_record.device_name);
            }
            Err(error) => {
                eprintln!("{TAG} could not start peer session transport: {error:?}");
            }
        }

        if stopped.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
    }
