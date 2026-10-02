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
    outbound_storage: Arc<tokio::sync::RwLock<Option<OutboundStorage>>>,
}

// Checks that the addressed transport peer still owns this connection and that
// the outbound storage allows the signed frame for this transport endpoint.
async fn authorize_signed_integration_frame(
    registry: &DeviceRegistry,
    outbound_storage: &tokio::sync::RwLock<Option<OutboundStorage>>,
    frame: &crate::integration_replication::SignedSyncEnvelope,
    remote_endpoint_id: &EndpointId,
) -> Result<(), String> {
    let target_is_current = registry
        .authenticated_endpoint(&frame.recipient_node_id)
        .is_some_and(|endpoint| endpoint == *remote_endpoint_id);
    if !target_is_current {
        return Err("iroh addressed transport peer is no longer authorized".into());
    }
    match outbound_storage.read().await.clone() {
        Some(context) => {
            context
                .storage
                .validate_outbound_signed_integration_frame_with_transport(
                    frame,
                    &context.space_id,
                    &context.origin_node_id,
                    &remote_endpoint_id.to_string(),
                )
                .await
        }
        None => Err("iroh outbound integration authorization is unavailable".into()),
    }
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
        outbound_storage,
    } = params;
    let remote_endpoint_id = conn.remote_id();
    let role = if is_dialer { "dialer" } else { "listener" };

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
    let writer_notify = done_notify.clone();
    let writer_registry = registry.clone();
    let writer_event_tx = event_tx.clone();
    let writer_our_device_id = our_device_id.clone();
    let mut writer_stop_rx = stop_rx.clone();

    let writer_handle = tokio::spawn(async move {
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
                            if outgoing.target.is_some_and(|target| target != remote_endpoint_id) {
                                continue;
                            }
                            let variant = message_variant_name(&outgoing.msg);
                            let authorization = if let LanSyncMessage::SignedIntegrationFrame {
                                frame
                            } = &outgoing.msg {
                                authorize_signed_integration_frame(
                                    &writer_registry,
                                    &outbound_storage,
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

        // Resolve final device_id for Disconnected from registry.
        let disconnected_device_id = writer_registry
            .device_id_for(&remote_endpoint_id)
            .unwrap_or(writer_our_device_id);
        writer_registry.remove_endpoint(&remote_endpoint_id);
        let _ = writer_event_tx.send(TransportEvent::Disconnected {
            device_id: disconnected_device_id,
        });
    });

    // ── Reader task: владеет `recv`. ──────────────────────────────────────────
    // loop на read_frame — не делит select! с writer, поэтому cancellation-
    // safe: read_exact никогда не дропается на полуслове.
    let reader_notify = done_notify.clone();
    let reader_registry = registry.clone();
    let reader_event_tx = event_tx.clone();

    let reader_handle = tokio::spawn(async move {
        let mut recv = recv; // move into task
        loop {
            match read_frame(&mut recv).await {
                Ok(raw) => {
                    if let Some(msg) = deserialize_message(&raw) {
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

                        // Резолвим from_device_id: сначала из самого сообщения
                        // (Hello/VersionVector/LiveChange с origin_device_id),
                        // иначе — из реестра по EndpointId соединения.
                        let from_device_id = message_origin_device_id(&msg)
                            .or_else(|| reader_registry.device_id_for(&remote_endpoint_id))
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
    let _ = writer_handle.await;
    let _ = reader_handle.await;

    eprintln!("[iroh] connection closed ({role}) remote={remote_endpoint_id}");
}
