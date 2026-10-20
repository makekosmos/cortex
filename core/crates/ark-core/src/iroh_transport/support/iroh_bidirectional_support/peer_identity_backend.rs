use super::*;

#[async_trait::async_trait]
impl PeerIdentityBackend for TestStorage {
    async fn load_local_sync_profile(
        &self,
        local_device_id: &str,
    ) -> Result<Option<ark_application::sync_policy::SyncProfile>, String> {
        self.inner.load_local_sync_profile(local_device_id).await
    }
    async fn accept_peer_sync_policy(
        &self,
        peer_device_id: &str,
        revision: i64,
        digest: &str,
        active: bool,
        rules: &[ark_application::sync_policy::SyncProfileRule],
    ) -> Result<ark_application::sync_policy::AcceptedPeerSyncPolicy, String> {
        self.inner
            .accept_peer_sync_policy(peer_device_id, revision, digest, active, rules)
            .await
    }
    async fn load_accepted_peer_sync_policy(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<ark_application::sync_policy::AcceptedPeerSyncPolicy>, String> {
        self.inner
            .load_accepted_peer_sync_policy(peer_device_id)
            .await
    }
    async fn sync_session_is_admitted(&self, peer_device_id: &str) -> Result<bool, String> {
        self.inner.sync_session_is_admitted(peer_device_id).await
    }
    async fn registry_device_is_enabled(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<bool>, String> {
        self.inner.registry_device_is_enabled(peer_device_id).await
    }
    async fn enroll_paired_device(
        &self,
        local_device_id: &str,
        target_device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        self.inner
            .enroll_paired_device(local_device_id, target_device_id, transport_public_key)
            .await
    }
    async fn load_device_registry_events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<(i64, usize, Vec<ark_domain::device_registry::RegistryEvent>), String> {
        self.inner
            .load_device_registry_events_page(after_position, high_watermark, limit)
            .await
    }
    async fn apply_device_registry_events(
        &self,
        sender_device_id: &str,
        events: &[ark_domain::device_registry::RegistryEvent],
    ) -> Result<ark_core::replication_storage::RegistryPageApply, String> {
        self.inner
            .apply_device_registry_events(sender_device_id, events)
            .await
    }
    async fn sign_registry_handshake(
        &self,
        local_device_id: &str,
        transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<ark_domain::device_registry::RegistryHandshakeProof, String> {
        self.inner
            .sign_registry_handshake(local_device_id, transcript)
            .await
    }
    async fn verify_registry_handshake(
        &self,
        peer_device_id: &str,
        transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
        proof: &ark_domain::device_registry::RegistryHandshakeProof,
    ) -> Result<(), String> {
        self.inner
            .verify_registry_handshake(peer_device_id, transcript, proof)
            .await
    }

    async fn load_peer_transport_keys(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, String> {
        self.inner.load_peer_transport_keys().await
    }
    async fn store_peer_transport_keys(
        &self,
        keys: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        self.inner.store_peer_transport_keys(keys).await
    }
    async fn authorized_transport_public_key(
        &self,
        device_id: &str,
    ) -> Result<Option<String>, String> {
        self.inner.authorized_transport_public_key(device_id).await
    }
}
