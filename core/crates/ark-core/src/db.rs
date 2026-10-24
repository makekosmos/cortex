mod backend;
mod command;
mod committed_feed;
pub(crate) mod constants;

pub use crate::integration_replication::RefreshLeaseAcquireParams;
pub(crate) use ark_store::sync_kv::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY;
#[cfg(test)]
pub(crate) use ark_store::VERSION_VECTOR_KEY;
pub use backend::SqliteStorageBackend;
pub(crate) use command::{
    AcceptPeerSyncPolicyCommand, AcknowledgeTaskMergeBatchCommand, AppendLocalRegistryEventCommand,
    AppendTaskMergeOperationRequest, ApplyRegistryEventsCommand,
    ApplySignedIntegrationFrameCommand, ApplyTaskMergeBatchCommand, AuthenticatedCoreTaskWrite,
    AuthorizedTransportKeyQuery, CommandClass, CommandContext, CommittedFeedDeliveryPageQuery,
    CommittedFeedPageQuery, CommittedFeedProgress, CommittedFeedProgressCommand,
    CommittedFeedSnapshotQuery, CompactUsageSyncLogCommand, DbError, DbHandle, DbOwner,
    EnrollPairedDeviceCommand, GetSyncKeyValueQuery, IdentityErrorStyle, IntegrationAdminCommand,
    IntegrationAdminOutput, IntegrationReadQuery,
    LoadAcceptedPeerSyncPolicyQuery, LoadActiveLocalSyncProfileQuery, LoadDeviceRegistryQuery,
    LoadEntitiesQuery, LoadPolicyBoundEntitiesPageQuery, LoadRegistryEventsQuery,
    LoadSyncProfileForSyncQuery, LocalCoreTaskWrite, ObjectReadQuery,
    PolicyBoundFeedDeliveryPageQuery, PrepareTaskMergeBatchCommand, RegistryDeviceEnabledQuery,
    RegistryPeerAdmissionQuery, RemoteCoreTaskOutcome, RestoreSnapshotCommand,
    SaveLocalSyncProfileCommand, SystemReadQuery, TaskConflictSetQuery, TypeReadQuery,
    UsageCompleteThroughQuery, UsageReadQuery, ValidateOutboundSignedFrameQuery,
    VerifyRegistryHandshakeQuery,
};
pub use committed_feed::{InboxState, MAX_FEED_DELIVERY_BYTES};

mod integration_replication_admission;
#[cfg(test)]
mod integration_replication_apply;
mod integration_replication_credentials;
mod integration_replication_issuer_lookup;

pub use integration_replication_admission::integration_verification_status;
#[cfg(test)]
pub(in crate::db) use integration_replication_apply::apply_signed_integration_changes;
pub use integration_replication_credentials::{
    load_integration_credential_envelope, load_latest_integration_credential_envelope,
};
pub use integration_replication_issuer_lookup::{
    check_integration_credential_fence, load_issuer_encryption_key,
    load_issuer_encryption_key_for_publish, CredentialFenceExpectation,
};
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::hlc::HLC;
    use crate::integration_replication::*;
    use crate::types::*;
    use ark_application::integration_export::export_integration_replication;
    use ark_application::sync_storage::{SyncEntityMutationPort, SyncEntityReadPort};
    use serde_json::{json, Value};

    include!("db/test_fixtures.rs");
    include!("db/tests_sync.rs");
    include!("db/tests_integration_replication.rs");
    include!("db/tests_integration_replication_issuer_lookup.rs");
    include!("db/tests_integration_replication_issuer_publish_lookup.rs");
    include!("db/tests_integration_replication_export.rs");
    include!("db/tests_integration_replication_apply.rs");
    include!("db/tests_integration_replication_adapter.rs");
    include!("db/tests_integration_replication_security.rs");
}
