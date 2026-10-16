use std::collections::HashMap;

use crate::canonical_types::pending::ReplayNotice;
use crate::types::{PeerRecord, SyncEntity, VersionVector};

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
    TooLarge {
        position: i64,
        entity_id: String,
        serialized_bytes: usize,
        max_bytes: usize,
    },
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

pub(crate) const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";
pub(crate) const KNOWN_PEERS_KEY: &str = "sync.peers";
pub(crate) const REMOVED_PEERS_KEY: &str = "sync.removed_peers";
pub(crate) const PEER_TRANSPORT_KEYS_KEY: &str = "sync.peer_transport_keys";

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
    async fn load_known_peers(&self) -> Result<Option<Vec<PeerRecord>>, String>;
    async fn store_known_peers(&self, peers: &[PeerRecord]) -> Result<(), String>;
    async fn load_removed_peer_ids(&self) -> Result<Vec<String>, String>;
    async fn store_removed_peer_ids(&self, peer_ids: &[String]) -> Result<(), String>;
}

/// Persists the receiver-owned policy a peer sent through an authenticated
/// sync session. The accepted copy is scoped to that peer and controls only
/// outbound selection.
#[async_trait::async_trait]
pub trait PeerSyncPolicyPort: Send + Sync {
    async fn accept_peer_sync_policy(
        &self,
        peer_device_id: &str,
        revision: i64,
        digest: &str,
        rules: &[crate::data_platform::SyncProfileRule],
    ) -> Result<crate::data_platform::AcceptedPeerSyncPolicy, String>;

    async fn load_accepted_peer_sync_policy(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<crate::data_platform::AcceptedPeerSyncPolicy>, String>;
}

/// Durable recovery reads. A caller pins one high watermark and pages by
/// committed position so concurrent writes cannot shift a page boundary.
#[async_trait::async_trait]
pub trait CommittedFeedPort: Send + Sync {
    fn selective_sync_policy_generation(&self) -> i64;

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

    async fn begin_outbound_policy(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
    ) -> Result<OutboundProgress, String>;

    async fn complete_outbound_snapshot(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
    ) -> Result<OutboundProgress, String>;

    async fn record_outbound_scan(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
        scanned_through: i64,
    ) -> Result<OutboundProgress, String>;

    async fn acknowledge_outbound(
        &self,
        peer_device_id: &str,
        policy_generation: i64,
        position: i64,
        ack: OutboundAck,
    ) -> Result<OutboundProgress, String>;
}
