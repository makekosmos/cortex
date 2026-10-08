impl RelaySync {
    pub fn new(storage: Arc<dyn StorageBackend>, config: RelaySyncConfig) -> Arc<Self> {
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
        storage: Arc<dyn StorageBackend>,
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
            incoming_sync: Arc::new(Mutex::new(None)),
            on_change: Arc::new(Mutex::new(None)),
            on_peer_connect: Arc::new(Mutex::new(None)),
            on_peer_disconnect: Arc::new(Mutex::new(None)),
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
        self.transport.stop();
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

    async fn handle_message(
        &self,
        from_device_id: String,
        msg: LanSyncMessage,
        transport_public_key: Option<String>,
    ) {
        match msg {
            LanSyncMessage::Hello {
                protocol_version,
                device_id,
                device_name,
                space_id,
                auth_nonce,
                auth_hmac,
                platform,
                app_version,
                ..
            } => {
                if protocol_version != PROTOCOL_VERSION {
                    eprintln!("{TAG} protocol mismatch from relay peer {device_id}");
                    return;
                }
                if space_id != self.config.space_id {
                    return;
                }
                if device_id == self.config.device_id {
                    return;
                }

                if let Some(secret) = self.auth_secret.as_ref() {
                    let valid = match (auth_nonce.as_deref(), auth_hmac.as_deref()) {
                        (Some(nonce), Some(hmac)) => {
                            verify_hello_auth_hmac(secret, &space_id, &device_id, nonce, hmac)
                        }
                        _ => false,
                    };
                    if !valid {
                        eprintln!("{TAG} rejecting relay hello with invalid HMAC");
                        return;
                    }
                }

                let was_new = {
                    let mut peers = self.peers.lock().await;
                    let was_new = !peers.contains_key(&device_id);
                    peers.insert(
                        device_id.clone(),
                        RelayPeerState {
                            device_name: device_name.clone(),
                            platform,
                            app_version,
                            authenticated: true,
                        },
                    );
                    was_new
                };

                if was_new {
                    if let Some(handler) = self.on_peer_connect.lock().await.as_ref() {
                        handler(device_id.clone());
                    }
                    let _ = self.transport.send(self.make_hello());
                }

                self.send_local_version_vector().await;
            }

            LanSyncMessage::VersionVector {
                vector: remote_vector,
                origin_device_id,
            } => {
                let origin = origin_device_id.unwrap_or(from_device_id);
                if !self.is_authenticated_peer(&origin).await {
                    return;
                }
                self.send_missing_entities(&remote_vector).await;
            }

            LanSyncMessage::SyncChanges {
                entities,
                is_last,
                origin_device_id,
                usage_complete_through,
                ..
            } => {
                let origin = origin_device_id.unwrap_or(from_device_id);
                if !self.is_authenticated_peer(&origin).await {
                    return;
                }
                self.apply_entities(&entities, is_last, usage_complete_through.as_ref())
                    .await;
            }

            LanSyncMessage::LiveChange {
                entity,
                origin_device_id,
                ..
            } => {
                let origin = origin_device_id.unwrap_or(from_device_id);
                if !self.is_authenticated_peer(&origin).await {
                    return;
                }
                // A single live change is not a final sync page — no
                // complete-through claims accompany it.
                self.apply_entities(&[entity], true, None).await;
            }

            LanSyncMessage::SignedIntegrationFrame { frame } => {
                self.handle_signed_integration_frame(from_device_id, frame, transport_public_key)
                    .await;
            }

            _ => {}
        }
    }

    async fn is_authenticated_peer(&self, device_id: &str) -> bool {
        self.peers
            .lock()
            .await
            .get(device_id)
            .map(|peer| peer.authenticated)
            .unwrap_or(false)
    }
}
