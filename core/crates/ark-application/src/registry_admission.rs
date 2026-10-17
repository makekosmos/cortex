use crate::replication_storage::{PeerIdentityBackend, RegistryPageApply};
use ark_domain::device_registry::{
    RegistryEvent, RegistryHandshakeProof, RegistryHandshakeTranscript,
};

/// Narrow storage port used by the direct sync admission state machine.
#[async_trait::async_trait]
pub trait RegistryAdmissionStorage: Send + Sync {
    async fn load_device_registry_events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<(i64, usize, Vec<RegistryEvent>), String>;

    async fn apply_device_registry_events(
        &self,
        sender_device_id: &str,
        events: &[RegistryEvent],
    ) -> Result<RegistryPageApply, String>;

    async fn sign_registry_handshake(
        &self,
        local_device_id: &str,
        transcript: &RegistryHandshakeTranscript,
    ) -> Result<RegistryHandshakeProof, String>;

    async fn verify_registry_handshake(
        &self,
        peer_device_id: &str,
        transcript: &RegistryHandshakeTranscript,
        proof: &RegistryHandshakeProof,
    ) -> Result<(), String>;
}

#[async_trait::async_trait]
impl<T: PeerIdentityBackend + ?Sized> RegistryAdmissionStorage for T {
    async fn load_device_registry_events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<(i64, usize, Vec<RegistryEvent>), String> {
        PeerIdentityBackend::load_device_registry_events_page(
            self,
            after_position,
            high_watermark,
            limit,
        )
        .await
    }

    async fn apply_device_registry_events(
        &self,
        sender_device_id: &str,
        events: &[RegistryEvent],
    ) -> Result<RegistryPageApply, String> {
        PeerIdentityBackend::apply_device_registry_events(self, sender_device_id, events).await
    }

    async fn sign_registry_handshake(
        &self,
        local_device_id: &str,
        transcript: &RegistryHandshakeTranscript,
    ) -> Result<RegistryHandshakeProof, String> {
        PeerIdentityBackend::sign_registry_handshake(self, local_device_id, transcript).await
    }

    async fn verify_registry_handshake(
        &self,
        peer_device_id: &str,
        transcript: &RegistryHandshakeTranscript,
        proof: &RegistryHandshakeProof,
    ) -> Result<(), String> {
        PeerIdentityBackend::verify_registry_handshake(self, peer_device_id, transcript, proof)
            .await
    }
}
