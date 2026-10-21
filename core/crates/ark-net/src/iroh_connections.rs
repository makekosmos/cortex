/// Runs one long-lived Iroh connection with separate reader and writer tasks.
// Each field is an independently-owned channel/endpoint the connection pumps
// between; the struct only names the bundle the spawn sites already clone.
struct ConnectionParams {
    conn: iroh::endpoint::Connection,
    is_dialer: bool,
    hello: LanSyncMessage,
    out_rx: broadcast::Receiver<OutgoingMessage>,
    stop_rx: watch::Receiver<bool>,
    event_tx: mpsc::UnboundedSender<TransportEvent>,
    registry: Arc<DeviceRegistry>,
    connections: Arc<Mutex<HashMap<EndpointId, (u64, iroh::endpoint::Connection)>>>,
    connection_generation: Arc<std::sync::atomic::AtomicU64>,
    outbound_authorizer:
        Arc<tokio::sync::RwLock<Option<Arc<dyn OutboundIntegrationFrameAuthorizer>>>>,
}

fn is_current_connection(
    connections: &Arc<Mutex<HashMap<EndpointId, (u64, iroh::endpoint::Connection)>>>,
    endpoint_id: &EndpointId,
    generation: u64,
) -> bool {
    connections
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(endpoint_id)
        .is_some_and(|(current_generation, _)| *current_generation == generation)
}

fn take_current_connection<T>(
    connections: &mut HashMap<EndpointId, (u64, T)>,
    endpoint_id: &EndpointId,
    generation: u64,
) -> Option<T> {
    if connections
        .get(endpoint_id)
        .is_some_and(|(current_generation, _)| *current_generation == generation)
    {
        connections
            .remove(endpoint_id)
            .map(|(_, connection)| connection)
    } else {
        None
    }
}

async fn authorize_signed_integration_frame(
    registry: &DeviceRegistry,
    outbound_authorizer: &tokio::sync::RwLock<Option<Arc<dyn OutboundIntegrationFrameAuthorizer>>>,
    frame: &SignedSyncEnvelope,
    remote_endpoint_id: &EndpointId,
) -> Result<(), String> {
    let target_is_current = registry
        .data_ready_endpoint(&frame.recipient_node_id)
        .is_some_and(|endpoint| endpoint == *remote_endpoint_id);
    if !target_is_current {
        return Err("iroh addressed transport peer is no longer authorized".into());
    }
    let authorizer = outbound_authorizer
        .read()
        .await
        .clone()
        .ok_or_else(|| "iroh outbound integration authorization is unavailable".to_string())?;
    authorizer
        .authorize(frame, &remote_endpoint_id.to_string())
        .await
}

fn is_pre_admission_control_message(message: &LanSyncMessage) -> bool {
    matches!(
        message,
        LanSyncMessage::Hello { .. }
            | LanSyncMessage::PairingRejected { .. }
            | LanSyncMessage::PairingAccepted { .. }
            | LanSyncMessage::ProtocolRejected { .. }
            | LanSyncMessage::RegistrySyncStart { .. }
            | LanSyncMessage::RegistryEventsPage { .. }
            | LanSyncMessage::RegistrySyncReady { .. }
            | LanSyncMessage::RegistryEventUpdate { .. }
            | LanSyncMessage::SyncPolicy { .. }
    )
}

fn control_message_is_safe_without_admission(message: &LanSyncMessage, addressed: bool) -> bool {
    if matches!(message, LanSyncMessage::RegistryEventUpdate { .. }) {
        return addressed;
    }
    is_pre_admission_control_message(message)
}

async fn handle_connection(params: ConnectionParams) {
    let ConnectionParams {
        conn,
        is_dialer,
        hello,
        mut out_rx,
        stop_rx,
        event_tx,
        registry,
        connections,
        connection_generation,
        outbound_authorizer,
    } = params;
    let remote_endpoint_id = conn.remote_id();
    let role = if is_dialer { "dialer" } else { "listener" };
    let generation = connection_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    if let Some(device_id) = registry.device_id_for(&remote_endpoint_id) {
        registry.unbind_peer(&device_id);
    }
    let replaced = connections
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(remote_endpoint_id, (generation, conn.clone()));
    if let Some((_, previous)) = replaced {
        previous.close(0u32.into(), b"replaced by newer connection");
    }

    // ── Получаем единственный bi-стрим для всего соединения. ─────────────────
    let (mut send, recv) = if is_dialer {
        match conn.open_bi().await {
            Ok(streams) => streams,
            Err(e) => {
                eprintln!("[iroh] open_bi failed ({role}): {e}");
                return;
            }
        }
    } else {
        match conn.accept_bi().await {
            Ok(streams) => streams,
            Err(e) => {
                eprintln!("[iroh] accept_bi failed ({role}): {e}");
                return;
            }
        }
    };
    eprintln!("[iroh] bi-stream established ({role}) remote={remote_endpoint_id}");

    // ── Инжектируем Hello как первый фрейм. ──────────────────────────────────
    // Запускает CRDT-рукопожатие через RelaySync::handle_message без явного
    // вызова send() сверху — зеркалит relay_transport.rs строки 128-148.
    let hello_payload = serialize_message(&hello);
    if let Err(e) = write_frame(&mut send, hello_payload.as_bytes()).await {
        eprintln!("[iroh] write Hello frame failed ({role}): {e}");
        return;
    }
    eprintln!("[iroh] Hello injected ({role}) remote={remote_endpoint_id}");

    // ── Резолвим наш device_id для Connected/Disconnected событий. ────────────
    let our_device_id = match &hello {
        LanSyncMessage::Hello { device_id, .. } => device_id.clone(),
        _ => String::new(),
    };

    let _ = event_tx.send(TransportEvent::Connected {
        device_id: our_device_id.clone(),
    });

    // ── Shared teardown notify: первый завершившийся таск будит второй. ───────
    let done_notify = Arc::new(Notify::new());

    // ── Writer task: владеет `send`. ──────────────────────────────────────────
    // Никогда не конкурирует с reader в одном select! — поэтому read_frame
    // больше не может быть дропнут во время частичного чтения.
    // A JoinSet — not two detached handles: when the transport's `stop()`
    // aborts this connection task, the set drops and aborts writer and
    // reader, so they cannot outlive teardown holding the authorizer
    // (KOS-369).
    let mut conn_tasks = tokio::task::JoinSet::new();

    let writer_notify = done_notify.clone();
    let writer_registry = registry.clone();
    let writer_connections = connections.clone();
    let mut writer_stop_rx = stop_rx.clone();

    conn_tasks.spawn(async move {
        let mut send = send; // move into task
        loop {
            tokio::select! {
                biased; // check stop first

                _ = writer_stop_rx.changed() => {
                    if *writer_stop_rx.borrow() {
                        eprintln!("[iroh] writer stop signal ({role}) remote={remote_endpoint_id}");
                        let _ = send.finish();
                        break;
                    }
                }

                _ = writer_notify.notified() => {
                    // Reader ended — tear down writer.
                    eprintln!(
                        "[iroh] writer notified of reader end ({role}) remote={remote_endpoint_id}"
                    );
                    let _ = send.finish();
                    break;
                }

                recv_result = out_rx.recv() => {
                    match recv_result {
                        Ok(outgoing) => {
                            if !is_current_connection(
                                &writer_connections,
                                &remote_endpoint_id,
                                generation,
                            ) {
                                break;
                            }
                            if outgoing.target.is_some_and(|target| target != remote_endpoint_id) {
                                continue;
                            }
                            if !writer_registry.is_data_ready_endpoint(&remote_endpoint_id)
                                && !control_message_is_safe_without_admission(
                                    &outgoing.msg,
                                    outgoing.target.is_some(),
                                )
                            {
                                if let Some(completion) = outgoing.completion {
                                    if let Some(
                                        tx
                                    ) = completion.lock().unwrap_or_else(
                                        |e| e.into_inner()
                                    ).take() {
                                        let _ = tx.send(Err(
                                            "iroh transport peer is not authenticated".into()
                                        ));
                                    }
                                }
                                continue;
                            }
                            let variant = message_variant_name(&outgoing.msg);
                            let authorization = if let LanSyncMessage::SignedIntegrationFrame {
                                frame
                            } = &outgoing.msg {
                                authorize_signed_integration_frame(
                                    &writer_registry,
                                    &outbound_authorizer,
                                    frame,
                                    &remote_endpoint_id,
                                )
                                .await
                            } else {
                                Ok(())
                            };
                            if let Err(error) = authorization {
                                if let Some(completion) = outgoing.completion {
                                    if let Some(
                                        tx
                                    ) = completion.lock().unwrap_or_else(
                                        |e| e.into_inner()
                                    ).take() {
                                        let _ = tx.send(Err(error));
                                    }
                                }
                                continue;
                            }
                            if !is_current_connection(
                                &writer_connections,
                                &remote_endpoint_id,
                                generation,
                            ) {
                                break;
                            }
                            let payload = serialize_message(&outgoing.msg);
                            if let Err(e) = write_frame(&mut send, payload.as_bytes()).await {
                                if let Some(completion) = outgoing.completion {
                                    if let Some(
                                        tx
                                    ) = completion.lock().unwrap_or_else(
                                        |e| e.into_inner()
                                    ).take() {
                                        let _ = tx.send(Err(e.to_string()));
                                    }
                                }
                                eprintln!("[iroh] write_frame failed ({role}) msg={variant}: {e}");
                                break;
                            }
                            if let Some(completion) = outgoing.completion {
                                if let Some(
                                    tx
                                ) = completion.lock().unwrap_or_else(|e| e.into_inner()).take() {
                                    let _ = tx.send(Ok(()));
                                }
                            }
                            eprintln!(
                                "[iroh] → sent {variant} ({role}) remote={remote_endpoint_id}"
                            );
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            // Burst превысил ёмкость — пропускаем; VV-обмен
                            // компенсирует при следующем коннекте.
                            eprintln!("[iroh] broadcast lagged by {n} messages ({role})");
                            continue;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Sender дропнут — транспорт остановлен.
                            eprintln!("[iroh] broadcast sender closed ({role})");
                            break;
                        }
                    }
                }
            }
        }
    });

    // ── Reader task: владеет `recv`. ──────────────────────────────────────────
    // loop на read_frame — не делит select! с writer, поэтому cancellation-
    // safe: read_exact никогда не дропается на полуслове.
    let reader_notify = done_notify.clone();
    let reader_registry = registry.clone();
    let reader_connections = connections.clone();
    let reader_event_tx = event_tx.clone();

    conn_tasks.spawn(async move {
        let mut recv = recv; // move into task
        loop {
            match read_frame(&mut recv).await {
                Ok(raw) => {
                    if let Some(msg) = deserialize_message(&raw) {
                        if !is_current_connection(
                            &reader_connections,
                            &remote_endpoint_id,
                            generation,
                        ) {
                            break;
                        }
                        let variant = message_variant_name(&msg);

                        // На Hello учим реестр (EndpointId → CRDT device_id).
                        if let LanSyncMessage::Hello { device_id, .. } = &msg {
                            if !reader_registry
                                .insert_untrusted(remote_endpoint_id, device_id.clone())
                            {
                                eprintln!(
                                    "[iroh] rejected Hello remap for trusted endpoint \
                                         {remote_endpoint_id}"
                                );
                                continue;
                            }
                        }

                        if !reader_registry.is_data_ready_endpoint(&remote_endpoint_id)
                            && !is_pre_admission_control_message(&msg)
                        {
                            eprintln!(
                                "[iroh] dropped pre-admission frame {} remote={remote_endpoint_id}",
                                message_variant_name(&msg)
                            );
                            continue;
                        }

                        // Once paired, the endpoint registry is authoritative.
                        // Only an initial Hello may establish the claimed id;
                        // later message fields cannot replace that binding.
                        let from_device_id = reader_registry
                            .device_id_for(&remote_endpoint_id)
                            .or_else(|| {
                                matches!(&msg, LanSyncMessage::Hello { .. })
                                    .then(|| message_origin_device_id(&msg))
                                    .flatten()
                            })
                            .unwrap_or_default();

                        eprintln!(
                            "[iroh] ← recv {variant} from={from_device_id} ({role}) \
                                 remote={remote_endpoint_id}"
                        );

                        let _ =
                            reader_event_tx.send(TransportEvent::MessageReceivedFromTransport {
                                from_device_id,
                                transport_public_key: remote_endpoint_id.to_string(),
                                msg,
                            });
                    }
                }
                Err(e) => {
                    // Стрим закрыт или сброшен — будим writer и выходим.
                    eprintln!("[iroh] read_frame ended ({role}) remote={remote_endpoint_id}: {e}");
                    break;
                }
            }
        }
        // Notify writer to shut down.
        reader_notify.notify_one();
    });

    // ── Ждём завершения обоих тасков. ─────────────────────────────────────────
    // handle_connection должна вернуться только после полного завершения,
    // чтобы dial-loop мог переподключиться.
    //
    // Если stop_rx срабатывает здесь, writer увидит его сам (он тоже слушает
    // stop_rx). Мы просто ждём join'а.
    while conn_tasks.join_next().await.is_some() {}
    let disconnected_connection = {
        let mut connections = connections
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        take_current_connection(&mut connections, &remote_endpoint_id, generation)
    };
    if disconnected_connection.is_some() {
        let device_id = registry
            .device_id_for(&remote_endpoint_id)
            .unwrap_or(our_device_id);
        registry.remove_endpoint(&remote_endpoint_id);
        let _ = event_tx.send(TransportEvent::Disconnected { device_id });
    }

    eprintln!("[iroh] connection closed ({role}) remote={remote_endpoint_id}");
}

#[cfg(test)]
#[path = "iroh_connections_tests.rs"]
mod tests;
