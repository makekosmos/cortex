use crate::integration_replication::SignedSyncEnvelope;
use crate::types::SyncEntity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryPageApply {
    pub digest: String,
    pub sender_is_admitted: bool,
    pub local_device_is_admitted: bool,
    pub disabled_device_ids: Vec<String>,
    pub applied_events: Vec<crate::domain::device_registry::RegistryEvent>,
}

/// Applies an ordinary entity only after the sync boundary authenticated its peer.
#[async_trait::async_trait]
pub trait AuthenticatedEntityBackend: Send + Sync {
    /// Commits the bounded inbox receipt before applying the payload. Returns
    /// `true` only when this backend provides that durable boundary.
    async fn receive_authenticated_entity_with_id(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
        _message_id: &str,
    ) -> Result<bool, String> {
        Ok(false)
    }

    async fn apply_authenticated_entity(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
    ) -> Result<(), String> {
        Err("authenticated_apply_unsupported".into())
    }

    async fn apply_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        _message_id: &str,
    ) -> Result<crate::application::sync_storage::InboundApplyOutcome, String> {
        self.apply_authenticated_entity(entity, authenticated_peer_id)
            .await
            .map(|()| crate::application::sync_storage::InboundApplyOutcome::Applied)
    }
}

/// Trusted transport identity records used while authenticating a paired peer.
#[async_trait::async_trait]
pub trait PeerIdentityBackend: Send + Sync {
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
    ) -> Result<(), String> {
        let _ = (local_device_id, target_device_id, transport_public_key);
        Err("signed device registry enrollment is unsupported by this storage backend".into())
    }

    async fn load_device_registry_events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<
        (
            i64,
            usize,
            Vec<crate::domain::device_registry::RegistryEvent>,
        ),
        String,
    > {
        let _ = (after_position, high_watermark, limit);
        Err("signed device registry history is unsupported by this storage backend".into())
    }

    async fn apply_device_registry_events(
        &self,
        sender_device_id: &str,
        events: &[crate::domain::device_registry::RegistryEvent],
    ) -> Result<RegistryPageApply, String> {
        let _ = (sender_device_id, events);
        Err("signed device registry history is unsupported by this storage backend".into())
    }

    async fn sign_registry_handshake(
        &self,
        _local_device_id: &str,
        _transcript: &crate::domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<crate::domain::device_registry::RegistryHandshakeProof, String> {
        Err("registry handshake signing is unsupported by this storage backend".into())
    }

    async fn verify_registry_handshake(
        &self,
        _peer_device_id: &str,
        _transcript: &crate::domain::device_registry::RegistryHandshakeTranscript,
        _proof: &crate::domain::device_registry::RegistryHandshakeProof,
    ) -> Result<(), String> {
        Err("registry handshake verification is unsupported by this storage backend".into())
    }
}

/// Persistence operations for signed integration replication. Kept separate
/// from ordinary entity storage so application and sync code can depend on
/// only the capabilities they use.
#[async_trait::async_trait]
pub trait IntegrationReplicationBackend: Send + Sync {
    async fn validate_outbound_signed_integration_frame(
        &self,
        _frame: &SignedSyncEnvelope,
        _expected_space_id: &str,
        _expected_origin_node_id: &str,
    ) -> Result<(), String> {
        Err("signed integration outbound validation is unsupported by this storage backend".into())
    }
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
    ) -> Result<(), String> {
        let _ = (
            frame,
            expected_space_id,
            authenticated_peer_id,
            expected_recipient_node_id,
        );
        Err("signed integration replication is unsupported by this storage backend".into())
    }
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

/// The complete storage surface required by the sync runtime.
pub trait SyncRuntimeStorage
where
    Self: crate::application::sync_storage::SyncEntityReadPort
        + crate::application::sync_storage::SyncEntityMutationPort
        + crate::application::sync_storage::PeerProgressPort
        + crate::application::sync_storage::CommittedFeedPort
        + AuthenticatedEntityBackend
        + PeerIdentityBackend
        + IntegrationReplicationBackend,
{
}

impl<T> SyncRuntimeStorage for T where
    T: crate::application::sync_storage::SyncEntityReadPort
        + crate::application::sync_storage::SyncEntityMutationPort
        + crate::application::sync_storage::PeerProgressPort
        + crate::application::sync_storage::CommittedFeedPort
        + AuthenticatedEntityBackend
        + PeerIdentityBackend
        + IntegrationReplicationBackend
        + ?Sized
{
}

#[cfg(test)]
mod tests {
    use super::IntegrationReplicationBackend;
    use crate::application::sync_storage::SyncReadClass;
    use crate::integration_replication::SignedSyncEnvelope;
    use crate::types::VersionVector;

    #[test]
    fn usage_only_peer_vector_is_still_backfill() {
        let mut vector = VersionVector::new();
        assert_eq!(
            SyncReadClass::for_peer_vector(&vector),
            SyncReadClass::Backfill
        );
        vector.insert("@usage:phone".into(), "23".into());
        assert_eq!(
            SyncReadClass::for_peer_vector(&vector),
            SyncReadClass::Backfill
        );
        vector.insert("object-1".into(), "2026-01-01:000001:desktop".into());
        assert_eq!(
            SyncReadClass::for_peer_vector(&vector),
            SyncReadClass::SyncLive
        );
    }

    #[test]
    fn unfinished_sync_stays_backfill_even_with_a_partial_vector() {
        assert_eq!(
            SyncReadClass::for_sync_state(false),
            SyncReadClass::Backfill
        );
        assert_eq!(SyncReadClass::for_sync_state(true), SyncReadClass::SyncLive);
    }

    struct BaseOnlyBackend;

    #[async_trait::async_trait]
    impl IntegrationReplicationBackend for BaseOnlyBackend {
        async fn apply_signed_integration_frame(
            &self,
            _frame: &SignedSyncEnvelope,
            _expected_space_id: &str,
            _authenticated_peer_id: &str,
            _expected_recipient_node_id: &str,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn transport_key_is_rejected_when_backend_cannot_authorize_it() {
        let frame =
            SignedSyncEnvelope::new("space", "origin", "recipient", 1, "msg", vec![], "sig");

        let result = BaseOnlyBackend
            .apply_signed_integration_frame_with_transport(
                &frame,
                "space",
                "peer",
                "recipient",
                Some("transport-key"),
            )
            .await;

        assert_eq!(
            result.unwrap_err(),
            "signed integration transport authorization is unsupported by this storage backend"
        );
    }
}
