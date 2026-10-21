use std::collections::HashMap;

use ark_domain::types::{PeerRecord, SyncEntity, VersionVector};
use sha2::{Digest, Sha256};

#[path = "sync_storage_progress.rs"]
mod sync_progress_write;
pub use sync_progress_write::{
    apply_sync_progress_write, merge_version_vector_entries, SyncProgressWrite,
    SyncProgressWriteError, SyncProgressWriteSession,
};

pub use crate::sync_policy::PolicyIdentity;
pub use crate::task_merge::{
    TaskMergeBatch, TaskMergeBatchAck, TaskMergeDeliveryPort, TaskMergeJournalEntry,
    TaskMergeJournalPage, TaskMergeJournalRecord, TaskMergePrepareError,
};

pub const MAX_SYNC_BATCH_BYTES: usize = 1_048_576;

#[derive(Clone, Debug)]
pub struct InboxReceiptPolicy {
    pub local_device_id: String,
    pub expected_identity: PolicyIdentity,
}

#[derive(Clone, Debug)]
pub struct InboxReceiptWrite {
    pub peer_device_id: String,
    pub message_id: String,
    pub payload_json: String,
    pub policy: Option<InboxReceiptPolicy>,
}

impl InboxReceiptWrite {
    pub fn new(peer_device_id: String, message_id: String, payload_json: String) -> Self {
        Self {
            peer_device_id,
            message_id,
            payload_json,
            policy: None,
        }
    }

    pub fn with_policy(
        peer_device_id: String,
        message_id: String,
        payload_json: String,
        local_device_id: String,
        expected_identity: PolicyIdentity,
    ) -> Self {
        Self {
            peer_device_id,
            message_id,
            payload_json,
            policy: Some(InboxReceiptPolicy {
                local_device_id,
                expected_identity,
            }),
        }
    }

    pub fn weight(&self) -> usize {
        self.peer_device_id.len() + self.message_id.len() + self.payload_json.len() + 128
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InboxReceiptWriteError {
    PolicyStorage(String),
    PolicyChanged,
    RegistryStorage(String),
    PeerNotAdmitted,
    RemovedPeersStorage(String),
    PeerRemoved,
    Inbox(String),
}

/// Stable identity for the durable receipt of one authenticated entity revision.
/// Length-prefixing keeps component boundaries unambiguous without changing
/// the persisted inbox key format.
pub fn entity_inbox_message_id(
    peer_device_id: &str,
    entity_type: &str,
    entity_id: &str,
    revision: &str,
) -> String {
    let mut hasher = Sha256::new();
    for component in [peer_device_id, entity_type, entity_id, revision] {
        hasher.update((component.len() as u64).to_be_bytes());
        hasher.update(component.as_bytes());
    }
    format!("entity:{:x}", hasher.finalize())
}

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
    use super::{entity_inbox_message_id, ReplayNotice};

    #[test]
    fn entity_inbox_message_id_preserves_stable_receipt_key() {
        assert_eq!(
            entity_inbox_message_id("peer", "object", "id-1", "hlc-1"),
            "entity:39e1e96a889887856dd2ac05c363cffc1ba5ce7af69dec35ea188b685313d914"
        );
        assert_ne!(
            entity_inbox_message_id("peer", "object", "id-1", "hlc-1"),
            entity_inbox_message_id("peer", "obj", "ectid-1", "hlc-1")
        );
    }

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyBoundReadError {
    StalePolicy,
    Storage(String),
}

impl std::fmt::Display for PolicyBoundReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StalePolicy => formatter.write_str("stale_or_unnegotiated_sync_policy"),
            Self::Storage(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for PolicyBoundReadError {}

impl From<String> for PolicyBoundReadError {
    fn from(error: String) -> Self {
        Self::Storage(error)
    }
}

impl From<&str> for PolicyBoundReadError {
    fn from(error: &str) -> Self {
        Self::Storage(error.to_owned())
    }
}

pub type PolicyBoundSnapshotReadError = PolicyBoundReadError;
pub type PolicyBoundFeedReadError = PolicyBoundReadError;

mod outbound_progress;
pub use outbound_progress::{
    InitialSnapshot, OutboundAck, OutboundProgress, OutboundProgressMutation,
    OutboundProgressPolicy, OutboundProgressWrite, OutboundProgressWriteError,
};

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
    ) -> Result<PolicyBoundSnapshotPage, PolicyBoundSnapshotReadError>;
    async fn usage_complete_through(&self) -> Result<HashMap<String, u64>, String>;
    async fn usage_complete_through_for_peer(
        &self,
        peer_device_id: &str,
        class: SyncReadClass,
    ) -> Result<HashMap<String, u64>, String>;
    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity>;

    async fn filter_outgoing_entity_for_peer(
        &self,
        peer_device_id: &str,
        entity: &SyncEntity,
    ) -> Result<Option<SyncEntity>, String>;
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
    async fn replace_version_vector(&self, vector: &VersionVector) -> Result<(), String>;
    async fn replace_version_vector_with_policy(
        &self,
        local_device_id: &str,
        policy_identity: &PolicyIdentity,
        vector: &VersionVector,
    ) -> Result<(), String>;
    async fn merge_version_vector(&self, vector: &VersionVector) -> Result<(), String>;
    async fn merge_version_vector_with_policy(
        &self,
        local_device_id: &str,
        policy_identity: &PolicyIdentity,
        vector: &VersionVector,
    ) -> Result<(), String>;
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
        peer_device_id: &str,
    ) -> Result<i64, String>;

    async fn committed_feed_snapshot(&self) -> Result<FeedSnapshot, String>;

    async fn committed_feed_page(
        &self,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
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
    ) -> Result<PolicyBoundFeedPage, PolicyBoundFeedReadError>;

    async fn begin_outbound_policy(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
    ) -> Result<OutboundProgress, String>;

    async fn begin_outbound_policy_identity(
        &self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
    ) -> Result<OutboundProgress, String>;

    async fn complete_outbound_snapshot(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
    ) -> Result<OutboundProgress, String>;

    async fn complete_outbound_snapshot_identity(
        &self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
    ) -> Result<OutboundProgress, String>;

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
    ) -> Result<OutboundProgress, String>;

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
    ) -> Result<OutboundProgress, String>;
}
