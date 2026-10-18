mod backend;
mod command;
mod committed_feed;
pub(crate) mod constants;
mod legacy;
mod load;
mod objects;
mod open;
mod sync;
mod task_merge;
mod usage;

pub use ark_application::sync_storage::{
    CommittedFeedDeliveryPage, CommittedFeedEntry, FeedSnapshot, InitialSnapshot, OutboundAck,
    OutboundProgress,
};
pub(crate) use ark_store::snapshot::recover_interrupted_restore;
pub use ark_store::snapshot::restore_snapshot;
pub use ark_store::snapshot::{
    backups_dir, list_snapshots, schema_fingerprint, validate_snapshot, RestoreReport,
    SnapshotEntry, SnapshotValidation,
};
pub(crate) use ark_store::sync_kv::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY;
pub(crate) use backend::SqliteLegacyMutationSession;
pub(crate) use backend::SqliteObjectMutationSession;
pub(crate) use backend::SqliteObjectWriteSession;
pub(crate) use backend::SqliteSignedIntegrationFrameSession;
pub(crate) use backend::SqliteTypeMutationSession;
pub(crate) use backend::SqliteUsageWriteSession;
pub use backend::{delete_tag, SqliteStorageBackend};
pub use command::CommandSource;
pub(crate) use command::{
    AcceptPeerSyncPolicyCommand, AcknowledgeTaskMergeBatchCommand,
    AcquireIntegrationRefreshLeaseCommand, AppendLocalRegistryEventCommand,
    AppendTaskMergeOperationCommand, ApplyAuthenticatedSyncEntityCommand,
    ApplyRegistryEventsCommand, ApplySignedIntegrationFrameCommand, ApplySyncEntityCommand,
    ApplyTaskMergeBatchCommand, AuthenticatedCoreTaskWrite, AuthorizedTransportKeyQuery,
    CanonicalGameUpsertCommand, ClearAllCommand, CommandClass, CommandContext,
    CommittedFeedDeliveryPageQuery, CommittedFeedPageQuery, CommittedFeedProgress,
    CommittedFeedProgressCommand, CommittedFeedProgressOperation, CommittedFeedSnapshotQuery,
    CompactUsageSyncLogCommand, DbError, DbHandle, DbOwner, DeleteObjectLinkCommand,
    DeleteObjectTypeCommand, DeleteTrashedCommand, EnrollPairedDeviceCommand,
    EnsureUsageSequenceMigratedCommand, GetSyncKeyValueQuery, IdentityErrorStyle,
    IntegrationReadQuery, LegacyDeleteCommand, LegacyGraphWriteCommand,
    LoadAcceptedPeerSyncPolicyQuery, LoadActiveLocalSyncProfileQuery, LoadDeviceRegistryQuery,
    LoadEntitiesQuery, LoadOrGenerateIrohSecretKeyCommand, LoadPolicyBoundEntitiesPageQuery,
    LoadRegistryEventsQuery, LoadSyncProfileForSyncQuery, LocalBookCoverCommand,
    LocalCoreTaskWrite, LocalObjectDeleteCommand, LocalObjectUpsertCommand, LocalUsageWriteCommand,
    ObjectReadQuery, PersistIntegrationGrantCommand, PersistNodeAuthorizationCommand,
    PolicyBoundFeedDeliveryPageQuery, PrepareTaskMergeBatchCommand,
    PublishIntegrationCredentialEnvelopeCommand, ReceiveInboxReceiptCommand,
    RegisterPackageTypesCommand, RegistryDeviceEnabledQuery, RegistryPeerAdmissionQuery,
    RemoteCoreTaskOutcome, ReplaceObjectLinkCommand, RestoreSnapshotCommand,
    SaveLocalSyncProfileCommand, SetLocalSyncKeyValueCommand, SetSyncKeyValueCommand,
    SignRegistryHandshakeCommand, SystemReadQuery, TaskConflictSetQuery, TrackedAppDeleteCommand,
    TrackedAppUpsertCommand, TypeReadQuery, UpsertExternalRefCommand, UpsertObjectLinkCommand,
    UpsertObjectTypeCommand, UsageCompleteThroughQuery, UsageReadQuery,
    ValidateOutboundSignedFrameQuery, VerifyRegistryHandshakeQuery,
};
pub use committed_feed::{
    acknowledge_outbound, acknowledge_outbound_identity, append_committed_change,
    begin_outbound_policy, begin_outbound_policy_identity, committed_feed_snapshot,
    complete_outbound_snapshot, complete_outbound_snapshot_identity, entity_inbox_message_id,
    load_outbound_progress, read_committed_feed_delivery_page,
    read_committed_feed_delivery_page_for_peer, read_committed_feed_page, receive_inbox_payload,
    record_outbound_scan, record_outbound_scan_identity, set_inbox_state, InboxState,
    MAX_FEED_PAGE_SIZE, MAX_INBOX_PAYLOAD_BYTES,
};
pub(in crate::db) use integration_replication_publication::{
    publish_integration_credential_envelope_in_transaction,
    try_acquire_integration_refresh_lease_in_transaction,
};
pub use legacy::{
    batch_upsert_todos, delete_project, delete_todo, upsert_project, upsert_tag, upsert_todo,
};
pub use load::{clear_all, delete_trashed, load_all};
pub(in crate::db) use load::{
    load_all_projects, load_all_tags, load_all_todos, load_all_tracked_apps, load_usage_event,
    load_usage_session,
};
pub(in crate::db) use objects::list_other_object_link_ids;
pub use objects::{
    delete_object, delete_object_link, delete_object_type, get_object, get_object_type,
    get_objects_by_ids, is_object_definition_known, is_object_type_known, list_object_links,
    list_object_summaries, list_object_summaries_by_type, list_object_types, list_objects,
    list_objects_by_type, list_running_time_entries, search_objects, upsert_object,
    upsert_object_link, upsert_object_type, SearchResult,
};
#[cfg(test)]
pub(in crate::db) use open::USAGE_SEQUENCE_MIGRATION_KEY;
pub(crate) use open::VERSION_VECTOR_KEY;
pub use open::{
    backup_to_file, backup_to_file_chunked, init_schema, init_schema_prerequisites_for_phase3,
    is_sequenced_usage_entity,
};
pub use open::{count_pending_for_type, insert_pending_object};
pub(crate) use sync::upsert_sync_tombstone;
pub use sync::{
    bump_sync_version_vector, delete_sync_tombstone, ensure_usage_sequence_migrated,
    get_object_revision, record_remote_sync_revision, record_sync_tombstone,
    record_sync_tombstone_with_type,
};
pub(in crate::db) use sync::{
    finish_usage_entity_sync, load_sync_tombstones, record_usage_sequence, usage_entity_is_newer,
    usage_origin_for_entity,
};
pub(in crate::db) use usage::load_usage_day;
pub use usage::{
    canonical_app_key, delete_tracked_app, delete_usage_day, delete_usage_event,
    delete_usage_session, get_usage_title_total, is_system_path, list_recent_usage_processes,
    load_usage_analytics, load_usage_game_playtime_summary, search_usage_processes,
    upsert_tracked_app, upsert_usage_day, upsert_usage_event, upsert_usage_session,
    upsert_usage_span,
};
mod integration_replication_admission;
mod integration_replication_apply;
mod integration_replication_common;
mod integration_replication_credentials;
mod integration_replication_export;
mod integration_replication_grants;
mod integration_replication_issuer_lookup;
mod integration_replication_lease;
mod integration_replication_publication;
mod integration_replication_records;

pub use ark_store::integration_replication::load_integration_node_grant;
pub(in crate::db) use integration_credentials::upsert_integration_credential_envelope_record;
pub use integration_replication_admission::integration_verification_status;
#[cfg(test)]
pub(in crate::db) use integration_replication_apply::{
    apply_signed_integration_changes, load_version_vector,
};
use integration_replication_credentials as integration_credentials;
pub use integration_replication_credentials::{
    load_integration_credential_envelope, load_latest_integration_credential_envelope,
};
pub use integration_replication_export::export_integration_replication;
pub use integration_replication_grants::upsert_integration_node_grant;
pub use integration_replication_issuer_lookup::{
    check_integration_credential_fence, load_issuer_encryption_key,
    load_issuer_encryption_key_for_publish, CredentialFenceExpectation,
};
pub(in crate::db) use integration_replication_lease::upsert_integration_refresh_lease_record;
pub use integration_replication_publication::{
    publish_integration_credential_envelope, try_acquire_integration_refresh_lease,
    RefreshLeaseAcquireParams,
};
pub use integration_replication_records::{
    upsert_authorized_node, upsert_integration_configuration,
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

    use super::objects::object_search_fts_exists;
    use super::open::replay_pending_for_type;
    use super::usage::merge_top_apps;
    use super::*;
    use crate::hlc::HLC;
    use crate::integration_replication::*;
    use crate::type_registry;
    use crate::types::*;
    use ark_application::sync_storage::{SyncEntityMutationPort, SyncEntityReadPort};
    use ark_store::open::open_db;
    use ark_store::schema::*;
    use ark_store::snapshot::{
        default_rollback_restore_for_test, restore_snapshot_with_hooks,
        validate_snapshot_id_for_test,
    };
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
