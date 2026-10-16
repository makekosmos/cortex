use super::*;

const REGISTRY_PAGE_SIZE: usize = 32;
const MAX_REGISTRY_PAGE_COUNT: u32 = 4096;

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
    )
}

impl RelaySync {
    pub(super) async fn registry_ready_for_peer(
        &self,
        peer_device_id: &str,
        transport_public_key: Option<&str>,
    ) -> bool {
        let Some(endpoint) = transport_public_key else {
            return false;
        };
        self.registry_ready
            .lock()
            .await
            .get(peer_device_id)
            .is_some_and(|ready_endpoint| ready_endpoint == endpoint)
    }

    pub(super) async fn start_registry_exchange(
        &self,
        peer_device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        if self.config.device_id.as_str() >= peer_device_id {
            return Ok(());
        }
        let session_id = generate_id();
        self.registry_ready.lock().await.remove(peer_device_id);
        self.registry_exchanges.lock().await.insert(
            peer_device_id.to_owned(),
            RegistryExchangeState {
                session_id: session_id.clone(),
                transport_public_key: transport_public_key.to_owned(),
                incoming_page_count: None,
                next_incoming_page: 0,
                local_digest: None,
                remote_digest: None,
            },
        );
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
        self.registry_ready.lock().await.remove(peer_device_id);
        self.registry_exchanges.lock().await.insert(
            peer_device_id.to_owned(),
            RegistryExchangeState {
                session_id: session_id.clone(),
                transport_public_key: transport_public_key.to_owned(),
                incoming_page_count: None,
                next_incoming_page: 0,
                local_digest: None,
                remote_digest: None,
            },
        );
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
            let exchanges = self.registry_exchanges.lock().await;
            let state = exchanges
                .get(peer_device_id)
                .ok_or_else(|| "registry_sync_without_start".to_owned())?;
            if state.session_id != session_id
                || state.transport_public_key != transport_public_key
                || !self
                    .transport_key_matches(peer_device_id, transport_public_key)
                    .await
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
        if page_index + 1 < page_count && events.len() != REGISTRY_PAGE_SIZE {
            return Err("registry_sync_short_nonterminal_page".into());
        }
        let page_apply = self
            .storage
            .apply_device_registry_events(peer_device_id, &events)
            .await?;
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
            let mut exchanges = self.registry_exchanges.lock().await;
            let state = exchanges
                .get_mut(peer_device_id)
                .ok_or_else(|| "registry_sync_session_lost".to_owned())?;
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
        let mut exchanges = self.registry_exchanges.lock().await;
        let state = exchanges
            .get_mut(peer_device_id)
            .ok_or_else(|| "registry_sync_ready_without_start".to_owned())?;
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
        drop(exchanges);
        self.try_complete_registry_exchange(peer_device_id).await
    }

    async fn try_complete_registry_exchange(&self, peer_device_id: &str) -> Result<(), String> {
        let (endpoint, local_digest, remote_digest) = {
            let exchanges = self.registry_exchanges.lock().await;
            let Some(state) = exchanges.get(peer_device_id) else {
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
            )
        };
        if local_digest != remote_digest {
            self.registry_exchanges.lock().await.remove(peer_device_id);
            self.transport.unbind_peer(peer_device_id)?;
            self.transport.disconnect_transport_peer(&endpoint)?;
            return Err("registry_sync_digest_mismatch".into());
        }
        self.transport
            .mark_peer_data_ready(peer_device_id, &endpoint)?;
        self.registry_ready
            .lock()
            .await
            .insert(peer_device_id.to_owned(), endpoint.clone());
        self.registry_exchanges.lock().await.remove(peer_device_id);
        self.mark_outgoing_accepted(&endpoint).await;
        self.send_local_version_vector().await;
        self.notify_pairing_changed().await;
        Ok(())
    }

    async fn send_registry_control(
        &self,
        transport_public_key: &str,
        message: LanSyncMessage,
    ) -> Result<(), String> {
        self.transport
            .send_to_transport_peer(transport_public_key, message)
            .await
    }

    async fn transport_key_matches(&self, peer_device_id: &str, endpoint: &str) -> bool {
        self.transport_keys
            .lock()
            .await
            .get(peer_device_id)
            .is_some_and(|key| key == endpoint)
    }
}
