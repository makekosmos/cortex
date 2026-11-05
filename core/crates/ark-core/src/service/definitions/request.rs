use super::*;

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DataVolumeScope {
    pub(crate) type_id: String,
    pub(crate) type_version: String,
    pub(crate) fields: Vec<String>,
}

impl TryFrom<DataVolumeScope> for ark_application::data_volume::DataVolumeScope {
    type Error = String;

    fn try_from(scope: DataVolumeScope) -> Result<Self, Self::Error> {
        Self::new(scope.type_id, scope.type_version, scope.fields)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SyncPolicyRuleInput {
    pub(crate) resource_kind: String,
    pub(crate) resource_id: String,
    pub(crate) mode: ark_domain::selective_sync::SyncMode,
    #[serde(default = "default_sync_filter")]
    pub(crate) filter: Value,
}

fn default_sync_filter() -> Value {
    json!({})
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub(crate) enum Request {
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
    EstimateDataVolume {
        scopes: Vec<DataVolumeScope>,
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
    #[serde(rename = "task.merge.conflicts")]
    TaskMergeConflicts {
        #[serde(rename = "objectId")]
        object_id: String,
        unit: ark_domain::task_merge::TaskMergeUnit,
    },
    #[serde(rename = "task.merge.append")]
    TaskMergeAppend {
        #[serde(rename = "mergeOperation")]
        merge_operation: ark_domain::task_merge::TaskMergeOperation,
        #[serde(default, rename = "expectedConflictDigest")]
        expected_conflict_digest: Option<String>,
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
        #[serde(default, rename = "operationId")]
        operation_id: Option<String>,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteObject {
        id: String,
        #[serde(default, rename = "expectedSnapshot")]
        expected_snapshot: Option<ObjectWriteSnapshot>,
        #[serde(default, rename = "operationId")]
        operation_id: Option<String>,
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
    /// Register an application-owned type from a bare JSON schema. A new
    /// version must be an additive schema change.
    #[serde(rename = "types.register")]
    TypesRegister {
        #[serde(rename = "typeId")]
        type_id: String,
        version: String,
        #[serde(alias = "schemaJson", alias = "schema_json")]
        schema: Value,
        #[serde(default)]
        name: Option<String>,
        #[serde(
            default,
            rename = "appId",
            alias = "ownerId",
            alias = "app_id",
            alias = "owner_id"
        )]
        app_id: Option<String>,
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
    /// Move a link to a new target in one transaction: delete every other link
    /// with the same source and link_type, then upsert `object_link`.
    ReplaceObjectLink {
        object_link: ObjectLink,
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
    ClearAll,

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
    /// Компактор usage-журнала находится в
    /// `ark-store::usage_sync_log`). Engine maintenance loop вызывает его
    /// повторно: каждый вызов — короткая транзакция ≤ batch_limit строк,
    /// worker остаётся отзывчивым между батчами.
    CompactUsageSyncLog {
        #[serde(default)]
        older_than_days: Option<i64>,
        #[serde(default)]
        batch_limit: Option<i64>,
        /// The device id our own usage writes are stamped with — own refs
        /// are bounded by the journal head; anything else is bounded by our
        /// contiguous cursor (rule 3 in `ark-store::usage_sync_log`).
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
    #[serde(rename = "sync_policy.get")]
    SyncPolicyGet,
    #[serde(rename = "sync_policy.set")]
    SyncPolicySet {
        #[serde(rename = "profileId")]
        profile_id: String,
        name: String,
        active: bool,
        #[serde(default, rename = "expectedRevision")]
        expected_revision: Option<i64>,
        rules: Vec<SyncPolicyRuleInput>,
    },
    #[serde(rename = "device_registry.snapshot")]
    DeviceRegistrySnapshot,
    #[serde(rename = "device_registry.rename")]
    DeviceRegistryRename {
        target_device_id: ark_domain::device_registry::DeviceId,
        display_name: ark_domain::device_registry::DisplayName,
    },
    #[serde(rename = "device_registry.disable")]
    DeviceRegistryDisable {
        target_device_id: ark_domain::device_registry::DeviceId,
    },
    DisconnectPeer {
        device_id: String,
    },
    ConnectWithPairingCode {
        #[serde(alias = "code")]
        pairing_code: String,
        bind: SyncBind,
    },
    /// KOS-369 consent decisions on the receiving side. `accept_pairing`
    /// completes the handshake for a pending request; `decline_pairing`
    /// sends `pairing_rejected` and closes the connection.
    AcceptPairing {
        device_id: String,
    },
    DeclinePairing {
        device_id: String,
    },
    /// KOS-369 «Отмена» on the initiator (or dismissal of a terminal
    /// outcome): drops the outbound attempt and, while still pending, cuts
    /// the dialled connection.
    CancelPairing,
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
    ShowPairingCode {
        bind: SyncBind,
    },

    /// Test-only fault-injection op: the service worker panics on it so tests
    /// can verify that the service fails closed at the request boundary.
    #[cfg(test)]
    #[serde(rename = "test.panic")]
    TestPanic,
}
