#[async_trait::async_trait]
impl SyncTransport for IrohTransport {
    /// Запускает транспорт: байндит `Endpoint`, поднимает accept-loop для
    /// входящих соединений и (если `config.peer_addr/peer_ticket`) dial-loop
    /// с экспоненциальным backoff.
    async fn start(&self, event_tx: mpsc::UnboundedSender<TransportEvent>) -> Result<(), String> {
        let relay_mode = self.config.relay_mode.clone().unwrap_or(RelayMode::Default);

        let mut builder = Endpoint::builder(presets::Minimal)
            .relay_mode(relay_mode)
            .alpns(vec![ARK_SYNC_ALPN.to_vec()]);

        if let Some(secret_key) = self.config.secret_key.clone() {
            builder = builder.secret_key(secret_key);
        }

        let endpoint = builder
            .bind()
            .await
            .map_err(|e| format!("iroh transport: endpoint bind failed: {e}"))?;

        let our_endpoint_id = endpoint.id();
        eprintln!(
            "[iroh] start: endpoint={our_endpoint_id} device={} role=listening",
            self.config.device_id
        );

        *self.endpoint.lock().unwrap_or_else(|e| e.into_inner()) = Some(endpoint.clone());

        // ── Accept-loop ───────────────────────────────────────────────────────
        // Каждое входящее соединение порождает `handle_connection(is_dialer=false)`.
        {
            let accept_endpoint = endpoint.clone();
            let accept_event_tx = event_tx.clone();
            let accept_registry = self.registry.clone();
            let mut accept_stop = self.stop_rx.clone();
            let out_tx = self.out_tx.clone();
            let hello = self.make_hello();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = accept_stop.changed() => {
                            if *accept_stop.borrow() { return; }
                        }
                        incoming = accept_endpoint.accept() => {
                            let Some(incoming) = incoming else {
                                // Endpoint закрыт.
                                return;
                            };
                            let event_tx = accept_event_tx.clone();
                            let registry = accept_registry.clone();
                            let out_rx = out_tx.subscribe();
                            let stop_rx = accept_stop.clone();
                            let hello = hello.clone();

                            tokio::spawn(async move {
                                let conn = match incoming.await {
                                    Ok(conn) => conn,
                                    Err(e) => {
                                        eprintln!("[iroh] incoming connection failed: {e}");
                                        return;
                                    }
                                };
                                eprintln!("[iroh] accepted incoming connection remote={}", conn.remote_id());
                                handle_connection(
                                    conn,
                                    false, // is_dialer
                                    hello,
                                    out_rx,
                                    stop_rx,
                                    event_tx,
                                    registry,
                                )
                                .await;
                            });
                        }
                    }
                }
            });
        }

        // ── Dial-loop (если задан peer) ───────────────────────────────────────
        // Подключается к пиру, запускает `handle_connection(is_dialer=true)`,
        // при разрыве переподключается с экспоненциальным backoff.
        if self.config.peer_addr.is_some() || self.config.peer_ticket.is_some() {
            let peer_addr = self.resolve_peer_addr()?;
            eprintln!(
                "[iroh] start: dialing peer={} device={}",
                peer_addr.id, self.config.device_id
            );
            let dial_endpoint = endpoint.clone();
            let dial_event_tx = event_tx.clone();
            let dial_registry = self.registry.clone();
            let mut dial_stop = self.stop_rx.clone();
            let out_tx = self.out_tx.clone();
            let hello = self.make_hello();

            tokio::spawn(async move {
                let mut backoff_secs: u64 = 1;

                loop {
                    // Проверяем stop до попытки коннекта.
                    if *dial_stop.borrow() {
                        return;
                    }

                    eprintln!(
                        "[iroh] dial attempt peer={} (retry_in={backoff_secs}s if fail)",
                        peer_addr.id
                    );
                    match dial_endpoint
                        .connect(peer_addr.clone(), ARK_SYNC_ALPN)
                        .await
                    {
                        Err(e) => {
                            eprintln!(
                                "[iroh] connect to peer={} failed: {e}; retry in {backoff_secs}s",
                                peer_addr.id
                            );
                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_secs(backoff_secs)) => {}
                                _ = dial_stop.changed() => {
                                    if *dial_stop.borrow() { return; }
                                }
                            }
                            backoff_secs = (backoff_secs * 2).min(60);
                        }
                        Ok(conn) => {
                            eprintln!("[iroh] connected to peer={}", conn.remote_id());
                            backoff_secs = 1; // сбрасываем backoff при успехе
                            let out_rx = out_tx.subscribe();
                            let stop_rx = dial_stop.clone();
                            handle_connection(
                                conn,
                                true, // is_dialer
                                hello.clone(),
                                out_rx,
                                stop_rx,
                                dial_event_tx.clone(),
                                dial_registry.clone(),
                            )
                            .await;

                            // handle_connection вернулась — соединение закрыто.
                            // Переподключаемся с backoff (если не остановлены).
                            if *dial_stop.borrow() {
                                return;
                            }
                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_secs(backoff_secs)) => {}
                                _ = dial_stop.changed() => {
                                    if *dial_stop.borrow() { return; }
                                }
                            }
                            backoff_secs = (backoff_secs * 2).min(60);
                        }
                    }
                }
            });
        }

        Ok(())
    }

    /// Ставит сообщение в broadcast-канал исходящих. Все активные соединения
    /// получат его через свои `out_rx`. Если подписчиков нет (соединение ещё
    /// не установлено), сообщение молча дропается: pre-connect live-changes
    /// будут скомпенсированы VersionVector обменом при (ре)коннекте.
    fn send(&self, msg: LanSyncMessage) -> Result<(), String> {
        // send() возвращает Err только если нет подписчиков — это нормально
        // (нет активного соединения). Не возвращаем ошибку вызывающему.
        let _ = self.out_tx.send(msg);
        Ok(())
    }

    /// Сигнализирует всем фоновым задачам остановиться и закрывает `Endpoint`.
    fn stop(&self) {
        let _ = self.stop_tx.send(true);
        // Закрываем endpoint явно — это разбудит accept_endpoint.accept().
        if let Some(ep) = self
            .endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            // close() — async fn, но мы на sync пути. Spawn best-effort close.
            tokio::spawn(async move {
                ep.close().await;
            });
        }
    }
}
