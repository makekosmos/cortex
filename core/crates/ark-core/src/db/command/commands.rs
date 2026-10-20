mod device_registry;
mod device_registry_admission;
mod device_registry_command;
mod device_registry_handshake;
mod device_registry_sync;
mod integration_admin;
mod integration_read;
mod integration_replication;
mod legacy_mutation;
mod object_read;
mod sync_mutation;
mod sync_policy;
mod sync_read;
mod sync_read_policy;
mod system_mutation;
mod system_read;
mod task_merge;
mod type_read;
mod usage_read;

pub(crate) use ark_application::task_object_write::{
    AuthenticatedCoreTaskWrite, AuthenticatedCoreTaskWriteError, LocalCoreTaskWrite,
    RemoteCoreTaskOutcome, TaskWriteOperationResult,
};
#[cfg(test)]
pub(crate) use ark_store::writer::DeviceEnrollmentOutcome;
pub(crate) use device_registry::EnrollPairedDeviceCommand;
pub(crate) use device_registry_admission::{
    RegistryDeviceEnabledQuery, RegistryPeerAdmissionQuery,
};
pub(crate) use device_registry_command::AppendLocalRegistryEventCommand;
pub(crate) use device_registry_handshake::VerifyRegistryHandshakeQuery;
pub(crate) use device_registry_sync::{
    ApplyRegistryEventsCommand, LoadDeviceRegistryQuery, LoadRegistryEventsQuery,
};
pub(crate) use integration_admin::{IntegrationAdminCommand, IntegrationAdminOutput};
pub(crate) use integration_read::IntegrationReadQuery;
#[cfg(test)]
pub(crate) use integration_replication::map_signed_frame_operation_error;
pub(crate) use integration_replication::{
    ApplySignedIntegrationFrameCommand, IdentityErrorStyle, ValidateOutboundSignedFrameQuery,
};
pub(crate) use legacy_mutation::{LegacyDeleteCommand, LegacyGraphWriteCommand};
pub(crate) use object_read::ObjectReadQuery;
pub(crate) use sync_mutation::{CommittedFeedProgress, CommittedFeedProgressCommand};
pub(crate) use sync_policy::{
    AcceptPeerSyncPolicyCommand, LoadAcceptedPeerSyncPolicyQuery, LoadActiveLocalSyncProfileQuery,
    LoadSyncProfileForSyncQuery, SaveLocalSyncProfileCommand, SyncPolicyWrite,
    SyncPolicyWriteOutput,
};
pub(crate) use sync_read::{
    AuthorizedTransportKeyQuery, CommittedFeedDeliveryPageQuery, CommittedFeedPageQuery,
    CommittedFeedSnapshotQuery, GetSyncKeyValueQuery, LoadEntitiesQuery, UsageCompleteThroughQuery,
};
pub(crate) use sync_read_policy::{
    LoadPolicyBoundEntitiesPageQuery, PolicyBoundFeedDeliveryPageQuery,
};
pub(crate) use system_mutation::{CompactUsageSyncLogCommand, RestoreSnapshotCommand};
pub(crate) use system_read::SystemReadQuery;
pub(crate) use task_merge::{
    AcknowledgeTaskMergeBatchCommand, AppendTaskMergeOperationRequest, ApplyTaskMergeBatchCommand,
    PrepareTaskMergeBatchCommand, TaskConflictSetQuery,
};
pub(crate) use type_read::TypeReadQuery;
pub(crate) use usage_read::UsageReadQuery;

fn serialized_weight(value: &impl serde::Serialize) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len())
}
