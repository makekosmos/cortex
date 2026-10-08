pub struct IrohTransport {
    config: IrohConfig,
    /// Запущенный `Endpoint` после `start()`. `None` до старта.
    endpoint: Mutex<Option<Endpoint>>,
    /// Broadcast-канал исходящих сообщений — `send()` кладёт сюда, каждая
    /// активная connection-задача подписывается и пишет в свой QUIC bi-стрим.
    out_tx: broadcast::Sender<OutgoingMessage>,
    /// Shutdown watch channel. `true` = нужно остановиться.
    stop_tx: watch::Sender<bool>,
    stop_rx: watch::Receiver<bool>,
    /// Шаг 3: device_id ↔ EndpointId реестр, заполняется по входящим Hello.
    registry: std::sync::Arc<DeviceRegistry>,
    outbound_storage: Arc<tokio::sync::RwLock<Option<OutboundStorage>>>,
}

impl IrohTransport {
    pub fn new(config: IrohConfig) -> Self {
        let (out_tx, _) = broadcast::channel(OUTGOING_BROADCAST_CAPACITY);
        let (stop_tx, stop_rx) = watch::channel(false);
        IrohTransport {
            config,
            endpoint: Mutex::new(None),
            out_tx,
            stop_tx,
            stop_rx,
            registry: std::sync::Arc::new(DeviceRegistry::new()),
            outbound_storage: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Текущий снимок реестра device_id ↔ EndpointId (для тестов/диагностики).
    #[doc(hidden)]
    pub fn registry(&self) -> &DeviceRegistry {
        &self.registry
    }

    /// Тестовый хук: то же кодирование, что использует `our_ticket()`, но
    /// без необходимости поднимать реальный `Endpoint`.
    #[doc(hidden)]
    pub fn ticket_string_for_addr(addr: &EndpointAddr) -> String {
        ticket_string(addr)
    }

    /// Возвращает нашу ticket-строку — то, что нужно передать другому
    /// устройству для pairing. Требует, чтобы транспорт уже был запущен.
    ///
    /// BUG 2 FIX: `endpoint.online().await` может висеть ~50s на реальной сети
    /// (relay homing). Оборачиваем в 8-секундный таймаут. При таймауте логируем
    /// предупреждение и используем `endpoint.addr()` best-effort — прямые адреса
    /// могут сработать даже без relay homing. Не возвращаем ошибку: это не
    /// блокирующий путь. Relay homing может завершиться позже.
    pub async fn our_ticket(&self) -> Result<String, String> {
        let relay_disabled = matches!(self.config.relay_mode, Some(RelayMode::Disabled));
        if relay_disabled {
            let addr = self
                .endpoint_addr()
                .ok_or_else(|| "iroh transport: our_ticket() called before start()".to_string())?;
            return Ok(ticket_string(&addr));
        }

        let endpoint = self
            .endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| "iroh transport: our_ticket() called before start()".to_string())?;

        // Wait for relay homing with an 8-second timeout to avoid blocking
        // unrelated RPC calls (e.g. app_index.list_all, pomodoro.get_state)
        // which would time out at 30s if our_ticket() hangs here for ~50s.
        match tokio::time::timeout(Duration::from_secs(8), endpoint.online()).await {
            Ok(()) => {
                // Relay homing completed — addr now includes relay URL.
            }
            Err(_elapsed) => {
                eprintln!(
                    "[iroh] WARNING: endpoint.online() timed out after 8s; \
                     building ticket from best-effort local addr (relay homing may complete \
                     later)"
                );
            }
        }
        Ok(ticket_string(&endpoint.addr()))
    }

    /// Резолвит эффективный адрес пира: явный `peer_addr` имеет приоритет,
    /// иначе парсим `peer_ticket`.
    fn resolve_peer_addr(&self) -> Result<EndpointAddr, String> {
        if let Some(addr) = self.config.peer_addr.clone() {
            return Ok(addr);
        }
        let ticket = self.config.peer_ticket.as_deref().ok_or_else(|| {
            "iroh transport: send() requires config.peer_addr or config.peer_ticket".to_string()
        })?;
        from_ticket(ticket)
    }

    /// Возвращает локальный `EndpointId` транспорта, если он уже запущен.
    pub fn endpoint_id(&self) -> Option<EndpointId> {
        self.endpoint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|ep| ep.id())
    }

    /// Возвращает локальный адрес транспорта (для ручной передачи пиру в
    /// фазе 0, без pairing UI/discovery). Использует `bound_sockets()` +
    /// явную подстановку `127.0.0.1`.
    pub fn endpoint_addr(&self) -> Option<EndpointAddr> {
        let guard = self.endpoint.lock().unwrap_or_else(|e| e.into_inner());
        let ep = guard.as_ref()?;
        let port = ep
            .bound_sockets()
            .iter()
            .find(|addr| addr.is_ipv4())
            .map(|addr| addr.port())?;
        let socket: SocketAddr = (Ipv4Addr::LOCALHOST, port).into();
        Some(EndpointAddr::new(ep.id()).with_ip_addr(socket))
    }

    /// Строит аутентифицированный `Hello` для инжекции при подключении.
    /// Зеркалит `relay_transport.rs` строки 128-148.
    fn make_hello(&self) -> LanSyncMessage {
        let auth_secret = normalize_auth_secret(self.config.auth_secret.clone());
        let (auth_nonce, auth_hmac) = match auth_secret.as_ref() {
            Some(secret) => {
                let nonce = generate_auth_nonce();
                let hmac = compute_hello_auth_hmac(
                    secret,
                    &self.config.space_id,
                    &self.config.device_id,
                    &nonce,
                );
                (Some(nonce), Some(hmac))
            }
            None => (None, None),
        };
        LanSyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: self.config.device_id.clone(),
            device_name: self.config.device_name.clone(),
            space_id: self.config.space_id.clone(),
            addresses: None,
            auth_nonce,
            auth_hmac,
            platform: Some(crate::host::local_platform()),
            app_version: crate::host::app_version(),
        }
    }
}
