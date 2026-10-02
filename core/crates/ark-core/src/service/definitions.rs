// ---------------------------------------------------------------------------
// Request envelope + sync-runtime state types. The former process-wide
// globals (DB / DB_PATH / SYNC / BACKUP_GATE) now live on `ServiceState`
// (see `service.rs`) so one process can host several services.
// ---------------------------------------------------------------------------

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct SyncStartParams {
    pub(crate) space_id: String,
    pub(crate) device_id: String,
    pub(crate) device_name: String,
    pub(crate) port: Option<u16>,
    pub(crate) seed_addresses: Option<Vec<String>>,
    pub(crate) relay_url: Option<String>,
    pub(crate) relay_api_key: Option<String>,
    pub(crate) auth_secret: Option<String>,
    pub(crate) use_iroh: bool,
    pub(crate) iroh_peer_ticket: Option<String>,
    pub(crate) discovery_enabled: bool,
    pub(crate) bind: SyncBind,
}

fn default_discovery_enabled() -> bool {
    true
}

pub(crate) struct SyncRuntime {
    pub(crate) server: Arc<SyncServer>,
    pub(crate) storage: Arc<SqliteStorageBackend>,
    pub(crate) clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    pub(crate) relay: Option<Arc<RelaySync>>,
    pub(crate) transport_choice: Option<TransportChoice>,
    pub(crate) start_params: SyncStartParams,
    /// Step 4a: our iroh pairing ticket, captured at construction time when
    /// the iroh transport was selected (`our_ticket()` needs the transport
    /// object directly — `RelaySync` only exposes `Arc<dyn SyncTransport>`,
    /// which is not downcastable — so we snapshot the ticket string instead
    /// of threading a concrete `IrohTransport` handle through `SyncRuntime`).
    /// `None` when iroh wasn't selected.
    pub(crate) iroh_our_ticket: Option<String>,
    pub(crate) beacon: Arc<BroadcastDiscovery>,
    pub(crate) space_id: String,
    pub(crate) device_id: String,
    pub(crate) device_name: String,
    pub(crate) auth_secret: Option<String>,
    pub(crate) own_addresses: Arc<TokioMutex<Vec<String>>>,
}

// The `SyncRuntime` slot lives on `ServiceState::sync` (`service.rs`).
// Events go out through `crate::events` — a process-wide broadcast bus also
// used by lib modules (notably `db::apply_entity_blocking` for schema-drift
// sync_error/sync_replay events).

// ---------------------------------------------------------------------------
// Request enum
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObjectWriteSnapshot {
    pub(crate) exists: bool,
    pub(crate) type_id: Option<String>,
    pub(crate) type_version: Option<String>,
    pub(crate) revision: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub(crate) enum Request {
    // --- DB ops (unchanged wire format) ---
    Init {
        #[serde(rename = "dbPath")]
        db_path: String,
    },
    LoadAll,
    UpsertTodo {
        todo: TodoItem,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteTodo {
        id: String,
        #[serde(default)]
        device_id: Option<String>,
    },
    BatchUpsertTodos {
        todos: Vec<TodoItem>,
        #[serde(default)]
        device_id: Option<String>,
    },
    UpsertProject {
        project: Project,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteProject {
        id: String,
        #[serde(default)]
        device_id: Option<String>,
    },
    UpsertTag {
        tag: Tag,
        #[serde(default)]
        device_id: Option<String>,
    },
    UpsertTrackedApp {
        tracked_app: TrackedApp,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteTrackedApp {
        id: String,
        #[serde(default)]
        device_id: Option<String>,
    },
    UpsertUsageSession {
        usage_session: UsageSession,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteUsageSession {
        id: String,
        #[serde(default)]
        device_id: Option<String>,
    },
    UpsertUsageEvent {
        usage_event: UsageEvent,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteUsageEvent {
        id: String,
        #[serde(default)]
        device_id: Option<String>,
    },
    UpsertUsageSpan {
        usage_span: UsageSpanWrite,
    },
    GetUsageTitleTotal {
        query: String,
    },
    GetUsageAnalytics {
        #[serde(default)]
        range_days: Option<i64>,
        #[serde(default)]
        top_apps_limit: Option<i64>,
        #[serde(default)]
        recent_sessions_limit: Option<i64>,
    },
    ListRecentUsageProcesses {
        #[serde(default)]
        limit: Option<i64>,
    },
    SearchUsageProcesses {
        query: String,
        #[serde(default)]
        limit: Option<i64>,
    },
    GetUsageGamePlaytimeSummary {
        bindings: Vec<UsageGamePlaytimeBinding>,
        #[serde(default)]
        range_start: Option<String>,
        #[serde(default)]
        range_end: Option<String>,
    },
    ListObjects,
    ListObjectSummaries,
    ListObjectsByType {
        type_id: String,
    },
    ListObjectSummariesByType {
        type_id: String,
    },
    ListRunningTimeEntries {
        #[serde(default)]
        source: Option<String>,
    },
    GetObjectsByIds {
        ids: Vec<String>,
    },
    SearchObjects {
        query: String,
    },
    GetObject {
        id: String,
    },
    GetObjectWriteSnapshot {
        id: String,
    },
    #[serde(rename = "canonical.game.list")]
    CanonicalGameList {
        #[serde(default)]
        device_id: Option<String>,
    },
    #[serde(rename = "canonical.game.get")]
    CanonicalGameGet {
        id: String,
        #[serde(default)]
        device_id: Option<String>,
    },
    #[serde(rename = "canonical.game.upsert")]
    CanonicalGameUpsert {
        game: crate::canonical_types::game::GameUpsertCommand,
        #[serde(default)]
        device_id: Option<String>,
    },
    #[serde(rename = "canonical.asset_sources")]
    CanonicalAssetSources {
        #[serde(rename = "objectIds")]
        object_ids: Vec<String>,
    },
    #[serde(rename = "canonical.set_book_cover")]
    CanonicalSetBookCover {
        #[serde(rename = "bookId")]
        book_id: String,
        #[serde(default, rename = "sourceRef")]
        source_ref: Option<String>,
        #[serde(default, rename = "existingImageId")]
        existing_image_id: Option<String>,
        #[serde(default)]
        alt_text: String,
        #[serde(default)]
        device_id: Option<String>,
    },
    UpsertObject {
        object: ArkObjectWrite,
        #[serde(default, rename = "expectedSnapshot")]
        expected_snapshot: Option<ObjectWriteSnapshot>,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteObject {
        id: String,
        #[serde(default, rename = "expectedSnapshot")]
        expected_snapshot: Option<ObjectWriteSnapshot>,
        #[serde(default)]
        device_id: Option<String>,
    },
    #[serde(rename = "types.list")]
    TypesList,
    #[serde(rename = "types.get")]
    TypesGet {
        #[serde(rename = "typeId")]
        type_id: String,
        #[serde(default)]
        version: Option<String>,
    },
    #[serde(rename = "types.listVersions")]
    TypesListVersions {
        #[serde(rename = "typeId")]
        type_id: String,
    },
    #[serde(rename = "types.resolveAlias")]
    TypesResolveAlias {
        alias: String,
    },
    #[serde(rename = "types.registerPackageDefinitions")]
    TypesRegisterPackageDefinitions {
        registrations: Vec<crate::type_registry::TypeRegistration>,
    },
    ListObjectTypes,
    GetObjectType {
        id: String,
    },
    UpsertObjectType {
        object_type: ObjectType,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteObjectType {
        id: String,
        #[serde(default)]
        device_id: Option<String>,
    },
    ListObjectLinks,
    UpsertObjectLink {
        object_link: ObjectLink,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteObjectLink {
        id: String,
        #[serde(default)]
        device_id: Option<String>,
    },
    GetSyncKv {
        key: String,
    },
    SetSyncKv {
        key: String,
        value: String,
    },
    #[serde(rename = "external_refs.upsert")]
    ExternalRefsUpsert(crate::data_platform::ExternalRefUpsert),
    ClearAll,
    DeleteTrashed,

    /// SQLite Online Backup в указанный path. Source DB остаётся live —
    /// concurrent readers/writer safe. Используется db_backup scheduler
    /// в mundus-engine (раз в 24h). См. hardening proof loop 2026-05-18.
    DbBackup {
        dest_path: String,
    },

    /// KOS-51: privileged snapshot ops над Core-owned `<db_dir>/backups/`.
    /// `backup_id` — только basename файла из этой директории (см.
    /// `db::validate_snapshot_id`), никаких произвольных путей.
    /// Restore применяется через SQLite Online Backup API в live conn под
    /// `BACKUP_GATE` + глобальным DB mutex — нет момента с отсутствующей
    /// primary DB. См. `.agent/tasks/2026-09-15-ark-snapshot-restore-rpc/`.
    DbBackupList,
    DbBackupValidate {
        backup_id: String,
    },
    DbBackupRestore {
        backup_id: String,
    },

    /// KOS-302: один батч компакции `usage_sync_log` (правило — в
    /// `db::compact_usage_sync_log`). Engine maintenance loop вызывает его
    /// повторно: каждый вызов — короткая транзакция ≤ batch_limit строк,
    /// worker остаётся отзывчивым между батчами.
    CompactUsageSyncLog {
        #[serde(default)]
        older_than_days: Option<i64>,
        #[serde(default)]
        batch_limit: Option<i64>,
        /// The device id our own usage writes are stamped with — own refs
        /// are bounded by the journal head; anything else is bounded by our
        /// contiguous cursor (rule 3 in `db::compact_usage_sync_log`).
        #[serde(default)]
        device_id: Option<String>,
    },

    // --- Sync ops (new) ---
    StartSync(StartSyncParams),
    StopSync,
    BroadcastChange {
        entity: SyncEntity,
    },
    GetConnectedPeers,
    GetSyncSnapshot,
    DisconnectPeer {
        device_id: String,
    },
    ConnectWithPairingCode {
        #[serde(alias = "code")]
        pairing_code: String,
    },
    LeaveSpace,
    AddSeedPeer {
        addresses: Vec<String>,
    },
    GetOwnAddresses {
        #[serde(default)]
        port: Option<u16>,
    },
    GetHostDeviceName,
    /// Step 4a: fetch our iroh pairing ticket, if the running sync runtime
    /// selected the iroh transport. `null`/error otherwise (e.g. relay
    /// selected or sync not running).
    GetOwnIrohTicket,
    /// KOS-269: explicit "show my pairing code" — escalates a loopback-bound
    /// runtime to `all_interfaces` (and the iroh transport) so the returned
    /// ticket is LAN-reachable, then returns it. This is the user action
    /// that owns the firewall prompt; `get_own_iroh_ticket` stays a passive
    /// read and never changes the bind.
    ShowPairingCode,

    #[serde(rename = "integration.persist_node_authorization")]
    IntegrationPersistNodeAuthorization {
        authorization_operation: String,
        node: AuthorizedNode,
        #[serde(default)]
        grant: Option<IntegrationNodeGrant>,
        device_id: String,
    },
    #[serde(rename = "integration.persist_integration_grant")]
    IntegrationPersistIntegrationGrant {
        grant: IntegrationNodeGrant,
        device_id: String,
    },
    #[serde(rename = "integration.prepare_signed_sync")]
    IntegrationPrepareSignedSync {
        space_id: String,
        origin_node_id: String,
        integration_id: String,
        recipient_node_id: String,
        message_id: String,
    },
    #[serde(rename = "integration.validate_outbound_signed_sync")]
    IntegrationValidateOutboundSignedSync {
        space_id: String,
        origin_node_id: String,
        frame: SignedSyncEnvelope,
    },
    #[serde(rename = "integration.send_signed_sync")]
    IntegrationSendSignedSync {
        frame: SignedSyncEnvelope,
    },
    #[serde(rename = "integration.acquire_refresh_lease")]
    IntegrationAcquireRefreshLease(crate::db::RefreshLeaseAcquireParams),
    #[serde(rename = "integration.publish_credential_envelope")]
    IntegrationPublishCredentialEnvelope {
        envelope: IntegrationCredentialEnvelope,
        device_id: String,
        now_ms: u64,
    },
    #[serde(rename = "integration.load_latest_credential_envelope")]
    IntegrationLoadLatestCredentialEnvelope {
        integration_id: String,
        recipient_node_id: String,
    },
    #[serde(rename = "integration.lookup_issuer_encryption_key")]
    IntegrationLookupIssuerEncryptionKey {
        space_id: String,
        integration_id: String,
        recipient_node_id: String,
        issuer_node_id: String,
        credential_generation: u64,
        expected_issuer_key_id: String,
    },
    /// Storage-time fence check for a received credential envelope: the
    /// issuer binding is revalidated and the fence the envelope was minted
    /// under must still be the current refresh lease (see
    /// `db::check_integration_credential_fence`).
    #[serde(rename = "integration.check_credential_fence")]
    IntegrationCheckCredentialFence {
        space_id: String,
        integration_id: String,
        recipient_node_id: String,
        issuer_node_id: String,
        credential_generation: u64,
        refresh_fencing_token: u64,
        expected_issuer_key_id: String,
    },
    #[serde(rename = "integration.lookup_issuer_encryption_key_for_publish")]
    IntegrationLookupIssuerEncryptionKeyForPublish {
        space_id: String,
        integration_id: String,
        recipient_node_id: String,
        issuer_node_id: String,
        expected_issuer_key_id: String,
    },
    #[serde(rename = "integration.verification_status")]
    IntegrationVerificationStatus {
        integration_id: String,
        local_node_id: String,
        now_ms: u64,
    },

    /// Test-only fault-injection op: the service worker panics on it so tests
    /// can verify catch_unwind + reopen at the service boundary.
    #[cfg(test)]
    #[serde(rename = "test.panic")]
    TestPanic,
}

/// Params of `start_sync`. The variant is a newtype over this struct so the
/// handler receives the request params object directly.
#[derive(Debug, Deserialize)]
pub(crate) struct StartSyncParams {
    pub(crate) space_id: String,
    pub(crate) device_id: String,
    #[serde(default)]
    pub(crate) device_name: Option<String>,
    #[serde(default)]
    pub(crate) port: Option<u16>,
    #[serde(default)]
    pub(crate) seed_addresses: Option<Vec<String>>,
    /// Optional relay server WebSocket URL (e.g. "wss://relay.example.com").
    #[serde(default)]
    pub(crate) relay_url: Option<String>,
    /// API key for the relay server.
    #[serde(default)]
    pub(crate) relay_api_key: Option<String>,
    /// Optional shared secret for LAN/P2P hello HMAC authentication.
    #[serde(default)]
    pub(crate) auth_secret: Option<String>,
    /// Step 4a: select the iroh p2p transport instead of relay — see
    /// `select_transport`.
    #[serde(default)]
    pub(crate) use_iroh: bool,
    /// Pairing ticket string for the iroh peer (see
    /// `iroh_transport::IrohTransport::our_ticket`/`from_ticket`).
    #[serde(default)]
    pub(crate) iroh_peer_ticket: Option<String>,
    /// Whether to start LAN beacon discovery. Defaults to true for compatibility.
    #[serde(default = "default_discovery_enabled")]
    pub(crate) discovery_enabled: bool,
    /// Where the sync stack binds its listeners. `loopback` binds every
    /// socket to 127.0.0.1 / ::1 (tests, single-machine pairing);
    /// `all_interfaces` (default) keeps LAN behaviour.
    #[serde(default)]
    pub(crate) bind: SyncBind,
}
