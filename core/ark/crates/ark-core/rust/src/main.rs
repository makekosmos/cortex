#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(
    all(windows, feature = "windows-gui-subsystem"),
    windows_subsystem = "windows"
)]

//! ark-core-rpc: stdin/stdout JSON-RPC binary used as a sidecar by Electron
//! and any other embedder that wants the full DB + sync runtime without
//! linking the Rust crate directly.
//!
//! Two stream shapes share stdout:
//!
//!   - **Response lines**: `{"ok": true|false, "data"?, "error"?}` — one per
//!     stdin request, emitted in request order.
//!   - **Event lines**: `{"event": "<kind>", ...}` — asynchronous broadcasts
//!     from the sync layer. Distinguishable from responses by the presence of
//!     an `event` field (and absence of `ok`). The embedder must demux.
//!
//! Request envelope (unchanged from the pre-sync binary):
//!   `{"operation": "<snake_case>", ...params}`
//!
//! New sync operations added in this file: `start_sync`, `stop_sync`,
//! `broadcast_change`, `get_connected_peers`, `leave_space`.

use std::collections::HashMap;
use std::io::Write as IoWrite;
use std::sync::{Arc, Mutex as StdMutex};

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, Mutex as TokioMutex};

use ark_core::beacon::{BeaconPeer, BroadcastDiscovery, BroadcastDiscoveryOptions};
use ark_core::db::{self, SqliteStorageBackend};
use ark_core::events::{emit_event, set_event_sender};
use ark_core::hlc::HLC;
use ark_core::host::{get_host_device_name, get_own_addresses};
use ark_core::net::is_address_routable;
use ark_core::protocol::LAN_SYNC_PORT;
use ark_core::relay_sync::{RelaySync, RelaySyncConfig};
use ark_core::sync_client::SyncClient;
use ark_core::sync_server::{StorageBackend, SyncServer};
use ark_core::transport_select::{select_transport, TransportChoice};
use ark_core::types::*;

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

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async {
        if let Err(e) = serve().await {
            eprintln!("ark-core-rpc fatal: {e}");
            std::process::exit(1);
        }
    });
}

async fn serve() -> Result<(), String> {
    // Event channel + stdout writer
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Value>();
    set_event_sender(event_tx);

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            write_event_line(&event);
        }
    });

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        let line: String = line;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(trimmed) {
            Ok(raw) => {
                let request_id = request_id_from_value(&raw);
                match serde_json::from_value::<Request>(raw) {
                    Ok(req) => match handle_request(req).await {
                        Ok(data) => response_ok(data, request_id),
                        Err(e) => response_error(e, request_id),
                    },
                    Err(e) => response_error(e.to_string(), request_id),
                }
            }
            Err(e) => response_error(e.to_string(), None),
        };
        write_response_line(&response);
    }
    Ok(())
}

fn request_id_from_value(value: &Value) -> Option<Value> {
    // SDK кладёт envelope-id в `_req_id`. Старый формат (`id`) тоже принимаем
    // для обратной совместимости с прежним протоколом.
    value
        .get("_req_id")
        .cloned()
        .or_else(|| value.get("id").cloned())
}

fn response_ok(data: Value, request_id: Option<Value>) -> Value {
    response_with_optional_id(json!({ "ok": true, "data": data }), request_id)
}

fn response_error(error: String, request_id: Option<Value>) -> Value {
    response_with_optional_id(json!({ "ok": false, "error": error }), request_id)
}

fn response_with_optional_id(mut response: Value, request_id: Option<Value>) -> Value {
    if let (Value::Object(map), Some(id)) = (&mut response, request_id) {
        // Эхо envelope-id под `_req_id`. Старый формат (`id`) тоже дублируем
        // для SDK-версий, читающих legacy-поле.
        map.insert("_req_id".to_string(), id.clone());
        map.insert("id".to_string(), id);
    }
    response
}

// ---------------------------------------------------------------------------
// Stdout writers (shared lock to prevent interleaving of response + event)
// ---------------------------------------------------------------------------

static STDOUT_LOCK: StdMutex<()> = StdMutex::new(());

fn write_line(value: &Value) {
    let json = match serde_json::to_string(value) {
        Ok(s) => s,
        Err(_) => return,
    };
    let _guard = STDOUT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(json.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

fn write_response_line(value: &Value) {
    write_line(value);
}

fn write_event_line(value: &Value) {
    write_line(value);
}

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

fn with_conn<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    let outer = DB.lock().unwrap_or_else(|e| e.into_inner());
    let shared = outer
        .as_ref()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())?
        .clone();
    drop(outer);
    let inner = shared.lock().unwrap_or_else(|e| e.into_inner());
    f(&inner)
}

/// Выполняет write-замыкание в одной SQLite-транзакции поверх shared conn.
/// COMMIT при Ok, ROLLBACK при Err. Гарантирует атомарность entity + FTS + sync-meta:
/// вложенные SAVEPOINT внутри db::* работают внутри этого BEGIN, при ошибке откатывается всё.
fn with_write_tx<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    with_conn(|conn| {
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| e.to_string())?;
        match f(conn) {
            Ok(v) => {
                conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
                Ok(v)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    })
}

fn get_shared_conn() -> Result<Arc<StdMutex<rusqlite::Connection>>, String> {
    let guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())
}

// --- DB backup: background-priority thread + chunking config ---------------

/// Страниц за один шаг online-backup. Меньше шаг → мягче для диска. Override
/// `ARK_BACKUP_PAGES_PER_STEP`.
fn backup_pages_per_step() -> i32 {
    std::env::var("ARK_BACKUP_PAGES_PER_STEP")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(256)
}

/// Пауза (мс) между шагами online-backup, чтобы не насыщать диск. Override
/// `ARK_BACKUP_PAUSE_MS`.
fn backup_pause_ms() -> u64 {
    std::env::var("ARK_BACKUP_PAUSE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5)
}

/// Thread-scoped background priority (CPU + I/O) для backup-потока.
/// `THREAD_MODE_BACKGROUND_BEGIN` на Windows; no-op иначе. RAII (Drop → END).
/// НЕ process-wide — ark-core-rpc обслуживает интерактивные ARK ops.
#[cfg(windows)]
mod background_priority {
    pub struct BackgroundThreadGuard {
        active: bool,
    }
    impl BackgroundThreadGuard {
        pub fn enter() -> Self {
            use windows::Win32::System::Threading::{
                GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
            };
            // SAFETY: меняем приоритет только текущего потока через псевдо-handle.
            let ok = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
            Self { active: ok.is_ok() }
        }
    }
    impl Drop for BackgroundThreadGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            use windows::Win32::System::Threading::{
                GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_END,
            };
            // SAFETY: симметричный END для ранее успешного BEGIN на том же потоке.
            unsafe {
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
            }
        }
    }
}

#[cfg(not(windows))]
mod background_priority {
    pub struct BackgroundThreadGuard;
    impl BackgroundThreadGuard {
        pub fn enter() -> Self {
            Self
        }
    }
}

fn local_write_device_id(device_id: Option<String>) -> String {
    device_id
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "ark-core-rpc-local".to_string())
}

fn record_local_upsert(
    conn: &rusqlite::Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: Option<String>,
) -> Result<String, String> {
    let device_id = local_write_device_id(device_id);
    let hlc = db::bump_sync_version_vector(conn, entity_type, entity_id, &device_id, false)?;
    db::delete_sync_tombstone(conn, entity_id)?;
    Ok(hlc)
}

fn record_local_delete(
    conn: &rusqlite::Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: Option<String>,
) -> Result<String, String> {
    let device_id = local_write_device_id(device_id);
    let hlc = db::bump_sync_version_vector(conn, entity_type, entity_id, &device_id, true)?;
    db::record_sync_tombstone(conn, entity_type, entity_id, &hlc)?;
    Ok(hlc)
}

/// Строит SyncEntity из сырого serde_json::Value объекта (уже сериализованного).
/// `data_value` ожидается `Value::Object`; "id" удаляется как в `to_data_map` в db.rs.
/// Для delete: передавай `Value::Object(Map::new())` + `deleted = Some(true)`.
fn make_sync_entity(
    entity_type: &str,
    id: &str,
    data_value: Value,
    hlc: String,
    deleted: Option<bool>,
) -> SyncEntity {
    let data = match data_value {
        Value::Object(mut map) => {
            map.remove("id");
            map
        }
        _ => serde_json::Map::new(),
    };
    let origin = db::is_sequenced_usage_entity(entity_type).then(|| HLC::from_string(&hlc));
    SyncEntity {
        entity_type: entity_type.to_string(),
        id: id.to_string(),
        data,
        hlc,
        deleted,
        origin_device_id: origin.as_ref().map(|value| value.device_id.clone()),
        origin_seq: origin.map(|value| value.counter),
    }
}

/// Fan-out локального изменения всем подключённым пирам (server sessions +
/// outbound clients + relay/iroh). Не применяет entity к storage и не
/// перебивает HLC — данные уже записаны через `record_local_*`.
/// Если SYNC не запущен — тихий no-op.
async fn broadcast_local_change(entity: SyncEntity) {
    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return,
        }
    };

    runtime
        .server
        .broadcast_live_change(entity.clone(), None)
        .await;

    let clients: Vec<_> = runtime.clients.lock().await.values().cloned().collect();
    for client in clients {
        client.broadcast_live_change(entity.clone()).await;
    }
    if let Some(relay) = runtime.relay.as_ref() {
        // Ошибку отправки логируем, но не пробрасываем — live broadcast best-effort.
        if let Err(e) = relay.broadcast_live_change(entity.clone()) {
            eprintln!("[ark-core] broadcast_local_change relay error: {e}");
        }
    }
}

fn legacy_content() -> Value {
    json!({"type":"doc","content":[{"type":"paragraph"}]})
}

fn legacy_record(
    id: &str,
    legacy_type_id: &str,
    title: &str,
    props: Value,
    created_at: &str,
    deleted_at: Option<String>,
) -> ark_core::canonical_types::compatibility::LegacyRecord {
    ark_core::canonical_types::compatibility::LegacyRecord {
        id: id.into(),
        legacy_type_id: legacy_type_id.into(),
        title: title.into(),
        content: legacy_content(),
        props,
        created_at: created_at.into(),
        updated_at: created_at.into(),
        deleted_at,
    }
}

fn write_legacy_graph(
    conn: &rusqlite::Connection,
    records: &[ark_core::canonical_types::compatibility::LegacyRecord],
    device_id: Option<String>,
) -> Result<Vec<SyncEntity>, String> {
    ark_core::canonical_types::facades::write_legacy_records(conn, records, "rpc", device_id)
}

fn tombstone_legacy(
    conn: &rusqlite::Connection,
    id: &str,
    expected_type_id: &str,
    device_id: Option<String>,
) -> Result<SyncEntity, String> {
    ark_core::canonical_types::facades::delete_legacy_object(conn, id, expected_type_id, device_id)
}

// ---------------------------------------------------------------------------

async fn handle_request(request: Request) -> Result<Value, String> {
    match request {
        Request::Init { db_path } => {
            let conn = db::open_db(&db_path)?;
            db::init_schema(&conn)?;
            *DB.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(StdMutex::new(conn)));
            *DB_PATH.lock().unwrap_or_else(|e| e.into_inner()) = Some(db_path);
            Ok(json!(true))
        }

        Request::LoadAll => with_conn(|conn| {
            let data = db::load_all(conn)?;
            serde_json::to_value(data).map_err(|e| e.to_string())
        }),

        // Legacy entity write handlers: до 2026-05-18 они не вызывали
        // record_local_upsert/record_local_delete → sync_kv.version_vector не
        // двигался, peers не видели локальных правок todo/project/area/tag/
        // heading через LAN sync. Тихий data loss bug. Все новые handler'ы
        // (UpsertObject, UpsertUsageSession и т.д.) делают это правильно;
        // приводим legacy к тому же контракту.
        // 2026-06-17: все write-handlers теперь рассылают LiveChange через
        // broadcast_local_change — фикс live-sync gap.
        Request::UpsertTodo { todo, device_id } => {
            let record = legacy_record(
                &todo.id,
                "task_obj",
                &todo.title,
                json!({
                    "priority": todo.priority, "scheduled_date": todo.scheduled_date, "deadline": todo.deadline,
                    "reminder_date": todo.reminder_date, "is_today": todo.is_today, "is_evening": todo.is_evening,
                    "is_someday": todo.is_someday, "is_completed": todo.is_completed, "completed_at": todo.completed_at,
                    "is_cancelled": todo.is_cancelled, "cancelled_at": todo.cancelled_at, "checklist_items": todo.checklist_items,
                    "recurrence_rule": todo.recurrence_rule, "project_id": todo.project_id, "tag_ids": todo.tag_ids,
                }),
                &todo.created_at,
                None,
            );
            let entities = with_write_tx(|conn| write_legacy_graph(conn, &[record], device_id))?;
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(entity).await;
                }
            });
            Ok(json!(true))
        }

        Request::DeleteTodo { id, device_id } => {
            let entity =
                with_write_tx(|conn| tombstone_legacy(conn, &id, "com.kosmos.task", device_id))?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::DeleteProject { id, device_id } => {
            let entity =
                with_write_tx(|conn| tombstone_legacy(conn, &id, "com.kosmos.project", device_id))?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::BatchUpsertTodos { todos, device_id } => {
            let records = todos.iter().map(|todo| legacy_record(&todo.id, "task_obj", &todo.title, json!({
                "priority": todo.priority, "scheduled_date": todo.scheduled_date, "deadline": todo.deadline,
                "reminder_date": todo.reminder_date, "is_today": todo.is_today, "is_evening": todo.is_evening,
                "is_someday": todo.is_someday, "is_completed": todo.is_completed, "completed_at": todo.completed_at,
                "is_cancelled": todo.is_cancelled, "cancelled_at": todo.cancelled_at, "checklist_items": todo.checklist_items,
                "recurrence_rule": todo.recurrence_rule, "project_id": todo.project_id, "tag_ids": todo.tag_ids,
            }), &todo.created_at, None)).collect::<Vec<_>>();
            let entities = with_write_tx(|conn| write_legacy_graph(conn, &records, device_id))?;
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(entity).await;
                }
            });
            Ok(json!(true))
        }

        Request::UpsertProject { project, device_id } => {
            let record = legacy_record(
                &project.id,
                "project_obj",
                &project.title,
                json!({"status": project.status, "scheduled_date": project.scheduled_date, "deadline": project.deadline, "color": project.color_tag}),
                &project.created_at,
                None,
            );
            let entities = with_write_tx(|conn| write_legacy_graph(conn, &[record], device_id))?;
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(entity).await;
                }
            });
            Ok(json!(true))
        }

        Request::UpsertArea {
            area: _area,
            device_id: _device_id,
        } => Err("LegacyPlanningReadOnly".to_string()),

        Request::UpsertTag { tag, device_id } => {
            let record = legacy_record(
                &tag.id,
                "tag_obj",
                &tag.title,
                json!({"color": tag.color}),
                &tag.created_at,
                None,
            );
            let entities = with_write_tx(|conn| write_legacy_graph(conn, &[record], device_id))?;
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(entity).await;
                }
            });
            Ok(json!(true))
        }

        Request::UpsertHeading {
            heading: _heading,
            device_id: _device_id,
        } => Err("LegacyPlanningReadOnly".to_string()),

        Request::DeleteHeading {
            id: _id,
            device_id: _device_id,
        } => Err("LegacyPlanningReadOnly".to_string()),

        Request::UpsertTrackedApp {
            tracked_app,
            device_id,
        } => {
            let entity = with_write_tx(|conn| {
                db::upsert_tracked_app(conn, &tracked_app)?;
                let hlc = record_local_upsert(conn, "tracked_app", &tracked_app.id, device_id)?;
                Ok(make_sync_entity(
                    "tracked_app",
                    &tracked_app.id,
                    serde_json::to_value(&tracked_app).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::DeleteTrackedApp { id, device_id } => {
            let entity = with_write_tx(|conn| {
                db::delete_tracked_app(conn, &id)?;
                let hlc = record_local_delete(conn, "tracked_app", &id, device_id)?;
                Ok(make_sync_entity(
                    "tracked_app",
                    &id,
                    json!({}),
                    hlc,
                    Some(true),
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::UpsertUsageSession {
            usage_session,
            device_id,
        } => {
            let entity = with_write_tx(|conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::upsert_usage_session(conn, &usage_session)?;
                let hlc = record_local_upsert(conn, "usage_session", &usage_session.id, device_id)?;
                Ok(make_sync_entity(
                    "usage_session",
                    &usage_session.id,
                    serde_json::to_value(&usage_session).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::DeleteUsageSession { id, device_id } => {
            let entity = with_write_tx(|conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::delete_usage_session(conn, &id)?;
                let hlc = record_local_delete(conn, "usage_session", &id, device_id)?;
                Ok(make_sync_entity(
                    "usage_session",
                    &id,
                    json!({}),
                    hlc,
                    Some(true),
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::UpsertUsageEvent {
            usage_event,
            device_id,
        } => {
            let entity = with_write_tx(|conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::upsert_usage_event(conn, &usage_event)?;
                let hlc = record_local_upsert(conn, "usage_event", &usage_event.id, device_id)?;
                Ok(make_sync_entity(
                    "usage_event",
                    &usage_event.id,
                    serde_json::to_value(&usage_event).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::DeleteUsageEvent { id, device_id } => {
            let entity = with_write_tx(|conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::delete_usage_event(conn, &id)?;
                let hlc = record_local_delete(conn, "usage_event", &id, device_id)?;
                Ok(make_sync_entity(
                    "usage_event",
                    &id,
                    json!({}),
                    hlc,
                    Some(true),
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }
        Request::UpsertUsageSpan { usage_span } => {
            let entities = with_write_tx(|conn| {
                db::ensure_usage_sequence_migrated(conn, &usage_span.device_id)?;
                db::upsert_usage_span(conn, &usage_span)?
                    .into_iter()
                    .map(|day| {
                        let hlc = record_local_upsert(
                            conn,
                            "usage_day",
                            &day.id,
                            Some(usage_span.device_id.clone()),
                        )?;
                        Ok(make_sync_entity(
                            "usage_day",
                            &day.id,
                            serde_json::to_value(&day).unwrap_or(json!({})),
                            hlc,
                            None,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()
            })?;
            let ids = entities
                .iter()
                .map(|entity| entity.id.clone())
                .collect::<Vec<_>>();
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(entity).await;
                }
            });
            Ok(json!({ "usageDayIds": ids }))
        }
        Request::GetUsageTitleTotal { query } => with_conn(|conn| {
            serde_json::to_value(db::get_usage_title_total(conn, &query)?)
                .map_err(|error| error.to_string())
        }),
        Request::GetUsageAnalytics {
            range_days,
            top_apps_limit,
            recent_sessions_limit,
        } => with_conn(|conn| {
            let snapshot = db::load_usage_analytics(
                conn,
                range_days.unwrap_or(21),
                top_apps_limit.unwrap_or(8),
                recent_sessions_limit.unwrap_or(24),
            )?;
            serde_json::to_value(snapshot).map_err(|e| e.to_string())
        }),
        Request::ListRecentUsageProcesses { limit } => with_conn(|conn| {
            let candidates = db::list_recent_usage_processes(conn, limit.unwrap_or(10))?;
            serde_json::to_value(candidates).map_err(|e| e.to_string())
        }),
        Request::SearchUsageProcesses { query, limit } => with_conn(|conn| {
            let candidates = db::search_usage_processes(conn, &query, limit.unwrap_or(10))?;
            serde_json::to_value(candidates).map_err(|e| e.to_string())
        }),
        Request::GetUsageGamePlaytimeSummary {
            bindings,
            range_start,
            range_end,
        } => with_conn(|conn| {
            let summary = db::load_usage_game_playtime_summary(
                conn,
                &bindings,
                range_start.as_deref(),
                range_end.as_deref(),
            )?;
            serde_json::to_value(summary).map_err(|e| e.to_string())
        }),
        Request::ListObjects => with_conn(|conn| {
            let objects = db::list_objects(conn)?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::ListObjectSummaries => with_conn(|conn| {
            let objects = db::list_object_summaries(conn)?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::ListObjectsByType { type_id } => with_conn(|conn| {
            let objects = db::list_objects_by_type(conn, &type_id)?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::ListObjectSummariesByType { type_id } => with_conn(|conn| {
            let objects = db::list_object_summaries_by_type(conn, &type_id)?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::ListRunningTimeEntries { source } => with_conn(|conn| {
            let objects = db::list_running_time_entries(conn, source.as_deref())?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::GetObjectsByIds { ids } => with_conn(|conn| {
            let objects = db::get_objects_by_ids(conn, &ids)?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::SearchObjects { query } => with_conn(|conn| {
            let results = db::search_objects(conn, &query)?;
            serde_json::to_value(results).map_err(|e| e.to_string())
        }),
        Request::GetObject { id } => with_conn(|conn| {
            let object = db::get_object(conn, &id)?;
            serde_json::to_value(object).map_err(|e| e.to_string())
        }),
        Request::CanonicalGameList { device_id } => {
            let device = local_write_device_id(device_id);
            with_conn(|conn| {
                serde_json::to_value(ark_core::canonical_types::game::list_games(conn, &device)?)
                    .map_err(|e| e.to_string())
            })
        }
        Request::CanonicalGameGet { id, device_id } => {
            let device = local_write_device_id(device_id);
            with_conn(|conn| {
                serde_json::to_value(ark_core::canonical_types::game::get_game(
                    conn, &id, &device,
                )?)
                .map_err(|e| e.to_string())
            })
        }
        Request::CanonicalGameUpsert { game, device_id } => {
            let device = local_write_device_id(device_id);
            let record = with_write_tx(|conn| {
                ark_core::canonical_types::game::upsert_game(conn, game, &device)
            })?;
            if record.changed {
                emit_event(json!({"event":"arrancador.changed"}));
            }
            serde_json::to_value(record).map_err(|e| e.to_string())
        }
        Request::CanonicalAssetSources { object_ids } => with_conn(|conn| {
            serde_json::to_value(
                ark_core::canonical_types::facades::asset_sources(conn, &object_ids)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|e| e.to_string())
        }),
        Request::CanonicalSetBookCover {
            book_id,
            source_ref,
            existing_image_id,
            alt_text,
            device_id,
        } => {
            let device_id = local_write_device_id(device_id);
            let book_id_for_event = book_id.clone();
            let mutation = with_write_tx(|conn| {
                let mutation = ark_core::canonical_types::facades::set_book_cover(
                    conn,
                    &book_id,
                    source_ref.as_deref(),
                    existing_image_id.as_deref(),
                    &alt_text,
                    &device_id,
                )
                .map_err(|error| error.to_string())?;
                if !mutation.changed {
                    return Ok(mutation);
                }
                let book_hlc =
                    record_local_upsert(conn, "object", &book_id, Some(device_id.clone()))?;
                conn.execute(
                    "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE excluded.hlc > object_sync_versions.hlc",
                    rusqlite::params![book_id, book_hlc],
                )
                .map_err(|e| e.to_string())?;
                for link in &mutation.links {
                    let link_hlc = record_local_upsert(
                        conn,
                        "object_link",
                        &link.id,
                        Some(device_id.clone()),
                    )?;
                    conn.execute(
                        "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE excluded.hlc > object_sync_versions.hlc",
                        rusqlite::params![link.id, link_hlc],
                    )
                    .map_err(|e| e.to_string())?;
                }
                for id in &mutation.deleted_link_ids {
                    record_local_delete(conn, "object_link", id, Some(device_id.clone()))?;
                }
                if let Some(image) = &mutation.image {
                    record_local_upsert(conn, "object", &image.id, Some(device_id.clone()))?;
                }
                Ok(mutation)
            })?;
            if mutation.changed {
                emit_event(json!({
                    "event": "canonical_book_cover_changed",
                    "bookId": book_id_for_event,
                }));
            }
            Ok(json!(true))
        }
        Request::UpsertObject { object, device_id } => {
            let object_id = object.id.clone();
            let object_type_id = object.type_id.clone();
            let entity = with_write_tx(|conn| {
                let object = ark_core::canonical_types::ingress::prepare_object(conn, object)
                    .map_err(|error| error.to_string())?;
                db::upsert_object(conn, &object)?;
                let hlc = record_local_upsert(conn, "object", &object.id, device_id)?;
                Ok(make_sync_entity(
                    "object",
                    &object.id,
                    serde_json::to_value(&object).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            let eid = object_id.clone();
            let etid = object_type_id.clone();
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            emit_event(json!({
                "event": "object_upserted",
                "id": eid,
                "type_id": etid,
            }));
            Ok(json!(true))
        }
        Request::DeleteObject { id, device_id } => {
            let object_id = id.clone();
            let entity = with_write_tx(|conn| {
                db::delete_object(conn, &id)?;
                let hlc = record_local_delete(conn, "object", &id, device_id)?;
                Ok(make_sync_entity("object", &id, json!({}), hlc, Some(true)))
            })?;
            let eid = object_id.clone();
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            emit_event(json!({
                "event": "object_deleted",
                "id": eid,
            }));
            Ok(json!(true))
        }
        Request::TypesList => with_conn(|conn| {
            serde_json::to_value(ark_core::type_registry::list_type_summaries(conn)?)
                .map_err(|e| e.to_string())
        }),
        Request::TypesRegisterPackageDefinitions { registrations } => with_write_tx(|conn| {
            for registration in &registrations {
                ark_core::type_registry::register_type(conn, registration)?;
            }
            Ok(json!(true))
        }),
        Request::TypesGet { type_id, version } => with_conn(|conn| {
            serde_json::to_value(ark_core::type_registry::get_type(
                conn,
                &type_id,
                version.as_deref(),
            )?)
            .map_err(|e| e.to_string())
        }),
        Request::TypesListVersions { type_id } => with_conn(|conn| {
            let Some(canonical) = ark_core::type_registry::resolve_type_id(conn, &type_id)? else {
                return Ok(json!([]));
            };
            serde_json::to_value(ark_core::type_registry::list_type_versions(
                conn, &canonical,
            )?)
            .map_err(|e| e.to_string())
        }),
        Request::TypesResolveAlias { alias } => with_conn(|conn| {
            serde_json::to_value(ark_core::type_registry::resolve_alias(conn, &alias)?)
                .map_err(|e| e.to_string())
        }),
        Request::ListObjectTypes => with_conn(|conn| {
            let object_types = db::list_object_types(conn)?;
            serde_json::to_value(object_types).map_err(|e| e.to_string())
        }),
        Request::GetObjectType { id } => with_conn(|conn| {
            let object_type = db::get_object_type(conn, &id)?;
            serde_json::to_value(object_type).map_err(|e| e.to_string())
        }),
        Request::UpsertObjectType {
            object_type,
            device_id,
        } => {
            // replay_pending_for_type теперь использует SAVEPOINT ark_replay_pending
            // вместо BEGIN IMMEDIATE, поэтому вкладывается в транзакцию из with_write_tx.
            // entity-строка + sync-meta записываются атомарно.
            let entity = with_write_tx(|conn| {
                db::upsert_object_type(conn, &object_type)?;
                let hlc = record_local_upsert(conn, "object_type", &object_type.id, device_id)?;
                Ok(make_sync_entity(
                    "object_type",
                    &object_type.id,
                    serde_json::to_value(&object_type).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }
        Request::DeleteObjectType { id, device_id } => {
            let entity = with_write_tx(|conn| {
                db::delete_object_type(conn, &id)?;
                let hlc = record_local_delete(conn, "object_type", &id, device_id)?;
                Ok(make_sync_entity(
                    "object_type",
                    &id,
                    json!({}),
                    hlc,
                    Some(true),
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }
        Request::ListObjectLinks => with_conn(|conn| {
            let object_links = db::list_object_links(conn)?;
            serde_json::to_value(object_links).map_err(|e| e.to_string())
        }),
        Request::UpsertObjectLink {
            object_link,
            device_id,
        } => {
            let entity = with_write_tx(|conn| {
                db::upsert_object_link(conn, &object_link)?;
                let hlc = record_local_upsert(conn, "object_link", &object_link.id, device_id)?;
                Ok(make_sync_entity(
                    "object_link",
                    &object_link.id,
                    serde_json::to_value(&object_link).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }
        Request::DeleteObjectLink { id, device_id } => {
            let entity = with_write_tx(|conn| {
                db::delete_object_link(conn, &id)?;
                let hlc = record_local_delete(conn, "object_link", &id, device_id)?;
                Ok(make_sync_entity(
                    "object_link",
                    &id,
                    json!({}),
                    hlc,
                    Some(true),
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::GetSyncKv { key } => with_conn(|conn| {
            let value = db::get_sync_kv(conn, &key)?;
            Ok(json!(value))
        }),

        Request::SetSyncKv { key, value } => with_conn(|conn| {
            db::set_sync_kv(conn, &key, &value)?;
            Ok(json!(true))
        }),

        Request::ClearAll => with_conn(|conn| {
            db::clear_all(conn)?;
            Ok(json!(null))
        }),

        Request::DeleteTrashed => with_conn(|conn| {
            let count = db::delete_trashed(conn)?;
            Ok(json!(count))
        }),

        Request::DbBackup { dest_path } => {
            let src_path = DB_PATH
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .ok_or_else(|| "Database not initialized. Call Init first.".to_string())?;
            let pages = backup_pages_per_step();
            let pause = std::time::Duration::from_millis(backup_pause_ms());
            let dest_for_thread = dest_path.clone();
            // Fire-and-forget: копирование идёт на отдельном background-priority
            // потоке через ОТДЕЛЬНЫЙ read-коннекшн (не держит глобальный DB
            // mutex) — серийный RPC-loop сразу свободен для других запросов.
            // Завершение сообщается событием `db_backup_result`; kepler-backend
            // ждёт его, чтобы записать last_backup_ts + ротацию.
            std::thread::Builder::new()
                .name("ark-db-backup".to_string())
                .spawn(move || {
                    let _bg = background_priority::BackgroundThreadGuard::enter();
                    let result =
                        db::backup_to_file_chunked(&src_path, &dest_for_thread, pages, pause);
                    let event = match &result {
                        Ok(()) => json!({
                            "event": "db_backup_result",
                            "ok": true,
                            "dest": dest_for_thread,
                        }),
                        Err(e) => json!({
                            "event": "db_backup_result",
                            "ok": false,
                            "dest": dest_for_thread,
                            "error": e,
                        }),
                    };
                    emit_event(event);
                })
                .map_err(|e| format!("spawn db_backup thread failed: {e}"))?;
            Ok(json!({ "started": true, "dest": dest_path }))
        }

        Request::StartSync {
            space_id,
            device_id,
            device_name,
            port,
            seed_addresses,
            relay_url,
            relay_api_key,
            auth_secret,
            use_iroh,
            iroh_peer_ticket,
        } => {
            handle_start_sync(
                space_id,
                device_id,
                device_name,
                port,
                seed_addresses,
                relay_url,
                relay_api_key,
                auth_secret,
                use_iroh,
                iroh_peer_ticket,
            )
            .await
        }

        Request::StopSync => {
            handle_stop_sync().await;
            Ok(json!(true))
        }

        Request::BroadcastChange { entity } => handle_broadcast_change(entity).await,

        Request::GetConnectedPeers => handle_get_connected_peers().await,

        Request::GetSyncSnapshot => handle_get_sync_snapshot().await,

        Request::DisconnectPeer { device_id } => handle_disconnect_peer(device_id).await,

        Request::ConnectWithPairingCode { pairing_code } => {
            handle_connect_with_pairing_code(pairing_code).await
        }

        Request::LeaveSpace => {
            handle_stop_sync().await;
            // Also purge any persisted self-reference peer records so the
            // next start_sync on this or any other space does not inherit
            // phantom records. The underlying SyncServer did this inside its
            // own `start()` but we may be leaving without restarting.
            Ok(json!(true))
        }

        Request::AddSeedPeer { addresses } => handle_add_seed_peer(addresses).await,

        Request::GetOwnAddresses { port } => {
            let port = port.unwrap_or(LAN_SYNC_PORT);
            Ok(json!(get_own_addresses(port)))
        }

        Request::GetHostDeviceName => Ok(json!(get_host_device_name())),

        Request::GetOwnIrohTicket => handle_get_own_iroh_ticket().await,
    }
}

// ---------------------------------------------------------------------------
// Sync handlers
// ---------------------------------------------------------------------------

/// Wires the standard event-stream callbacks (`entity_changed`,
/// `peer_connected`, `peer_disconnected`) onto a `RelaySync` instance.
/// Shared between the relay and iroh branches of `handle_start_sync` — the
/// orchestration layer (`RelaySync`) is transport-neutral, so the same
/// callback wiring applies regardless of which `SyncTransport` drives it.
async fn wire_relay_sync_events(relay_sync: &Arc<RelaySync>) {
    relay_sync
        .set_on_change(Arc::new(|entity| {
            // Incoming peer-write notification. `entity_changed` — общее событие
            // (raw consumers). Но renderer-подписчики (Eden) слушают типизированные
            // `object_upserted`/`object_deleted` — те же, что эмитит локальная
            // запись (`Request::UpsertObject`/`DeleteObject`). Без этого входящий
            // sync долетает в БД, но UI не перерисовывается до перезагрузки.
            emit_event(json!({
                "event": "entity_changed",
                "entity": entity,
            }));
            if entity.entity_type == "object" {
                if entity.deleted == Some(true) {
                    emit_event(json!({
                        "event": "object_deleted",
                        "id": entity.id,
                    }));
                } else {
                    emit_event(json!({
                        "event": "object_upserted",
                        "id": entity.id,
                        "type_id": entity.data.get("type_id").cloned().unwrap_or(Value::Null),
                    }));
                }
            }
        }))
        .await;
    relay_sync
        .set_on_peer_connect(Arc::new(|peer_device_id| {
            emit_event(json!({
                "event": "peer_connected",
                "device_id": peer_device_id,
            }));
        }))
        .await;
    relay_sync
        .set_on_peer_disconnect(Arc::new(|peer_device_id, remaining| {
            emit_event(json!({
                "event": "peer_disconnected",
                "device_id": peer_device_id,
                "remaining": remaining,
            }));
        }))
        .await;
}

#[allow(clippy::too_many_arguments)]
async fn handle_start_sync(
    space_id: String,
    device_id: String,
    device_name: Option<String>,
    port: Option<u16>,
    seed_addresses: Option<Vec<String>>,
    relay_url: Option<String>,
    relay_api_key: Option<String>,
    auth_secret: Option<String>,
    use_iroh: bool,
    iroh_peer_ticket: Option<String>,
) -> Result<Value, String> {
    // Idempotency: tear down any running runtime first.
    handle_stop_sync().await;

    let device_name = device_name.unwrap_or_else(get_host_device_name);
    let ws_port = port.unwrap_or(LAN_SYNC_PORT);

    let shared_conn = get_shared_conn()?;
    let storage = Arc::new(SqliteStorageBackend::new(shared_conn.clone()));
    storage.set_device_id(&device_id)?;

    let server = Arc::new(SyncServer::new(storage.clone() as Arc<dyn StorageBackend>));
    server.set_auth_secret(auth_secret.clone()).await;

    // Wire server callbacks -> event stream
    {
        let device_id_for_cb = device_id.clone();
        server
            .set_on_change(Arc::new(move |entity| {
                emit_event(json!({
                    "event": "entity_changed",
                    "entity": entity,
                }));
                let _ = device_id_for_cb; // silence unused capture lint
            }))
            .await;
    }
    {
        server
            .set_on_peer_connect(Arc::new(move |peer_device_id| {
                emit_event(json!({
                    "event": "peer_connected",
                    "device_id": peer_device_id,
                }));
            }))
            .await;
    }
    {
        server
            .set_on_peer_disconnect(Arc::new(move |peer_device_id, remaining| {
                emit_event(json!({
                    "event": "peer_disconnected",
                    "device_id": peer_device_id,
                    "remaining": remaining,
                }));
            }))
            .await;
    }

    let own_addresses: Vec<String> = get_own_addresses(ws_port)
        .into_iter()
        .filter(|a| is_address_routable(a))
        .collect();
    let own_addresses_shared = Arc::new(TokioMutex::new(own_addresses.clone()));

    server
        .start_with_addr(
            &space_id,
            &device_id,
            Some(&device_name),
            Some(own_addresses.clone()),
            &format!("0.0.0.0:{ws_port}"),
        )
        .await?;

    let transport_choice = select_transport(use_iroh, &relay_url);
    #[allow(unused_mut, unused_assignments)]
    let mut iroh_our_ticket: Option<String> = None;
    // `iroh_peer_ticket` is only read inside the `#[cfg(feature = "iroh-spike")]`
    // branch below; reference it here so a no-feature build doesn't warn about
    // an unused parameter (the field itself must stay on the wire schema
    // regardless of build per the UniFFI/JSON-RPC surface-stability rule).
    let _ = &iroh_peer_ticket;

    let transport_state = Some(transport_choice.clone());
    let start_params = SyncStartParams {
        space_id: space_id.clone(),
        device_id: device_id.clone(),
        device_name: device_name.clone(),
        port,
        seed_addresses: seed_addresses.clone(),
        relay_url: relay_url.clone(),
        relay_api_key: relay_api_key.clone(),
        auth_secret: auth_secret.clone(),
        use_iroh,
        iroh_peer_ticket: iroh_peer_ticket.clone(),
    };
    let relay = if transport_choice == TransportChoice::Relay {
        let relay_url = relay_url.clone().expect("Relay choice implies relay_url");
        let transport: Arc<dyn ark_core::sync_transport::SyncTransport> =
            Arc::new(ark_core::relay_transport::RelayTransport::new(
                ark_core::relay_transport::RelayConfig {
                    url: relay_url.clone(),
                    space_id: space_id.clone(),
                    device_id: device_id.clone(),
                    device_name: device_name.clone(),
                    api_key: relay_api_key.clone().unwrap_or_default(),
                    auth_secret: auth_secret.clone(),
                },
            ));
        let relay_sync = RelaySync::with_transport(
            storage.clone() as Arc<dyn StorageBackend>,
            RelaySyncConfig {
                relay_url,
                relay_api_key: relay_api_key.clone(),
                space_id: space_id.clone(),
                device_id: device_id.clone(),
                device_name: device_name.clone(),
                auth_secret: auth_secret.clone(),
            },
            transport,
        );
        wire_relay_sync_events(&relay_sync).await;
        relay_sync.start().await?;
        Some(relay_sync)
    } else if transport_choice == TransportChoice::Iroh {
        #[cfg(feature = "iroh-spike")]
        {
            let secret_key = {
                let conn = shared_conn.lock().unwrap_or_else(|e| e.into_inner());
                ark_core::iroh_transport::load_or_generate_secret_key(&conn)
                    .map_err(|e| format!("iroh transport: failed to load identity: {e}"))?
            };
            let peer_addr = match iroh_peer_ticket.as_deref() {
                Some(ticket) => Some(ark_core::iroh_transport::from_ticket(ticket)?),
                None => None,
            };
            let iroh_transport = Arc::new(ark_core::iroh_transport::IrohTransport::new(
                ark_core::iroh_transport::IrohConfig {
                    device_id: device_id.clone(),
                    device_name: device_name.clone(),
                    space_id: space_id.clone(),
                    secret_key: Some(secret_key),
                    peer_addr,
                    peer_ticket: iroh_peer_ticket.clone(),
                    relay_mode: None,
                    auth_secret: auth_secret.clone(),
                },
            ));
            // RelaySyncConfig.relay_url is unused by `with_transport` (only
            // `RelaySync::new` reads it to build a `RelayTransport`) — pass an
            // empty string rather than widening the struct for one unused field.
            let relay_sync = RelaySync::with_transport(
                storage.clone() as Arc<dyn StorageBackend>,
                RelaySyncConfig {
                    relay_url: String::new(),
                    relay_api_key: None,
                    space_id: space_id.clone(),
                    device_id: device_id.clone(),
                    device_name: device_name.clone(),
                    auth_secret: auth_secret.clone(),
                },
                iroh_transport.clone() as Arc<dyn ark_core::sync_transport::SyncTransport>,
            );
            wire_relay_sync_events(&relay_sync).await;
            relay_sync.start().await?;
            // `start()` binds the endpoint, so `our_ticket()` is available now.
            // Snapshot it onto `SyncRuntime` for `GetOwnIrohTicket` — capture
            // failures are logged but not fatal (pairing UI degrades to "no
            // ticket yet" rather than aborting an otherwise-successful start).
            iroh_our_ticket = match iroh_transport.our_ticket().await {
                Ok(ticket) => Some(ticket),
                Err(e) => {
                    eprintln!("[handle_start_sync] our_ticket() failed: {e}");
                    None
                }
            };
            Some(relay_sync)
        }
        #[cfg(not(feature = "iroh-spike"))]
        {
            return Err(
                "iroh transport requested (use_iroh) but this build was compiled without the \
                 iroh-spike feature; rebuild with --features iroh-spike or use relay_url instead"
                    .to_string(),
            );
        }
    } else {
        None
    };

    // Start SyncClient connections for every known peer.
    let clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let known_peers = server.get_known_peers().await;
    let removed_peer_ids = server.get_removed_peer_ids().await;
    for peer in known_peers {
        if peer.device_id == device_id || removed_peer_ids.iter().any(|id| id == &peer.device_id) {
            continue;
        }
        let reachable: Vec<String> = peer
            .addresses
            .iter()
            .filter(|a| !own_addresses.contains(a))
            .cloned()
            .collect();
        if reachable.is_empty() {
            continue;
        }
        let peer_rec = PeerRecord {
            addresses: reachable,
            ..peer
        };
        spawn_sync_client(
            &server,
            &storage,
            &clients,
            peer_rec,
            device_id.clone(),
            device_name.clone(),
            space_id.clone(),
            own_addresses.clone(),
            auth_secret.clone(),
        )
        .await;
    }

    // Seed addresses from QR payload / initial-join flow.
    if let Some(addrs) = seed_addresses {
        if !addrs.is_empty() {
            start_seed_client(
                &server,
                &storage,
                &clients,
                addrs,
                device_id.clone(),
                device_name.clone(),
                space_id.clone(),
                own_addresses.clone(),
                auth_secret.clone(),
            )
            .await;
        }
    }

    // Start beacon discovery.
    let beacon = Arc::new(BroadcastDiscovery::new());
    let beacon_clone = beacon.clone();
    let server_for_beacon = server.clone();
    let clients_for_beacon = clients.clone();
    let storage_for_beacon = storage.clone();
    let device_id_for_beacon = device_id.clone();
    let device_name_for_beacon = device_name.clone();
    let space_id_for_beacon = space_id.clone();
    let own_addresses_for_beacon = own_addresses.clone();
    let auth_secret_for_beacon = auth_secret.clone();
    beacon
        .set_on_peer_discovered(Arc::new(move |peer: BeaconPeer| {
            // Callbacks from UDP recv run on tokio tasks — but this one is
            // invoked from a sync closure. Spawn to an async context so we
            // can await the storage / server.
            let server = server_for_beacon.clone();
            let clients = clients_for_beacon.clone();
            let storage = storage_for_beacon.clone();
            let device_id = device_id_for_beacon.clone();
            let device_name = device_name_for_beacon.clone();
            let space_id = space_id_for_beacon.clone();
            let own = own_addresses_for_beacon.clone();
            let auth_secret = auth_secret_for_beacon.clone();
            tokio::spawn(async move {
                let addrs: Vec<String> = if peer.addresses.is_empty() {
                    vec![peer.address.clone()]
                } else {
                    peer.addresses.clone()
                };
                let reachable: Vec<String> = addrs
                    .into_iter()
                    .filter(|a| !own.contains(a) && is_address_routable(a))
                    .collect();
                if reachable.is_empty() {
                    return;
                }
                if server
                    .get_removed_peer_ids()
                    .await
                    .iter()
                    .any(|id| id == &peer.device_id)
                {
                    return;
                }
                server
                    .register_external_peer(&peer.device_id, &peer.device_name, reachable.clone())
                    .await;
                emit_event(json!({
                    "event": "peer_list_updated",
                    "peers": server.get_connected_peer_entries().await.iter().map(|(id, name)| {
                        json!({"device_id": id, "device_name": name})
                    }).collect::<Vec<_>>(),
                }));

                if server.is_connected_to(&peer.device_id).await {
                    return;
                }
                let existing = clients.lock().await.get(&peer.device_id).cloned();
                if let Some(existing) = existing {
                    // Update its peer record so reconnect picks the new addresses.
                    existing
                        .update_peer(PeerRecord {
                            device_id: peer.device_id.clone(),
                            device_name: peer.device_name.clone(),
                            addresses: reachable.clone(),
                            last_seen: chrono::Utc::now()
                                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                            last_address: None,
                        })
                        .await;
                    return;
                }

                let peer_rec = PeerRecord {
                    device_id: peer.device_id.clone(),
                    device_name: peer.device_name.clone(),
                    addresses: reachable,
                    last_seen: chrono::Utc::now()
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    last_address: None,
                };
                spawn_sync_client(
                    &server,
                    &storage,
                    &clients,
                    peer_rec,
                    device_id,
                    device_name,
                    space_id,
                    own,
                    auth_secret,
                )
                .await;
            });
        }))
        .await;

    beacon_clone
        .start(BroadcastDiscoveryOptions {
            space_id: space_id.clone(),
            device_id: device_id.clone(),
            device_name: device_name.clone(),
            ws_port,
        })
        .await?;

    let runtime = SyncRuntime {
        server,
        storage,
        clients,
        relay,
        transport_choice: transport_state,
        start_params,
        iroh_our_ticket,
        beacon: beacon_clone,
        space_id,
        device_id,
        device_name,
        auth_secret,
        own_addresses: own_addresses_shared,
    };
    *SYNC.lock().await = Some(Arc::new(runtime));

    Ok(json!(true))
}

#[allow(clippy::too_many_arguments)]
async fn spawn_sync_client(
    server: &Arc<SyncServer>,
    storage: &Arc<SqliteStorageBackend>,
    clients: &Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    peer: PeerRecord,
    device_id: String,
    device_name: String,
    space_id: String,
    own_addresses: Vec<String>,
    auth_secret: Option<String>,
) {
    if peer.device_id == device_id {
        return;
    }

    let client = Arc::new(SyncClient::new(
        storage.clone() as Arc<dyn StorageBackend>,
        peer.clone(),
        device_id.clone(),
        device_name.clone(),
        space_id.clone(),
        own_addresses.clone(),
        auth_secret,
    ));

    client
        .set_on_change(Arc::new(|entity| {
            emit_event(json!({
                "event": "entity_changed",
                "entity": entity,
            }));
        }))
        .await;

    let server_for_connect = server.clone();
    let peer_addrs = peer.addresses.clone();
    client
        .set_on_connected(Arc::new(move |peer_device_id, peer_name| {
            let server = server_for_connect.clone();
            let addrs = peer_addrs.clone();
            let peer_device_id_clone = peer_device_id.clone();
            let peer_name_clone = peer_name.clone();
            tokio::spawn(async move {
                server
                    .register_external_peer(&peer_device_id_clone, &peer_name_clone, addrs)
                    .await;
            });
            emit_event(json!({
                "event": "peer_connected",
                "device_id": peer_device_id,
                "device_name": peer_name,
            }));
        }))
        .await;

    let server_for_disconnect = server.clone();
    client
        .set_on_disconnected(Arc::new(move |peer_device_id| {
            let server = server_for_disconnect.clone();
            let peer_device_id_clone = peer_device_id.clone();
            tokio::spawn(async move {
                let remaining = server.connected_peer_count().await;
                emit_event(json!({
                    "event": "peer_disconnected",
                    "device_id": peer_device_id_clone,
                    "remaining": remaining,
                }));
            });
        }))
        .await;

    client.start();
    clients.lock().await.insert(peer.device_id.clone(), client);
}

#[allow(clippy::too_many_arguments)]
async fn start_seed_client(
    server: &Arc<SyncServer>,
    storage: &Arc<SqliteStorageBackend>,
    clients: &Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    addresses: Vec<String>,
    device_id: String,
    device_name: String,
    space_id: String,
    own_addresses: Vec<String>,
    auth_secret: Option<String>,
) {
    let reachable: Vec<String> = addresses
        .into_iter()
        .filter(|a| !own_addresses.contains(a) && is_address_routable(a))
        .collect();
    if reachable.is_empty() {
        return;
    }
    let temp_id = format!("seed-{}", chrono::Utc::now().timestamp_millis());
    let peer = PeerRecord {
        device_id: temp_id.clone(),
        device_name: "Bootstrap".to_string(),
        addresses: reachable,
        last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        last_address: None,
    };
    spawn_sync_client(
        server,
        storage,
        clients,
        peer,
        device_id,
        device_name,
        space_id,
        own_addresses,
        auth_secret,
    )
    .await;
}

async fn handle_stop_sync() {
    let runtime = {
        let mut guard = SYNC.lock().await;
        guard.take()
    };
    if let Some(runtime) = runtime {
        runtime.beacon.stop().await;
        if let Some(relay) = runtime.relay.as_ref() {
            relay.stop();
        }
        let clients: Vec<Arc<SyncClient>> = {
            let clients = runtime.clients.lock().await;
            clients.values().cloned().collect()
        };
        for client in clients {
            client.disconnect().await;
        }
        runtime.server.stop().await;
    }
}

async fn handle_start_sync_with_params(params: SyncStartParams) -> Result<Value, String> {
    handle_start_sync(
        params.space_id,
        params.device_id,
        Some(params.device_name),
        params.port,
        params.seed_addresses,
        params.relay_url,
        params.relay_api_key,
        params.auth_secret,
        params.use_iroh,
        params.iroh_peer_ticket,
    )
    .await
}

fn build_pairing_restart_params(runtime: &SyncRuntime, pairing_code: &str) -> SyncStartParams {
    let mut params = runtime.start_params.clone();
    params.use_iroh = true;
    params.iroh_peer_ticket = Some(pairing_code.trim().to_string());
    params
}

async fn handle_broadcast_change(mut entity: SyncEntity) -> Result<Value, String> {
    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return Err("Sync not running".to_string()),
        }
    };

    // Stamp HLC on the outgoing entity.
    let hlc = runtime.server.update_entity_hlc(&entity.id).await;
    entity.hlc = hlc;

    // Persist locally so future version-vector exchanges reflect it.
    runtime.storage.apply_entity(&entity).await?;

    // Broadcast via the inbound server sessions.
    runtime
        .server
        .broadcast_live_change(entity.clone(), None)
        .await;

    // Broadcast via outbound clients.
    let clients: Vec<_> = runtime.clients.lock().await.values().cloned().collect();
    for client in clients {
        client.broadcast_live_change(entity.clone()).await;
    }
    if let Some(relay) = runtime.relay.as_ref() {
        relay.broadcast_live_change(entity.clone())?;
    }
    Ok(json!(true))
}

/// Step 4a: surface our iroh pairing ticket for the runtime/UI. `null` when
/// sync isn't running or the running runtime didn't select iroh (relay/no
/// transport, or a build without `iroh-spike`).
async fn handle_get_own_iroh_ticket() -> Result<Value, String> {
    let guard = SYNC.lock().await;
    let ticket = guard
        .as_ref()
        .and_then(|runtime| runtime.iroh_our_ticket.clone());
    Ok(json!(ticket))
}

async fn handle_get_sync_snapshot() -> Result<Value, String> {
    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => {
                return Ok(json!({
                    "running": false,
                    "transport": "unknown",
                    "pairing_available": false,
                    "own_pairing_code_available": false,
                    "peers": [],
                }))
            }
        }
    };

    let mut connected = runtime.server.get_connected_peer_entries().await;
    if let Some(relay) = runtime.relay.as_ref() {
        connected.extend(relay.get_connected_peer_entries().await);
    }
    let known = runtime.server.get_known_peers().await;
    let connected_ids: std::collections::HashSet<String> =
        connected.iter().map(|(id, _)| id.clone()).collect();
    let mut represented_ids = std::collections::HashSet::new();
    let mut peers: Vec<Value> = known
        .into_iter()
        .filter(|peer| peer.device_id != runtime.device_id)
        .map(|peer| {
            represented_ids.insert(peer.device_id.clone());
            let status = if connected_ids.contains(&peer.device_id) {
                "online"
            } else {
                "offline"
            };
            json!({
                "device_id": peer.device_id,
                "device_name": peer.device_name,
                "last_seen": peer.last_seen,
                "status": status,
            })
        })
        .collect();
    peers.extend(
        connected
            .into_iter()
            .filter(|(device_id, _)| {
                device_id != &runtime.device_id && !represented_ids.contains(device_id)
            })
            .map(|(device_id, device_name)| {
                json!({
                    "device_id": device_id,
                    "device_name": device_name,
                    "last_seen": chrono::Utc::now().to_rfc3339(),
                    "status": "online",
                })
            }),
    );

    Ok(json!({
        "running": true,
        "transport": match runtime.transport_choice {
            Some(TransportChoice::Iroh) => "iroh",
            Some(TransportChoice::Relay) => "relay",
            Some(TransportChoice::None) => "lan",
            None => "unknown",
        },
        "pairing_available": runtime.iroh_our_ticket.is_some(),
        "own_pairing_code_available": runtime.iroh_our_ticket.is_some(),
        "peers": peers,
        "local_device": {
            "device_id": runtime.device_id.clone(),
            "device_name": runtime.device_name.clone(),
        },
    }))
}

async fn handle_disconnect_peer(device_id: String) -> Result<Value, String> {
    let guard = SYNC.lock().await;
    let runtime = match guard.as_ref() {
        Some(r) => r.clone(),
        None => return Err("Sync not running".to_string()),
    };
    drop(guard);
    let device_id = device_id.trim();
    if device_id.is_empty() {
        return Err("device_id is empty".to_string());
    }

    let _ = runtime.server.disconnect_peer(device_id).await;

    let client_entries: Vec<(String, Arc<SyncClient>)> = {
        let clients = runtime.clients.lock().await;
        clients
            .iter()
            .map(|(key, client)| (key.clone(), client.clone()))
            .collect()
    };
    let mut removed_client_ids: Vec<String> = Vec::new();
    for (client_key, client) in client_entries {
        let peer = client.current_peer().await;
        if peer.device_id == device_id {
            client.disconnect().await;
            removed_client_ids.push(client_key);
        }
    }
    if !removed_client_ids.is_empty() {
        let mut clients = runtime.clients.lock().await;
        for client_id in removed_client_ids {
            clients.remove(&client_id);
        }
    }

    let remaining = runtime.server.connected_peer_count().await;
    emit_event(json!({
        "event": "peer_disconnected",
        "device_id": device_id,
        "remaining": remaining,
    }));
    emit_event(json!({"event": "peer_list_updated"}));
    Ok(json!(true))
}

async fn handle_connect_with_pairing_code(pairing_code: String) -> Result<Value, String> {
    let code = pairing_code.trim();
    if code.is_empty() {
        return Err("pairing code is empty".to_string());
    }

    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return Err("Sync not running".to_string()),
        }
    };

    let restart_params = build_pairing_restart_params(&runtime, code);
    let restore_params = runtime.start_params.clone();

    handle_stop_sync().await;
    match handle_start_sync_with_params(restart_params).await {
        Ok(result) => Ok(result),
        Err(err) => {
            if let Err(restore_err) = handle_start_sync_with_params(restore_params).await {
                return Err(format!(
                    "connect_with_pairing_code failed: {err}; restoring previous sync also failed: {restore_err}"
                ));
            }
            Err(format!(
                "connect_with_pairing_code failed: {err}; previous sync restored"
            ))
        }
    }
}

async fn handle_get_connected_peers() -> Result<Value, String> {
    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return Ok(json!([])),
        }
    };
    let entries = runtime.server.get_connected_peer_entries().await;

    // Merge in outbound-connected clients (not yet visible in the server peer
    // table if the connection is outbound-only).
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for (id, name) in entries {
        if !seen.contains_key(&id) {
            order.push(id.clone());
        }
        seen.insert(id, name);
    }
    let clients: Vec<_> = runtime
        .clients
        .lock()
        .await
        .iter()
        .map(|(device_id, client)| (device_id.clone(), client.clone()))
        .collect();
    for (device_id, client) in clients {
        if seen.contains_key(&device_id) {
            continue;
        }
        let peer = client.current_peer().await;
        if !peer.device_id.is_empty() {
            order.push(peer.device_id.clone());
            seen.insert(peer.device_id, peer.device_name);
        }
    }

    if let Some(relay) = runtime.relay.as_ref() {
        for (device_id, device_name) in relay.get_connected_peer_entries().await {
            if seen.contains_key(&device_id) {
                continue;
            }
            order.push(device_id.clone());
            seen.insert(device_id, device_name);
        }
    }

    let list: Vec<Value> = order
        .into_iter()
        .filter_map(|id| {
            seen.remove(&id).map(|name| {
                json!({
                    "device_id": id,
                    "device_name": name,
                })
            })
        })
        .collect();
    Ok(json!(list))
}

async fn handle_add_seed_peer(addresses: Vec<String>) -> Result<Value, String> {
    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return Err("Sync not running".to_string()),
        }
    };
    let own = runtime.own_addresses.lock().await.clone();
    start_seed_client(
        &runtime.server,
        &runtime.storage,
        &runtime.clients,
        addresses,
        runtime.device_id.clone(),
        runtime.device_name.clone(),
        runtime.space_id.clone(),
        own,
        runtime.auth_secret.clone(),
    )
    .await;
    Ok(json!(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_package_wires_windows_gui_subsystem_only_for_ark_binary() {
        let source = include_str!("main.rs");
        let cargo = include_str!("../Cargo.toml");
        let package_build =
            include_str!("../../../../../../platform/desktop/scripts/build-backend.mjs");

        assert!(cargo.contains("windows-gui-subsystem = []"));
        assert!(source.contains("all(windows, feature = \"windows-gui-subsystem\")"));
        assert!(source.contains("windows_subsystem = \"windows\""));
        assert!(package_build.contains("\"-p\",\n    \"ark-core\""));
        assert!(package_build.contains("\"--features\",\n    \"iroh-spike,windows-gui-subsystem\""));
    }

    static TEST_DB_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    static TEST_EVENT_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[test]
    fn response_omits_id_for_legacy_request() {
        let response = response_ok(json!(true), None);
        assert_eq!(response.get("ok"), Some(&json!(true)));
        assert!(response.get("id").is_none());
    }

    #[test]
    fn response_echoes_request_id_on_success() {
        let response = response_ok(json!(true), Some(json!("req-1")));
        assert_eq!(response.get("ok"), Some(&json!(true)));
        assert_eq!(response.get("id"), Some(&json!("req-1")));
    }

    #[test]
    fn response_echoes_request_id_on_error() {
        let response = response_error("bad request".to_string(), Some(json!("req-2")));
        assert_eq!(response.get("ok"), Some(&json!(false)));
        assert_eq!(response.get("id"), Some(&json!("req-2")));
        assert_eq!(response.get("error"), Some(&json!("bad request")));
    }

    #[test]
    fn request_deserialization_ignores_optional_id_field() {
        let request = serde_json::from_value::<Request>(json!({
            "id": "req-3",
            "operation": "get_host_device_name"
        }))
        .expect("request id must be backward-compatible metadata");

        assert!(matches!(request, Request::GetHostDeviceName));
    }

    #[test]
    fn request_deserialization_accepts_iroh_config() {
        let request = serde_json::from_value::<Request>(json!({
            "operation": "start_sync",
            "space_id": "space",
            "device_id": "device",
            "use_iroh": true,
            "iroh_peer_ticket": "endpointsometicketvalue"
        }))
        .expect("iroh config should be accepted by the request schema");

        let Request::StartSync {
            use_iroh,
            iroh_peer_ticket,
            ..
        } = request
        else {
            unreachable!("expected start_sync");
        };
        assert!(use_iroh);
        assert_eq!(iroh_peer_ticket.as_deref(), Some("endpointsometicketvalue"));
    }

    #[test]
    fn request_deserialization_accepts_code_alias_for_pairing() {
        let request = serde_json::from_value::<Request>(json!({
            "operation": "connect_with_pairing_code",
            "code": "endpointdemo123"
        }))
        .expect("code alias should deserialize for pairing requests");

        let Request::ConnectWithPairingCode { pairing_code } = request else {
            unreachable!("expected connect_with_pairing_code");
        };
        assert_eq!(pairing_code, "endpointdemo123");
    }

    #[test]
    fn pairing_restart_params_force_iroh_and_replace_ticket() {
        let storage = Arc::new(ark_core::db::SqliteStorageBackend::new(Arc::new(
            StdMutex::new(rusqlite::Connection::open_in_memory().unwrap()),
        )));
        let runtime = SyncRuntime {
            server: Arc::new(ark_core::sync_server::SyncServer::new(
                storage.clone() as Arc<dyn ark_core::sync_server::StorageBackend>
            )),
            storage,
            clients: Arc::new(TokioMutex::new(HashMap::new())),
            relay: None,
            transport_choice: Some(TransportChoice::None),
            start_params: SyncStartParams {
                space_id: "space-a".to_string(),
                device_id: "device-a".to_string(),
                device_name: "Device A".to_string(),
                port: Some(21531),
                seed_addresses: Some(vec!["127.0.0.1:21531".to_string()]),
                relay_url: Some("ws://relay.example".to_string()),
                relay_api_key: Some("relay-key".to_string()),
                auth_secret: Some("secret".to_string()),
                use_iroh: false,
                iroh_peer_ticket: None,
            },
            iroh_our_ticket: None,
            beacon: Arc::new(ark_core::beacon::BroadcastDiscovery::new()),
            space_id: "space-a".to_string(),
            device_id: "device-a".to_string(),
            device_name: "Device A".to_string(),
            auth_secret: Some("secret".to_string()),
            own_addresses: Arc::new(TokioMutex::new(Vec::new())),
        };

        let params = build_pairing_restart_params(&runtime, "  endpointdemo123  ");
        assert!(params.use_iroh);
        assert_eq!(params.iroh_peer_ticket.as_deref(), Some("endpointdemo123"));
        assert_eq!(params.relay_url.as_deref(), Some("ws://relay.example"));
        assert_eq!(params.space_id, "space-a");
        assert_eq!(params.device_id, "device-a");
    }

    #[cfg(feature = "iroh-spike")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn start_sync_with_use_iroh_selects_iroh_transport_and_exposes_ticket() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        // Before start_sync, our ticket must be unavailable.
        let before = handle_request(Request::GetOwnIrohTicket).await.unwrap();
        assert_eq!(before, Value::Null);

        let port = {
            // Bind an ephemeral port for the LAN/WS server side of
            // start_sync so this test doesn't collide with LAN_SYNC_PORT
            // across parallel test runs.
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };

        let start_result = handle_request(Request::StartSync {
            space_id: "iroh-space".to_string(),
            device_id: "device-iroh".to_string(),
            device_name: Some("Iroh Device".to_string()),
            port: Some(port),
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: true,
            iroh_peer_ticket: None,
        })
        .await;

        // `handle_start_sync` binds the shared UDP beacon discovery port
        // (`beacon::BEACON_PORT`, fixed/non-configurable, unrelated to this
        // change) AFTER the iroh transport is already constructed and
        // started. On a dev machine that also has the real Kosmos app
        // running, that fixed port is already taken — a pre-existing
        // environment hazard for any `start_sync` integration test, not a
        // regression from this change (and out of scope: the task says LAN
        // discovery code must stay untouched). Treat that specific bind
        // failure as inconclusive rather than asserting the whole iroh path
        // failed; any other error is a real failure.
        match start_result {
            Ok(_) => {
                let ticket = handle_request(Request::GetOwnIrohTicket)
                    .await
                    .expect("get_own_iroh_ticket should succeed once iroh transport is running");
                assert!(
                    ticket.as_str().is_some_and(|s| !s.is_empty()),
                    "expected a non-empty iroh ticket string, got {ticket:?}"
                );
                handle_request(Request::StopSync).await.unwrap();
            }
            Err(e) if e.contains("Failed to bind UDP") => {
                eprintln!(
                    "skipping ticket assertion: beacon UDP port unavailable in this \
                     environment (unrelated to iroh transport selection): {e}"
                );
            }
            Err(e) => assert!(false, "start_sync with use_iroh failed unexpectedly: {e}"),
        }
    }

    #[cfg(not(feature = "iroh-spike"))]
    #[tokio::test]
    async fn start_sync_with_use_iroh_fails_gracefully_without_iroh_spike_feature() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };

        let result = handle_request(Request::StartSync {
            space_id: "iroh-space".to_string(),
            device_id: "device-iroh".to_string(),
            device_name: Some("Iroh Device".to_string()),
            port: Some(port),
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: true,
            iroh_peer_ticket: None,
        })
        .await;

        assert!(
            result.is_err(),
            "use_iroh must fail with a clear error when built without iroh-spike, not silently no-op"
        );

        handle_request(Request::StopSync).await.unwrap();
    }

    #[test]
    fn request_deserialization_defaults_iroh_fields_when_absent() {
        let request = serde_json::from_value::<Request>(json!({
            "operation": "start_sync",
            "space_id": "space",
            "device_id": "device"
        }))
        .expect("start_sync without iroh fields should still deserialize");

        let Request::StartSync {
            use_iroh,
            iroh_peer_ticket,
            ..
        } = request
        else {
            unreachable!("expected start_sync");
        };
        assert!(!use_iroh);
        assert_eq!(iroh_peer_ticket, None);
    }

    #[test]
    fn request_deserialization_accepts_relay_and_auth_config() {
        let request = serde_json::from_value::<Request>(json!({
            "operation": "start_sync",
            "space_id": "space",
            "device_id": "device",
            "relay_url": "ws://127.0.0.1:8765",
            "relay_api_key": "key",
            "auth_secret": "secret"
        }))
        .expect("relay config should be accepted by the request schema");

        let Request::StartSync {
            relay_url,
            relay_api_key,
            auth_secret,
            ..
        } = request
        else {
            unreachable!("expected start_sync");
        };
        assert_eq!(relay_url.as_deref(), Some("ws://127.0.0.1:8765"));
        assert_eq!(relay_api_key.as_deref(), Some("key"));
        assert_eq!(auth_secret.as_deref(), Some("secret"));
    }

    #[tokio::test]
    async fn local_object_and_usage_writes_record_sync_state() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        let object = ArkObjectWrite {
            id: "obj-local-write".to_string(),
            type_id: "com.kosmos.game".to_string(),
            type_version: Some("1.0.0".to_string()),
            title: "Local Game".to_string(),
            content_json: json!({}),
            props_json: json!({
                "playStatus": null,
                "userRating": null,
                "genres": [],
                "platforms": [],
                "released": null,
                "description": null,
                "extensions": {}
            }),
            created_at: "2026-04-24T00:00:00.000Z".to_string(),
            updated_at: "2026-04-24T00:00:00.000Z".to_string(),
            deleted_at: None,
        };
        handle_request(Request::UpsertObject {
            object,
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();

        let tracked_app = TrackedApp {
            id: "app-local-write".to_string(),
            platform: "windows".to_string(),
            exe_path: "C:\\Games\\Demo\\demo.exe".to_string(),
            normalized_exe_path: "c:\\games\\demo\\demo.exe".to_string(),
            process_name: "demo.exe".to_string(),
            display_name: Some("Demo".to_string()),
            publisher: None,
            icon_ref: None,
            first_seen_at: "2026-04-24T00:00:00.000Z".to_string(),
            last_seen_at: "2026-04-24T00:00:00.000Z".to_string(),
        };
        handle_request(Request::UpsertTrackedApp {
            tracked_app,
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();

        let session = UsageSession {
            id: "session-local-write".to_string(),
            tracked_app_id: "app-local-write".to_string(),
            device_id: "device-local".to_string(),
            device_name: "Device".to_string(),
            platform: "windows".to_string(),
            started_at: "2026-04-24T00:00:00.000Z".to_string(),
            ended_at: None,
            runtime_ms: 1000,
            foreground_ms: 1000,
            idle_ms: 0,
            window_title: Some("Demo".to_string()),
            process_name: "demo.exe".to_string(),
            exe_path: "C:\\Games\\Demo\\demo.exe".to_string(),
            pid_start: Some(1),
            pid_end: None,
            meta_json: json!({}),
        };
        handle_request(Request::UpsertUsageSession {
            usage_session: session,
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();

        let event = UsageEvent {
            id: "event-local-write".to_string(),
            tracked_app_id: "app-local-write".to_string(),
            usage_session_id: Some("session-local-write".to_string()),
            device_id: "device-local".to_string(),
            device_name: "Device".to_string(),
            platform: "windows".to_string(),
            occurred_at: "2026-04-24T00:00:01.000Z".to_string(),
            kind: "foreground".to_string(),
            window_title: Some("Demo".to_string()),
            process_name: "demo.exe".to_string(),
            exe_path: "C:\\Games\\Demo\\demo.exe".to_string(),
            pid: Some(1),
            is_foreground: true,
            is_idle: false,
            meta_json: json!({}),
        };
        handle_request(Request::UpsertUsageEvent {
            usage_event: event,
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();

        handle_request(Request::DeleteObject {
            id: "obj-local-write".to_string(),
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();

        let shared = get_shared_conn().unwrap();
        let guard = shared.lock().unwrap();
        let raw = db::get_sync_kv(&guard, "lan_sync.version_vector")
            .unwrap()
            .expect("version vector should be stored");
        let vector: VersionVector = serde_json::from_str(&raw).unwrap();
        for id in ["obj-local-write", "app-local-write"] {
            assert!(
                vector
                    .get(id)
                    .is_some_and(|hlc| hlc.ends_with(":device-local")),
                "{id} should have a local HLC in the version vector",
            );
        }
        assert_eq!(
            vector.get("@usage:device-local").map(String::as_str),
            Some("2")
        );
        assert!(!vector.contains_key("session-local-write"));
        assert!(!vector.contains_key("event-local-write"));

        let tombstone_count: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
                rusqlite::params!["obj-local-write", "object"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tombstone_count, 1);
    }

    #[tokio::test]
    async fn local_object_type_and_link_writes_record_sync_state() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        let timestamp = "2026-04-24T00:00:00.000Z".to_string();
        let object_type = ObjectType {
            id: "rpc-game-type".to_string(),
            name: "Game".to_string(),
            schema_json: "{}".to_string(),
            ui_schema_json: "{}".to_string(),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
            system_locked: false,
        };
        handle_request(Request::UpsertObjectType {
            object_type: object_type.clone(),
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();
        handle_request(Request::UpsertObjectType {
            object_type: ObjectType {
                id: "empty_type_for_delete".to_string(),
                name: "Empty".to_string(),
                schema_json: "{}".to_string(),
                ui_schema_json: "{}".to_string(),
                created_at: timestamp.clone(),
                updated_at: timestamp.clone(),
                system_locked: false,
            },
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();

        let source = ArkObjectWrite {
            id: "source-object".to_string(),
            type_id: object_type.id.clone(),
            type_version: Some("0.0.0-legacy".to_string()),
            title: "Source".to_string(),
            content_json: json!({}),
            props_json: json!({}),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
            deleted_at: None,
        };
        let target = ArkObjectWrite {
            id: "target-object".to_string(),
            type_id: object_type.id.clone(),
            type_version: Some("0.0.0-legacy".to_string()),
            title: "Target".to_string(),
            content_json: json!({}),
            props_json: json!({}),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
            deleted_at: None,
        };
        handle_request(Request::UpsertObject {
            object: source,
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();
        handle_request(Request::UpsertObject {
            object: target,
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();

        let link = ObjectLink {
            id: "link-local-write".to_string(),
            source_object_id: "source-object".to_string(),
            target_object_id: "target-object".to_string(),
            link_type: "related".to_string(),
            created_at: timestamp,
        };
        handle_request(Request::UpsertObjectLink {
            object_link: link,
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();

        handle_request(Request::DeleteObjectLink {
            id: "link-local-write".to_string(),
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();
        handle_request(Request::DeleteObjectType {
            id: "empty_type_for_delete".to_string(),
            device_id: Some("device-local".to_string()),
        })
        .await
        .unwrap();

        let shared = get_shared_conn().unwrap();
        let guard = shared.lock().unwrap();
        let raw = db::get_sync_kv(&guard, "lan_sync.version_vector")
            .unwrap()
            .expect("version vector should be stored");
        let vector: VersionVector = serde_json::from_str(&raw).unwrap();
        for id in ["rpc-game-type", "link-local-write", "empty_type_for_delete"] {
            assert!(
                vector
                    .get(id)
                    .is_some_and(|hlc| hlc.ends_with(":device-local")),
                "{id} should have a local HLC in the version vector",
            );
        }

        let link_tombstone_count: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
                rusqlite::params!["link-local-write", "object_link"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(link_tombstone_count, 1);

        let type_tombstone_count: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
                rusqlite::params!["empty_type_for_delete", "object_type"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(type_tombstone_count, 1);
    }

    /// Regression for the legacy Todo/Project/Tag write handlers: local writes
    /// must record an HLC in the version vector and deletes must write a tombstone.
    #[tokio::test]
    async fn legacy_entity_writes_bump_version_vector() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        let device = Some("device-legacy".to_string());
        let timestamp = "2026-05-18T00:00:00.000Z".to_string();

        let project = Project {
            id: "project-legacy".to_string(),
            title: "Project".to_string(),
            notes: None,
            status: "active".to_string(),
            scheduled_date: None,
            deadline: None,
            sort_order: 0,
            color_tag: None,
            area_id: Some("area-legacy".to_string()),
            created_at: timestamp.clone(),
        };
        handle_request(Request::UpsertProject {
            project,
            device_id: device.clone(),
        })
        .await
        .unwrap();

        let tag = Tag {
            id: "tag-legacy".to_string(),
            title: "Tag".to_string(),
            color: None,
            created_at: timestamp.clone(),
        };
        handle_request(Request::UpsertTag {
            tag,
            device_id: device.clone(),
        })
        .await
        .unwrap();

        let make_todo = |id: &str| TodoItem {
            id: id.to_string(),
            title: "Todo".to_string(),
            notes: None,
            priority: 0,
            scheduled_date: None,
            deadline: None,
            reminder_date: None,
            is_today: false,
            is_evening: false,
            is_someday: false,
            is_completed: true,
            completed_at: None,
            is_cancelled: false,
            cancelled_at: None,
            is_trashed: false,
            sort_order: 0,
            heading_id: None,
            project_id: Some("project-legacy".to_string()),
            area_id: None,
            tag_ids: vec![],
            checklist_items: json!([]),
            recurrence_rule: None,
            created_at: timestamp.clone(),
        };

        handle_request(Request::UpsertTodo {
            todo: make_todo("todo-legacy"),
            device_id: device.clone(),
        })
        .await
        .unwrap();

        // Batch upsert тоже должен bump'ать version vector per-entity.
        handle_request(Request::BatchUpsertTodos {
            todos: vec![make_todo("todo-batch-1"), make_todo("todo-batch-2")],
            device_id: device.clone(),
        })
        .await
        .unwrap();

        // Delete legacy — должен записать tombstone.
        handle_request(Request::DeleteTodo {
            id: "todo-batch-1".to_string(),
            device_id: device.clone(),
        })
        .await
        .unwrap();

        let shared = get_shared_conn().unwrap();
        let guard = shared.lock().unwrap();
        let raw = db::get_sync_kv(&guard, "lan_sync.version_vector")
            .unwrap()
            .expect("version vector should be stored");
        let vector: VersionVector = serde_json::from_str(&raw).unwrap();
        for id in [
            "project-legacy",
            "tag-legacy",
            "todo-legacy",
            "todo-batch-1",
            "todo-batch-2",
        ] {
            assert!(
                vector
                    .get(id)
                    .is_some_and(|hlc| hlc.ends_with(":device-legacy")),
                "{id} should have a local HLC in the version vector after legacy upsert",
            );
        }

        let todo_tombstone: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
                rusqlite::params!["todo-batch-1", "object"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(todo_tombstone, 1, "DeleteTodo должен записать tombstone");
    }

    #[tokio::test]
    async fn legacy_planning_writes_are_read_only_without_sync_side_effects() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        let snapshot = || {
            let shared = get_shared_conn().unwrap();
            let conn = shared.lock().unwrap();
            let vector = db::get_sync_kv(&conn, "lan_sync.version_vector").unwrap();
            let counts = [
                "areas",
                "headings",
                "todos",
                "projects",
                "tags",
                "objects",
                "object_links",
                "sync_tombstones",
                "sync_kv",
            ]
            .map(|table| {
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap()
            });
            (vector, counts)
        };

        let before = snapshot();
        let area = Area {
            id: "area-read-only".to_string(),
            title: "Area".to_string(),
            sort_order: 0,
            created_at: "2026-05-18T00:00:00.000Z".to_string(),
        };
        let heading = Heading {
            id: "heading-read-only".to_string(),
            title: "Heading".to_string(),
            sort_order: 0,
            project_id: "project-read-only".to_string(),
        };
        for request in [
            Request::UpsertArea {
                area,
                device_id: Some("device-read-only".to_string()),
            },
            Request::UpsertHeading {
                heading,
                device_id: Some("device-read-only".to_string()),
            },
            Request::DeleteHeading {
                id: "heading-read-only".to_string(),
                device_id: Some("device-read-only".to_string()),
            },
        ] {
            assert_eq!(
                handle_request(request).await.unwrap_err(),
                "LegacyPlanningReadOnly"
            );
        }
        assert_eq!(snapshot(), before);
    }

    // -----------------------------------------------------------------------
    // RED-тесты: локальная запись должна рассылать LiveChange пирам
    // -----------------------------------------------------------------------

    /// Fake SyncTransport: захватывает все отправленные LanSyncMessage в shared buf.
    struct CapturingTransport {
        sent: Arc<TokioMutex<Vec<ark_core::protocol::LanSyncMessage>>>,
    }

    #[async_trait::async_trait]
    impl ark_core::sync_transport::SyncTransport for CapturingTransport {
        async fn start(
            &self,
            _event_tx: tokio::sync::mpsc::UnboundedSender<ark_core::sync_transport::TransportEvent>,
        ) -> Result<(), String> {
            Ok(())
        }

        fn send(&self, msg: ark_core::protocol::LanSyncMessage) -> Result<(), String> {
            // `send` — sync, но нам нужен lock на TokioMutex из sync контекста.
            // Используем blocking_lock через spawn_blocking или try_lock; в тестах
            // конкурентности нет, try_lock гарантированно успевает.
            self.sent
                .try_lock()
                .expect("CapturingTransport: lock")
                .push(msg);
            Ok(())
        }

        fn stop(&self) {}
    }

    /// Вспомогательная функция: строит минимальный SyncRuntime с CapturingTransport
    /// и выставляет в глобальный SYNC. Возвращает буфер перехваченных сообщений.
    async fn setup_sync_with_capturing_transport(
        shared_conn: Arc<StdMutex<rusqlite::Connection>>,
    ) -> Arc<TokioMutex<Vec<ark_core::protocol::LanSyncMessage>>> {
        use ark_core::relay_sync::{RelaySync, RelaySyncConfig};
        use ark_core::sync_server::StorageBackend;

        let captured: Arc<TokioMutex<Vec<ark_core::protocol::LanSyncMessage>>> =
            Arc::new(TokioMutex::new(Vec::new()));
        let transport = Arc::new(CapturingTransport {
            sent: captured.clone(),
        });

        let backend = Arc::new(ark_core::db::SqliteStorageBackend::new(shared_conn));
        backend.set_device_id("test-device").unwrap();

        let relay = RelaySync::with_transport(
            backend.clone() as Arc<dyn StorageBackend>,
            RelaySyncConfig {
                relay_url: String::new(),
                relay_api_key: None,
                space_id: "test-space".to_string(),
                device_id: "test-device".to_string(),
                device_name: "Test Device".to_string(),
                auth_secret: None,
            },
            transport as Arc<dyn ark_core::sync_transport::SyncTransport>,
        );
        relay.start().await.unwrap();

        let server = Arc::new(ark_core::sync_server::SyncServer::new(
            backend.clone() as Arc<dyn StorageBackend>
        ));

        let runtime = SyncRuntime {
            server,
            storage: backend,
            clients: Arc::new(TokioMutex::new(std::collections::HashMap::new())),
            relay: Some(relay),
            transport_choice: Some(TransportChoice::Relay),
            start_params: SyncStartParams {
                space_id: "test-space".to_string(),
                device_id: "test-device".to_string(),
                device_name: "Test Device".to_string(),
                port: None,
                seed_addresses: None,
                relay_url: None,
                relay_api_key: None,
                auth_secret: None,
                use_iroh: false,
                iroh_peer_ticket: None,
            },
            iroh_our_ticket: None,
            beacon: Arc::new(ark_core::beacon::BroadcastDiscovery::new()),
            space_id: "test-space".to_string(),
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            auth_secret: None,
            own_addresses: Arc::new(TokioMutex::new(Vec::new())),
        };
        *SYNC.lock().await = Some(Arc::new(runtime));

        captured
    }

    #[tokio::test]
    async fn disconnect_peer_stops_matching_outbound_client_and_emits_events() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let _event_guard = TEST_EVENT_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        let shared = get_shared_conn().unwrap();
        let backend = Arc::new(ark_core::db::SqliteStorageBackend::new(shared));
        backend.set_device_id("device-local").unwrap();

        let peer = PeerRecord {
            device_id: "peer-123".to_string(),
            device_name: "Peer 123".to_string(),
            addresses: vec!["192.168.1.20:21531".to_string()],
            last_seen: "2026-06-17T00:00:00.000Z".to_string(),
            last_address: None,
        };
        let client = Arc::new(SyncClient::new(
            backend.clone() as Arc<dyn ark_core::sync_server::StorageBackend>,
            peer,
            "device-local".to_string(),
            "Local Device".to_string(),
            "space-a".to_string(),
            vec![],
            None,
        ));

        let clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>> = Arc::new(TokioMutex::new(
            HashMap::from([("peer-123".to_string(), client.clone())]),
        ));
        let runtime = SyncRuntime {
            server: Arc::new(ark_core::sync_server::SyncServer::new(
                backend.clone() as Arc<dyn ark_core::sync_server::StorageBackend>
            )),
            storage: backend,
            clients: clients.clone(),
            relay: None,
            transport_choice: Some(TransportChoice::None),
            start_params: SyncStartParams {
                space_id: "space-a".to_string(),
                device_id: "device-local".to_string(),
                device_name: "Local Device".to_string(),
                port: None,
                seed_addresses: None,
                relay_url: None,
                relay_api_key: None,
                auth_secret: None,
                use_iroh: false,
                iroh_peer_ticket: None,
            },
            iroh_our_ticket: None,
            beacon: Arc::new(ark_core::beacon::BroadcastDiscovery::new()),
            space_id: "space-a".to_string(),
            device_id: "device-local".to_string(),
            device_name: "Local Device".to_string(),
            auth_secret: None,
            own_addresses: Arc::new(TokioMutex::new(Vec::new())),
        };
        *SYNC.lock().await = Some(Arc::new(runtime));

        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<serde_json::Value>();
        ark_core::events::set_event_sender(event_tx);

        handle_disconnect_peer("peer-123".to_string())
            .await
            .unwrap();

        assert!(client.is_stopped());
        assert!(clients.lock().await.is_empty());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut saw_disconnect = false;
        let mut saw_list = false;
        while !(saw_disconnect && saw_list) {
            match event_rx.try_recv() {
                Ok(event) => {
                    if event["event"] == "peer_disconnected" {
                        assert_eq!(event["device_id"], "peer-123");
                        saw_disconnect = true;
                    } else if event["event"] == "peer_list_updated" {
                        saw_list = true;
                    }
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    assert!(
                        std::time::Instant::now() <= deadline,
                        "timed out waiting for disconnect events"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    assert!(false, "event channel disconnected unexpectedly");
                }
            }
        }
    }

    #[tokio::test]
    async fn upsert_object_broadcasts_live_change_to_peers() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        // Нужен объектный тип перед вставкой объекта (FK constraint)
        handle_request(Request::UpsertObjectType {
            object_type: ObjectType {
                id: "note".to_string(),
                name: "Note".to_string(),
                schema_json: "{}".to_string(),
                ui_schema_json: "{}".to_string(),
                created_at: "2026-06-17T00:00:00.000Z".to_string(),
                updated_at: "2026-06-17T00:00:00.000Z".to_string(),
                system_locked: false,
            },
            device_id: Some("test-device".to_string()),
        })
        .await
        .unwrap();

        let shared = get_shared_conn().unwrap();
        let captured = setup_sync_with_capturing_transport(shared).await;

        let object = ArkObjectWrite {
            id: "live-obj-1".to_string(),
            type_id: "note".to_string(),
            type_version: Some("0.0.0-legacy".to_string()),
            title: "Live Test".to_string(),
            content_json: json!({ "type": "doc", "content": [] }),
            props_json: json!({}),
            created_at: "2026-06-17T00:00:00.000Z".to_string(),
            updated_at: "2026-06-17T00:00:00.000Z".to_string(),
            deleted_at: None,
        };
        handle_request(Request::UpsertObject {
            object,
            device_id: Some("test-device".to_string()),
        })
        .await
        .unwrap();

        // Небольшая пауза — broadcast_local_change запускается как spawn
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let msgs = captured.lock().await;
        let live_change = msgs.iter().find(|m| {
            matches!(m, ark_core::protocol::LanSyncMessage::LiveChange { entity, .. }
                if entity.id == "live-obj-1" && entity.entity_type == "object" && entity.deleted.is_none())
        });
        assert!(
            live_change.is_some(),
            "UpsertObject должен рассылать LiveChange(entity_type=object, id=live-obj-1, deleted=None); \
             получено сообщений: {}, содержимое: {:?}",
            msgs.len(),
            msgs.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>()
        );

        handle_request(Request::StopSync).await.unwrap();
    }

    #[tokio::test]
    async fn delete_object_broadcasts_live_change_with_deleted_flag() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        // Нужен объектный тип
        handle_request(Request::UpsertObjectType {
            object_type: ObjectType {
                id: "note".to_string(),
                name: "Note".to_string(),
                schema_json: "{}".to_string(),
                ui_schema_json: "{}".to_string(),
                created_at: "2026-06-17T00:00:00.000Z".to_string(),
                updated_at: "2026-06-17T00:00:00.000Z".to_string(),
                system_locked: false,
            },
            device_id: Some("test-device".to_string()),
        })
        .await
        .unwrap();

        // Создаём объект сначала
        let object = ArkObjectWrite {
            id: "live-obj-del".to_string(),
            type_id: "note".to_string(),
            type_version: Some("0.0.0-legacy".to_string()),
            title: "To Delete".to_string(),
            content_json: json!({}),
            props_json: json!({}),
            created_at: "2026-06-17T00:00:00.000Z".to_string(),
            updated_at: "2026-06-17T00:00:00.000Z".to_string(),
            deleted_at: None,
        };
        handle_request(Request::UpsertObject {
            object,
            device_id: Some("test-device".to_string()),
        })
        .await
        .unwrap();

        let shared = get_shared_conn().unwrap();
        let captured = setup_sync_with_capturing_transport(shared).await;

        handle_request(Request::DeleteObject {
            id: "live-obj-del".to_string(),
            device_id: Some("test-device".to_string()),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let msgs = captured.lock().await;
        let live_change = msgs.iter().find(|m| {
            matches!(m, ark_core::protocol::LanSyncMessage::LiveChange { entity, .. }
                if entity.id == "live-obj-del"
                    && entity.entity_type == "object"
                    && entity.deleted == Some(true))
        });
        assert!(
            live_change.is_some(),
            "DeleteObject должен рассылать LiveChange(deleted=Some(true)); \
             получено: {:?}",
            msgs.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>()
        );

        handle_request(Request::StopSync).await.unwrap();
    }

    // -----------------------------------------------------------------------
    // RED-тесты: fail-closed + атомарность entity/sync-meta (2026-06-18)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn legacy_alias_object_write_read_and_filter_is_canonical() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        handle_request(Request::Init {
            db_path: dir.path().join("ark.db").to_string_lossy().into_owned(),
        })
        .await
        .unwrap();

        handle_request(Request::UpsertObject {
            object: ArkObjectWrite {
                id: "alias-note".into(),
                type_id: "com.kosmos.note".into(),
                type_version: Some("1.0.0".into()),
                title: "Alias note".into(),
                content_json: json!({"type":"doc","content":[]}),
                props_json: json!({"description":null,"extensions":{}}),
                created_at: "2026-06-18T00:00:00.000Z".into(),
                updated_at: "2026-06-18T00:00:00.000Z".into(),
                deleted_at: None,
            },
            device_id: None,
        })
        .await
        .unwrap();

        let object = handle_request(Request::GetObject {
            id: "alias-note".into(),
        })
        .await
        .unwrap();
        assert_eq!(object["typeId"], "com.kosmos.note");
        assert_eq!(object["typeVersion"], "1.0.0");
        for type_id in ["note_obj", "com.kosmos.note"] {
            let objects = handle_request(Request::ListObjectsByType {
                type_id: type_id.into(),
            })
            .await
            .unwrap();
            assert_eq!(objects.as_array().unwrap().len(), 1);
            assert_eq!(objects[0]["id"], "alias-note");
        }
    }

    /// FK violation → handle_request должен возвращать Err.
    /// Текущий код проглатывает ошибку и возвращает Ok(true) → RED.
    #[tokio::test]
    async fn upsert_object_with_invalid_type_id_is_err() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        // type_id "nonexistent_type" не существует → FK violation в db::upsert_object
        let object = ArkObjectWrite {
            id: "obj-bad-type".to_string(),
            type_id: "nonexistent_type".to_string(),
            type_version: Some("0.0.0-legacy".to_string()),
            title: "Bad Object".to_string(),
            content_json: json!({}),
            props_json: json!({}),
            created_at: "2026-06-18T00:00:00.000Z".to_string(),
            updated_at: "2026-06-18T00:00:00.000Z".to_string(),
            deleted_at: None,
        };

        let result = handle_request(Request::UpsertObject {
            object,
            device_id: None,
        })
        .await;

        assert!(
            result.is_err(),
            "UpsertObject с несуществующим type_id должен возвращать Err (FK violation); \
             получено: {:?}",
            result
        );
    }

    /// После неудачного upsert строки в objects нет И нет записи в sync
    /// version-vector для этого id (атомарность, дефект №2).
    #[tokio::test]
    async fn failed_upsert_object_persists_nothing() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        // type_id "ghost_type" не существует → upsert упадёт на FK
        let object = ArkObjectWrite {
            id: "obj-ghost".to_string(),
            type_id: "ghost_type".to_string(),
            type_version: Some("0.0.0-legacy".to_string()),
            title: "Ghost".to_string(),
            content_json: json!({}),
            props_json: json!({}),
            created_at: "2026-06-18T00:00:00.000Z".to_string(),
            updated_at: "2026-06-18T00:00:00.000Z".to_string(),
            deleted_at: None,
        };

        // Ожидаем Err; после него проверяем что ничего не записалось
        let _ = handle_request(Request::UpsertObject {
            object,
            device_id: Some("test-device".to_string()),
        })
        .await;

        let shared = get_shared_conn().unwrap();
        let guard = shared.lock().unwrap();

        // Объект не должен быть в таблице objects
        let obj_count: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE id = ?1",
                rusqlite::params!["obj-ghost"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            obj_count, 0,
            "objects не должны содержать строку для obj-ghost после провального upsert"
        );

        // Version-vector не должен содержать запись для этого id
        let vv_raw = db::get_sync_kv(&guard, "lan_sync.version_vector").unwrap();
        if let Some(raw) = vv_raw {
            let vector: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(&raw).unwrap_or_default();
            assert!(
                !vector.contains_key("obj-ghost"),
                "version_vector не должен содержать запись для obj-ghost после провального upsert; \
                 vector: {:?}",
                vector
            );
        }
        // Если vv_raw == None — version_vector ещё не создавался, тест проходит
    }

    #[test]
    fn dotted_type_rpc_hits_real_handler_and_omitted_upsert_resolves_current() {
        let _guard = TEST_DB_MUTEX.blocking_lock();
        let path = std::env::temp_dir().join(format!("ark-phase2-rpc-{}.db", std::process::id()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            handle_request(Request::Init { db_path: path.to_string_lossy().into_owned() }).await.unwrap();
            let type_request: Request = serde_json::from_value(json!({
                "operation": "upsert_object_type",
                "object_type": {"id":"rpc-phase2", "name":"RPC", "schemaJson":"{}", "uiSchemaJson":"{}", "createdAt":"c", "updatedAt":"u", "systemLocked":false}
            })).unwrap();
            handle_request(type_request).await.unwrap();
            let object_request: Request = serde_json::from_value(json!({
                "operation": "upsert_object",
                "object": {"id":"rpc-object", "typeId":"rpc-phase2", "title":"x", "contentJson":{}, "propsJson":{}, "createdAt":"c", "updatedAt":"u", "deletedAt":null}
            })).unwrap();
            handle_request(object_request).await.unwrap();
            let stored = handle_request(Request::GetObject { id: "rpc-object".into() }).await.unwrap();
            assert!(stored["typeVersion"]
                .as_str()
                .is_some_and(|version| version.starts_with("0.0.0+legacy.")));
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
            set_event_sender(event_tx);
            let before = with_conn(|conn| {
                Ok((
                    conn.query_row("SELECT COUNT(*) FROM objects", [], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?,
                    db::get_sync_kv(conn, "lan_sync.version_vector")?,
                ))
            }).unwrap();
            let unknown_request: Request = serde_json::from_value(json!({
                "operation": "upsert_object",
                "object": {"id":"rpc-unknown", "typeId":"rpc-phase2", "typeVersion":"9.9.9", "title":"unknown", "contentJson":{}, "propsJson":{}, "createdAt":"c", "updatedAt":"u", "deletedAt":null}
            })).unwrap();
            assert!(handle_request(unknown_request).await.is_err());
            let after = with_conn(|conn| {
                Ok((
                    conn.query_row("SELECT COUNT(*) FROM objects", [], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?,
                    db::get_sync_kv(conn, "lan_sync.version_vector")?,
                ))
            }).unwrap();
            assert_eq!(before, after);
            assert!(!matches!(event_rx.try_recv(), Ok(event) if event["event"] == "object_upserted"));
            let result = handle_request(Request::TypesGet {
                type_id: "rpc-phase2".into(),
                version: None,
            })
            .await
            .unwrap();
            assert!(result["summary"].is_object());
            assert!(result["definition"].is_object());
            let alias = handle_request(Request::TypesResolveAlias { alias: "not-an-alias".into() }).await.unwrap();
            assert!(alias.is_null());
            let versions = handle_request(Request::TypesListVersions { type_id: "missing".into() }).await.unwrap();
            assert_eq!(versions, json!([]));
        });
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn dotted_type_rpc_handlers_cover_aliases_versions_nulls_and_ordering() {
        let _guard = TEST_DB_MUTEX.blocking_lock();
        let path =
            std::env::temp_dir().join(format!("ark-phase2-rpc-matrix-{}.db", std::process::id()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            handle_request(Request::Init {
                db_path: path.to_string_lossy().into_owned(),
            })
            .await
            .unwrap();
            for id in ["rpc-z", "rpc-a"] {
                handle_request(Request::UpsertObjectType {
                    object_type: ObjectType {
                        id: id.into(),
                        name: id.into(),
                        schema_json: "{}".into(),
                        ui_schema_json: "{}".into(),
                        created_at: "c".into(),
                        updated_at: "u".into(),
                        system_locked: false,
                    },
                    device_id: None,
                })
                .await
                .unwrap();
            }
            with_conn(|conn| {
                ark_core::type_registry::register_alias(
                    conn,
                    &ark_core::type_registry::AliasRecord {
                        alias: "rpc.alias".into(),
                        canonical_type_id: "rpc-a".into(),
                        created_at: "a".into(),
                    },
                )
            })
            .unwrap();
            let list = handle_request(Request::TypesList).await.unwrap();
            let list_ids = list
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["typeId"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert!(list_ids.windows(2).all(|w| w[0] <= w[1]));
            let alias_get = handle_request(Request::TypesGet {
                type_id: "rpc.alias".into(),
                version: None,
            })
            .await
            .unwrap();
            assert_eq!(alias_get["summary"]["typeId"], "rpc-a");
            let alias_versions = handle_request(Request::TypesListVersions {
                type_id: "rpc.alias".into(),
            })
            .await
            .unwrap();
            assert!(!alias_versions.as_array().unwrap().is_empty());
            assert!(handle_request(Request::TypesGet {
                type_id: "unknown".into(),
                version: None
            })
            .await
            .unwrap()
            .is_null());
            assert_eq!(
                handle_request(Request::TypesResolveAlias {
                    alias: "unknown".into()
                })
                .await
                .unwrap(),
                Value::Null
            );
            assert_eq!(
                handle_request(Request::TypesListVersions {
                    type_id: "unknown".into()
                })
                .await
                .unwrap(),
                json!([])
            );
            let exact_keys = alias_get
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                exact_keys.contains(&"summary".into()) && exact_keys.contains(&"definition".into())
            );
        });
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn dotted_type_operations_are_exact_and_underscore_aliases_rejected() {
        for operation in [
            "types.list",
            "types.get",
            "types.listVersions",
            "types.resolveAlias",
        ] {
            let mut value = json!({"operation": operation});
            if operation == "types.get" || operation == "types.listVersions" {
                value["typeId"] = json!("x");
            }
            if operation == "types.resolveAlias" {
                value["alias"] = json!("x");
            }
            assert!(
                serde_json::from_value::<Request>(value).is_ok(),
                "{operation}"
            );
        }
        for operation in [
            "types_list",
            "types_get",
            "types_list_versions",
            "types_resolve_alias",
        ] {
            assert!(
                serde_json::from_value::<Request>(json!({"operation": operation})).is_err(),
                "{operation}"
            );
        }
    }

    fn canonical_task_object(type_version: Option<&str>, props_json: Value) -> ArkObjectWrite {
        ArkObjectWrite {
            id: "canonical-task-ingress".to_string(),
            type_id: "com.kosmos.task".to_string(),
            type_version: type_version.map(str::to_owned),
            title: "Canonical task".to_string(),
            content_json: json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hello"}]}]}),
            props_json,
            created_at: "2026-08-11T00:00:00.000Z".to_string(),
            updated_at: "2026-08-11T00:00:00.000Z".to_string(),
            deleted_at: None,
        }
    }

    fn canonical_task_props() -> Value {
        json!({"status":"todo","priority":"medium","scheduledAt":null,"dueAt":null,"reminderAt":null,"completedAt":null,"canceledAt":null,"recurrence":null,"checklist":[],"extensions":{"vendor":{"opaque":true}}})
    }

    #[tokio::test]
    async fn canonical_upsert_object_rpc_persists_registered_identity() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();
        handle_request(Request::UpsertObject {
            object: canonical_task_object(Some("1.0.0"), canonical_task_props()),
            device_id: Some("device-canonical".to_string()),
        })
        .await
        .unwrap();
        let object = with_conn(|conn| db::get_object(conn, "canonical-task-ingress"))
            .unwrap()
            .unwrap();
        assert_eq!(object.type_id, "com.kosmos.task");
        assert_eq!(object.type_version, "1.0.0");
        assert_eq!(object.props_json["status"], "todo");
    }

    #[tokio::test]
    async fn canonical_upsert_object_rpc_rejects_omitted_version_without_side_effects() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();
        let result = handle_request(Request::UpsertObject {
            object: canonical_task_object(None, canonical_task_props()),
            device_id: Some("device-canonical".to_string()),
        })
        .await;
        assert_eq!(
            result.unwrap_err(),
            "canonical_ingress:invalid_request:canonical_version_required"
        );
        let object_count = with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM objects", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| error.to_string())
        })
        .unwrap();
        assert_eq!(object_count, 0);
    }

    #[tokio::test]
    async fn canonical_upsert_object_rpc_rejects_invalid_payload_without_sync_mutation() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();

        let mut props = canonical_task_props();
        props["unexpected"] = json!(true);
        let result = handle_request(Request::UpsertObject {
            object: canonical_task_object(Some("1.0.0"), props),
            device_id: Some("device-canonical".to_string()),
        })
        .await;
        assert_eq!(
            result.unwrap_err(),
            "canonical_ingress:invalid_request:canonical_field:/unexpected"
        );
        let (objects, versions) = with_conn(|conn| {
            let objects = conn
                .query_row("SELECT COUNT(*) FROM objects", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(|error| error.to_string())?;
            let versions = conn
                .query_row(
                    "SELECT COUNT(*) FROM sync_kv WHERE key = 'lan_sync.version_vector'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|error| error.to_string())?;
            Ok((objects, versions))
        })
        .unwrap();
        assert_eq!((objects, versions), (0, 0));
    }

    #[tokio::test]
    async fn canonical_upsert_object_rpc_rejects_legacy_alias_as_new_write() {
        let _guard = TEST_DB_MUTEX.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        handle_request(Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        })
        .await
        .unwrap();
        let mut object = canonical_task_object(Some("1.0.0"), canonical_task_props());
        object.type_id = "task_obj".to_string();
        let result = handle_request(Request::UpsertObject {
            object,
            device_id: None,
        })
        .await;
        assert_eq!(
            result.unwrap_err(),
            "canonical_ingress:invalid_request:legacy_alias_new_write"
        );
    }
}
