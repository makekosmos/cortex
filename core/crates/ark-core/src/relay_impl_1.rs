impl RelaySync {
    async fn handle_message(
        &self,
        from_device_id: String,
        msg: LanSyncMessage,
        transport_public_key: Option<String>,
        defer_initial_sync: bool,
    ) {
        if crate::protocol::requires_registry_admission(&msg)
            && !self
                .registry_ready_for_peer(&from_device_id, transport_public_key.as_deref())
                .await
        {
            eprintln!("{TAG} dropped pre-registry data frame from {from_device_id}");
            return;
        }
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

                let local_offer = if self.transport.supports_registry_admission() {
                    ProtocolOffer::current_with_registry_history(true)
                        .with_task_merge_bootstrap_v2()
                } else {
                    ProtocolOffer::current(self.auth_secret.is_some())
                };
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
                if !negotiated
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::RegistryHistoryV1)
                {
                    self.reject_protocol_offer(
                        &device_id,
                        crate::protocol::ProtocolNegotiationError::MissingRegistryHistory,
                        &remote_offer.version_ids(),
                    )
                    .await;
                    return;
                }

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

                if !defer_initial_sync {
                    self.send_local_version_vector().await;
                }
            }

            LanSyncMessage::RegistrySyncStart { session_id } => {
                let Some(endpoint) = transport_public_key.as_deref() else {
                    eprintln!("{TAG} dropped registry start without transport identity");
                    return;
                };
                if let Err(error) = self
                    .handle_registry_sync_start(&from_device_id, endpoint, session_id)
                    .await
                {
                    eprintln!("{TAG} registry exchange start failed: {error}");
                    let _ = self.transport.disconnect_transport_peer(endpoint);
                }
            }
            LanSyncMessage::RegistryEventsPage {
                session_id,
                page_index,
                page_count,
                events,
            } => {
                let Some(endpoint) = transport_public_key.as_deref() else {
                    eprintln!("{TAG} dropped registry page without transport identity");
                    return;
                };
                if let Err(error) = self
                    .handle_registry_events_page(
                        &from_device_id,
                        endpoint,
                        session_id,
                        page_index,
                        page_count,
                        events,
                    )
                    .await
                {
                    eprintln!("{TAG} registry event page rejected: {error}");
                    let _ = self.transport.disconnect_transport_peer(endpoint);
                }
            }
            LanSyncMessage::RegistrySyncReady { session_id, digest } => {
                let Some(endpoint) = transport_public_key.as_deref() else {
                    eprintln!("{TAG} dropped registry proof without transport identity");
                    return;
                };
                if let Err(error) = self
                    .handle_registry_sync_ready(&from_device_id, endpoint, session_id, digest)
                    .await
                {
                    eprintln!("{TAG} registry proof rejected: {error}");
                    let _ = self.transport.disconnect_transport_peer(endpoint);
                }
            }
            LanSyncMessage::RegistryEventUpdate { session_id, event } => {
                let Some(endpoint) = transport_public_key.as_deref() else {
                    eprintln!("{TAG} dropped registry event without transport identity");
                    return;
                };
                if let Err(error) = self
                    .handle_registry_event_update(&from_device_id, endpoint, &session_id, event)
                    .await
                {
                    eprintln!("{TAG} registry event update rejected: {error}");
                    let _ = self.transport.disconnect_transport_peer(endpoint);
                }
            }

            LanSyncMessage::SyncPolicy {
                revision,
                digest,
                active,
                rules,
            } => {
                let Some(endpoint) = transport_public_key.as_deref() else {
                    return;
                };
                if !self.transport_key_matches(&from_device_id, endpoint).await {
                    let _ = self.transport.disconnect_transport_peer(endpoint);
                    return;
                }
                let negotiated = self
                    .peers
                    .lock()
                    .await
                    .get(&from_device_id)
                    .is_some_and(|peer| {
                        peer.authenticated
                            && peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                                protocol.capabilities.contains(
                                    &crate::protocol::ProtocolCapability::SelectiveSyncPolicyV1,
                                )
                            })
                    });
                if !negotiated {
                    let _ = self.transport.disconnect_transport_peer(endpoint);
                    return;
                }
                let rules = match crate::protocol::parse_sync_policy_rules(rules) {
                    Ok(rules) => rules,
                    Err(error) => {
                        eprintln!("{TAG} invalid sync policy from {from_device_id}: {error}");
                        let _ = self.transport.disconnect_transport_peer(endpoint);
                        return;
                    }
                };
                if let Err(error) = self
                    .storage
                    .accept_peer_sync_policy(&from_device_id, revision, &digest, active, &rules)
                    .await
                {
                    eprintln!("{TAG} failed to accept sync policy from {from_device_id}: {error}");
                    let _ = self.transport.disconnect_transport_peer(endpoint);
                }
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
            message @ (LanSyncMessage::TaskMergeBatch { .. }
            | LanSyncMessage::TaskMergeBatchAck { .. }) => {
                task_merge::handle_message(
                    self,
                    &from_device_id,
                    transport_public_key.as_deref(),
                    message,
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
        let _ = self.send_user_data_message(LanSyncMessage::VersionVector {
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
        self.registry.disconnect_peer(device_id).await;
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
