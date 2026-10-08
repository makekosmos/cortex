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
                    let trusted = self
                        .storage
                        .authorized_transport_public_key(&from_device_id)
                        .await
                        .is_some_and(|key| key == transport_public_key);
                    if trusted {
                        self.handle_message(from_device_id.clone(), msg, None).await;
                        if self.is_authenticated_peer(&from_device_id).await {
                            let _ = self
                                .transport
                                .bind_authenticated_peer(&from_device_id, &transport_public_key);
                            self.transport_keys
                                .lock()
                                .await
                                .insert(from_device_id, transport_public_key);
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
