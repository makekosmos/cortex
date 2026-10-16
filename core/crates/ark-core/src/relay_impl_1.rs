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
            pending_pairing: Mutex::new(HashMap::new()),
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

    async fn notify_pairing_changed(&self) {
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
                addresses,
                auth_nonce,
                auth_hmac,
                platform,
                app_version,
                supported_protocol_versions,
                supported_capabilities,
                negotiated_capabilities,
                negotiation_hmac,
            } => {
                if negotiated_capabilities.is_some() {
                    self.reject_protocol_offer(
                        &device_id,
                        crate::protocol::ProtocolNegotiationError::InvalidSelection,
                        &[],
                    )
                    .await;
                    return;
                }
                if space_id != self.config.space_id {
                    return;
                }
                if device_id == self.config.device_id {
                    return;
                }
                // A peer the user removed via «Отключить» must not silently
                // re-authenticate — same block as the WS server Hello path.
                let removed = match crate::sync_server::load_removed_peer_ids(&self.storage).await {
                    Ok(ids) => ids,
                    Err(error) => {
                        eprintln!("{TAG} failed to load blocked peer list: {error}");
                        return;
                    }
                };
                if removed.iter().any(|id| id == &device_id) {
                    eprintln!("{TAG} rejecting hello from removed peer {device_id}");
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

                let local_offer = ProtocolOffer::current(self.auth_secret.is_some());
                let remote_offer = match ProtocolOffer::new(
                    protocol_version,
                    supported_protocol_versions.as_deref(),
                    supported_capabilities.as_deref(),
                ) {
                    Ok(offer) => offer,
                    Err(error) => {
                        self.reject_protocol_offer(&device_id, error, &[]).await;
                        return;
                    }
                };
                let offer_authenticated = match (
                    self.auth_secret.as_deref(),
                    auth_nonce.as_deref(),
                    negotiation_hmac.as_deref(),
                ) {
                    (Some(secret), Some(nonce), Some(proof)) => {
                        crate::protocol::verify_negotiation_offer_hmac(
                            secret,
                            &space_id,
                            &device_id,
                            nonce,
                            &remote_offer,
                            proof,
                        )
                    }
                    (_, _, Some(_)) => false,
                    _ => false,
                };
                if negotiation_hmac.is_some() && !offer_authenticated {
                    self.reject_protocol_offer(
                        &device_id,
                        crate::protocol::ProtocolNegotiationError::InvalidSelectionProof,
                        &remote_offer.version_ids(),
                    )
                    .await;
                    return;
                }
                let negotiated =
                    match negotiate_protocol(&local_offer, &remote_offer, offer_authenticated) {
                        Ok(protocol) => protocol,
                        Err(error) => {
                            self.reject_protocol_offer(
                                &device_id,
                                error,
                                &remote_offer.version_ids(),
                            )
                            .await;
                            return;
                        }
                    };

                if self.peers.lock().await.get(&device_id).is_some_and(|peer| {
                    peer.authenticated && peer.negotiated_protocol.as_ref() != Some(&negotiated)
                }) {
                    self.reject_protocol_offer(
                        &device_id,
                        crate::protocol::ProtocolNegotiationError::InvalidSelection,
                        &remote_offer.version_ids(),
                    )
                    .await;
                    return;
                }

                let was_new = {
                    let mut peers = self.peers.lock().await;
                    let was_new = !peers.contains_key(&device_id);
                    peers.insert(
                        device_id.clone(),
                        RelayPeerState {
                            device_name: device_name.clone(),
                            platform: platform.clone(),
                            app_version: app_version.clone(),
                            authenticated: true,
                            negotiated_protocol: Some(negotiated),
                        },
                    );
                    was_new
                };

                // Persist the pairing into `sync.peers` — the WS server does
                // this on Hello too. Without it a transport-paired device
                // vanishes from the snapshot and from `lan_bind_at_boot`'s
                // "sync is enabled" check after an Engine restart (KOS-367).
                let record = PeerRecord {
                    device_id: device_id.clone(),
                    device_name: device_name.clone(),
                    addresses: addresses.unwrap_or_default(),
                    last_seen: chrono::Utc::now()
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    last_address: None,
                    platform,
                    app_version,
                };
                let current = match crate::sync_server::load_known_peer_records(&self.storage).await
                {
                    Ok(peers) => peers,
                    Err(error) => {
                        eprintln!("{TAG} failed to load known peers: {error}");
                        return;
                    }
                };
                let known = merge_peer_records(&current, &[record]);
                if let Err(error) =
                    crate::sync_server::save_known_peers(&self.storage, &known).await
                {
                    eprintln!("{TAG} failed to persist known peers: {error}");
                    return;
                }

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
                self.send_missing_entities(&origin, &remote_vector).await;
            }

            LanSyncMessage::SyncChanges {
                entities,
                is_last,
                origin_device_id: _,
                usage_complete_through,
                ..
            } => {
                if !self.is_authenticated_peer(&from_device_id).await {
                    return;
                }
                self.apply_entities(
                    &entities,
                    is_last,
                    usage_complete_through.as_ref(),
                    transport_public_key
                        .as_deref()
                        .map(|_| from_device_id.as_str()),
                )
                .await;
            }

            LanSyncMessage::LiveChange { entity, .. } => {
                if !self.is_authenticated_peer(&from_device_id).await {
                    return;
                }
                // A single live change is not a final sync page — no
                // complete-through claims accompany it.
                self.apply_entities(
                    &[entity],
                    true,
                    None,
                    transport_public_key
                        .as_deref()
                        .map(|_| from_device_id.as_str()),
                )
                .await;
            }

            LanSyncMessage::SignedIntegrationFrame { frame } => {
                self.handle_signed_integration_frame(from_device_id, frame, transport_public_key)
                    .await;
            }

            LanSyncMessage::ProtocolRejected {
                origin_device_id,
                target_device_id,
                reason,
                supported_protocol_versions,
                ..
            } => {
                if target_device_id != self.config.device_id || origin_device_id != from_device_id {
                    return;
                }
                eprintln!(
                    "{TAG} peer {origin_device_id} rejected the sync protocol ({reason}); \
                     supported versions: {supported_protocol_versions:?}"
                );
                // Iroh can close just the addressed peer. The WebSocket relay
                // is shared by every peer, so never close that socket here.
                if transport_public_key.is_some() {
                    let _ = self.transport.disconnect_peer(&origin_device_id);
                }
            }

            _ => {}
        }
    }

    async fn send_local_version_vector(&self) {
        let vector = match load_version_vector(&self.storage).await {
            Ok(vector) => vector,
            Err(error) => {
                eprintln!("{TAG} failed to load local sync progress: {error}");
                return;
            }
        };
        let _ = self.transport.send(LanSyncMessage::VersionVector {
            vector,
            origin_device_id: Some(self.config.device_id.clone()),
        });
    }

    async fn reject_protocol_offer(
        &self,
        target_device_id: &str,
        error: crate::protocol::ProtocolNegotiationError,
        offered_protocol_versions: &[u32],
    ) {
        let message = LanSyncMessage::ProtocolRejected {
            origin_device_id: self.config.device_id.clone(),
            target_device_id: target_device_id.to_owned(),
            reason: error.code().to_owned(),
            supported_protocol_versions: SUPPORTED_PROTOCOL_VERSIONS.to_vec(),
            offered_protocol_versions: if offered_protocol_versions.len()
                <= MAX_PROTOCOL_OFFER_VERSIONS
            {
                offered_protocol_versions.to_vec()
            } else {
                Vec::new()
            },
        };
        if let Err(send_error) = self.transport.send(message) {
            eprintln!("{TAG} failed to send protocol refusal: {send_error}");
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

    /// Evict a peer before fallible persistence so failed writes cannot keep it live.
    pub async fn disconnect_peer(&self, device_id: &str) -> Result<(), String> {
        self.transport_keys.lock().await.remove(device_id);
        if self
            .pending_pairing
            .lock()
            .await
            .remove(device_id)
            .is_some()
        {
            self.notify_pairing_changed().await;
        }
        let was_connected = self.peers.lock().await.remove(device_id).is_some();
        let transport_result = self
            .transport
            .disconnect_peer(device_id)
            .map_err(|error| format!("failed to disconnect transport peer: {error}"));
        let persistence_result = async {
            let mut keys = crate::sync_server::load_peer_transport_keys(&self.storage).await?;
            if keys.remove(device_id).is_some() {
                crate::sync_server::save_peer_transport_keys(&self.storage, &keys).await?;
            }
            Ok::<(), String>(())
        }
        .await;
        if was_connected {
            if let Some(handler) = self.on_peer_disconnect.lock().await.as_ref() {
                handler(device_id.to_string(), self.peers.lock().await.len());
            }
        }
        match (transport_result, persistence_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(first), Err(second)) => Err(format!("{first}; {second}")),
        }
    }
}
