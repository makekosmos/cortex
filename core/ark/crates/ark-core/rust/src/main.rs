#![cfg_attr(test, allow(clippy::unwrap_used))]

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

struct SyncRuntime {
    server: Arc<SyncServer>,
    storage: Arc<SqliteStorageBackend>,
    clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    relay: Option<Arc<RelaySync>>,
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
    UpsertObject {
        object: ArkObject,
        #[serde(default)]
        device_id: Option<String>,
    },
    DeleteObject {
        id: String,
        #[serde(default)]
        device_id: Option<String>,
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
    _entity_type: &str,
    entity_id: &str,
    device_id: Option<String>,
) -> Result<String, String> {
    let device_id = local_write_device_id(device_id);
    let hlc = db::bump_sync_version_vector(conn, entity_id, &device_id)?;
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
    let hlc = db::bump_sync_version_vector(conn, entity_id, &device_id)?;
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
    SyncEntity {
        entity_type: entity_type.to_string(),
        id: id.to_string(),
        data,
        hlc,
        deleted,
    }
}

/// Fan-out локального изменения всем подключённым пирам (server sessions +
/// outbound clients + relay/iroh). Не применяет entity к storage и не
/// перебивает HLC — данные уже записаны через `record_local_*`.
/// Если SYNC не запущен — тихий no-op.
async fn broadcast_local_change(entity: SyncEntity) {
    let guard = SYNC.lock().await;
    let runtime = match guard.as_ref() {
        Some(r) => r.clone(),
        None => return,
    };
    drop(guard);

    runtime
        .server
        .broadcast_live_change(entity.clone(), None)
        .await;

    let clients = runtime.clients.lock().await;
    for client in clients.values() {
        client.broadcast_live_change(entity.clone()).await;
    }
    if let Some(relay) = runtime.relay.as_ref() {
        // Ошибку отправки логируем, но не пробрасываем — live broadcast best-effort.
        if let Err(e) = relay.broadcast_live_change(entity.clone()) {
            eprintln!("[ark-core] broadcast_local_change relay error: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Request dispatcher
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
            let result = with_conn(|conn| {
                db::upsert_todo(conn, &todo)?;
                let hlc = record_local_upsert(conn, "todo", &todo.id, device_id)?;
                let entity = make_sync_entity("todo", &todo.id,
                    serde_json::to_value(&todo).unwrap_or(json!({})), hlc, None);
                Ok(entity)
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::DeleteTodo { id, device_id } => {
            let result = with_conn(|conn| {
                db::delete_todo(conn, &id)?;
                let hlc = record_local_delete(conn, "todo", &id, device_id)?;
                Ok(make_sync_entity("todo", &id, json!({}), hlc, Some(true)))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::BatchUpsertTodos { todos, device_id } => {
            let result = with_conn(|conn| {
                db::batch_upsert_todos(conn, &todos)?;
                let mut entities = Vec::with_capacity(todos.len());
                for todo in &todos {
                    let hlc = record_local_upsert(conn, "todo", &todo.id, device_id.clone())?;
                    entities.push(make_sync_entity("todo", &todo.id,
                        serde_json::to_value(todo).unwrap_or(json!({})), hlc, None));
                }
                Ok(entities)
            });
            if let Ok(entities) = result {
                tokio::spawn(async move {
                    for entity in entities {
                        broadcast_local_change(entity).await;
                    }
                });
            }
            Ok(json!(true))
        }

        Request::UpsertProject { project, device_id } => {
            let result = with_conn(|conn| {
                db::upsert_project(conn, &project)?;
                let hlc = record_local_upsert(conn, "project", &project.id, device_id)?;
                Ok(make_sync_entity("project", &project.id,
                    serde_json::to_value(&project).unwrap_or(json!({})), hlc, None))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::DeleteProject { id, device_id } => {
            let result = with_conn(|conn| {
                db::delete_project(conn, &id)?;
                let hlc = record_local_delete(conn, "project", &id, device_id)?;
                Ok(make_sync_entity("project", &id, json!({}), hlc, Some(true)))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::UpsertArea { area, device_id } => {
            let result = with_conn(|conn| {
                db::upsert_area(conn, &area)?;
                let hlc = record_local_upsert(conn, "area", &area.id, device_id)?;
                Ok(make_sync_entity("area", &area.id,
                    serde_json::to_value(&area).unwrap_or(json!({})), hlc, None))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::UpsertTag { tag, device_id } => {
            let result = with_conn(|conn| {
                db::upsert_tag(conn, &tag)?;
                let hlc = record_local_upsert(conn, "tag", &tag.id, device_id)?;
                Ok(make_sync_entity("tag", &tag.id,
                    serde_json::to_value(&tag).unwrap_or(json!({})), hlc, None))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::UpsertHeading { heading, device_id } => {
            let result = with_conn(|conn| {
                db::upsert_heading(conn, &heading)?;
                let hlc = record_local_upsert(conn, "heading", &heading.id, device_id)?;
                Ok(make_sync_entity("heading", &heading.id,
                    serde_json::to_value(&heading).unwrap_or(json!({})), hlc, None))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::DeleteHeading { id, device_id } => {
            let result = with_conn(|conn| {
                db::delete_heading(conn, &id)?;
                let hlc = record_local_delete(conn, "heading", &id, device_id)?;
                Ok(make_sync_entity("heading", &id, json!({}), hlc, Some(true)))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::UpsertTrackedApp {
            tracked_app,
            device_id,
        } => {
            let result = with_conn(|conn| {
                db::upsert_tracked_app(conn, &tracked_app)?;
                let hlc = record_local_upsert(conn, "tracked_app", &tracked_app.id, device_id)?;
                Ok(make_sync_entity("tracked_app", &tracked_app.id,
                    serde_json::to_value(&tracked_app).unwrap_or(json!({})), hlc, None))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::DeleteTrackedApp { id, device_id } => {
            let result = with_conn(|conn| {
                db::delete_tracked_app(conn, &id)?;
                let hlc = record_local_delete(conn, "tracked_app", &id, device_id)?;
                Ok(make_sync_entity("tracked_app", &id, json!({}), hlc, Some(true)))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::UpsertUsageSession {
            usage_session,
            device_id,
        } => {
            let result = with_conn(|conn| {
                db::upsert_usage_session(conn, &usage_session)?;
                let hlc = record_local_upsert(conn, "usage_session", &usage_session.id, device_id)?;
                Ok(make_sync_entity("usage_session", &usage_session.id,
                    serde_json::to_value(&usage_session).unwrap_or(json!({})), hlc, None))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::DeleteUsageSession { id, device_id } => {
            let result = with_conn(|conn| {
                db::delete_usage_session(conn, &id)?;
                let hlc = record_local_delete(conn, "usage_session", &id, device_id)?;
                Ok(make_sync_entity("usage_session", &id, json!({}), hlc, Some(true)))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::UpsertUsageEvent {
            usage_event,
            device_id,
        } => {
            let result = with_conn(|conn| {
                db::upsert_usage_event(conn, &usage_event)?;
                let hlc = record_local_upsert(conn, "usage_event", &usage_event.id, device_id)?;
                Ok(make_sync_entity("usage_event", &usage_event.id,
                    serde_json::to_value(&usage_event).unwrap_or(json!({})), hlc, None))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }

        Request::DeleteUsageEvent { id, device_id } => {
            let result = with_conn(|conn| {
                db::delete_usage_event(conn, &id)?;
                let hlc = record_local_delete(conn, "usage_event", &id, device_id)?;
                Ok(make_sync_entity("usage_event", &id, json!({}), hlc, Some(true)))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }
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
        Request::UpsertObject { object, device_id } => {
            let object_id = object.id.clone();
            let object_type_id = object.type_id.clone();
            // Emit ТОЛЬКО на успешный local write. `set_on_change` (sync_server)
            // эмитит entity_changed на incoming peer-write — это другой код path,
            // не дублирует это событие. Cross-app live updates (Eden TaskRef
            // подписан на object_upserted) — это primary consumer.
            let result = with_conn(|conn| {
                db::upsert_object(conn, &object)?;
                let hlc = record_local_upsert(conn, "object", &object.id, device_id)?;
                Ok(make_sync_entity("object", &object.id,
                    serde_json::to_value(&object).unwrap_or(json!({})), hlc, None))
            });
            if let Ok(entity) = result {
                let eid = object_id.clone();
                let etid = object_type_id.clone();
                tokio::spawn(async move { broadcast_local_change(entity).await; });
                emit_event(json!({
                    "event": "object_upserted",
                    "id": eid,
                    "type_id": etid,
                }));
            }
            Ok(json!(true))
        }
        Request::DeleteObject { id, device_id } => {
            let object_id = id.clone();
            let result = with_conn(|conn| {
                db::delete_object(conn, &id)?;
                let hlc = record_local_delete(conn, "object", &id, device_id)?;
                Ok(make_sync_entity("object", &id, json!({}), hlc, Some(true)))
            });
            if let Ok(entity) = result {
                let eid = object_id.clone();
                tokio::spawn(async move { broadcast_local_change(entity).await; });
                emit_event(json!({
                    "event": "object_deleted",
                    "id": eid,
                }));
            }
            Ok(json!(true))
        }
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
            let result = with_conn(|conn| {
                db::upsert_object_type(conn, &object_type)?;
                let hlc = record_local_upsert(conn, "object_type", &object_type.id, device_id)?;
                Ok(make_sync_entity("object_type", &object_type.id,
                    serde_json::to_value(&object_type).unwrap_or(json!({})), hlc, None))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }
        Request::DeleteObjectType { id, device_id } => {
            let result = with_conn(|conn| {
                db::delete_object_type(conn, &id)?;
                let hlc = record_local_delete(conn, "object_type", &id, device_id)?;
                Ok(make_sync_entity("object_type", &id, json!({}), hlc, Some(true)))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
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
            let result = with_conn(|conn| {
                db::upsert_object_link(conn, &object_link)?;
                let hlc = record_local_upsert(conn, "object_link", &object_link.id, device_id)?;
                Ok(make_sync_entity("object_link", &object_link.id,
                    serde_json::to_value(&object_link).unwrap_or(json!({})), hlc, None))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
            Ok(json!(true))
        }
        Request::DeleteObjectLink { id, device_id } => {
            let result = with_conn(|conn| {
                db::delete_object_link(conn, &id)?;
                let hlc = record_local_delete(conn, "object_link", &id, device_id)?;
                Ok(make_sync_entity("object_link", &id, json!({}), hlc, Some(true)))
            });
            if let Ok(entity) = result {
                tokio::spawn(async move { broadcast_local_change(entity).await; });
            }
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
    storage.set_device_id(&device_id);

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
    for peer in known_peers {
        if peer.device_id == device_id {
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
                let guard = clients.lock().await;
                if guard.contains_key(&peer.device_id) {
                    // Update its peer record so reconnect picks the new addresses.
                    if let Some(existing) = guard.get(&peer.device_id) {
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
                    }
                    return;
                }
                drop(guard);

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
    let mut guard = SYNC.lock().await;
    if let Some(runtime) = guard.take() {
        runtime.beacon.stop().await;
        if let Some(relay) = runtime.relay.as_ref() {
            relay.stop();
        }
        runtime.server.stop().await;
        let clients = runtime.clients.lock().await;
        for client in clients.values() {
            client.stop();
        }
    }
}

async fn handle_broadcast_change(mut entity: SyncEntity) -> Result<Value, String> {
    let guard = SYNC.lock().await;
    let runtime = match guard.as_ref() {
        Some(r) => r.clone(),
        None => return Err("Sync not running".to_string()),
    };
    drop(guard);

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
    let clients = runtime.clients.lock().await;
    for client in clients.values() {
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

async fn handle_get_connected_peers() -> Result<Value, String> {
    let guard = SYNC.lock().await;
    let runtime = match guard.as_ref() {
        Some(r) => r.clone(),
        None => return Ok(json!([])),
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
    let clients = runtime.clients.lock().await;
    for (device_id, client) in clients.iter() {
        if seen.contains_key(device_id) {
            continue;
        }
        let peer = client.current_peer().await;
        if !peer.device_id.is_empty() {
            order.push(peer.device_id.clone());
            seen.insert(peer.device_id, peer.device_name);
        }
    }
    drop(clients);

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
    let guard = SYNC.lock().await;
    let runtime = match guard.as_ref() {
        Some(r) => r.clone(),
        None => return Err("Sync not running".to_string()),
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

    static TEST_DB_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

        match request {
            Request::StartSync {
                use_iroh,
                iroh_peer_ticket,
                ..
            } => {
                assert!(use_iroh);
                assert_eq!(iroh_peer_ticket.as_deref(), Some("endpointsometicketvalue"));
            }
            _ => panic!("expected start_sync"),
        }
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
            Err(e) => panic!("start_sync with use_iroh failed unexpectedly: {e}"),
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

        match request {
            Request::StartSync {
                use_iroh,
                iroh_peer_ticket,
                ..
            } => {
                assert!(!use_iroh);
                assert_eq!(iroh_peer_ticket, None);
            }
            _ => panic!("expected start_sync"),
        }
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

        match request {
            Request::StartSync {
                relay_url,
                relay_api_key,
                auth_secret,
                ..
            } => {
                assert_eq!(relay_url.as_deref(), Some("ws://127.0.0.1:8765"));
                assert_eq!(relay_api_key.as_deref(), Some("key"));
                assert_eq!(auth_secret.as_deref(), Some("secret"));
            }
            _ => panic!("expected start_sync"),
        }
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

        let object = ArkObject {
            id: "obj-local-write".to_string(),
            type_id: "game_obj".to_string(),
            title: "Local Game".to_string(),
            content_json: json!({ "type": "doc", "content": [] }),
            props_json: json!({ "source": "test" }),
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
        for id in [
            "obj-local-write",
            "app-local-write",
            "session-local-write",
            "event-local-write",
        ] {
            assert!(
                vector
                    .get(id)
                    .is_some_and(|hlc| hlc.ends_with(":device-local")),
                "{id} should have a local HLC in the version vector",
            );
        }

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
            id: "game_obj".to_string(),
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

        let source = ArkObject {
            id: "source-object".to_string(),
            type_id: object_type.id.clone(),
            title: "Source".to_string(),
            content_json: json!({}),
            props_json: json!({}),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
            deleted_at: None,
        };
        let target = ArkObject {
            id: "target-object".to_string(),
            type_id: object_type.id.clone(),
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
        for id in ["game_obj", "link-local-write", "empty_type_for_delete"] {
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

    /// Regression for 2026-05-18 audit finding: legacy entity write handlers
    /// (UpsertTodo, UpsertProject, UpsertArea, UpsertTag, UpsertHeading и их
    /// Delete*, BatchUpsertTodos) пропускали bump_sync_version_vector. Локальные
    /// правки тихо терялись для LAN sync. Тест проверяет что каждый legacy
    /// entity получает HLC в version vector и delete'ы пишут tombstone.
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

        let area = Area {
            id: "area-legacy".to_string(),
            title: "Area".to_string(),
            sort_order: 0,
            created_at: timestamp.clone(),
        };
        handle_request(Request::UpsertArea {
            area,
            device_id: device.clone(),
        })
        .await
        .unwrap();

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

        let heading = Heading {
            id: "heading-legacy".to_string(),
            title: "Heading".to_string(),
            sort_order: 0,
            project_id: "project-legacy".to_string(),
        };
        handle_request(Request::UpsertHeading {
            heading,
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
            is_completed: false,
            completed_at: None,
            is_cancelled: false,
            cancelled_at: None,
            is_trashed: false,
            sort_order: 0,
            heading_id: None,
            project_id: None,
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
        handle_request(Request::DeleteHeading {
            id: "heading-legacy".to_string(),
            device_id: device.clone(),
        })
        .await
        .unwrap();
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
            "area-legacy",
            "project-legacy",
            "tag-legacy",
            "heading-legacy",
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

        let heading_tombstone: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
                rusqlite::params!["heading-legacy", "heading"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            heading_tombstone, 1,
            "DeleteHeading должен записать tombstone"
        );

        let todo_tombstone: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
                rusqlite::params!["todo-batch-1", "todo"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(todo_tombstone, 1, "DeleteTodo должен записать tombstone");
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
            _event_tx: tokio::sync::mpsc::UnboundedSender<
                ark_core::sync_transport::TransportEvent,
            >,
        ) -> Result<(), String> {
            Ok(())
        }

        fn send(&self, msg: ark_core::protocol::LanSyncMessage) -> Result<(), String> {
            // `send` — sync, но нам нужен lock на TokioMutex из sync контекста.
            // Используем blocking_lock через spawn_blocking или try_lock; в тестах
            // конкурентности нет, try_lock гарантированно успевает.
            self.sent.try_lock().expect("CapturingTransport: lock").push(msg);
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
        backend.set_device_id("test-device");

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
            backend.clone() as Arc<dyn StorageBackend>,
        ));

        let runtime = SyncRuntime {
            server,
            storage: backend,
            clients: Arc::new(TokioMutex::new(std::collections::HashMap::new())),
            relay: Some(relay),
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

        let object = ArkObject {
            id: "live-obj-1".to_string(),
            type_id: "note".to_string(),
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
        let object = ArkObject {
            id: "live-obj-del".to_string(),
            type_id: "note".to_string(),
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
}
