use ark_domain::integration_replication::SignedSyncEnvelope;
use ark_domain::types::SyncEntity;

/// Persistence operations for signed integration replication. Kept separate
/// from ordinary entity storage so application and sync code can depend on
/// only the capabilities they use.
#[async_trait::async_trait]
pub trait IntegrationReplicationBackend: Send + Sync {
    async fn validate_outbound_signed_integration_frame(
        &self,
        frame: &SignedSyncEnvelope,
        expected_space_id: &str,
        expected_origin_node_id: &str,
    ) -> Result<(), String>;
    async fn validate_outbound_signed_integration_frame_with_transport(
        &self,
        frame: &SignedSyncEnvelope,
        expected_space_id: &str,
        expected_origin_node_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        let _ = (
            frame,
            expected_space_id,
            expected_origin_node_id,
            transport_public_key,
        );
        Err(concat!(
            "signed integration outbound transport authorization is unsupported by this ",
            "storage backend"
        )
        .into())
    }
    async fn apply_signed_integration_frame(
        &self,
        frame: &SignedSyncEnvelope,
        expected_space_id: &str,
        authenticated_peer_id: &str,
        expected_recipient_node_id: &str,
    ) -> Result<(), String>;
    async fn apply_signed_integration_frame_with_transport(
        &self,
        frame: &SignedSyncEnvelope,
        expected_space_id: &str,
        authenticated_peer_id: &str,
        expected_recipient_node_id: &str,
        authenticated_transport_public_key: Option<&str>,
    ) -> Result<(), String> {
        if authenticated_transport_public_key.is_some() {
            return Err(
                "signed integration transport authorization is unsupported by this storage backend"
                    .into(),
            );
        }
        self.apply_signed_integration_frame(
            frame,
            expected_space_id,
            authenticated_peer_id,
            expected_recipient_node_id,
        )
        .await
    }
}

/// Applies an ordinary entity only after the sync boundary authenticated its peer.
#[async_trait::async_trait]
pub trait AuthenticatedEntityBackend: Send + Sync {
    /// Commits the bounded inbox receipt before applying the payload. Returns
    /// `true` only when this backend provides that durable boundary.
    async fn receive_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
    ) -> Result<bool, String>;

    async fn receive_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
        local_device_id: &str,
        policy_identity: Option<ark_domain::sync_policy::PolicyIdentity>,
    ) -> Result<bool, String>;

    async fn apply_authenticated_entity(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
    ) -> Result<(), String>;

    async fn apply_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
    ) -> Result<crate::sync_storage::InboundApplyOutcome, String>;

    async fn apply_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
        local_device_id: &str,
        policy_identity: Option<ark_domain::sync_policy::PolicyIdentity>,
    ) -> Result<crate::sync_storage::InboundApplyOutcome, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryPageApply {
    pub digest: String,
    pub sender_is_admitted: bool,
    pub local_device_is_admitted: bool,
    pub disabled_device_ids: Vec<String>,
    pub applied_events: Vec<ark_domain::device_registry::RegistryEvent>,
}

/// Trusted transport identity records used while authenticating a paired peer.
#[async_trait::async_trait]
pub trait PeerIdentityBackend: Send + Sync {
    async fn load_local_sync_profile(
        &self,
        local_device_id: &str,
    ) -> Result<Option<crate::sync_policy::SyncProfile>, String>;

    async fn accept_peer_sync_policy(
        &self,
        peer_device_id: &str,
        revision: i64,
        digest: &str,
        active: bool,
        rules: &[crate::sync_policy::SyncProfileRule],
    ) -> Result<crate::sync_policy::AcceptedPeerSyncPolicy, String>;

    async fn load_accepted_peer_sync_policy(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<crate::sync_policy::AcceptedPeerSyncPolicy>, String>;

    async fn sync_session_is_admitted(&self, peer_device_id: &str) -> Result<bool, String>;

    async fn registry_device_is_enabled(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<bool>, String>;

    async fn authorized_transport_public_key(
        &self,
        device_id: &str,
    ) -> Result<Option<String>, String>;
    async fn load_peer_transport_keys(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, String>;
    async fn store_peer_transport_keys(
        &self,
        keys: &std::collections::HashMap<String, String>,
    ) -> Result<(), String>;
    async fn enroll_paired_device(
        &self,
        local_device_id: &str,
        target_device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String>;

    async fn load_device_registry_events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<(i64, usize, Vec<ark_domain::device_registry::RegistryEvent>), String>;

    async fn apply_device_registry_events(
        &self,
        sender_device_id: &str,
        events: &[ark_domain::device_registry::RegistryEvent],
    ) -> Result<RegistryPageApply, String>;

    async fn sign_registry_handshake(
        &self,
        local_device_id: &str,
        transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<ark_domain::device_registry::RegistryHandshakeProof, String>;

    async fn verify_registry_handshake(
        &self,
        peer_device_id: &str,
        transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
        proof: &ark_domain::device_registry::RegistryHandshakeProof,
    ) -> Result<(), String>;
}
