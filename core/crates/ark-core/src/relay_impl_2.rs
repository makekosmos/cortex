use crate::integration_replication::SignedSyncEnvelope;

impl RelaySync {
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
        let offer = if self.transport.supports_registry_admission() {
            ProtocolOffer::current_with_registry_history(true)
        } else {
            ProtocolOffer::current(self.auth_secret.is_some())
        };
        let negotiation_hmac =
            self.auth_secret
                .as_deref()
                .zip(auth_nonce.as_deref())
                .map(|(secret, nonce)| {
                    compute_negotiation_offer_hmac(
                        secret,
                        &self.config.space_id,
                        &self.config.device_id,
                        nonce,
                        &offer,
                    )
                });

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
            supported_protocol_versions: Some(offer.version_ids()),
            supported_capabilities: Some(offer.capability_ids()),
            negotiated_capabilities: None,
            negotiation_hmac,
        }
    }

    async fn send_missing_entities(&self, peer_device_id: &str, remote_vector: &VersionVector) {
        let mut load_vector = match load_version_vector(&self.storage).await {
            Ok(vector) => vector,
            Err(error) => {
                eprintln!("{TAG} failed to load local sync progress for {peer_device_id}: {error}");
                return;
            }
        };
        merge_usage_cursors(&mut load_vector, remote_vector);
        // Hole-tolerant cursors (KOS-302): a relay must advertise these too —
        // it may itself have compacted refs the next hop still needs a claim
        // for. Snapshot BEFORE the first page: a claim computed afterwards
        // could cover a seq allocated mid-pull that never got streamed.
        let read_class =
            crate::application::sync_storage::SyncReadClass::for_peer_vector(remote_vector);
        let complete_through = match self
            .storage
            .usage_complete_through_for_peer(peer_device_id, read_class)
            .await
        {
            Ok(through) => through,
            Err(error) => {
                eprintln!(
                    "[RelaySync] outbound usage progress read failed for {peer_device_id}: {error}"
                );
                return;
            }
        };
        let mut offset = 0;

        loop {
            let entities = match self
                .storage
                .load_entities_page_for_peer(
                    peer_device_id,
                    read_class,
                    &load_vector,
                    offset,
                    SYNC_LOAD_PAGE_SIZE,
                )
                .await
            {
                Ok(entities) => entities,
                Err(error) => {
                    eprintln!(
                        "[RelaySync] outbound sync page read failed for {peer_device_id}: {error}"
                    );
                    return;
                }
            };
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
                    policy_identity: None,
                    batch_id: generate_id(),
                    entities: batch,
                    is_last: false,
                    origin_device_id: Some(self.config.device_id.clone()),
                    usage_complete_through: None,
                });
            }
        }

        let _ = self.transport.send(LanSyncMessage::SyncChanges {
            policy_identity: None,
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
        authenticated_peer_id: Option<&str>,
    ) {
        let mut incoming = self.incoming_sync.lock().await;
        if incoming.is_none() {
            let vector = match load_version_vector(&self.storage).await {
                Ok(vector) => vector,
                Err(error) => {
                    eprintln!("{TAG} failed to load inbound sync progress: {error}");
                    return;
                }
            };
            *incoming = Some(IncomingSyncState {
                vector,
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

            match apply_relay_entity(&self.storage, entity, authenticated_peer_id).await {
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
            if let Err(error) = crate::sync_server::persist_pull_vector(
                &self.storage,
                &mut state.vector,
                state.changed,
                true,
                usage_complete_through,
            )
            .await
            {
                eprintln!("{TAG} failed to persist relay sync progress: {error}");
                return;
            }
            *incoming = None;
            drop(incoming);
            trim_process_heap();
        }
    }
}
