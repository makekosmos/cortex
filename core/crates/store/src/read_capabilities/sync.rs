use super::SyncReadCapabilities;
use domain::sync_policy::PolicyIdentity;
use domain::sync_policy::{AcceptedPeerSyncPolicy, SyncProfile};
use domain::sync_storage::{
    CommittedFeedDeliveryPage, CommittedFeedEntry, FeedSnapshot, PolicyBoundFeedPage,
    PolicyBoundFeedReadError, PolicyBoundSnapshotPage, PolicyBoundSnapshotReadError,
};
use domain::types::{SyncEntity, VersionVector};
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
    ) -> Result<Option<AcceptedPeerSyncPolicy>, String> {
        crate::data_platform::load_accepted_peer_sync_policy(self.connection, peer_device_id)
    }

    pub fn active_local_sync_profile(
        &self,
        device_id: &str,
    ) -> Result<Option<SyncProfile>, String> {
        crate::data_platform::load_active_sync_profile(self.connection, device_id)
    }

    pub fn sync_profile(&self, local_device_id: &str) -> Result<Option<SyncProfile>, String> {
        crate::data_platform::load_sync_profile_for_sync(self.connection, local_device_id)
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

    pub fn sync_peer_is_admitted(&self, peer_device_id: &str) -> Result<bool, String> {
        let peer_admitted =
            crate::device_registry::is_device_admitted(self.connection, peer_device_id)?
                .unwrap_or(false);
        let local_device_id = crate::sync_kv::get_sync_kv(
            self.connection,
            crate::sync_kv::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY,
        )?;
        let local_admitted = local_device_id
            .as_deref()
            .map(|device_id| crate::device_registry::is_device_admitted(self.connection, device_id))
            .transpose()?
            .flatten()
            .unwrap_or(true);
        Ok(peer_admitted && local_admitted)
    }

    pub fn device_is_admitted(&self, device_id: &str) -> Result<Option<bool>, String> {
        crate::device_registry::is_device_admitted(self.connection, device_id)
    }

    pub fn authorized_transport_key(&self, device_id: &str) -> Result<Option<String>, String> {
        let raw = crate::sync_kv::get_sync_kv(
            self.connection,
            crate::device_registry::PEER_TRANSPORT_KEYS_KEY,
        )?;
        let keys: std::collections::HashMap<String, String> = raw
            .map(|raw| serde_json::from_str(&raw))
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        Ok(keys.get(device_id).cloned())
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
