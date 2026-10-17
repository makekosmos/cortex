use ark_application::registry_admission::RegistryAdmissionStorage;
use ark_domain::device_registry::{RegistryEvent, RegistryOperation};

use crate::protocol::{generate_id, LanSyncMessage};

use super::{
    state::{exchanging, valid_digest, Admission},
    RegistryFrame, RegistryOutcome, RelayRegistryCoordinator,
};

#[path = "exchange/session.rs"]
mod session;

#[cfg(test)]
#[path = "exchange_tests.rs"]
mod tests;

const MAX_RETRIES: u8 = 2;

struct IncomingPage {
    session_id: String,
    page_index: u32,
    page_count: u32,
    events: Vec<RegistryEvent>,
}

impl<S: RegistryAdmissionStorage + ?Sized + 'static> RelayRegistryCoordinator<S> {
    pub(super) async fn start_exchange_inner(
        &self,
        peer: &str,
        endpoint: &str,
    ) -> Result<(), String> {
        if self.local_device_id.as_str() >= peer {
            return Ok(());
        }
        let session = generate_id();
        {
            let mut peers = self.peers.lock().await;
            let entry = peers
                .get_mut(peer)
                .filter(|entry| entry.endpoint == endpoint)
                .ok_or_else(|| "registry_sync_peer_state_missing".to_owned())?;
            entry.admission = exchanging(session.clone(), 0);
        }
        if !self.is_exchanging(peer, endpoint, &session, Some(0)).await {
            return Err("registry_sync_session_lost".into());
        }
        self.send(
            endpoint,
            LanSyncMessage::RegistrySyncStart {
                session_id: session.clone(),
            },
        )
        .await?;
        self.send_pages(peer, endpoint, &session).await
    }

    pub(super) async fn handle_frame_inner(
        &self,
        peer: &str,
        endpoint: &str,
        frame: RegistryFrame,
        connected: &[(String, String)],
    ) -> Result<RegistryOutcome, String> {
        match frame {
            RegistryFrame::SyncStart { session_id } => {
                self.handle_start(peer, endpoint, &session_id).await?;
                Ok(RegistryOutcome::default())
            }
            RegistryFrame::EventsPage {
                session_id,
                page_index,
                page_count,
                events,
            } => {
                self.handle_page(
                    peer,
                    endpoint,
                    IncomingPage {
                        session_id,
                        page_index,
                        page_count,
                        events,
                    },
                    connected,
                )
                .await
            }
            RegistryFrame::SyncReady { session_id, digest } => {
                self.handle_ready(peer, endpoint, &session_id, digest).await
            }
            RegistryFrame::EventUpdate { session_id, event } => {
                self.handle_update(peer, endpoint, &session_id, *event, connected)
                    .await
            }
        }
    }

    async fn handle_start(&self, peer: &str, endpoint: &str, session: &str) -> Result<(), String> {
        if peer >= self.local_device_id.as_str()
            || session.is_empty()
            || session.len() > 128
            || !self.endpoint_is_current(peer, endpoint).await
        {
            return Err("registry_sync_start_identity_mismatch".into());
        }
        let mut peers = self.peers.lock().await;
        let entry = peers
            .get_mut(peer)
            .filter(|entry| entry.endpoint == endpoint)
            .ok_or_else(|| "registry_sync_peer_state_missing".to_owned())?;
        let retries = match &entry.admission {
            Admission::Exchanging {
                session_id,
                retry_count,
                ..
            } if session_id != session => retry_count.saturating_add(1),
            Admission::Exchanging { retry_count, .. } => *retry_count,
            _ => 0,
        };
        entry.admission = exchanging(session.to_owned(), retries);
        drop(peers);
        self.send_pages(peer, endpoint, session).await
    }

    async fn handle_page(
        &self,
        peer: &str,
        endpoint: &str,
        incoming: IncomingPage,
        connected: &[(String, String)],
    ) -> Result<RegistryOutcome, String> {
        let IncomingPage {
            session_id: session,
            page_index: page,
            page_count: count,
            events,
        } = incoming;
        if count == 0
            || count > super::super::registry_admission::MAX_REGISTRY_EVENT_PAGE_COUNT
            || page >= count
            || events.len() > super::super::registry_admission::REGISTRY_EVENT_PAGE_SIZE
        {
            return Err("registry_sync_page_bounds".into());
        }
        {
            let peers = self.peers.lock().await;
            let entry = peers
                .get(peer)
                .ok_or_else(|| "registry_sync_without_start".to_owned())?;
            let Admission::Exchanging {
                session_id,
                incoming_page_count,
                next_page,
                ..
            } = &entry.admission
            else {
                return Err("registry_sync_without_start".into());
            };
            if entry.endpoint != endpoint || session_id != &session {
                return Err("registry_sync_session_mismatch".into());
            }
            if incoming_page_count.is_some_and(|old| old != count) || *next_page != page {
                return Err("registry_sync_page_order".into());
            }
            if page + 1 < count
                && events.len() != super::super::registry_admission::REGISTRY_EVENT_PAGE_SIZE
            {
                return Err("registry_sync_short_nonterminal_page".into());
            }
        }
        if !self.endpoint_is_current(peer, endpoint).await
            || !self
                .is_exchanging(peer, endpoint, &session, Some(page))
                .await
        {
            return Err("registry_sync_session_mismatch".into());
        }

        let applied = self
            .storage
            .apply_device_registry_events(peer, &events)
            .await?;
        let mut outcome = RegistryOutcome::applied(&applied);
        if !self
            .commit_page(peer, endpoint, &session, page, count, &applied.digest)
            .await
        {
            outcome.operation_error = Some("registry_sync_session_changed_during_apply".into());
        }
        let publication = self
            .publish_delta_inner(
                &applied.applied_events,
                &applied.disabled_device_ids,
                Some(peer),
                connected,
            )
            .await;
        outcome.merge(publication);
        if outcome.operation_error.is_some() {
            return Ok(outcome);
        }

        if !applied.sender_is_admitted || !applied.local_device_is_admitted {
            if !outcome.revoked_peer_ids.iter().any(|id| id == peer) {
                outcome.merge(self.revoke_peer(peer, endpoint).await);
            }
            return Ok(outcome);
        }
        if page + 1 == count {
            if let Err(error) = self
                .send(
                    endpoint,
                    LanSyncMessage::RegistrySyncReady {
                        session_id: session.clone(),
                        digest: applied.digest,
                    },
                )
                .await
            {
                outcome.set_error(error);
                return Ok(outcome);
            }
            if let Err(error) = self.try_complete(peer, endpoint).await {
                outcome.set_error(error);
            }
            outcome.became_ready = self.is_ready(peer, endpoint).await;
            if outcome.became_ready {
                outcome.policy_negotiated = self.policy_negotiated(peer, endpoint).await;
            }
        }
        Ok(outcome)
    }

    async fn handle_ready(
        &self,
        peer: &str,
        endpoint: &str,
        session: &str,
        digest: String,
    ) -> Result<RegistryOutcome, String> {
        if !valid_digest(&digest) {
            return Err("registry_sync_digest_invalid".into());
        }
        {
            let mut peers = self.peers.lock().await;
            let entry = peers
                .get_mut(peer)
                .ok_or_else(|| "registry_sync_ready_without_start".to_owned())?;
            if entry.endpoint != endpoint {
                return Err("registry_sync_session_mismatch".into());
            }
            let Admission::Exchanging {
                session_id,
                remote_digest,
                ..
            } = &mut entry.admission
            else {
                return Err("registry_sync_ready_without_start".into());
            };
            if session_id != session {
                return Err("registry_sync_session_mismatch".into());
            }
            if remote_digest.as_ref().is_some_and(|old| old != &digest) {
                return Err("registry_sync_ready_conflict".into());
            }
            *remote_digest = Some(digest);
        }
        let mut outcome = RegistryOutcome::default();
        if let Err(error) = self.try_complete(peer, endpoint).await {
            outcome.set_error(error);
        }
        outcome.became_ready = self.is_ready(peer, endpoint).await;
        if outcome.became_ready {
            outcome.policy_negotiated = self.policy_negotiated(peer, endpoint).await;
        }
        Ok(outcome)
    }

    async fn handle_update(
        &self,
        peer: &str,
        endpoint: &str,
        session: &str,
        event: RegistryEvent,
        connected: &[(String, String)],
    ) -> Result<RegistryOutcome, String> {
        let allowed = self.peers.lock().await.get(peer).is_some_and(|entry| {
            let ready = matches!(
                &entry.admission,
                Admission::Ready { session_id } if session_id == session
            );
            let local_disable_during_exchange =
                matches!(
                    &entry.admission,
                    Admission::Exchanging { session_id, .. } if session_id == session
                ) && matches!(event.operation(), RegistryOperation::Disable)
                    && event.target_device_id().as_str() == self.local_device_id;
            entry.endpoint == endpoint && (ready || local_disable_during_exchange)
        });
        if !allowed || !self.endpoint_is_current(peer, endpoint).await {
            return Err("registry_sync_session_mismatch".into());
        }
        let applied = self
            .storage
            .apply_device_registry_events(peer, std::slice::from_ref(&event))
            .await?;
        let mut outcome = RegistryOutcome::applied(&applied);
        if !self.is_bound_session(peer, endpoint, session).await {
            outcome.set_error("registry_sync_session_changed_during_apply");
        }
        let publication = self
            .publish_delta_inner(
                &applied.applied_events,
                &applied.disabled_device_ids,
                Some(peer),
                connected,
            )
            .await;
        outcome.merge(publication);
        if (!applied.sender_is_admitted || !applied.local_device_is_admitted)
            && !outcome.revoked_peer_ids.iter().any(|id| id == peer)
        {
            outcome.merge(self.revoke_peer(peer, endpoint).await);
        }
        Ok(outcome)
    }

    async fn try_complete(&self, peer: &str, endpoint: &str) -> Result<(), String> {
        let snapshot = {
            let peers = self.peers.lock().await;
            let Some(entry) = peers.get(peer) else {
                return Ok(());
            };
            let Admission::Exchanging {
                session_id,
                retry_count,
                local_digest: Some(local),
                remote_digest: Some(remote),
                ..
            } = &entry.admission
            else {
                return Ok(());
            };
            if entry.endpoint != endpoint {
                return Ok(());
            }
            (
                session_id.clone(),
                *retry_count,
                local.clone(),
                remote.clone(),
            )
        };
        let (session, retries, local, remote) = snapshot;
        if local != remote {
            if self.local_device_id.as_str() < peer {
                if retries >= MAX_RETRIES {
                    let mut peers = self.peers.lock().await;
                    if let Some(entry) = peers.get_mut(peer).filter(|entry| {
                        entry.endpoint == endpoint
                            && matches!(
                                &entry.admission,
                                Admission::Exchanging { session_id, .. }
                                    if session_id == &session
                            )
                    }) {
                        entry.admission = Admission::Waiting;
                    }
                    drop(peers);
                    self.transport.unbind_peer(peer)?;
                    return Err("registry_sync_digest_mismatch".into());
                }
                let retry = generate_id();
                {
                    let mut peers = self.peers.lock().await;
                    let Some(entry) = peers.get_mut(peer).filter(|entry| {
                        entry.endpoint == endpoint
                            && matches!(
                                &entry.admission,
                                Admission::Exchanging {
                                    session_id,
                                    local_digest: Some(current_local),
                                    remote_digest: Some(current_remote),
                                    ..
                                } if session_id == &session
                                    && current_local == &local
                                    && current_remote == &remote
                            )
                    }) else {
                        return Ok(());
                    };
                    entry.admission = exchanging(retry.clone(), retries + 1);
                }
                self.send(
                    endpoint,
                    LanSyncMessage::RegistrySyncStart {
                        session_id: retry.clone(),
                    },
                )
                .await?;
                self.send_pages(peer, endpoint, &retry).await?;
            } else if retries >= MAX_RETRIES {
                let mut peers = self.peers.lock().await;
                if let Some(entry) = peers.get_mut(peer).filter(|entry| {
                    entry.endpoint == endpoint
                        && matches!(
                            &entry.admission,
                            Admission::Exchanging { session_id, .. }
                                if session_id == &session
                        )
                }) {
                    entry.admission = Admission::Waiting;
                }
            }
            return Ok(());
        }

        let mut peers = self.peers.lock().await;
        let Some(entry) = peers
            .get_mut(peer)
            .filter(|entry| entry.endpoint == endpoint)
        else {
            return Ok(());
        };
        let matches_snapshot = matches!(
            &entry.admission,
            Admission::Exchanging {
                session_id,
                local_digest: Some(current_local),
                remote_digest: Some(current_remote),
                ..
            } if session_id == &session
                && current_local == &local
                && current_remote == &remote
        );
        if !matches_snapshot {
            return Ok(());
        }
        self.transport.mark_peer_data_ready(peer, endpoint)?;
        entry.admission = Admission::Ready {
            session_id: session,
        };
        Ok(())
    }

    async fn commit_page(
        &self,
        peer: &str,
        endpoint: &str,
        session: &str,
        page: u32,
        count: u32,
        digest: &str,
    ) -> bool {
        let mut peers = self.peers.lock().await;
        let Some(entry) = peers
            .get_mut(peer)
            .filter(|entry| entry.endpoint == endpoint)
        else {
            return false;
        };
        let Admission::Exchanging {
            session_id,
            incoming_page_count,
            next_page,
            local_digest,
            ..
        } = &mut entry.admission
        else {
            return false;
        };
        if session_id != session || *next_page != page {
            return false;
        }
        *incoming_page_count = Some(count);
        *next_page += 1;
        if page + 1 == count {
            *local_digest = Some(digest.to_owned());
        }
        true
    }
}
