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

        // The builder comes pre-configured to bind 0.0.0.0 and [::]; in
        // loopback mode replace both per-family defaults with loopback
        // sockets so the endpoint never listens on a LAN interface. The
        // IPv6 bind is not required: hosts without an IPv6 stack must
        // still start.
        if self.config.bind == SyncBind::Loopback {
            let (v4, v6) = self.config.bind.iroh_bind_addrs();
            builder = builder
                .bind_addr(v4)
                .map_err(|e| format!("iroh transport: invalid bind {v4}: {e}"))?
                .bind_addr_with_opts(v6, BindOpts::default().set_is_required(false))
                .map_err(|e| format!("iroh transport: invalid bind {v6}: {e}"))?;
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

        // ── Relay connectivity watcher ───────────────────────────────────────
        // iroh's relay actor retries internally and warns per attempt — the
        // Engine filters those to error (runtime `init_tracing`), so one WARN
        // per up→down / down→up transition is the signal that survives.
        spawn_relay_connectivity_watch(
            &endpoint,
            self.config.relay_mode.as_ref(),
            self.stop_rx.clone(),
        );

        // ── Accept-loop ───────────────────────────────────────────────────────
        // Каждое входящее соединение порождает `handle_connection(is_dialer=false)`.
        {
            let accept_endpoint = endpoint.clone();
            let accept_event_tx = event_tx.clone();
            let accept_registry = self.registry.clone();
            let accept_connections = self.connections.clone();
            let accept_outbound_storage = self.outbound_storage.clone();
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
                            let connections = accept_connections.clone();
                            let outbound_storage = accept_outbound_storage.clone();
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
                                eprintln!(
                                    "[iroh] accepted incoming connection remote={}",
                                    conn.remote_id(),
                                );
                                handle_connection(ConnectionParams {
                                    conn,
                                    is_dialer: false,
                                    hello,
                                    out_rx,
                                    stop_rx,
                                    event_tx,
                                    registry,
                                    connections,
                                    outbound_storage,
                                })
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
            let dial_connections = self.connections.clone();
            let dial_outbound_storage = self.outbound_storage.clone();
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
                            handle_connection(ConnectionParams {
                                conn,
                                is_dialer: true,
                                hello: hello.clone(),
                                out_rx,
                                stop_rx,
                                event_tx: dial_event_tx.clone(),
                                registry: dial_registry.clone(),
                                connections: dial_connections.clone(),
                                outbound_storage: dial_outbound_storage.clone(),
                            })
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
        if matches!(
            &msg,
            LanSyncMessage::SignedIntegrationFrame { .. }
                | LanSyncMessage::SignedIntegrationAck { .. }
        ) {
            return Err("iroh broadcast cannot carry addressed integration messages".into());
        }
        let _ = self.out_tx.send(OutgoingMessage {
            target: None,
            msg,
            completion: None,
        });
        Ok(())
    }

    async fn send_to(&self, device_id: &str, msg: LanSyncMessage) -> Result<(), String> {
        let target = self
            .registry
            .authenticated_endpoint(device_id)
            .ok_or_else(|| "iroh addressed peer is not authenticated".to_string())?;
        if let LanSyncMessage::SignedIntegrationFrame { frame } = &msg {
            if frame.recipient_node_id != device_id {
                return Err("iroh frame recipient does not match addressed peer".into());
            }
        }
        if !matches!(
            &msg,
            LanSyncMessage::SignedIntegrationFrame { .. }
                | LanSyncMessage::SignedIntegrationAck { .. }
        ) {
            return Err("iroh send_to only supports addressed integration messages".into());
        }
        if let LanSyncMessage::SignedIntegrationFrame { frame } = &msg {
            let context = self.outbound_storage.read().await.clone().ok_or_else(|| {
                "iroh outbound integration authorization is unavailable".to_string()
            })?;
            context
                .storage
                .validate_outbound_signed_integration_frame_with_transport(
                    frame,
                    &context.space_id,
                    &context.origin_node_id,
                    &target.to_string(),
                )
                .await?;
        }
        let (completion_tx, completion_rx) = tokio::sync::oneshot::channel();
        self.out_tx
            .send(OutgoingMessage {
                target: Some(target),
                msg,
                completion: Some(Arc::new(std::sync::Mutex::new(Some(completion_tx)))),
            })
            .map_err(|_| "iroh addressed peer is not connected".to_string())?;
        tokio::time::timeout(Duration::from_secs(5), completion_rx)
            .await
            .map_err(|_| "iroh addressed writer timed out".to_string())?
            .map_err(|_| "iroh addressed writer stopped".to_string())?
    }

    fn set_outbound_storage(
        &self,
        storage: Arc<dyn crate::sync_server::StorageBackend>,
        space_id: &str,
        origin_node_id: &str,
    ) -> Result<(), String> {
        if let Ok(mut guard) = self.outbound_storage.try_write() {
            *guard = Some(OutboundStorage {
                storage,
                space_id: space_id.to_string(),
                origin_node_id: origin_node_id.to_string(),
            });
        } else {
            return Err("iroh outbound authorization context is busy".into());
        }
        Ok(())
    }

    fn bind_authenticated_peer(
        &self,
        device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        if self
            .registry
            .bind_authenticated(device_id, transport_public_key)
        {
            Ok(())
        } else {
            Err("iroh transport identity does not match peer endpoint".into())
        }
    }

    fn disconnect_peer(&self, device_id: &str) -> Result<(), String> {
        let endpoint = self
            .registry
            .endpoint_id_for(device_id)
            .ok_or_else(|| format!("iroh peer {device_id} has no bound endpoint"))?;
        self.disconnect_transport_peer(&endpoint.to_string())
    }

    fn disconnect_transport_peer(&self, transport_public_key: &str) -> Result<(), String> {
        let endpoint_id: EndpointId = transport_public_key
            .parse()
            .map_err(|_| "iroh transport key is not an endpoint id".to_string())?;
        let conn = self
            .connections
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&endpoint_id);
        match conn {
            Some(conn) => {
                conn.close(0u32.into(), b"peer removed");
                self.registry.remove_endpoint(&endpoint_id);
                Ok(())
            }
            None => Err("iroh peer has no live connection".to_string()),
        }
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
