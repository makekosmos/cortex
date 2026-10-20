use super::SyncReadCapabilities;
use ark_application::sync_storage::{
    CommittedFeedDeliveryPage, CommittedFeedEntry, FeedSnapshot, PolicyBoundFeedPage,
    PolicyBoundFeedReadError, PolicyBoundSnapshotPage, PolicyBoundSnapshotReadError,
    PolicyIdentity,
};
use ark_domain::types::{SyncEntity, VersionVector};
use std::collections::HashMap;

impl SyncReadCapabilities<'_> {
    pub fn collect_entities(
        &self,
        vector: &VersionVector,
        device_id: &str,
    ) -> Result<Vec<SyncEntity>, String> {
        crate::sync_entities::collect_entities(self.connection, vector, device_id)
    }

    pub fn collect_entities_page(
        &self,
        vector: &VersionVector,
        device_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        crate::sync_entities::collect_entities_page(
            self.connection,
            vector,
            device_id,
            offset,
            limit,
        )
    }

    pub fn accepted_peer_policy(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<crate::data_platform::AcceptedPeerSyncPolicy>, String> {
        crate::data_platform::load_accepted_peer_sync_policy(self.connection, peer_device_id)
    }

    pub fn usage_complete_through(&self) -> HashMap<String, u64> {
        crate::usage_sync_log::usage_log_complete_through(self.connection, None).unwrap_or_default()
    }

    pub fn committed_feed_snapshot(&self) -> Result<FeedSnapshot, String> {
        crate::committed_feed::committed_feed_snapshot(self.connection)
    }

    pub fn committed_feed_page(
        &self,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<Vec<CommittedFeedEntry>, String> {
        crate::committed_feed::read_committed_feed_page(
            self.connection,
            after_position,
            high_watermark,
            limit,
        )
    }

    pub fn committed_feed_delivery_page(
        &self,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
        max_bytes: usize,
    ) -> Result<CommittedFeedDeliveryPage, String> {
        crate::committed_feed::read_committed_feed_delivery_page(
            self.connection,
            after_position,
            high_watermark,
            limit,
            max_bytes,
        )
    }

    pub fn sync_key_value(&self, key: &str) -> Result<Option<String>, String> {
        crate::sync_kv::get_sync_kv(self.connection, key)
    }

    pub fn authorized_transport_key(&self, device_id: &str) -> Result<Option<String>, String> {
        Ok(
            crate::integration_replication::load_authorized_node(self.connection, device_id)?
                .and_then(|node| node.transport_public_key),
        )
    }

    pub fn policy_bound_entities_page(
        &self,
        peer_device_id: &str,
        expected_identity: &PolicyIdentity,
        vector: &VersionVector,
        device_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<PolicyBoundSnapshotPage, PolicyBoundSnapshotReadError> {
        crate::sync_entities::read_policy_bound_page(
            self.connection,
            peer_device_id,
            expected_identity,
            vector,
            device_id,
            offset,
            limit,
        )
    }

    pub fn policy_bound_feed_page(
        &self,
        peer_device_id: &str,
        expected_identity: &PolicyIdentity,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
        max_bytes: usize,
    ) -> Result<PolicyBoundFeedPage, PolicyBoundFeedReadError> {
        crate::committed_feed::read_committed_feed_delivery_page_for_peer(
            self.connection,
            peer_device_id,
            expected_identity,
            after_position,
            high_watermark,
            limit,
            max_bytes,
        )
    }
}
