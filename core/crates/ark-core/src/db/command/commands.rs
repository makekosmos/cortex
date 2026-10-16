mod core_task;
mod external_ref;
mod integration_admin;
mod integration_read;
mod integration_replication;
mod legacy_mutation;
mod object_mutation;
mod object_read;
mod sync_mutation;
mod sync_read;
mod system_mutation;
mod system_read;
mod type_mutation;
mod type_read;
mod usage;
mod usage_read;

pub(crate) use core_task::{AuthenticatedCoreTaskWrite, LocalCoreTaskWrite};
pub(crate) use external_ref::UpsertExternalRefCommand;
pub(crate) use integration_admin::{
    AcquireIntegrationRefreshLeaseCommand, PersistIntegrationGrantCommand,
    PersistNodeAuthorizationCommand, PublishIntegrationCredentialEnvelopeCommand,
};
pub(crate) use integration_read::IntegrationReadQuery;
pub(crate) use integration_replication::{
    ApplySignedIntegrationFrameCommand, IdentityErrorStyle, ValidateOutboundSignedFrameQuery,
};
pub(crate) use legacy_mutation::{
    LegacyDeleteCommand, LegacyGraphWriteCommand, TrackedAppDeleteCommand, TrackedAppUpsertCommand,
};
pub(crate) use object_mutation::{
    CanonicalGameUpsertCommand, LocalBookCoverCommand, LocalObjectDeleteCommand,
    LocalObjectUpsertCommand,
};
pub(crate) use object_read::ObjectReadQuery;
pub(crate) use sync_mutation::{
    ApplyAuthenticatedSyncEntityCommand, ApplySyncEntityCommand, CommittedFeedProgress,
    CommittedFeedProgressCommand, CommittedFeedProgressOperation,
    EnsureUsageSequenceMigratedCommand, ReceiveInboxReceiptCommand, SetLocalSyncKeyValueCommand,
    SetSyncKeyValueCommand,
};
pub(crate) use sync_read::{
    AuthorizedTransportKeyQuery, CommittedFeedDeliveryPageQuery, CommittedFeedPageQuery,
    CommittedFeedSnapshotQuery, GetSyncKeyValueQuery, LoadEntitiesQuery, UsageCompleteThroughQuery,
};
pub(crate) use system_mutation::{
    ClearAllCommand, CompactUsageSyncLogCommand, DeleteTrashedCommand,
    LoadOrGenerateIrohSecretKeyCommand, RestoreSnapshotCommand,
};
pub(crate) use system_read::SystemReadQuery;
pub(crate) use type_mutation::{
    DeleteObjectLinkCommand, DeleteObjectTypeCommand, RegisterPackageTypesCommand,
    ReplaceObjectLinkCommand, UpsertObjectLinkCommand, UpsertObjectTypeCommand,
};
pub(crate) use type_read::TypeReadQuery;
pub(crate) use usage::LocalUsageWriteCommand;
pub(crate) use usage_read::UsageReadQuery;

fn serialized_weight(value: &impl serde::Serialize) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len())
}
