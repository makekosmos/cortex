use super::*;

impl RelaySync {
    pub(super) async fn start_registry_exchange(
        &self,
        peer_device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        let policy_negotiated = self
            .peers
            .lock()
            .await
            .get(peer_device_id)
            .and_then(|peer| peer.negotiated_protocol.as_ref())
            .is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&ark_sync::protocol::ProtocolCapability::SelectiveSyncPolicyV1)
            });
        let registry = self
            .registry
            .as_ref()
            .ok_or_else(|| "registry admission requires a peer-session transport".to_owned())?;
        registry
            .begin_peer(peer_device_id, transport_public_key, policy_negotiated)
            .await;
        registry
            .start_exchange(peer_device_id, transport_public_key)
            .await
    }

    pub(super) async fn handle_registry_sync_start(
        &self,
        peer_device_id: &str,
        endpoint: &str,
        session_id: String,
    ) -> Result<(), String> {
        self.handle_registry_frame(
            peer_device_id,
            endpoint,
            ark_sync::relay_registry::RegistryFrame::SyncStart { session_id },
        )
        .await
    }

    pub(super) async fn handle_registry_events_page(
        &self,
        peer_device_id: &str,
        endpoint: &str,
        session_id: String,
        page_index: u32,
        page_count: u32,
        events: Vec<ark_domain::device_registry::RegistryEvent>,
    ) -> Result<(), String> {
        self.handle_registry_frame(
            peer_device_id,
            endpoint,
            ark_sync::relay_registry::RegistryFrame::EventsPage {
                session_id,
                page_index,
                page_count,
                events,
            },
        )
        .await
    }

    pub(super) async fn handle_registry_sync_ready(
        &self,
        peer_device_id: &str,
        endpoint: &str,
        session_id: String,
        digest: String,
    ) -> Result<(), String> {
        self.handle_registry_frame(
            peer_device_id,
            endpoint,
            ark_sync::relay_registry::RegistryFrame::SyncReady { session_id, digest },
        )
        .await
    }

    pub(super) async fn handle_registry_event_update(
        &self,
        peer_device_id: &str,
        endpoint: &str,
        session_id: &str,
        event: ark_domain::device_registry::RegistryEvent,
    ) -> Result<(), String> {
        self.handle_registry_frame(
            peer_device_id,
            endpoint,
            ark_sync::relay_registry::RegistryFrame::EventUpdate {
                session_id: session_id.to_owned(),
                event: Box::new(event),
            },
        )
        .await
    }

    pub(super) async fn handle_registry_frame(
        &self,
        peer_device_id: &str,
        endpoint: &str,
        frame: ark_sync::relay_registry::RegistryFrame,
    ) -> Result<(), String> {
        let check_admission = matches!(
            &frame,
            ark_sync::relay_registry::RegistryFrame::EventsPage { .. }
                | ark_sync::relay_registry::RegistryFrame::EventUpdate { .. }
        );
        let connected = self.connected_transport_peers().await;
        let registry = self
            .registry
            .as_ref()
            .ok_or_else(|| "registry admission requires a peer-session transport".to_owned())?;
        let outcome = registry
            .handle_registry_frame(peer_device_id, endpoint, frame, &connected)
            .await?;
        self.finish_registry_outcome(outcome, true, Some(endpoint), check_admission)
            .await
    }

    pub(super) async fn finish_registry_outcome(
        &self,
        outcome: ark_sync::relay_registry::RegistryOutcome,
        notify_events: bool,
        endpoint: Option<&str>,
        check_admission: bool,
    ) -> Result<(), String> {
        if notify_events {
            self.notify_registry_events(&outcome.applied_events).await;
        }
        self.remove_revoked_peer_state(&outcome.revoked_peer_ids)
            .await;
        if let Some(error) = outcome.operation_error {
            return Err(error);
        }
        if check_admission {
            if !outcome.sender_is_admitted {
                return Err("registry_sync_peer_disabled".into());
            }
            if !outcome.local_device_is_admitted {
                return Err("registry_sync_local_device_disabled".into());
            }
        }
        if !outcome.became_ready {
            return Ok(());
        }

        let endpoint = endpoint.ok_or_else(|| "registry_sync_ready_endpoint_missing".to_owned())?;
        self.mark_outgoing_accepted(endpoint).await;
        if outcome.policy_negotiated {
            if let Some(profile) = self
                .storage
                .load_local_sync_profile(&self.config.device_id)
                .await?
            {
                self.send_registry_control(
                    endpoint,
                    ark_sync::protocol::sync_policy_message(&profile),
                )
                .await?;
            }
        }
        self.send_local_version_vector_to_peer(endpoint).await?;
        self.notify_pairing_changed().await;
        Ok(())
    }

    pub(super) async fn connected_transport_peers(&self) -> Vec<(String, String)> {
        let peers = self.peers.lock().await;
        let transport_keys = self.transport_keys.lock().await;
        peers
            .iter()
            .filter(|(_, peer)| peer.authenticated)
            .filter_map(|(device_id, _)| {
                transport_keys
                    .get(device_id)
                    .map(|endpoint| (device_id.clone(), endpoint.clone()))
            })
            .collect()
    }

    pub(super) async fn remove_revoked_peer_state(&self, peer_ids: &[String]) {
        for peer_id in peer_ids {
            self.peers.lock().await.remove(peer_id);
            self.transport_keys.lock().await.remove(peer_id);
            if let Some(registry) = &self.registry {
                registry.disconnect_peer(peer_id).await;
            }
        }
    }

    async fn send_local_version_vector_to_peer(&self, endpoint: &str) -> Result<(), String> {
        let vector = load_version_vector(&self.storage).await?;
        self.send_registry_control(
            endpoint,
            LanSyncMessage::VersionVector {
                vector,
                origin_device_id: Some(self.config.device_id.clone()),
            },
        )
        .await
    }

    pub(super) async fn send_registry_control(
        &self,
        endpoint: &str,
        message: LanSyncMessage,
    ) -> Result<(), String> {
        let peer_control = self
            .transport
            .peer_control()
            .ok_or_else(|| "peer-session control requires Iroh transport".to_owned())?;
        match peer_control
            .send_control_to_endpoint(endpoint, message)
            .await?
        {
            ark_sync::relay_registry::PeerControlSendOutcome::Sent => Ok(()),
            ark_sync::relay_registry::PeerControlSendOutcome::PeerOffline => {
                Err("transport peer is offline".into())
            }
        }
    }
}
