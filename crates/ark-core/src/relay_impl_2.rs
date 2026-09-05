use crate::integration_replication::SignedSyncEnvelope;

impl RelaySync {
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
            && self.storage.apply_signed_integration_frame_with_transport(
                &frame,
                &self.config.space_id,
                &from_device_id,
                &self.config.device_id,
                transport_public_key.as_deref(),
            ).await.is_ok();
        let _ = self.transport.send_to(
            &from_device_id,
            LanSyncMessage::SignedIntegrationAck {
                message_id: frame.message_id,
                accepted,
            },
        ).await;
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
                });
            }
        }

        let _ = self.transport.send(LanSyncMessage::SyncChanges {
            batch_id: generate_id(),
            entities: vec![],
            is_last: true,
            origin_device_id: Some(self.config.device_id.clone()),
        });
    }

    async fn apply_entities(&self, entities: &[SyncEntity], is_last: bool) {
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
            if state.changed {
                merge_usage_cursors(&mut state.vector, &load_version_vector(&self.storage).await);
                save_version_vector(&self.storage, &state.vector).await;
            }
            *incoming = None;
            drop(incoming);
            trim_process_heap();
        }
    }
}
