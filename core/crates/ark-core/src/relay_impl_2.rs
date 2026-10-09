use crate::integration_replication::SignedSyncEnvelope;

impl RelaySync {
    async fn handle_event(&self, event: TransportEvent) {
        match event {
            TransportEvent::MessageReceived {
                from_device_id,
                msg,
            } => {
                let from = if from_device_id.is_empty() {
                    message_origin_device_id(&msg).unwrap_or_default()
                } else {
                    from_device_id
                };
                if from == self.config.device_id {
                    return;
                }
                self.handle_message(from, msg, None).await;
            }
            TransportEvent::MessageReceivedFromTransport {
                from_device_id,
                transport_public_key,
                msg,
            } => {
                if matches!(&msg, LanSyncMessage::Hello { .. }) {
                    // A Hello is trusted in exactly two cases. The claimed
                    // device has a stored endpoint key — persisted from an
                    // earlier pairing (`sync.peer_transport_keys`) or granted
                    // through integration replication (`authorized_nodes`) —
                    // and it matches (a mismatch is the rotation/revocation
                    // guard). Or a user opened the first-contact window with
                    // «Показать код» / «Подключить»: the ticket is the
                    // capability there (KOS-367). Anything else is dropped —
                    // the endpoint stays reachable by id forever, so
                    // unconditional acceptance would turn a once-disclosed
                    // ticket into a permanent key. `auth_secret` HMAC still
                    // applies inside `handle_message`.
                    let authorized = self
                        .storage
                        .authorized_transport_public_key(&from_device_id)
                        .await;
                    let mut paired_keys =
                        crate::sync_server::load_peer_transport_keys(&self.storage).await;
                    let known_key = authorized
                        .as_deref()
                        .or_else(|| paired_keys.get(&from_device_id).map(String::as_str));
                    let (window_open, admits_any, expected_match) = {
                        let pending = self.pairing_accept.lock().await;
                        let active =
                            pending.as_ref().is_some_and(|p| p.until > Instant::now());
                        let admits_any = active
                            && pending
                                .as_ref()
                                .is_some_and(|p| p.expected_endpoint.is_none());
                        let expected_match = active
                            && pending.as_ref().is_some_and(|p| {
                                p.expected_endpoint.as_deref()
                                    == Some(transport_public_key.as_str())
                            });
                        (active, admits_any, expected_match)
                    };
                    // A stored integration-replication key never yields to a
                    // pairing window — that trust domain owns the binding.
                    // A stored *pairing* key may rotate under explicit
                    // re-consent: entering that endpoint's code rebinds it.
                    let accept = match known_key {
                        Some(key) if key == transport_public_key => true,
                        Some(_) => expected_match && authorized.is_none(),
                        None => window_open && (admits_any || expected_match),
                    };
                    if !accept {
                        eprintln!(
                            "{TAG} rejecting Hello from {from_device_id}: {}",
                            if known_key.is_some() {
                                "transport key does not match the stored record"
                            } else {
                                "unknown endpoint and no pairing window is open"
                            }
                        );
                    } else {
                        // Re-pairing a previously removed device is explicit
                        // re-consent — unblock it before handle_message's
                        // removed-peer check would drop the Hello forever.
                        if expected_match {
                            crate::sync_server::unblock_peer_id(&self.storage, &from_device_id)
                                .await;
                        }
                        self.handle_message(from_device_id.clone(), msg, None).await;
                        if self.is_authenticated_peer(&from_device_id).await {
                            let _ = self
                                .transport
                                .bind_authenticated_peer(&from_device_id, &transport_public_key);
                            self.transport_keys
                                .lock()
                                .await
                                .insert(from_device_id.clone(), transport_public_key.clone());
                            if paired_keys.get(&from_device_id)
                                != Some(&transport_public_key)
                            {
                                paired_keys
                                    .insert(from_device_id, transport_public_key);
                                crate::sync_server::save_peer_transport_keys(
                                    &self.storage,
                                    &paired_keys,
                                )
                                .await;
                            }
                        }
                    }
                } else if self
                    .transport_keys
                    .lock()
                    .await
                    .get(&from_device_id)
                    .is_some_and(|key| key == &transport_public_key)
                {
                    self.handle_message(from_device_id, msg, Some(transport_public_key))
                        .await;
                }
            }
            TransportEvent::Connected { .. } => {}
            TransportEvent::Disconnected { device_id } => {
                // Transports fall back to our own id when a remote's id can't
                // be resolved (iroh registry miss) — never evict that.
                if device_id != self.config.device_id {
                    self.peers.lock().await.remove(&device_id);
                    self.transport_keys.lock().await.remove(&device_id);
                    if let Some(handler) = self.on_peer_disconnect.lock().await.as_ref() {
                        handler(device_id, self.peers.lock().await.len());
                    }
                }
            }
            TransportEvent::TransportDropped => {
                // Whole transport went down — every peer reached through it is
                // gone. Evict all peer state so reconnect re-announces them.
                let evicted: Vec<String> = {
                    let mut peers = self.peers.lock().await;
                    let evicted = peers.keys().cloned().collect();
                    peers.clear();
                    evicted
                };
                self.transport_keys.lock().await.clear();
                if let Some(handler) = self.on_peer_disconnect.lock().await.as_ref() {
                    let mut remaining = evicted.len();
                    for device_id in evicted {
                        remaining -= 1;
                        handler(device_id, remaining);
                    }
                }
            }
        }
        trim_process_heap();
    }

    /// Authenticated peer count over this transport — the WS server has
    /// `connected_peer_count`; the `peer_disconnected` event's `remaining`
    /// must count both.
    pub async fn connected_peer_count(&self) -> usize {
        self.peers
            .lock()
            .await
            .values()
            .filter(|p| p.authenticated)
            .count()
    }

    /// Open the first-contact window the Hello gate above consults.
    /// `expected_endpoint: Some(id)` pins it to the endpoint whose ticket
    /// was entered; `None` (the code-showing side) admits any unknown
    /// endpoint — whoever proves the ticket.
    pub async fn open_pairing_window(&self, expected_endpoint: Option<String>) {
        *self.pairing_accept.lock().await = Some(PairingAccept {
            expected_endpoint,
            until: Instant::now() + PAIRING_ACCEPT_TTL,
        });
    }

    /// The authenticated peer bound to this transport key, if any. Lets
    /// `connect_with_pairing_code` report the device that actually proved
    /// the dialed endpoint rather than any peer that happened to reconnect
    /// during the wait.
    pub async fn authenticated_peer_for_transport_key(
        &self,
        transport_public_key: &str,
    ) -> Option<PeerEntry> {
        let device_id = {
            let keys = self.transport_keys.lock().await;
            keys.iter()
                .find(|(_, key)| key.as_str() == transport_public_key)
                .map(|(device_id, _)| device_id.clone())
        }?;
        let peers = self.peers.lock().await;
        peers.get(&device_id).and_then(|peer| {
            peer.authenticated.then(|| PeerEntry {
                device_id,
                device_name: peer.device_name.clone(),
                platform: peer.platform.clone(),
                app_version: peer.app_version.clone(),
            })
        })
    }

    /// Evict a peer the user removed via «Отключить»: without this the
    /// authenticated entry survives removal, so a transport-paired device
    /// keeps syncing and stays "online" in the snapshot until the Engine
    /// restarts. `sync.removed_peers` (written by the caller) blocks
    /// re-authentication on the next Hello.
    pub async fn disconnect_peer(&self, device_id: &str) {
        self.transport_keys.lock().await.remove(device_id);
        // Forget the persisted endpoint key so the device has to pair again
        // (a stored match would otherwise re-admit it without a window).
        let mut keys = crate::sync_server::load_peer_transport_keys(&self.storage).await;
        if keys.remove(device_id).is_some() {
            crate::sync_server::save_peer_transport_keys(&self.storage, &keys).await;
        }
        if self.peers.lock().await.remove(device_id).is_none() {
            return;
        }
        let _ = self.transport.disconnect_peer(device_id);
        if let Some(handler) = self.on_peer_disconnect.lock().await.as_ref() {
            let remaining = self.peers.lock().await.len();
            handler(device_id.to_string(), remaining);
        }
    }

    /// Route an addressed integration frame through the configured transport.
    /// Broadcast transports reject this message class; addressed transports
    /// such as Iroh enforce authenticated recipient and writer-time authority.
    pub async fn send_signed_integration_frame(
        &self,
        frame: SignedSyncEnvelope,
    ) -> Result<(), String> {
        self.storage
            .validate_outbound_signed_integration_frame(
                &frame,
                &self.config.space_id,
                &self.config.device_id,
            )
            .await?;
        let recipient = frame.recipient_node_id.clone();
        self.transport
            .send_to(&recipient, LanSyncMessage::SignedIntegrationFrame { frame })
            .await
    }

    async fn handle_signed_integration_frame(
        &self,
        from_device_id: String,
        frame: SignedSyncEnvelope,
        transport_public_key: Option<String>,
    ) {
        let accepted = frame.space_id == self.config.space_id
            && frame.origin_node_id == from_device_id
            && frame.recipient_node_id == self.config.device_id
            && self.is_authenticated_peer(&from_device_id).await
            && self
                .storage
                .apply_signed_integration_frame_with_transport(
                    &frame,
                    &self.config.space_id,
                    &from_device_id,
                    &self.config.device_id,
                    transport_public_key.as_deref(),
                )
                .await
                .is_ok();
        let _ = self
            .transport
            .send_to(
                &from_device_id,
                LanSyncMessage::SignedIntegrationAck {
                    message_id: frame.message_id,
                    accepted,
                },
            )
            .await;
    }

    fn make_hello(&self) -> LanSyncMessage {
        let (auth_nonce, auth_hmac) = match self.auth_secret.as_ref() {
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

    async fn send_local_version_vector(&self) {
        let vector = load_version_vector(&self.storage).await;
        let _ = self.transport.send(LanSyncMessage::VersionVector {
            vector,
            origin_device_id: Some(self.config.device_id.clone()),
        });
    }

    async fn send_missing_entities(&self, remote_vector: &VersionVector) {
        let mut load_vector = load_version_vector(&self.storage).await;
        merge_usage_cursors(&mut load_vector, remote_vector);
        // Hole-tolerant cursors (KOS-302): a relay must advertise these too —
        // it may itself have compacted refs the next hop still needs a claim
        // for. Snapshot BEFORE the first page: a claim computed afterwards
        // could cover a seq allocated mid-pull that never got streamed.
        let complete_through = self.storage.usage_complete_through().await;
        let mut offset = 0;

        loop {
            let entities = self
                .storage
                .load_entities_page(&load_vector, offset, SYNC_LOAD_PAGE_SIZE)
                .await;
            if entities.is_empty() {
                break;
            }
            offset += entities.len();

            let mut to_send = Vec::new();
            for entity in entities {
                if should_send_entity(remote_vector, &entity) {
                    to_send.push(entity);
                }
            }

            for batch in split_into_batches(&to_send) {
                let _ = self.transport.send(LanSyncMessage::SyncChanges {
                    batch_id: generate_id(),
                    entities: batch,
                    is_last: false,
                    origin_device_id: Some(self.config.device_id.clone()),
                    usage_complete_through: None,
                });
            }
        }

        let _ = self.transport.send(LanSyncMessage::SyncChanges {
            batch_id: generate_id(),
            entities: vec![],
            is_last: true,
            origin_device_id: Some(self.config.device_id.clone()),
            usage_complete_through: (!complete_through.is_empty()).then_some(complete_through),
        });
    }

    async fn apply_entities(
        &self,
        entities: &[SyncEntity],
        is_last: bool,
        usage_complete_through: Option<&HashMap<String, u64>>,
    ) {
        let mut incoming = self.incoming_sync.lock().await;
        if incoming.is_none() {
            *incoming = Some(IncomingSyncState {
                vector: load_version_vector(&self.storage).await,
                changed: false,
                last_update: Instant::now(),
            });
        }
        let state = incoming.as_mut().expect("incoming sync state initialized");
        state.last_update = Instant::now();

        for entity in entities {
            let should_apply = is_usage_entity(entity)
                || state
                    .vector
                    .get(&entity.id)
                    .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));
            if !should_apply {
                continue;
            }

            match self.storage.apply_entity(entity).await {
                Ok(()) => {
                    state.changed |= observe_non_usage_entity(&mut state.vector, entity);
                    if let Some(handler) = self.on_change.lock().await.as_ref() {
                        handler(entity.clone());
                    }
                }
                Err(e) => {
                    eprintln!(
                        "{TAG} failed to apply relay entity {}:{}: {e}",
                        entity.entity_type, entity.id
                    );
                }
            }
        }

        if is_last {
            crate::sync_server::persist_pull_vector(
                &self.storage,
                &mut state.vector,
                state.changed,
                true,
                usage_complete_through,
            )
            .await;
            *incoming = None;
            drop(incoming);
            trim_process_heap();
        }
    }
}
