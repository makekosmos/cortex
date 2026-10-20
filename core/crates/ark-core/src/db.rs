mod backend;
mod command;
mod committed_feed;
pub(crate) mod constants;

pub use ark_application::sync_storage::{
    CommittedFeedDeliveryPage, CommittedFeedEntry, FeedSnapshot, InitialSnapshot, OutboundAck,
    OutboundProgress,
};
pub use ark_store::committed_feed::MAX_FEED_PAGE_SIZE;
#[cfg(test)]
pub use ark_store::snapshot::backup_to_file_chunked_verified as backup_to_file_chunked;
pub use ark_store::snapshot::restore_snapshot;
pub use ark_store::snapshot::{
    backups_dir, list_snapshots, schema_fingerprint, validate_snapshot, RestoreReport,
    SnapshotEntry, SnapshotValidation,
};
pub(crate) use ark_store::sync_kv::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY;
#[cfg(test)]
pub(crate) use ark_store::VERSION_VECTOR_KEY;
#[cfg(test)]
pub(crate) use backend::SqliteSignedIntegrationFrameSession;
pub use backend::SqliteStorageBackend;
pub use command::CommandSource;
pub(crate) use command::{
    AcceptPeerSyncPolicyCommand, AcknowledgeTaskMergeBatchCommand, AppendLocalRegistryEventCommand,
    AppendTaskMergeOperationRequest, ApplyRegistryEventsCommand,
    ApplySignedIntegrationFrameCommand, ApplyTaskMergeBatchCommand, AuthenticatedCoreTaskWrite,
    AuthorizedTransportKeyQuery, CommandClass, CommandContext, CommittedFeedDeliveryPageQuery,
    CommittedFeedPageQuery, CommittedFeedProgress, CommittedFeedProgressCommand,
    CommittedFeedSnapshotQuery, CompactUsageSyncLogCommand, DbError, DbHandle, DbOwner,
    EnrollPairedDeviceCommand, GetSyncKeyValueQuery, IdentityErrorStyle, IntegrationAdminCommand,
    IntegrationAdminOutput, IntegrationReadQuery, LegacyDeleteCommand, LegacyGraphWriteCommand,
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
mod integration_replication_apply;
#[cfg(test)]
mod integration_replication_common;
mod integration_replication_credentials;
mod integration_replication_export;
mod integration_replication_issuer_lookup;
#[cfg(test)]
mod integration_replication_lease;
mod integration_replication_publication;

pub use ark_store::integration_replication::load_integration_node_grant;
pub use ark_store::integration_replication::upsert_integration_node_grant;
pub use ark_store::integration_replication::{
    upsert_authorized_node, upsert_integration_configuration,
};
#[cfg(test)]
pub(in crate::db) use integration_credentials::upsert_integration_credential_envelope_record;
pub use integration_replication_admission::integration_verification_status;
#[cfg(test)]
pub(in crate::db) use integration_replication_apply::{
    apply_signed_integration_changes, load_version_vector,
};
#[cfg(test)]
use integration_replication_credentials as integration_credentials;
pub use integration_replication_credentials::{
    load_integration_credential_envelope, load_latest_integration_credential_envelope,
};
pub use integration_replication_export::export_integration_replication;
pub use integration_replication_issuer_lookup::{
    check_integration_credential_fence, load_issuer_encryption_key,
    load_issuer_encryption_key_for_publish, CredentialFenceExpectation,
};
#[cfg(test)]
pub(in crate::db) use integration_replication_lease::upsert_integration_refresh_lease_record;
pub use integration_replication_publication::RefreshLeaseAcquireParams;
#[cfg(test)]
pub use integration_replication_publication::{
    publish_integration_credential_envelope, try_acquire_integration_refresh_lease,
};
// Preserve the public database query surface for downstream Rust consumers;
// Core production paths use the Store adapters directly.
pub use ark_store::integration_replication::{
    load_authorized_node, load_integration_configuration, load_integration_refresh_lease,
};

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::hlc::HLC;
    use crate::integration_replication::*;
    use crate::type_registry;
    use crate::types::*;
    use ark_application::sync_storage::{SyncEntityMutationPort, SyncEntityReadPort};
    use ark_store::legacy_entities::{
        batch_upsert_todos, delete_project, delete_todo, upsert_project, upsert_tag, upsert_todo,
    };
    use ark_store::objects::{
        delete_object_link, get_object, get_objects_by_ids, list_object_links,
        list_object_summaries, list_object_summaries_by_type, list_objects, list_objects_by_type,
        list_running_time_entries, search_objects, upsert_object_link,
    };
    use ark_store::open::open_db;
    use ark_store::pending_objects::{
        count_for_type as count_pending_for_type, insert_pending_object, replay_pending_for_type,
    };
    use ark_store::schema::*;
    use ark_store::snapshot::{
        default_rollback_restore_for_test, restore_snapshot_with_hooks,
        validate_snapshot_id_for_test,
    };
    use ark_store::usage_queries::merge_top_apps;
    use ark_store::usage_sync_log::{compact_usage_sync_log, usage_log_complete_through};
    use rusqlite::{params, Connection, OptionalExtension};
    use serde_json::{json, Value};

    include!("db/tests_core.rs");
    include!("db/tests_snapshot.rs");
    include!("db/tests_sync.rs");
    include!("db/tests_integration_replication.rs");
    include!("db/tests_integration_replication_issuer_lookup.rs");
    include!("db/tests_integration_replication_issuer_publish_lookup.rs");
    include!("db/tests_integration_replication_export.rs");
    include!("db/tests_integration_replication_apply.rs");
    include!("db/tests_integration_replication_adapter.rs");
    include!("db/tests_integration_replication_security.rs");
    include!("db/tests_integration_replication_publication.rs");
}
