use super::*;

#[cfg(test)]
#[path = "registry_fanout_tests.rs"]
mod tests;

impl RelaySync {
    pub(super) async fn registry_ready_for_peer(
        &self,
        peer_device_id: &str,
        transport_public_key: Option<&str>,
    ) -> bool {
        let Some(endpoint) = transport_public_key else {
            return false;
        };
        self.registry.is_ready(peer_device_id, endpoint).await
    }

    pub(super) async fn registry_ready_for_connected_peer(&self, peer_device_id: &str) -> bool {
        let endpoint = self
            .transport_keys
            .lock()
            .await
            .get(peer_device_id)
            .cloned();
        self.registry_ready_for_peer(peer_device_id, endpoint.as_deref())
            .await
    }

    pub(super) async fn transport_key_matches(&self, peer_device_id: &str, endpoint: &str) -> bool {
        self.transport_keys
            .lock()
            .await
            .get(peer_device_id)
            .is_some_and(|key| key == endpoint)
    }

    pub(crate) async fn publish_registry_event(
        &self,
        event: ark_domain::device_registry::RegistryEvent,
    ) -> Result<(), String> {
        let disabled = matches!(
            event.operation(),
            ark_domain::device_registry::RegistryOperation::Disable
        )
        .then(|| event.target_device_id().as_str().to_owned())
        .into_iter()
        .collect::<Vec<_>>();
        let connected = self.connected_transport_peers().await;
        let outcome = self
            .registry
            .publish_delta(&[event], &disabled, None, &connected)
            .await;
        self.finish_registry_outcome(outcome, false, None, false)
            .await
    }
}
