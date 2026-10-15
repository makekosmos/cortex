use std::collections::HashMap;

use crate::integration_replication::SignedSyncEnvelope;
use crate::types::{SyncEntity, VersionVector};

#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn authorized_transport_public_key(&self, _device_id: &str) -> Option<String> {
        None
    }
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
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity>;
    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity>;
    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String>;
    async fn apply_authenticated_entity(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
    ) -> Result<(), String> {
        Err("authenticated_apply_unsupported".into())
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
        let _ = authenticated_transport_public_key;
        self.apply_signed_integration_frame(
            frame,
            expected_space_id,
            authenticated_peer_id,
            expected_recipient_node_id,
        )
        .await
    }
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

    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity> {
        Some(entity.clone())
    }
}
