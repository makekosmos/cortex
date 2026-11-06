use super::*;

impl<S: RegistryAdmissionStorage + ?Sized + 'static> RelayRegistryCoordinator<S> {
    pub(super) async fn send_pages(
        &self,
        peer: &str,
        endpoint: &str,
        session: &str,
    ) -> Result<(), String> {
        let pages =
            crate::registry_admission::load_registry_event_pages(self.storage.as_ref(), session)
                .await?;
        for message in pages {
            if !self.is_exchanging(peer, endpoint, session, None).await {
                return Err("registry_sync_session_lost".into());
            }
            self.send(endpoint, message).await?;
        }
        Ok(())
    }

    pub(super) async fn endpoint_is_current(&self, peer: &str, endpoint: &str) -> bool {
        self.peers
            .lock()
            .await
            .get(peer)
            .is_some_and(|entry| entry.endpoint == endpoint)
    }

    pub(super) async fn is_exchanging(
        &self,
        peer: &str,
        endpoint: &str,
        session: &str,
        page: Option<u32>,
    ) -> bool {
        self.peers.lock().await.get(peer).is_some_and(|entry| {
            entry.endpoint == endpoint
                && matches!(&entry.admission, Admission::Exchanging { session_id, next_page, .. }
                    if session_id == session && page.is_none_or(|page| *next_page == page))
        })
    }

    pub(super) async fn is_bound_session(&self, peer: &str, endpoint: &str, session: &str) -> bool {
        self.peers
            .lock()
            .await
            .get(peer)
            .is_some_and(|entry| entry.endpoint == endpoint && entry.session_id() == Some(session))
    }

    pub(super) async fn policy_negotiated(&self, peer: &str, endpoint: &str) -> bool {
        self.peers
            .lock()
            .await
            .get(peer)
            .is_some_and(|entry| entry.endpoint == endpoint && entry.policy_negotiated)
    }
}
