pub use ark_application::replication_storage::{
    AuthenticatedEntityBackend, IntegrationReplicationBackend, PeerIdentityBackend,
    RegistryPageApply,
};
pub use ark_application::sync_storage::SyncRuntimeStorage;

#[cfg(test)]
mod tests {
    use super::IntegrationReplicationBackend;
    use crate::integration_replication::SignedSyncEnvelope;
    use crate::types::VersionVector;
    use ark_application::sync_storage::SyncReadClass;

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
        async fn validate_outbound_signed_integration_frame(
            &self,
            _frame: &SignedSyncEnvelope,
            _expected_space_id: &str,
            _expected_origin_node_id: &str,
        ) -> Result<(), String> {
            Err("signed integration outbound validation unsupported in test backend".into())
        }

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
