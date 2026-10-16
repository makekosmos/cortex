use std::collections::HashMap;

use crate::integration_replication::SignedSyncEnvelope;
use crate::types::{SyncEntity, VersionVector};

/// Priority of an outbound sync read. Initial device hydration may run for a
/// long time and must not share the live-change admission class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncReadClass {
    SyncLive,
    Backfill,
}

impl SyncReadClass {
    /// A live session stays in backfill mode until it completes its first sync.
    pub fn for_sync_state(sync_complete: bool) -> Self {
        if sync_complete {
            Self::SyncLive
        } else {
            Self::Backfill
        }
    }

    /// Usage cursors alone do not prove that the peer has received ordinary
    /// records; treat a usage-only vector as an initial hydration.
    pub fn for_peer_vector(vector: &VersionVector) -> Self {
        if vector.keys().any(|key| !key.starts_with("@usage:")) {
            Self::SyncLive
        } else {
            Self::Backfill
        }
    }
}

#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity>;
    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity>;

    /// Load a page for a known peer. The peer and class are carried to storage
    /// so per-device selection and bounded Backfill admission can be enforced
    /// without changing the sync wire format. Backends that do not need the
    /// context retain the original paging behavior.
    async fn load_entities_page_for_peer(
        &self,
        _peer_device_id: &str,
        _class: SyncReadClass,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity> {
        self.load_entities_page(vector, offset, limit).await
    }
    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String>;
    async fn get_kv(&self, key: &str) -> Option<String>;
    async fn set_kv(&self, key: &str, value: &str);

    /// Per usage origin device, the highest seq this node can serve
    /// completely — sent as `usage_complete_through` on the final sync page
    /// so the receiver can raise its `@usage:` cursor past compacted holes
    /// (KOS-302). Backends without a usage journal return an empty map and
    /// keep contiguous-only cursor advancement.
    async fn usage_complete_through(&self) -> HashMap<String, u64> {
        HashMap::new()
    }

    /// Peer-aware form of the sync completeness query. The default preserves
    /// existing backends while allowing policy-aware stores to scope it.
    async fn usage_complete_through_for_peer(
        &self,
        _peer_device_id: &str,
        _class: SyncReadClass,
    ) -> HashMap<String, u64> {
        self.usage_complete_through().await
    }

    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity> {
        Some(entity.clone())
    }
}

/// Applies an ordinary entity only after the sync boundary authenticated its peer.
#[async_trait::async_trait]
pub trait AuthenticatedEntityBackend: Send + Sync {
    async fn apply_authenticated_entity(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
    ) -> Result<(), String> {
        Err("authenticated_apply_unsupported".into())
    }
}

/// Trusted transport identity records used while authenticating a paired peer.
#[async_trait::async_trait]
pub trait PeerIdentityBackend: Send + Sync {
    async fn authorized_transport_public_key(&self, _device_id: &str) -> Option<String> {
        None
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
pub trait SyncStorageBackend
where
    Self: StorageBackend
        + AuthenticatedEntityBackend
        + PeerIdentityBackend
        + IntegrationReplicationBackend,
{
}

impl<T> SyncStorageBackend for T where
    T: StorageBackend
        + AuthenticatedEntityBackend
        + PeerIdentityBackend
        + IntegrationReplicationBackend
        + ?Sized
{
}

#[cfg(test)]
mod tests {
    use super::{IntegrationReplicationBackend, SyncReadClass};
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
