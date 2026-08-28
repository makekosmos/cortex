// ---------------------------------------------------------------------------
// Global state — one SQLite connection plus (optionally) one sync runtime.
// ---------------------------------------------------------------------------

static DB: StdMutex<Option<Arc<StdMutex<rusqlite::Connection>>>> = StdMutex::new(None);

// Путь к ARK DB (из Init). Нужен чтобы db_backup открывал ОТДЕЛЬНЫЙ read-коннекшн
// и не держал глобальный DB mutex на всё копирование.
static DB_PATH: StdMutex<Option<String>> = StdMutex::new(None);

#[derive(Clone, Debug)]
struct SyncStartParams {
    space_id: String,
    device_id: String,
    device_name: String,
    port: Option<u16>,
    seed_addresses: Option<Vec<String>>,
    relay_url: Option<String>,
    relay_api_key: Option<String>,
    auth_secret: Option<String>,
    use_iroh: bool,
    iroh_peer_ticket: Option<String>,
}

struct SyncRuntime {
    server: Arc<SyncServer>,
    storage: Arc<SqliteStorageBackend>,
    clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    relay: Option<Arc<RelaySync>>,
    transport_choice: Option<TransportChoice>,
    start_params: SyncStartParams,
    /// Step 4a: our iroh pairing ticket, captured at construction time when
    /// the iroh transport was selected (`our_ticket()` needs the transport
    /// object directly — `RelaySync` only exposes `Arc<dyn SyncTransport>`,
    /// which is not downcastable — so we snapshot the ticket string instead
    /// of threading a concrete `IrohTransport` handle through `SyncRuntime`).
    /// `None` when iroh wasn't selected, or (in a no-`iroh-spike` build)
    /// always `None`.
    iroh_our_ticket: Option<String>,
    beacon: Arc<BroadcastDiscovery>,
    space_id: String,
    device_id: String,
    device_name: String,
    auth_secret: Option<String>,
    own_addresses: Arc<TokioMutex<Vec<String>>>,
}

static SYNC: TokioMutex<Option<Arc<SyncRuntime>>> = TokioMutex::const_new(None);

// Event emitter moved to `ark_core::events` so that lib modules (notably
// `db::apply_entity_blocking` for schema-drift sync_error/sync_replay events)
// can emit too. Binary registers the sender at startup via `set_event_sender`.

// ---------------------------------------------------------------------------
// Request enum
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum Request {
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
    UpsertArea {
        area: Area,
        #[serde(default)]
        device_id: Option<String>,
    },
    UpsertTag {
        tag: Tag,
        #[serde(default)]
        device_id: Option<String>,
    },
    UpsertHeading {
        heading: Heading,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteHeading {
        id: String,
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
        game: ark_core::canonical_types::game::GameUpsertCommand,
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
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteObject {
        id: String,
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
        registrations: Vec<ark_core::type_registry::TypeRegistration>,
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
    ExternalRefsUpsert {
        #[serde(rename = "connectorId")]
        connector_id: String,
        #[serde(rename = "accountId")]
        account_id: String,
        #[serde(rename = "externalType")]
        external_type: String,
        #[serde(rename = "externalId")]
        external_id: String,
        #[serde(rename = "objectId")]
        object_id: String,
        #[serde(default)]
        revision: Option<String>,
        #[serde(default)]
        hash: Option<String>,
        state: String,
    },
    ClearAll,
    DeleteTrashed,

    /// SQLite Online Backup в указанный path. Source DB остаётся live —
    /// concurrent readers/writer safe. Используется db_backup scheduler
    /// в kepler-backend (раз в 24h). См. hardening proof loop 2026-05-18.
    DbBackup {
        dest_path: String,
    },

    // --- Sync ops (new) ---
    StartSync {
        space_id: String,
        device_id: String,
        #[serde(default)]
        device_name: Option<String>,
        #[serde(default)]
        port: Option<u16>,
        #[serde(default)]
        seed_addresses: Option<Vec<String>>,
        /// Optional relay server WebSocket URL (e.g. "wss://relay.example.com").
        #[serde(default)]
        relay_url: Option<String>,
        /// API key for the relay server.
        #[serde(default)]
        relay_api_key: Option<String>,
        /// Optional shared secret for LAN/P2P hello HMAC authentication.
        #[serde(default)]
        auth_secret: Option<String>,
        /// Step 4a: select the iroh p2p transport instead of relay. Field
        /// exists regardless of build (stable wire schema); only acted on
        /// behind `#[cfg(feature = "iroh-spike")]` — see `select_transport`.
        #[serde(default)]
        use_iroh: bool,
        /// Pairing ticket string for the iroh peer (see
        /// `iroh_transport::IrohTransport::our_ticket`/`from_ticket`).
        #[serde(default)]
        iroh_peer_ticket: Option<String>,
    },
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
    /// selected, sync not running, or build without `iroh-spike`).
    GetOwnIrohTicket,
}
