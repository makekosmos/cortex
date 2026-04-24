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
use ark_core::host::{get_host_device_name, get_own_addresses};
use ark_core::net::is_address_routable;
use ark_core::protocol::LAN_SYNC_PORT;
use ark_core::sync_client::SyncClient;
use ark_core::sync_server::{StorageBackend, SyncServer};
use ark_core::types::*;

// ---------------------------------------------------------------------------
// Global state — one SQLite connection plus (optionally) one sync runtime.
// ---------------------------------------------------------------------------

static DB: StdMutex<Option<Arc<StdMutex<rusqlite::Connection>>>> = StdMutex::new(None);

struct SyncRuntime {
    server: Arc<SyncServer>,
    storage: Arc<SqliteStorageBackend>,
    clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    beacon: Arc<BroadcastDiscovery>,
    space_id: String,
    device_id: String,
    device_name: String,
    own_addresses: Arc<TokioMutex<Vec<String>>>,
}

static SYNC: TokioMutex<Option<Arc<SyncRuntime>>> = TokioMutex::const_new(None);

// The event sender is set once at startup; every callback forwards events
// into this channel. The stdout writer task drains the channel and writes
// JSON lines. Stored under a std Mutex so callbacks that are not in the
// async context can still clone it.
static EVENT_TX: StdMutex<Option<mpsc::UnboundedSender<Value>>> = StdMutex::new(None);

fn emit_event(event: Value) {
    if let Ok(guard) = EVENT_TX.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(event);
        }
    }
}

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
    },
    DeleteTodo {
        id: String,
    },
    BatchUpsertTodos {
        todos: Vec<TodoItem>,
    },
    UpsertProject {
        project: Project,
    },
    DeleteProject {
        id: String,
    },
    UpsertArea {
        area: Area,
    },
    UpsertTag {
        tag: Tag,
    },
    UpsertHeading {
        heading: Heading,
    },
    DeleteHeading {
        id: String,
    },
    UpsertTrackedApp {
        tracked_app: TrackedApp,
    },
    DeleteTrackedApp {
        id: String,
    },
    UpsertUsageSession {
        usage_session: UsageSession,
    },
    DeleteUsageSession {
        id: String,
    },
    UpsertUsageEvent {
        usage_event: UsageEvent,
    },
    DeleteUsageEvent {
        id: String,
    },
    ListObjects,
    SearchObjects {
        query: String,
    },
    GetObject {
        id: String,
    },
    UpsertObject {
        object: ArkObject,
    },
    DeleteObject {
        id: String,
    },
    ListObjectTypes,
    GetObjectType {
        id: String,
    },
    UpsertObjectType {
        object_type: ObjectType,
    },
    DeleteObjectType {
        id: String,
    },
    ListObjectLinks,
    UpsertObjectLink {
        object_link: ObjectLink,
    },
    DeleteObjectLink {
        id: String,
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
    *EVENT_TX.lock().unwrap() = Some(event_tx);

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
        let response = match serde_json::from_str::<Request>(trimmed) {
            Ok(req) => match handle_request(req).await {
                Ok(data) => json!({ "ok": true, "data": data }),
                Err(e) => json!({ "ok": false, "error": e }),
            },
            Err(e) => json!({ "ok": false, "error": e.to_string() }),
        };
        write_response_line(&response);
    }
    Ok(())
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
    let _guard = STDOUT_LOCK.lock().unwrap();
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
    let outer = DB.lock().unwrap();
    let shared = outer
        .as_ref()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())?
        .clone();
    drop(outer);
    let inner = shared.lock().unwrap();
    f(&inner)
}

fn get_shared_conn() -> Result<Arc<StdMutex<rusqlite::Connection>>, String> {
    let guard = DB.lock().unwrap();
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())
}

// ---------------------------------------------------------------------------
// Request dispatcher
// ---------------------------------------------------------------------------

async fn handle_request(request: Request) -> Result<Value, String> {
    match request {
        Request::Init { db_path } => {
            let conn = db::open_db(&db_path)?;
            db::init_schema(&conn)?;
            *DB.lock().unwrap() = Some(Arc::new(StdMutex::new(conn)));
            Ok(json!(true))
        }

        Request::LoadAll => with_conn(|conn| {
            let data = db::load_all(conn)?;
            serde_json::to_value(data).map_err(|e| e.to_string())
        }),

        Request::UpsertTodo { todo } => with_conn(|conn| {
            db::upsert_todo(conn, &todo)?;
            Ok(json!(true))
        }),

        Request::DeleteTodo { id } => with_conn(|conn| {
            db::delete_todo(conn, &id)?;
            Ok(json!(true))
        }),

        Request::BatchUpsertTodos { todos } => with_conn(|conn| {
            db::batch_upsert_todos(conn, &todos)?;
            Ok(json!(true))
        }),

        Request::UpsertProject { project } => with_conn(|conn| {
            db::upsert_project(conn, &project)?;
            Ok(json!(true))
        }),

        Request::DeleteProject { id } => with_conn(|conn| {
            db::delete_project(conn, &id)?;
            Ok(json!(true))
        }),

        Request::UpsertArea { area } => with_conn(|conn| {
            db::upsert_area(conn, &area)?;
            Ok(json!(true))
        }),

        Request::UpsertTag { tag } => with_conn(|conn| {
            db::upsert_tag(conn, &tag)?;
            Ok(json!(true))
        }),

        Request::UpsertHeading { heading } => with_conn(|conn| {
            db::upsert_heading(conn, &heading)?;
            Ok(json!(true))
        }),

        Request::DeleteHeading { id } => with_conn(|conn| {
            db::delete_heading(conn, &id)?;
            Ok(json!(true))
        }),

        Request::UpsertTrackedApp { tracked_app } => with_conn(|conn| {
            db::upsert_tracked_app(conn, &tracked_app)?;
            Ok(json!(true))
        }),

        Request::DeleteTrackedApp { id } => with_conn(|conn| {
            db::delete_tracked_app(conn, &id)?;
            Ok(json!(true))
        }),

        Request::UpsertUsageSession { usage_session } => with_conn(|conn| {
            db::upsert_usage_session(conn, &usage_session)?;
            Ok(json!(true))
        }),

        Request::DeleteUsageSession { id } => with_conn(|conn| {
            db::delete_usage_session(conn, &id)?;
            Ok(json!(true))
        }),

        Request::UpsertUsageEvent { usage_event } => with_conn(|conn| {
            db::upsert_usage_event(conn, &usage_event)?;
            Ok(json!(true))
        }),

        Request::DeleteUsageEvent { id } => with_conn(|conn| {
            db::delete_usage_event(conn, &id)?;
            Ok(json!(true))
        }),
        Request::ListObjects => with_conn(|conn| {
            let objects = db::list_objects(conn)?;
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
        Request::UpsertObject { object } => with_conn(|conn| {
            db::upsert_object(conn, &object)?;
            Ok(json!(true))
        }),
        Request::DeleteObject { id } => with_conn(|conn| {
            db::delete_object(conn, &id)?;
            Ok(json!(true))
        }),
        Request::ListObjectTypes => with_conn(|conn| {
            let object_types = db::list_object_types(conn)?;
            serde_json::to_value(object_types).map_err(|e| e.to_string())
        }),
        Request::GetObjectType { id } => with_conn(|conn| {
            let object_type = db::get_object_type(conn, &id)?;
            serde_json::to_value(object_type).map_err(|e| e.to_string())
        }),
        Request::UpsertObjectType { object_type } => with_conn(|conn| {
            db::upsert_object_type(conn, &object_type)?;
            Ok(json!(true))
        }),
        Request::DeleteObjectType { id } => with_conn(|conn| {
            db::delete_object_type(conn, &id)?;
            Ok(json!(true))
        }),
        Request::ListObjectLinks => with_conn(|conn| {
            let object_links = db::list_object_links(conn)?;
            serde_json::to_value(object_links).map_err(|e| e.to_string())
        }),
        Request::UpsertObjectLink { object_link } => with_conn(|conn| {
            db::upsert_object_link(conn, &object_link)?;
            Ok(json!(true))
        }),
        Request::DeleteObjectLink { id } => with_conn(|conn| {
            db::delete_object_link(conn, &id)?;
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

        Request::StartSync {
            space_id,
            device_id,
            device_name,
            port,
            seed_addresses,
            relay_url: _,
            relay_api_key: _,
        } => handle_start_sync(space_id, device_id, device_name, port, seed_addresses).await,

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
) -> Result<Value, String> {
    // Idempotency: tear down any running runtime first.
    handle_stop_sync().await;

    let device_name = device_name.unwrap_or_else(get_host_device_name);
    let ws_port = port.unwrap_or(LAN_SYNC_PORT);

    let shared_conn = get_shared_conn()?;
    let storage = Arc::new(SqliteStorageBackend::new(shared_conn));
    storage.set_device_id(&device_id);

    let server = Arc::new(SyncServer::new(storage.clone() as Arc<dyn StorageBackend>));

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
        beacon: beacon_clone,
        space_id,
        device_id,
        device_name,
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
    )
    .await;
}

async fn handle_stop_sync() {
    let mut guard = SYNC.lock().await;
    if let Some(runtime) = guard.take() {
        runtime.beacon.stop().await;
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
    runtime.storage.apply_entity(&entity).await;

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
    )
    .await;
    Ok(json!(true))
}
