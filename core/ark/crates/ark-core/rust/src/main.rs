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
    ListObjectsByType {
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
) -> Result<(), String> {
    let device_id = local_write_device_id(device_id);
    db::bump_sync_version_vector(conn, entity_id, &device_id)?;
    db::delete_sync_tombstone(conn, entity_id)
}

fn record_local_delete(
    conn: &rusqlite::Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: Option<String>,
) -> Result<(), String> {
    let device_id = local_write_device_id(device_id);
    let hlc = db::bump_sync_version_vector(conn, entity_id, &device_id)?;
    db::record_sync_tombstone(conn, entity_type, entity_id, &hlc)
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
        Request::UpsertTodo { todo, device_id } => with_conn(|conn| {
            db::upsert_todo(conn, &todo)?;
            record_local_upsert(conn, "todo", &todo.id, device_id)?;
            Ok(json!(true))
        }),

        Request::DeleteTodo { id, device_id } => with_conn(|conn| {
            db::delete_todo(conn, &id)?;
            record_local_delete(conn, "todo", &id, device_id)?;
            Ok(json!(true))
        }),

        Request::BatchUpsertTodos { todos, device_id } => with_conn(|conn| {
            db::batch_upsert_todos(conn, &todos)?;
            for todo in &todos {
                record_local_upsert(conn, "todo", &todo.id, device_id.clone())?;
            }
            Ok(json!(true))
        }),

        Request::UpsertProject { project, device_id } => with_conn(|conn| {
            db::upsert_project(conn, &project)?;
            record_local_upsert(conn, "project", &project.id, device_id)?;
            Ok(json!(true))
        }),

        Request::DeleteProject { id, device_id } => with_conn(|conn| {
            db::delete_project(conn, &id)?;
            record_local_delete(conn, "project", &id, device_id)?;
            Ok(json!(true))
        }),

        Request::UpsertArea { area, device_id } => with_conn(|conn| {
            db::upsert_area(conn, &area)?;
            record_local_upsert(conn, "area", &area.id, device_id)?;
            Ok(json!(true))
        }),

        Request::UpsertTag { tag, device_id } => with_conn(|conn| {
            db::upsert_tag(conn, &tag)?;
            record_local_upsert(conn, "tag", &tag.id, device_id)?;
            Ok(json!(true))
        }),

        Request::UpsertHeading { heading, device_id } => with_conn(|conn| {
            db::upsert_heading(conn, &heading)?;
            record_local_upsert(conn, "heading", &heading.id, device_id)?;
            Ok(json!(true))
        }),

        Request::DeleteHeading { id, device_id } => with_conn(|conn| {
            db::delete_heading(conn, &id)?;
            record_local_delete(conn, "heading", &id, device_id)?;
            Ok(json!(true))
        }),

        Request::UpsertTrackedApp {
            tracked_app,
            device_id,
        } => with_conn(|conn| {
            db::upsert_tracked_app(conn, &tracked_app)?;
            record_local_upsert(conn, "tracked_app", &tracked_app.id, device_id)?;
            Ok(json!(true))
        }),

        Request::DeleteTrackedApp { id, device_id } => with_conn(|conn| {
            db::delete_tracked_app(conn, &id)?;
            record_local_delete(conn, "tracked_app", &id, device_id)?;
            Ok(json!(true))
        }),

        Request::UpsertUsageSession {
            usage_session,
            device_id,
        } => with_conn(|conn| {
            db::upsert_usage_session(conn, &usage_session)?;
            record_local_upsert(conn, "usage_session", &usage_session.id, device_id)?;
            Ok(json!(true))
        }),

        Request::DeleteUsageSession { id, device_id } => with_conn(|conn| {
            db::delete_usage_session(conn, &id)?;
            record_local_delete(conn, "usage_session", &id, device_id)?;
            Ok(json!(true))
        }),

        Request::UpsertUsageEvent {
            usage_event,
            device_id,
        } => with_conn(|conn| {
            db::upsert_usage_event(conn, &usage_event)?;
            record_local_upsert(conn, "usage_event", &usage_event.id, device_id)?;
            Ok(json!(true))
        }),

        Request::DeleteUsageEvent { id, device_id } => with_conn(|conn| {
            db::delete_usage_event(conn, &id)?;
            record_local_delete(conn, "usage_event", &id, device_id)?;
            Ok(json!(true))
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
        Request::ListObjectsByType { type_id } => with_conn(|conn| {
            let objects = db::list_objects_by_type(conn, &type_id)?;
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
            let result = with_conn(|conn| {
                db::upsert_object(conn, &object)?;
                record_local_upsert(conn, "object", &object.id, device_id)?;
                Ok(json!(true))
            });
            // Emit ТОЛЬКО на успешный local write. `set_on_change` (sync_server)
            // эмитит entity_changed на incoming peer-write — это другой код path,
            // не дублирует это событие. Cross-app live updates (Eden TaskRef
            // подписан на object_upserted) — это primary consumer.
            if result.is_ok() {
                emit_event(json!({
                    "event": "object_upserted",
                    "id": object_id,
                    "type_id": object_type_id,
                }));
            }
            result
        }
        Request::DeleteObject { id, device_id } => {
            let object_id = id.clone();
            let result = with_conn(|conn| {
                db::delete_object(conn, &id)?;
                record_local_delete(conn, "object", &id, device_id)?;
                Ok(json!(true))
            });
            if result.is_ok() {
                emit_event(json!({
                    "event": "object_deleted",
                    "id": object_id,
                }));
            }
            result
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
        } => with_conn(|conn| {
            db::upsert_object_type(conn, &object_type)?;
            record_local_upsert(conn, "object_type", &object_type.id, device_id)?;
            Ok(json!(true))
        }),
        Request::DeleteObjectType { id, device_id } => with_conn(|conn| {
            db::delete_object_type(conn, &id)?;
            record_local_delete(conn, "object_type", &id, device_id)?;
            Ok(json!(true))
        }),
        Request::ListObjectLinks => with_conn(|conn| {
            let object_links = db::list_object_links(conn)?;
            serde_json::to_value(object_links).map_err(|e| e.to_string())
        }),
        Request::UpsertObjectLink {
            object_link,
            device_id,
        } => with_conn(|conn| {
            db::upsert_object_link(conn, &object_link)?;
            record_local_upsert(conn, "object_link", &object_link.id, device_id)?;
            Ok(json!(true))
        }),
        Request::DeleteObjectLink { id, device_id } => with_conn(|conn| {
            db::delete_object_link(conn, &id)?;
            record_local_delete(conn, "object_link", &id, device_id)?;
            Ok(json!(true))
        }),

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
    }
}

// ---------------------------------------------------------------------------
// Sync handlers
// ---------------------------------------------------------------------------

async fn handle_start_sync(
    space_id: String,
    device_id: String,
    device_name: Option<String>,
    port: Option<u16>,
    seed_addresses: Option<Vec<String>>,
    relay_url: Option<String>,
    relay_api_key: Option<String>,
    auth_secret: Option<String>,
) -> Result<Value, String> {
    // Idempotency: tear down any running runtime first.
    handle_stop_sync().await;

    let device_name = device_name.unwrap_or_else(get_host_device_name);
    let ws_port = port.unwrap_or(LAN_SYNC_PORT);

    let shared_conn = get_shared_conn()?;
    let storage = Arc::new(SqliteStorageBackend::new(shared_conn));
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

    let relay = if let Some(relay_url) = relay_url.clone() {
        let relay_sync = RelaySync::new(
            storage.clone() as Arc<dyn StorageBackend>,
            RelaySyncConfig {
                relay_url,
                relay_api_key: relay_api_key.clone(),
                space_id: space_id.clone(),
                device_id: device_id.clone(),
                device_name: device_name.clone(),
                auth_secret: auth_secret.clone(),
            },
        );
        relay_sync
            .set_on_change(Arc::new(|entity| {
                emit_event(json!({
                    "event": "entity_changed",
                    "entity": entity,
                }));
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
        relay_sync.start().await?;
        Some(relay_sync)
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
}
