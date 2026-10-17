use super::*;

const REGISTRY_PAGE_SIZE: usize = 32;
const MAX_REGISTRY_PAGE_COUNT: u32 = 4096;
const MAX_REGISTRY_EXCHANGE_RETRIES: u8 = 2;

pub(super) fn is_user_data_message(message: &LanSyncMessage) -> bool {
    matches!(
        message,
        LanSyncMessage::VersionVector { .. }
            | LanSyncMessage::SyncChanges { .. }
            | LanSyncMessage::SyncAck { .. }
            | LanSyncMessage::LiveChange { .. }
            | LanSyncMessage::LiveAck { .. }
            | LanSyncMessage::LiveReceiptAck { .. }
            | LanSyncMessage::SignedIntegrationFrame { .. }
            | LanSyncMessage::SignedIntegrationAck { .. }
            | LanSyncMessage::PeerList { .. }
            | LanSyncMessage::SyncPolicy { .. }
    )
}

impl RelaySync {
    pub(super) async fn start_registry_exchange(
        &self,
        peer_device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        if self.config.device_id.as_str() >= peer_device_id {
            return Ok(());
        }
        let session_id = generate_id();
        let mut peers = self.peers.lock().await;
        let peer = peers
            .get_mut(peer_device_id)
            .ok_or_else(|| "registry_sync_peer_state_missing".to_owned())?;
        peer.registry_admission = RelayRegistryAdmission::Exchanging(RegistryExchangeState {
            session_id: session_id.clone(),
            transport_public_key: transport_public_key.to_owned(),
            retry_count: 0,
            incoming_page_count: None,
            next_incoming_page: 0,
            local_digest: None,
            remote_digest: None,
        });
        drop(peers);
        self.send_registry_control(
            transport_public_key,
            LanSyncMessage::RegistrySyncStart {
                session_id: session_id.clone(),
            },
        )
        .await?;
        self.send_registry_pages(peer_device_id, transport_public_key, &session_id)
            .await
    }

    pub(super) async fn handle_registry_sync_start(
        &self,
        peer_device_id: &str,
        transport_public_key: &str,
        session_id: String,
    ) -> Result<(), String> {
        if peer_device_id >= self.config.device_id.as_str()
            || session_id.is_empty()
            || session_id.len() > 128
            || !self
                .transport_key_matches(peer_device_id, transport_public_key)
                .await
        {
            return Err("registry_sync_start_identity_mismatch".into());
        }
        let mut peers = self.peers.lock().await;
        let peer = peers
            .get_mut(peer_device_id)
            .ok_or_else(|| "registry_sync_peer_state_missing".to_owned())?;
        let retry_count = match &peer.registry_admission {
            RelayRegistryAdmission::Exchanging(state)
                if state.transport_public_key == transport_public_key =>
            {
                if state.session_id == session_id {
                    state.retry_count
                } else {
                    state.retry_count.saturating_add(1)
                }
            }
            _ => 0,
        };
        peer.registry_admission = RelayRegistryAdmission::Exchanging(RegistryExchangeState {
            session_id: session_id.clone(),
            transport_public_key: transport_public_key.to_owned(),
            retry_count,
            incoming_page_count: None,
            next_incoming_page: 0,
            local_digest: None,
            remote_digest: None,
        });
        drop(peers);
        self.send_registry_pages(peer_device_id, transport_public_key, &session_id)
            .await
    }

    async fn send_registry_pages(
        &self,
        peer_device_id: &str,
        transport_public_key: &str,
        session_id: &str,
    ) -> Result<(), String> {
        let (high_watermark, total_events, first_page) = self
            .storage
            .load_device_registry_events_page(0, None, REGISTRY_PAGE_SIZE)
            .await?;
        let page_count = total_events
            .saturating_add(REGISTRY_PAGE_SIZE - 1)
            .checked_div(REGISTRY_PAGE_SIZE)
            .unwrap_or_default()
            .max(1);
        let page_count = u32::try_from(page_count)
            .map_err(|_| "registry_sync_page_count_overflow".to_owned())?;
        if page_count > MAX_REGISTRY_PAGE_COUNT {
            return Err("registry_sync_page_count_too_large".into());
        }
        let mut after_position = 0_i64;
        for page_index in 0..page_count {
            let events = if page_index == 0 {
                first_page.clone()
            } else {
                self.storage
                    .load_device_registry_events_page(
                        after_position,
                        Some(high_watermark),
                        REGISTRY_PAGE_SIZE,
                    )
                    .await?
                    .2
            };
            if events.len() > REGISTRY_PAGE_SIZE
                || (page_index + 1 < page_count && events.len() != REGISTRY_PAGE_SIZE)
            {
                return Err("registry_sync_snapshot_changed_or_invalid".into());
            }
            after_position = after_position
                .checked_add(
                    i64::try_from(events.len())
                        .map_err(|_| "registry_sync_page_size_overflow".to_owned())?,
                )
                .ok_or_else(|| "registry_sync_cursor_overflow".to_owned())?;
            self.send_registry_control(
                transport_public_key,
                LanSyncMessage::RegistryEventsPage {
                    session_id: session_id.to_owned(),
                    page_index,
                    page_count,
                    events,
                },
            )
            .await?;
        }
        let _ = peer_device_id;
        Ok(())
    }

    pub(super) async fn handle_registry_events_page(
        &self,
        peer_device_id: &str,
        transport_public_key: &str,
        session_id: String,
        page_index: u32,
        page_count: u32,
        events: Vec<crate::domain::device_registry::RegistryEvent>,
    ) -> Result<(), String> {
        if page_count == 0
            || page_count > MAX_REGISTRY_PAGE_COUNT
            || page_index >= page_count
            || events.len() > REGISTRY_PAGE_SIZE
        {
            return Err("registry_sync_page_bounds".into());
        }
        let next_index = {
            let peers = self.peers.lock().await;
            let peer = peers
                .get(peer_device_id)
                .ok_or_else(|| "registry_sync_without_start".to_owned())?;
            let RelayRegistryAdmission::Exchanging(state) = &peer.registry_admission else {
                return Err("registry_sync_without_start".into());
            };
            if state.session_id != session_id || state.transport_public_key != transport_public_key
            {
                return Err("registry_sync_session_mismatch".into());
            }
            if state
                .incoming_page_count
                .is_some_and(|count| count != page_count)
                || state.next_incoming_page != page_index
            {
                return Err("registry_sync_page_order".into());
            }
            state.next_incoming_page
        };
        if !self
            .transport_key_matches(peer_device_id, transport_public_key)
            .await
        {
            return Err("registry_sync_session_mismatch".into());
        }
        if page_index + 1 < page_count && events.len() != REGISTRY_PAGE_SIZE {
            return Err("registry_sync_short_nonterminal_page".into());
        }
        let page_apply = self
            .storage
            .apply_device_registry_events(peer_device_id, &events)
            .await?;
        let publication = self
            .publish_registry_delta(
                &page_apply.applied_events,
                &page_apply.disabled_device_ids,
                Some(peer_device_id),
            )
            .await;
        self.notify_registry_events(&page_apply.applied_events)
            .await;
        publication?;
        if page_apply
            .disabled_device_ids
            .iter()
            .any(|device_id| device_id == self.config.device_id.as_str())
        {
            return Err("registry_sync_local_device_disabled".into());
        }
        if !page_apply.sender_is_admitted {
            return Err("registry_sync_peer_disabled".into());
        }
        let digest = page_apply.digest;
        let is_last = page_index + 1 == page_count;
        {
            let mut peers = self.peers.lock().await;
            let peer = peers
                .get_mut(peer_device_id)
                .ok_or_else(|| "registry_sync_session_lost".to_owned())?;
            let RelayRegistryAdmission::Exchanging(state) = &mut peer.registry_admission else {
                return Err("registry_sync_session_lost".into());
            };
            if state.session_id != session_id || state.next_incoming_page != next_index {
                return Err("registry_sync_session_changed_during_apply".into());
            }
            state.incoming_page_count = Some(page_count);
            state.next_incoming_page += 1;
            if is_last {
                state.local_digest = Some(digest.clone());
            }
        }
        if is_last {
            self.send_registry_control(
                transport_public_key,
                LanSyncMessage::RegistrySyncReady { session_id, digest },
            )
            .await?;
            self.try_complete_registry_exchange(peer_device_id).await?;
        }
        Ok(())
    }

    pub(super) async fn handle_registry_sync_ready(
        &self,
        peer_device_id: &str,
        transport_public_key: &str,
        session_id: String,
        digest: String,
    ) -> Result<(), String> {
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("registry_sync_digest_invalid".into());
        }
        let mut peers = self.peers.lock().await;
        let peer = peers
            .get_mut(peer_device_id)
            .ok_or_else(|| "registry_sync_ready_without_start".to_owned())?;
        let RelayRegistryAdmission::Exchanging(state) = &mut peer.registry_admission else {
            return Err("registry_sync_ready_without_start".into());
        };
        if state.session_id != session_id || state.transport_public_key != transport_public_key {
            return Err("registry_sync_session_mismatch".into());
        }
        match &state.remote_digest {
            Some(previous) if previous != &digest => {
                return Err("registry_sync_ready_conflict".into());
            }
            Some(_) => {}
            None => state.remote_digest = Some(digest),
        }
        drop(peers);
        self.try_complete_registry_exchange(peer_device_id).await
    }

    async fn try_complete_registry_exchange(&self, peer_device_id: &str) -> Result<(), String> {
        let (endpoint, local_digest, remote_digest, policy_negotiated) = {
            let peers = self.peers.lock().await;
            let Some(peer) = peers.get(peer_device_id) else {
                return Ok(());
            };
            let RelayRegistryAdmission::Exchanging(state) = &peer.registry_admission else {
                return Ok(());
            };
            let (Some(local_digest), Some(remote_digest)) =
                (&state.local_digest, &state.remote_digest)
            else {
                return Ok(());
            };
            (
                state.transport_public_key.clone(),
                local_digest.clone(),
                remote_digest.clone(),
                peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                    protocol
                        .capabilities
                        .contains(&crate::protocol::ProtocolCapability::SelectiveSyncPolicyV1)
                }),
            )
        };
        if local_digest != remote_digest {
            if self.config.device_id.as_str() < peer_device_id {
                let retry_session = {
                    let mut peers = self.peers.lock().await;
                    let Some(peer) = peers.get_mut(peer_device_id) else {
                        return Ok(());
                    };
                    let RelayRegistryAdmission::Exchanging(state) = &peer.registry_admission else {
                        return Ok(());
                    };
                    if state.transport_public_key != endpoint
                        || state.local_digest.as_deref() != Some(local_digest.as_str())
                        || state.remote_digest.as_deref() != Some(remote_digest.as_str())
                    {
                        return Ok(());
                    }
                    if state.retry_count >= MAX_REGISTRY_EXCHANGE_RETRIES {
                        peer.registry_admission = RelayRegistryAdmission::Waiting;
                        None
                    } else {
                        let session_id = generate_id();
                        let retry_count = state.retry_count + 1;
                        peer.registry_admission =
                            RelayRegistryAdmission::Exchanging(RegistryExchangeState {
                                session_id: session_id.clone(),
                                transport_public_key: endpoint.clone(),
                                retry_count,
                                incoming_page_count: None,
                                next_incoming_page: 0,
                                local_digest: None,
                                remote_digest: None,
                            });
                        Some(session_id)
                    }
                };
                if let Some(session_id) = retry_session {
                    self.send_registry_control(
                        &endpoint,
                        LanSyncMessage::RegistrySyncStart {
                            session_id: session_id.clone(),
                        },
                    )
                    .await?;
                    self.send_registry_pages(peer_device_id, &endpoint, &session_id)
                        .await?;
                    return Ok(());
                }
                self.transport.unbind_peer(peer_device_id)?;
                return Err("registry_sync_digest_mismatch".into());
            }
            let mut peers = self.peers.lock().await;
            if let Some(peer) = peers.get_mut(peer_device_id) {
                if let RelayRegistryAdmission::Exchanging(state) = &peer.registry_admission {
                    if state.retry_count >= MAX_REGISTRY_EXCHANGE_RETRIES {
                        peer.registry_admission = RelayRegistryAdmission::Waiting;
                    }
                }
            }
            return Ok(());
        }
        {
            let mut peers = self.peers.lock().await;
            let peer = peers
                .get_mut(peer_device_id)
                .ok_or_else(|| "registry_sync_session_lost".to_owned())?;
            let RelayRegistryAdmission::Exchanging(state) = &peer.registry_admission else {
                return Err("registry_sync_session_lost".into());
            };
            if state.transport_public_key != endpoint
                || state.local_digest.as_deref() != Some(local_digest.as_str())
                || state.remote_digest.as_deref() != Some(remote_digest.as_str())
            {
                return Ok(());
            }
            let session_id = state.session_id.clone();
            self.transport
                .mark_peer_data_ready(peer_device_id, &endpoint)?;
            peer.registry_admission = RelayRegistryAdmission::Ready {
                session_id: session_id.clone(),
                transport_public_key: endpoint.clone(),
            };
        }
        self.mark_outgoing_accepted(&endpoint).await;
        if policy_negotiated {
            if let Some(profile) = self
                .storage
                .load_local_sync_profile(&self.config.device_id)
                .await?
            {
                self.send_registry_control(
                    &endpoint,
                    crate::protocol::sync_policy_message(&profile),
                )
                .await?;
            }
        }
        self.send_local_version_vector_to_peer(&endpoint).await?;
        self.notify_pairing_changed().await;
        Ok(())
    }

    async fn send_local_version_vector_to_peer(
        &self,
        transport_public_key: &str,
    ) -> Result<(), String> {
        let vector = load_version_vector(&self.storage).await?;
        self.send_registry_control(
            transport_public_key,
            LanSyncMessage::VersionVector {
                vector,
                origin_device_id: Some(self.config.device_id.clone()),
            },
        )
        .await
    }

    pub(super) async fn send_registry_control(
        &self,
        transport_public_key: &str,
        message: LanSyncMessage,
    ) -> Result<(), String> {
        match self
            .transport
            .send_to_transport_peer(transport_public_key, message)
            .await
        {
            Ok(crate::sync_transport::AddressedSendOutcome::Sent) => Ok(()),
            Ok(crate::sync_transport::AddressedSendOutcome::PeerOffline) => {
                Err("transport peer is offline".into())
            }
            Err(error) => Err(error),
        }
    }

    pub(super) async fn transport_key_matches(&self, peer_device_id: &str, endpoint: &str) -> bool {
        self.transport_keys
            .lock()
            .await
            .get(peer_device_id)
            .is_some_and(|key| key == endpoint)
    }
}

#[cfg(test)]
mod tests {
    use super::is_user_data_message;
    use crate::protocol::LanSyncMessage;

    #[test]
    fn sync_policy_requires_registry_admission() {
        let message = LanSyncMessage::SyncPolicy {
            revision: 1,
            digest: "policy-digest".into(),
            active: true,
            rules: Vec::new(),
        };

        assert!(is_user_data_message(&message));
    }
}
