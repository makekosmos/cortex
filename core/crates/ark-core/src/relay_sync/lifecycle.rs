use super::*;

impl RelaySync {
    pub fn new(storage: Arc<dyn SyncRuntimeStorage>, config: RelaySyncConfig) -> Arc<Self> {
        let auth_secret = normalize_auth_secret(config.auth_secret.clone());
        let transport: Arc<dyn SyncTransport> = Arc::new(RelayTransport::new(RelayConfig {
            url: config.relay_url.clone(),
            space_id: config.space_id.clone(),
            device_id: config.device_id.clone(),
            device_name: config.device_name.clone(),
            api_key: config.relay_api_key.clone().unwrap_or_default(),
            auth_secret: auth_secret.clone(),
        }));
        Self::with_transport(storage, config, transport)
    }

    /// Generalized constructor: build `RelaySync` over an already-constructed
    /// transport. Lets callers (e.g. `handle_start_sync`) drive the same CRDT
    /// orchestration with `RelayTransport`, `IrohTransport` (behind
    /// `IrohTransport`), or any other `SyncTransport` impl, instead of always
    /// constructing a `RelayTransport` internally from `config.relay_url`.
    pub fn with_transport(
        storage: Arc<dyn SyncRuntimeStorage>,
        config: RelaySyncConfig,
        transport: Arc<dyn SyncTransport>,
    ) -> Arc<Self> {
        let auth_secret = normalize_auth_secret(config.auth_secret.clone());
        let _ =
            transport.set_outbound_storage(storage.clone(), &config.space_id, &config.device_id);

        Arc::new(Self {
            storage,
            transport,
            config,
            auth_secret,
            peers: Arc::new(Mutex::new(HashMap::new())),
            transport_keys: Arc::new(Mutex::new(HashMap::new())),
            registry_exchanges: Arc::new(Mutex::new(HashMap::new())),
            registry_ready: Arc::new(Mutex::new(HashMap::new())),
            pending_pairing: Mutex::new(HashMap::new()),
            pending_responder_hellos: Mutex::new(HashMap::new()),
            declined_pairing: Mutex::new(HashMap::new()),
            outgoing_pairing: Mutex::new(None),
            incoming_sync: Arc::new(Mutex::new(None)),
            on_change: Arc::new(Mutex::new(None)),
            on_peer_connect: Arc::new(Mutex::new(None)),
            on_peer_disconnect: Arc::new(Mutex::new(None)),
            on_pairing_changed: Arc::new(Mutex::new(None)),
            tasks: Mutex::new(Vec::new()),
        })
    }

    pub async fn set_on_change(&self, handler: OnChangeCallback) {
        *self.on_change.lock().await = Some(handler);
    }

    pub async fn set_on_peer_connect(&self, handler: OnPeerConnectCallback) {
        *self.on_peer_connect.lock().await = Some(handler);
    }

    pub async fn set_on_peer_disconnect(&self, handler: OnPeerDisconnectCallback) {
        *self.on_peer_disconnect.lock().await = Some(handler);
    }

    pub async fn set_on_pairing_changed(&self, handler: OnPairingChangedCallback) {
        *self.on_pairing_changed.lock().await = Some(handler);
    }

    pub(super) async fn notify_pairing_changed(&self) {
        if let Some(handler) = self.on_pairing_changed.lock().await.as_ref() {
            handler();
        }
    }

    pub async fn start(self: &Arc<Self>) -> Result<(), String> {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<TransportEvent>();
        self.transport.start(event_tx).await?;

        let this = self.clone();
        self.tasks.lock().await.push(tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                this.handle_event(event).await;
            }
        }));
        let incoming_sync = self.incoming_sync.clone();
        self.tasks.lock().await.push(tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let mut incoming = incoming_sync.lock().await;
                let stale = incoming
                    .as_ref()
                    .is_some_and(|state| state.last_update.elapsed() >= Duration::from_secs(60));
                if stale {
                    *incoming = None;
                    drop(incoming);
                    trim_process_heap();
                }
            }
        }));

        Ok(())
    }

    /// Stop transport, then abort and join the background tasks — both
    /// clone `self` and would otherwise keep `storage` alive past teardown.
    pub async fn stop(&self) {
        self.transport.stop().await;
        let tasks: Vec<_> = self.tasks.lock().await.drain(..).collect();
        for task in &tasks {
            task.abort();
        }
        for task in tasks {
            let _ = task.await;
        }
    }

    pub fn broadcast_live_change(&self, entity: SyncEntity) -> Result<(), String> {
        self.transport.send(LanSyncMessage::LiveChange {
            change_id: generate_id(),
            entity,
            origin_device_id: Some(self.config.device_id.clone()),
        })
    }

    pub async fn get_connected_peer_entries(&self) -> Vec<PeerEntry> {
        self.peers
            .lock()
            .await
            .iter()
            .filter_map(|(device_id, peer)| {
                if peer.authenticated {
                    Some(PeerEntry {
                        device_id: device_id.clone(),
                        device_name: peer.device_name.clone(),
                        platform: peer.platform.clone(),
                        app_version: peer.app_version.clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}
