use std::collections::HashMap;

use ark_domain::types::{PeerRecord, SyncEntity, VersionVector};

pub use crate::sync_policy::PolicyIdentity;
pub use crate::task_merge::{
    TaskMergeBatch, TaskMergeBatchAck, TaskMergeDeliveryPort, TaskMergeJournalEntry,
    TaskMergeJournalPage, TaskMergeJournalRecord, TaskMergePrepareError,
};

/// Complete storage surface required by the sync runtime.
pub trait SyncRuntimeStorage:
    SyncEntityReadPort
    + SyncEntityMutationPort
    + PeerProgressPort
    + PeerSyncPolicyPort
    + CommittedFeedPort
    + TaskMergeDeliveryPort
    + crate::replication_storage::AuthenticatedEntityBackend
    + crate::replication_storage::PeerIdentityBackend
    + crate::replication_storage::IntegrationReplicationBackend
{
}

impl<T> SyncRuntimeStorage for T where
    T: SyncEntityReadPort
        + SyncEntityMutationPort
        + PeerProgressPort
        + PeerSyncPolicyPort
        + CommittedFeedPort
        + TaskMergeDeliveryPort
        + crate::replication_storage::AuthenticatedEntityBackend
        + crate::replication_storage::PeerIdentityBackend
        + crate::replication_storage::IntegrationReplicationBackend
        + ?Sized
{
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayNotice {
    pub entity_id: String,
    pub type_id: String,
}

#[async_trait::async_trait]
pub trait PeerSyncPolicyPort: Send + Sync {
    async fn accepted_policy_identity_for_peer(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<PolicyIdentity>, String>;
}

#[cfg(test)]
mod tests {
    use super::ReplayNotice;

    #[test]
    fn replay_notice_keeps_its_event_wire_shape() {
        let notice = ReplayNotice {
            entity_id: "object-1".into(),
            type_id: "book".into(),
        };

        assert_eq!(
            notice.event_payload(),
            serde_json::json!({
                "event": "sync_replay",
                "entity_type": "object",
                "entity_id": "object-1",
                "type_id": "book"
            })
        );
    }
}

impl ReplayNotice {
    pub fn event_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "event": "sync_replay",
            "entity_type": "object",
            "entity_id": self.entity_id,
            "type_id": self.type_id,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommittedFeedEntry {
    pub position: i64,
    pub entity_type: String,
    pub entity_id: String,
    pub revision: String,
    pub deleted: bool,
    pub origin_device_id: Option<String>,
    pub origin_sequence: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeedSnapshot {
    /// Largest cursor that can be resumed without a resnapshot.
    pub retained_through: i64,
    /// Fixed upper bound for paging this snapshot.
    pub high_watermark: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommittedFeedDelivery {
    pub entry: CommittedFeedEntry,
    pub entity: SyncEntity,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CommittedFeedDeliveryPage {
    Deliveries(Vec<CommittedFeedDelivery>),
    /// The page contained only entities this peer must not receive. The
    /// sender can advance its cursor through this position without sending a
    /// payload.
    SkipTo {
        through_position: i64,
    },
    TooLarge {
        position: i64,
        entity_id: String,
        serialized_bytes: usize,
        max_bytes: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolicyBoundFeedPage {
    pub identity: PolicyIdentity,
    pub page: CommittedFeedDeliveryPage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolicyBoundSnapshotPage {
    pub identity: PolicyIdentity,
    pub entities: Vec<SyncEntity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutboundProgress {
    pub policy_generation: i64,
    pub initial_snapshot: InitialSnapshot,
    pub scanned_through: i64,
    pub receipt_through: i64,
    pub applied_through: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InitialSnapshot {
    Pending { through: i64 },
    Complete { through: i64 },
}

impl InitialSnapshot {
    pub fn is_complete(self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    pub fn through(self) -> i64 {
        match self {
            Self::Pending { through } | Self::Complete { through } => through,
        }
    }
}

impl Default for OutboundProgress {
    fn default() -> Self {
        Self {
            policy_generation: 0,
            initial_snapshot: InitialSnapshot::Complete { through: 0 },
            scanned_through: 0,
            receipt_through: 0,
            applied_through: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OutboundAck {
    DurableReceipt,
    Applied,
}

/// Priority of an outbound sync read. Initial hydration uses a separate
/// admission class from live changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncReadClass {
    SyncLive,
    Backfill,
}

impl SyncReadClass {
    pub fn for_sync_state(sync_complete: bool) -> Self {
        if sync_complete {
            Self::SyncLive
        } else {
            Self::Backfill
        }
    }

    pub fn for_peer_vector(vector: &VersionVector) -> Self {
        if vector.keys().any(|key| !key.starts_with("@usage:")) {
            Self::SyncLive
        } else {
            Self::Backfill
        }
    }
}

/// Outbound entity reads, including peer admission context.
///
/// Each page is collected by one serialized database query. Separate page calls
/// do not share a pinned snapshot, so writes between calls may affect later pages.
#[async_trait::async_trait]
pub trait SyncEntityReadPort: Send + Sync {
    async fn load_entities(&self, vector: &VersionVector) -> Result<Vec<SyncEntity>, String>;
    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String>;
    async fn load_entities_page_for_peer(
        &self,
        peer_device_id: &str,
        class: SyncReadClass,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String>;
    async fn load_policy_bound_entities_page_for_peer(
        &self,
        peer_device_id: &str,
        class: SyncReadClass,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
        expected_identity: &PolicyIdentity,
    ) -> Result<PolicyBoundSnapshotPage, String> {
        let _ = (
            peer_device_id,
            class,
            vector,
            offset,
            limit,
            expected_identity,
        );
        Err("policy_bound_snapshot_page_unsupported".into())
    }
    async fn usage_complete_through(&self) -> Result<HashMap<String, u64>, String>;
    async fn usage_complete_through_for_peer(
        &self,
        peer_device_id: &str,
        class: SyncReadClass,
    ) -> Result<HashMap<String, u64>, String>;
    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity>;

    async fn filter_outgoing_entity_for_peer(
        &self,
        _peer_device_id: &str,
        entity: &SyncEntity,
    ) -> Result<Option<SyncEntity>, String> {
        Ok(self.filter_outgoing_entity(entity))
    }
}

/// Applies one inbound ordinary entity atomically at the storage adapter.
#[async_trait::async_trait]
pub trait SyncEntityMutationPort: Send + Sync {
    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String>;
}

/// Transaction-scoped capability used by the inbound sync application operation.
/// The adapter owns the transaction and exposes only this synchronous operation.
pub trait SyncEntityMutationSession {
    fn apply_entity(
        &mut self,
        entity: &SyncEntity,
        authenticated_peer_id: Option<&str>,
    ) -> Result<(InboundApplyOutcome, Vec<ReplayNotice>), String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboundApplyOutcome {
    Applied,
    Stale,
    Pending,
}

pub fn apply_inbound_sync_entity(
    session: &mut dyn SyncEntityMutationSession,
    entity: &SyncEntity,
    authenticated_peer_id: Option<&str>,
) -> Result<(InboundApplyOutcome, Vec<ReplayNotice>), String> {
    session.apply_entity(entity, authenticated_peer_id)
}

/// Persists peer vectors and peer-list progress consumed by sync workflows.
#[async_trait::async_trait]
pub trait PeerProgressPort: Send + Sync {
    async fn load_version_vector(&self) -> Result<VersionVector, String>;
    async fn store_version_vector(&self, vector: &VersionVector) -> Result<(), String>;
    async fn store_version_vector_with_policy(
        &self,
        _local_device_id: &str,
        _policy_identity: &PolicyIdentity,
        _vector: &VersionVector,
    ) -> Result<(), String> {
        Err("policy_bound_vector_storage_unsupported".into())
    }
    async fn load_known_peers(&self) -> Result<Option<Vec<PeerRecord>>, String>;
    async fn store_known_peers(&self, peers: &[PeerRecord]) -> Result<(), String>;
    async fn load_removed_peer_ids(&self) -> Result<Vec<String>, String>;
    async fn store_removed_peer_ids(&self, peer_ids: &[String]) -> Result<(), String>;
}

/// Durable recovery reads. A caller pins one high watermark and pages by
/// committed position so concurrent writes cannot shift a page boundary.
#[async_trait::async_trait]
pub trait CommittedFeedPort: Send + Sync {
    fn selective_sync_policy_generation(&self) -> i64;

    async fn selective_sync_policy_generation_for_peer(
        &self,
        _peer_device_id: &str,
    ) -> Result<i64, String> {
        Ok(self.selective_sync_policy_generation())
    }

    async fn committed_feed_snapshot(&self) -> Result<FeedSnapshot, String>;

    async fn committed_feed_page(
        &self,
        _after_position: i64,
        _high_watermark: i64,
        _limit: usize,
    ) -> Result<Vec<CommittedFeedEntry>, String>;

    async fn committed_feed_delivery_page(
        &self,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<CommittedFeedDeliveryPage, String>;

    async fn committed_feed_delivery_page_for_peer(
        &self,
        peer_device_id: &str,
        expected_identity: &PolicyIdentity,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<PolicyBoundFeedPage, String> {
        let _ = (
            peer_device_id,
            expected_identity,
            after_position,
            high_watermark,
            limit,
        );
        Err("policy_bound_feed_page_unsupported".into())
    }

    async fn begin_outbound_policy(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
    ) -> Result<OutboundProgress, String>;

    async fn begin_outbound_policy_identity(
        &self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
    ) -> Result<OutboundProgress, String> {
        let _ = (peer_device_id, identity);
        Err("policy_bound_progress_unsupported".into())
    }

    async fn complete_outbound_snapshot(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
    ) -> Result<OutboundProgress, String>;

    async fn complete_outbound_snapshot_identity(
        &self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
    ) -> Result<OutboundProgress, String> {
        let _ = (peer_device_id, identity);
        Err("policy_bound_progress_unsupported".into())
    }

    async fn record_outbound_scan(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
        scanned_through: i64,
    ) -> Result<OutboundProgress, String>;

    async fn record_outbound_scan_identity(
        &self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
        scanned_through: i64,
    ) -> Result<OutboundProgress, String> {
        let _ = (peer_device_id, identity, scanned_through);
        Err("policy_bound_progress_unsupported".into())
    }

    async fn acknowledge_outbound(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
        position: i64,
        ack: OutboundAck,
    ) -> Result<OutboundProgress, String>;

    async fn acknowledge_outbound_identity(
        &self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
        position: i64,
        ack: OutboundAck,
    ) -> Result<OutboundProgress, String> {
        let _ = (peer_device_id, identity, position, ack);
        Err("policy_bound_progress_unsupported".into())
    }
}
