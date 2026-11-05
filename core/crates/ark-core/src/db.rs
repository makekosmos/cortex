mod backend;
mod command;
mod committed_feed;
pub(crate) mod constants;

pub(crate) use ark_store::sync_kv::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY;
#[cfg(test)]
pub(crate) use ark_store::VERSION_VECTOR_KEY;
pub use backend::SqliteStorageBackend;
pub(crate) use command::{
    AcceptPeerSyncPolicyCommand, AcknowledgeTaskMergeBatchCommand, AppendLocalRegistryEventCommand,
    AppendTaskMergeOperationRequest, ApplyRegistryEventsCommand, ApplyTaskMergeBatchCommand,
    AuthenticatedCoreTaskWrite, AuthorizedTransportKeyQuery, CommandClass, CommandContext,
    CommittedFeedDeliveryPageQuery, CommittedFeedPageQuery, CommittedFeedProgress,
    CommittedFeedProgressCommand, CommittedFeedSnapshotQuery, CompactUsageSyncLogCommand, DbError,
    DbHandle, DbOwner, EnrollPairedDeviceCommand, GetSyncKeyValueQuery,
    LoadAcceptedPeerSyncPolicyQuery, LoadActiveLocalSyncProfileQuery, LoadDeviceRegistryQuery,
    LoadEntitiesQuery, LoadPolicyBoundEntitiesPageQuery, LoadRegistryEventsQuery,
    LoadSyncProfileForSyncQuery, LocalCoreTaskWrite, ObjectReadQuery,
    PolicyBoundFeedDeliveryPageQuery, PrepareTaskMergeBatchCommand, RegistryDeviceEnabledQuery,
    RegistryPeerAdmissionQuery, RemoteCoreTaskOutcome, RestoreSnapshotCommand,
    SaveLocalSyncProfileCommand, SystemReadQuery, TaskConflictSetQuery, TypeReadQuery,
    UsageCompleteThroughQuery, UsageReadQuery, VerifyRegistryHandshakeQuery,
};
pub use committed_feed::{InboxState, MAX_FEED_DELIVERY_BYTES};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::hlc::HLC;
    use crate::types::*;
    use ark_application::sync_storage::{SyncEntityMutationPort, SyncEntityReadPort};
    use serde_json::{json, Value};

    include!("db/test_fixtures.rs");
    include!("db/tests_sync.rs");
}
