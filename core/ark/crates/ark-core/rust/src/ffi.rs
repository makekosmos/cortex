//! UniFFI-exposed facade used by Android (and any future Kotlin/Swift
//! embedders). Wraps the same DB + sync runtime the `ark-core-rpc` binary
//! drives, plus a callback-interface listener for async events.
//!
//! Design notes:
//!   - Each UniFFI-exposed struct holds its own tokio runtime. Callbacks run
//!     on a dedicated thread so they can invoke the Kotlin listener without
//!     blocking the sync engine.
//!   - Errors are flattened to `ArkCoreError::Generic(String)` — the binding
//!     layer does not need structured error variants right now.
//!   - Types that cross the FFI boundary are plain JSON strings for
//!     entities and simple owned strings/ints everywhere else. UniFFI
//!     callback interfaces only support primitive / String / owned struct
//!     parameters, so the `ArkEventListener` trait carries JSON payloads
//!     for entity changes to keep the FFI surface stable across protocol
//!     tweaks.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use rusqlite::Connection;
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex as TokioMutex, RwLock};

/// Shutdown signal sender kept alive while sync is running.
type ShutdownTx = tokio::sync::oneshot::Sender<()>;

use crate::beacon::{BeaconPeer, BroadcastDiscovery, BroadcastDiscoveryOptions};
use crate::db::{
    batch_upsert_todos as db_batch_upsert_todos, clear_all as db_clear_all,
    delete_heading as db_delete_heading, delete_project as db_delete_project,
    delete_todo as db_delete_todo, delete_tracked_app as db_delete_tracked_app,
    delete_trashed as db_delete_trashed, delete_usage_event as db_delete_usage_event,
    delete_usage_session as db_delete_usage_session, get_sync_kv as db_get_kv, init_schema,
    load_all as db_load_all, open_db, set_sync_kv as db_set_kv, upsert_area as db_upsert_area,
    upsert_heading as db_upsert_heading, upsert_project as db_upsert_project,
    upsert_tag as db_upsert_tag, upsert_todo as db_upsert_todo,
    upsert_tracked_app as db_upsert_tracked_app, upsert_usage_event as db_upsert_usage_event,
    upsert_usage_session as db_upsert_usage_session, SqliteStorageBackend,
};
use crate::host::{get_host_device_name, get_own_addresses};
use crate::net::is_address_routable;
use crate::protocol::LAN_SYNC_PORT;
use crate::relay_sync::{RelaySync, RelaySyncConfig};
use crate::sync_client::SyncClient;
use crate::sync_server::{StorageBackend, SyncServer};
use crate::types::{
    Area, Heading, LoadAllData, PeerRecord, Project, SyncEntity, Tag, TodoItem, TrackedApp,
    UsageEvent, UsageSession,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ArkCoreError {
    #[error("{0}")]
    Generic(String),
}

impl From<String> for ArkCoreError {
    fn from(value: String) -> Self {
        Self::Generic(value)
    }
}

impl From<&str> for ArkCoreError {
    fn from(value: &str) -> Self {
        Self::Generic(value.to_string())
    }
}

type Result<T> = std::result::Result<T, ArkCoreError>;

fn err<S: Into<String>>(msg: S) -> ArkCoreError {
    ArkCoreError::Generic(msg.into())
}

// ---------------------------------------------------------------------------
// Plain records that cross the FFI boundary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiConnectedPeer {
    pub device_id: String,
    pub device_name: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiSyncConfig {
    pub space_id: String,
    pub device_id: String,
    pub device_name: Option<String>,
    pub port: Option<u32>,
    pub db_path: Option<String>,
    pub seed_addresses: Vec<String>,
    /// Optional relay WebSocket URL (e.g. "wss://relay.example.com").
    /// Defaults to None — existing callers need no changes.
    #[uniffi(default = None)]
    pub relay_url: Option<String>,
    /// API key for the relay server. Required when relay_url is Some.
    #[uniffi(default = None)]
    pub relay_api_key: Option<String>,
    /// Optional shared secret for LAN/P2P hello HMAC authentication.
    #[uniffi(default = None)]
    pub auth_secret: Option<String>,
    /// Step 4a: select the iroh p2p transport instead of relay. Present on
    /// the UniFFI surface in every build (stable struct shape); only acted
    /// on behind `#[cfg(feature = "iroh-spike")]` in `start_sync` — a build
    /// without the feature returns an error if this is `true` instead of
    /// silently falling back to relay/LAN.
    #[uniffi(default = false)]
    pub use_iroh: bool,
    /// Pairing ticket string for the iroh peer (see
    /// `iroh_transport::IrohTransport::our_ticket`/`from_ticket`).
    #[uniffi(default = None)]
    pub iroh_peer_ticket: Option<String>,
}

// ---------------------------------------------------------------------------
// Callback interface — Kotlin implements this and passes it at start_sync
// ---------------------------------------------------------------------------

#[uniffi::export(callback_interface)]
pub trait ArkEventListener: Send + Sync {
    /// Called once for every inbound entity change (sync batch or live).
    /// `entity_json` is the serialised `SyncEntity` (snake_case protocol form)
    /// so the Kotlin side can parse it without re-exporting the full struct.
    fn on_entity_changed(&self, entity_json: String);

    /// A new peer authenticated. `device_name` may be empty if the peer did
    /// not advertise a name.
    fn on_peer_connected(&self, device_id: String, device_name: String);

    /// A previously-authenticated peer disconnected.
    fn on_peer_disconnected(&self, device_id: String, remaining: u32);
}

// ---------------------------------------------------------------------------
// ArkCore — the single FFI-exposed object
// ---------------------------------------------------------------------------

#[derive(uniffi::Object)]
pub struct ArkCore {
    runtime: Runtime,
    db: StdMutex<Option<Arc<StdMutex<Connection>>>>,
    /// Shutdown channel for the background sync thread.
    sync_shutdown: StdMutex<Option<ShutdownTx>>,
    /// Lightweight mirror of the running sync context for `broadcast_change_json` / `get_connected_peers`.
    sync: TokioMutex<Option<SyncRuntime>>,
    listener: RwLock<Option<Arc<dyn ArkEventListener>>>,
}

struct SyncRuntime {
    server: Arc<SyncServer>,
    storage: Arc<SqliteStorageBackend>,
    clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    relay: Option<Arc<RelaySync>>,
    /// Step 4a: our iroh pairing ticket, captured when the iroh transport
    /// was selected (mirrors `main.rs::SyncRuntime::iroh_our_ticket`).
    iroh_our_ticket: Option<String>,
    beacon: Arc<BroadcastDiscovery>,
    device_id: String,
    device_name: String,
    space_id: String,
    auth_secret: Option<String>,
    own_addresses: Vec<String>,
}

#[uniffi::export]
impl ArkCore {
    /// Construct a brand-new ArkCore facade. The DB is **not** opened yet —
    /// call `open_db(path)` before issuing any CRUD method.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("ark-core-ffi")
            .build()
            .expect("build tokio runtime");
        Arc::new(Self {
            runtime,
            db: StdMutex::new(None),
            sync_shutdown: StdMutex::new(None),
            sync: TokioMutex::new(None),
            listener: RwLock::new(None),
        })
    }

    // -----------------------------------------------------------------------
    // DB lifecycle + CRUD
    // -----------------------------------------------------------------------

    pub fn open_db(&self, path: String) -> Result<bool> {
        let conn = open_db(&path).map_err(ArkCoreError::from)?;
        init_schema(&conn).map_err(ArkCoreError::from)?;
        *self.db.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(StdMutex::new(conn)));
        Ok(true)
    }

    pub fn load_all_json(&self) -> Result<String> {
        self.with_conn(|conn| {
            let data: LoadAllData = db_load_all(conn).map_err(ArkCoreError::from)?;
            serde_json::to_string(&data).map_err(|e| err(e.to_string()))
        })
    }

    pub fn upsert_todo_json(&self, todo_json: String) -> Result<bool> {
        let todo: TodoItem = serde_json::from_str(&todo_json).map_err(|e| err(e.to_string()))?;
        self.with_conn(|conn| {
            db_upsert_todo(conn, &todo).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_todo(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_delete_todo(conn, &id).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn batch_upsert_todos_json(&self, todos_json: String) -> Result<bool> {
        let todos: Vec<TodoItem> =
            serde_json::from_str(&todos_json).map_err(|e| err(e.to_string()))?;
        self.with_conn(|conn| {
            db_batch_upsert_todos(conn, &todos).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn upsert_project_json(&self, project_json: String) -> Result<bool> {
        let project: Project =
            serde_json::from_str(&project_json).map_err(|e| err(e.to_string()))?;
        self.with_conn(|conn| {
            db_upsert_project(conn, &project).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_project(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_delete_project(conn, &id).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn upsert_area_json(&self, area_json: String) -> Result<bool> {
        let area: Area = serde_json::from_str(&area_json).map_err(|e| err(e.to_string()))?;
        self.with_conn(|conn| {
            db_upsert_area(conn, &area).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn upsert_tag_json(&self, tag_json: String) -> Result<bool> {
        let tag: Tag = serde_json::from_str(&tag_json).map_err(|e| err(e.to_string()))?;
        self.with_conn(|conn| {
            db_upsert_tag(conn, &tag).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn upsert_heading_json(&self, heading_json: String) -> Result<bool> {
        let heading: Heading =
            serde_json::from_str(&heading_json).map_err(|e| err(e.to_string()))?;
        self.with_conn(|conn| {
            db_upsert_heading(conn, &heading).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_heading(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_delete_heading(conn, &id).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn upsert_tracked_app_json(&self, tracked_app_json: String) -> Result<bool> {
        let tracked_app: TrackedApp =
            serde_json::from_str(&tracked_app_json).map_err(|e| err(e.to_string()))?;
        self.with_conn(|conn| {
            db_upsert_tracked_app(conn, &tracked_app).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_tracked_app(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_delete_tracked_app(conn, &id).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn upsert_usage_session_json(&self, usage_session_json: String) -> Result<bool> {
        let usage_session: UsageSession =
            serde_json::from_str(&usage_session_json).map_err(|e| err(e.to_string()))?;
        self.with_conn(|conn| {
            db_upsert_usage_session(conn, &usage_session).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_usage_session(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_delete_usage_session(conn, &id).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn upsert_usage_event_json(&self, usage_event_json: String) -> Result<bool> {
        let usage_event: UsageEvent =
            serde_json::from_str(&usage_event_json).map_err(|e| err(e.to_string()))?;
        self.with_conn(|conn| {
            db_upsert_usage_event(conn, &usage_event).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_usage_event(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_delete_usage_event(conn, &id).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn get_sync_kv(&self, key: String) -> Result<Option<String>> {
        self.with_conn(|conn| db_get_kv(conn, &key).map_err(ArkCoreError::from))
    }

    pub fn set_sync_kv(&self, key: String, value: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_set_kv(conn, &key, &value).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn clear_all(&self) -> Result<bool> {
        self.with_conn(|conn| {
            db_clear_all(conn).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_trashed(&self) -> Result<u32> {
        self.with_conn(|conn| {
            let n = db_delete_trashed(conn).map_err(ArkCoreError::from)?;
            Ok(n as u32)
        })
    }

    // -----------------------------------------------------------------------
    // Host helpers
    // -----------------------------------------------------------------------

    pub fn host_device_name(&self) -> String {
        get_host_device_name()
    }

    pub fn own_addresses(&self, port: u32) -> Vec<String> {
        get_own_addresses(port as u16)
    }

    // -----------------------------------------------------------------------
    // Sync lifecycle
    // -----------------------------------------------------------------------

    pub fn start_sync(
        &self,
        config: FfiSyncConfig,
        listener: Box<dyn ArkEventListener>,
    ) -> Result<bool> {
        // ---------------------------------------------------------------
        // AC1 fix: Do NOT call block_on on the JNI calling thread.
        //
        // Strategy:
        //  1. Stop any prior sync (via the existing runtime — this is fast).
        //  2. Install the listener and open DB on the current thread (sync ops).
        //  3. Spawn a dedicated OS thread that drives an async setup coroutine
        //     inside the shared tokio runtime.
        //  4. The spawned thread sends back a oneshot result once the server
        //     and beacon are bound.
        //  5. We wait on a std::sync::mpsc::channel for this result — this
        //     blocks the caller only until ports are bound (typically < 500 ms),
        //     NOT for the lifetime of the sync engine.
        // ---------------------------------------------------------------

        // 1. Tear down any prior sync runtime (fast async on our own runtime).
        self.runtime.block_on(async {
            self.stop_sync_inner().await;
        });

        // 2a. Install listener.
        let listener_arc: Arc<dyn ArkEventListener> = Arc::from(listener);
        self.runtime.block_on(async {
            *self.listener.write().await = Some(listener_arc.clone());
        });

        // 2b. Open DB if the caller supplied a path and we haven't opened one yet.
        if let Some(path) = config.db_path.as_ref() {
            if self.db.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
                let conn = open_db(path).map_err(ArkCoreError::from)?;
                init_schema(&conn).map_err(ArkCoreError::from)?;
                *self.db.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(Arc::new(StdMutex::new(conn)));
            }
        }

        let shared_conn = self
            .db
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .cloned()
            .ok_or_else(|| err("DB not opened; call open_db first"))?;

        // 3. Prepare a result channel and a shutdown channel.
        let (result_tx, result_rx) =
            std::sync::mpsc::channel::<std::result::Result<SyncRuntime, ArkCoreError>>();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Save the shutdown sender so stop_sync can signal the background thread.
        *self.sync_shutdown.lock().unwrap_or_else(|e| e.into_inner()) = Some(shutdown_tx);

        // Clone data needed inside the background thread.
        let runtime_handle = self.runtime.handle().clone();

        // We need to clone self for the background thread.
        // Use a raw pointer trick safe because ArkCore is Arc<ArkCore> and
        // its lifetime is tied to the JVM-referenced object.
        let self_arc = unsafe {
            // SAFETY: ArkCore is always heap-allocated as Arc<ArkCore>.
            // The caller (Kotlin/UniFFI) holds a reference that outlives
            // this thread's lifetime.
            Arc::increment_strong_count(self as *const ArkCore);
            Arc::from_raw(self as *const ArkCore)
        };

        let config_clone = config.clone();
        let listener_for_thread = listener_arc.clone();

        std::thread::Builder::new()
            .name("ark-sync-setup".to_string())
            .spawn(move || {
                let config = config_clone;
                let result_tx = result_tx;

                let setup_result = runtime_handle.block_on(async {
                    let storage = Arc::new(SqliteStorageBackend::new(shared_conn.clone()));
                    storage.set_device_id(&config.device_id);

                    let device_name = config
                        .device_name
                        .clone()
                        .unwrap_or_else(get_host_device_name);
                    let ws_port = config.port.unwrap_or(LAN_SYNC_PORT as u32) as u16;

                    let server =
                        Arc::new(SyncServer::new(storage.clone() as Arc<dyn StorageBackend>));
                    server.set_auth_secret(config.auth_secret.clone()).await;

                    // Wire server → listener (using the self_arc inside the thread).
                    self_arc.install_server_callbacks(&server).await;

                    let own_addresses: Vec<String> = get_own_addresses(ws_port)
                        .into_iter()
                        .filter(|a| is_address_routable(a))
                        .collect();

                    server
                        .start_with_addr(
                            &config.space_id,
                            &config.device_id,
                            Some(&device_name),
                            Some(own_addresses.clone()),
                            &format!("0.0.0.0:{ws_port}"),
                        )
                        .await
                        .map_err(ArkCoreError::from)?;

                    let transport_choice =
                        crate::transport_select::select_transport(config.use_iroh, &config.relay_url);
                    #[allow(unused_mut, unused_assignments)]
                    let mut iroh_our_ticket: Option<String> = None;

                    let relay = if transport_choice == crate::transport_select::TransportChoice::Relay
                    {
                        let relay_url = config
                            .relay_url
                            .clone()
                            .expect("Relay choice implies relay_url");
                        let relay_sync = RelaySync::new(
                            storage.clone() as Arc<dyn StorageBackend>,
                            RelaySyncConfig {
                                relay_url,
                                relay_api_key: config.relay_api_key.clone(),
                                space_id: config.space_id.clone(),
                                device_id: config.device_id.clone(),
                                device_name: device_name.clone(),
                                auth_secret: config.auth_secret.clone(),
                            },
                        );
                        self_arc.install_relay_callbacks(&relay_sync).await;
                        relay_sync.start().await.map_err(ArkCoreError::from)?;
                        Some(relay_sync)
                    } else if transport_choice == crate::transport_select::TransportChoice::Iroh {
                        #[cfg(feature = "iroh-spike")]
                        {
                            let secret_key = {
                                let conn = shared_conn.lock().unwrap_or_else(|e| e.into_inner());
                                crate::iroh_transport::load_or_generate_secret_key(&conn)
                                    .map_err(ArkCoreError::from)?
                            };
                            let peer_addr = match config.iroh_peer_ticket.as_deref() {
                                Some(ticket) => {
                                    Some(crate::iroh_transport::from_ticket(ticket)
                                        .map_err(ArkCoreError::from)?)
                                }
                                None => None,
                            };
                            let iroh_transport = Arc::new(crate::iroh_transport::IrohTransport::new(
                                crate::iroh_transport::IrohConfig {
                                    device_id: config.device_id.clone(),
                                    device_name: device_name.clone(),
                                    space_id: config.space_id.clone(),
                                    secret_key: Some(secret_key),
                                    peer_addr,
                                    peer_ticket: config.iroh_peer_ticket.clone(),
                                    relay_mode: None,
                                },
                            ));
                            let relay_sync = RelaySync::with_transport(
                                storage.clone() as Arc<dyn StorageBackend>,
                                RelaySyncConfig {
                                    relay_url: String::new(),
                                    relay_api_key: None,
                                    space_id: config.space_id.clone(),
                                    device_id: config.device_id.clone(),
                                    device_name: device_name.clone(),
                                    auth_secret: config.auth_secret.clone(),
                                },
                                iroh_transport.clone()
                                    as Arc<dyn crate::sync_transport::SyncTransport>,
                            );
                            self_arc.install_relay_callbacks(&relay_sync).await;
                            relay_sync.start().await.map_err(ArkCoreError::from)?;
                            iroh_our_ticket = match iroh_transport.our_ticket().await {
                                Ok(ticket) => Some(ticket),
                                Err(e) => {
                                    eprintln!("[ArkCore::start_sync] our_ticket() failed: {e}");
                                    None
                                }
                            };
                            Some(relay_sync)
                        }
                        #[cfg(not(feature = "iroh-spike"))]
                        {
                            return Err(err(
                                "iroh transport requested (use_iroh) but this build was \
                                 compiled without the iroh-spike feature; rebuild with \
                                 --features iroh-spike or use relay_url instead",
                            ));
                        }
                    } else {
                        None
                    };

                    // Start clients for known peers.
                    let clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>> =
                        Arc::new(TokioMutex::new(HashMap::new()));
                    let known = server.get_known_peers().await;
                    for peer in known {
                        if peer.device_id == config.device_id {
                            continue;
                        }
                        let reachable: Vec<String> = peer
                            .addresses
                            .iter()
                            .filter(|a| !own_addresses.contains(a) && is_address_routable(a))
                            .cloned()
                            .collect();
                        if reachable.is_empty() {
                            continue;
                        }
                        let peer_rec = PeerRecord {
                            addresses: reachable,
                            ..peer
                        };
                        self_arc
                            .spawn_client(
                                &server,
                                &storage,
                                &clients,
                                peer_rec,
                                config.device_id.clone(),
                                device_name.clone(),
                                config.space_id.clone(),
                                own_addresses.clone(),
                                config.auth_secret.clone(),
                            )
                            .await;
                    }

                    // Seed addresses (QR bootstrap).
                    if !config.seed_addresses.is_empty() {
                        self_arc
                            .spawn_seed_client(
                                &server,
                                &storage,
                                &clients,
                                config.seed_addresses.clone(),
                                config.device_id.clone(),
                                device_name.clone(),
                                config.space_id.clone(),
                                own_addresses.clone(),
                                config.auth_secret.clone(),
                            )
                            .await;
                    }

                    // Beacon.
                    let beacon = Arc::new(BroadcastDiscovery::new());
                    self_arc
                        .wire_beacon(
                            &beacon,
                            &server,
                            &storage,
                            &clients,
                            config.device_id.clone(),
                            device_name.clone(),
                            config.space_id.clone(),
                            own_addresses.clone(),
                            config.auth_secret.clone(),
                        )
                        .await;
                    beacon
                        .start(BroadcastDiscoveryOptions {
                            space_id: config.space_id.clone(),
                            device_id: config.device_id.clone(),
                            device_name: device_name.clone(),
                            ws_port,
                        })
                        .await
                        .map_err(ArkCoreError::from)?;

                    // Publish listener via self_arc so later helpers can see it.
                    *self_arc.listener.write().await = Some(listener_for_thread);

                    let sync_runtime = SyncRuntime {
                        server,
                        storage,
                        clients,
                        relay,
                        iroh_our_ticket,
                        beacon,
                        device_id: config.device_id.clone(),
                        device_name,
                        space_id: config.space_id.clone(),
                        auth_secret: config.auth_secret.clone(),
                        own_addresses,
                    };

                    Ok::<SyncRuntime, ArkCoreError>(sync_runtime)
                });

                // 4. Send the result back to the caller.
                let _ = result_tx.send(setup_result);

                // 5. Keep the thread alive so tokio tasks spawned inside the
                //    setup (beacon, server, etc.) continue running.
                // We wait for the shutdown signal.
                let _ = runtime_handle.block_on(shutdown_rx);

                // Drop self_arc to release the Arc reference.
                drop(self_arc);
            })
            .map_err(|e| err(format!("thread spawn failed: {e}")))?;

        // 5. Wait for the setup thread to report success/failure.
        let sync_runtime = result_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| err("start_sync timed out (>10 s)"))??;

        // Store the runtime handle so broadcast_change_json / get_connected_peers work.
        self.runtime.block_on(async {
            *self.sync.lock().await = Some(sync_runtime);
        });

        Ok(true)
    }

    pub fn stop_sync(&self) -> Result<bool> {
        // Signal the background sync thread to exit.
        if let Some(tx) = self
            .sync_shutdown
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = tx.send(());
        }
        self.runtime.block_on(async {
            self.stop_sync_inner().await;
            Ok(true)
        })
    }

    pub fn leave_space(&self) -> Result<bool> {
        self.stop_sync()
    }

    /// Step 4a: our iroh pairing ticket, if the running sync selected the
    /// iroh transport. `None` when sync isn't running, relay/no transport
    /// was selected instead, or this build lacks `iroh-spike`.
    pub fn get_own_iroh_ticket(&self) -> Result<Option<String>> {
        self.runtime.block_on(async {
            let guard = self.sync.lock().await;
            Ok(guard.as_ref().and_then(|r| r.iroh_our_ticket.clone()))
        })
    }

    pub fn broadcast_change_json(&self, entity_json: String) -> Result<bool> {
        let mut entity: SyncEntity =
            serde_json::from_str(&entity_json).map_err(|e| err(e.to_string()))?;
        self.runtime.block_on(async {
            let guard = self.sync.lock().await;
            let runtime = guard
                .as_ref()
                .ok_or_else(|| err("sync not running"))?
                .clone_refs();
            drop(guard);

            let hlc = runtime.server.update_entity_hlc(&entity.id).await;
            entity.hlc = hlc;
            runtime
                .storage
                .apply_entity(&entity)
                .await
                .map_err(ArkCoreError::from)?;
            runtime
                .server
                .broadcast_live_change(entity.clone(), None)
                .await;
            let clients = runtime.clients.lock().await;
            for client in clients.values() {
                client.broadcast_live_change(entity.clone()).await;
            }
            drop(clients);
            if let Some(relay) = runtime.relay.as_ref() {
                relay
                    .broadcast_live_change(entity.clone())
                    .map_err(ArkCoreError::from)?;
            }
            Ok(true)
        })
    }

    pub fn get_connected_peers(&self) -> Result<Vec<FfiConnectedPeer>> {
        self.runtime.block_on(async {
            let guard = self.sync.lock().await;
            let runtime = match guard.as_ref() {
                Some(r) => r.clone_refs(),
                None => return Ok(Vec::new()),
            };
            drop(guard);
            let entries = runtime.server.get_connected_peer_entries().await;
            let mut out: Vec<FfiConnectedPeer> = entries
                .into_iter()
                .map(|(id, name)| FfiConnectedPeer {
                    device_id: id,
                    device_name: name,
                })
                .collect();

            // Merge outbound-only connections.
            let clients = runtime.clients.lock().await;
            for client in clients.values() {
                let peer = client.current_peer().await;
                if peer.device_id.is_empty() {
                    continue;
                }
                if !out.iter().any(|e| e.device_id == peer.device_id) {
                    out.push(FfiConnectedPeer {
                        device_id: peer.device_id,
                        device_name: peer.device_name,
                    });
                }
            }
            drop(clients);

            if let Some(relay) = runtime.relay.as_ref() {
                for (device_id, device_name) in relay.get_connected_peer_entries().await {
                    if !out.iter().any(|e| e.device_id == device_id) {
                        out.push(FfiConnectedPeer {
                            device_id,
                            device_name,
                        });
                    }
                }
            }
            Ok(out)
        })
    }

    pub fn add_seed_peer(&self, addresses: Vec<String>) -> Result<bool> {
        self.runtime.block_on(async {
            let guard = self.sync.lock().await;
            let runtime = match guard.as_ref() {
                Some(r) => r.clone_refs(),
                None => return Err(err("sync not running")),
            };
            drop(guard);
            self.spawn_seed_client(
                &runtime.server,
                &runtime.storage,
                &runtime.clients,
                addresses,
                runtime.device_id,
                runtime.device_name,
                runtime.space_id,
                runtime.own_addresses,
                runtime.auth_secret,
            )
            .await;
            Ok(true)
        })
    }
}

// Rust-only helpers (not exposed to UniFFI).
impl ArkCore {
    fn with_conn<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let outer = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let shared = outer
            .as_ref()
            .cloned()
            .ok_or_else(|| err("Database not opened. Call open_db first."))?;
        drop(outer);
        let inner = shared.lock().unwrap_or_else(|e| e.into_inner());
        f(&inner)
    }

    async fn stop_sync_inner(&self) {
        let mut guard = self.sync.lock().await;
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

    async fn install_server_callbacks(&self, server: &Arc<SyncServer>) {
        let listener = self.listener.read().await.clone();
        let listener_for_change = listener.clone();
        server
            .set_on_change(Arc::new(move |entity| {
                if let Some(l) = listener_for_change.as_ref() {
                    if let Ok(json) = serde_json::to_string(&entity) {
                        l.on_entity_changed(json);
                    }
                }
            }))
            .await;
        let listener_for_connect = listener.clone();
        let server_for_lookup = server.clone();
        server
            .set_on_peer_connect(Arc::new(move |device_id| {
                let listener = listener_for_connect.clone();
                let server = server_for_lookup.clone();
                tokio::spawn(async move {
                    if let Some(l) = listener.as_ref() {
                        let entries = server.get_connected_peer_entries().await;
                        let name = entries
                            .into_iter()
                            .find(|(id, _)| id == &device_id)
                            .map(|(_, n)| n)
                            .unwrap_or_default();
                        l.on_peer_connected(device_id, name);
                    }
                });
            }))
            .await;
        let listener_for_disconnect = listener;
        server
            .set_on_peer_disconnect(Arc::new(move |device_id, remaining| {
                if let Some(l) = listener_for_disconnect.as_ref() {
                    l.on_peer_disconnected(device_id, remaining as u32);
                }
            }))
            .await;
    }

    async fn install_relay_callbacks(&self, relay: &Arc<RelaySync>) {
        let listener = self.listener.read().await.clone();
        let listener_for_change = listener.clone();
        relay
            .set_on_change(Arc::new(move |entity| {
                if let Some(l) = listener_for_change.as_ref() {
                    if let Ok(json) = serde_json::to_string(&entity) {
                        l.on_entity_changed(json);
                    }
                }
            }))
            .await;

        let listener_for_connect = listener.clone();
        relay
            .set_on_peer_connect(Arc::new(move |device_id| {
                if let Some(l) = listener_for_connect.as_ref() {
                    l.on_peer_connected(device_id, String::new());
                }
            }))
            .await;

        let listener_for_disconnect = listener;
        relay
            .set_on_peer_disconnect(Arc::new(move |device_id, remaining| {
                if let Some(l) = listener_for_disconnect.as_ref() {
                    l.on_peer_disconnected(device_id, remaining as u32);
                }
            }))
            .await;
    }

    #[allow(clippy::too_many_arguments)]
    async fn spawn_client(
        &self,
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
            device_id,
            device_name,
            space_id,
            own_addresses,
            auth_secret,
        ));

        let listener = self.listener.read().await.clone();
        let listener_change = listener.clone();
        client
            .set_on_change(Arc::new(move |entity| {
                if let Some(l) = listener_change.as_ref() {
                    if let Ok(json) = serde_json::to_string(&entity) {
                        l.on_entity_changed(json);
                    }
                }
            }))
            .await;
        let listener_connected = listener.clone();
        let server_for_register = server.clone();
        let peer_addresses = peer.addresses.clone();
        client
            .set_on_connected(Arc::new(move |peer_device_id, peer_name| {
                let listener = listener_connected.clone();
                let server = server_for_register.clone();
                let addrs = peer_addresses.clone();
                let peer_device_id_clone = peer_device_id.clone();
                let peer_name_clone = peer_name.clone();
                tokio::spawn(async move {
                    server
                        .register_external_peer(&peer_device_id_clone, &peer_name_clone, addrs)
                        .await;
                    if let Some(l) = listener.as_ref() {
                        l.on_peer_connected(peer_device_id, peer_name);
                    }
                });
            }))
            .await;
        let listener_disconnected = listener;
        let server_for_count = server.clone();
        client
            .set_on_disconnected(Arc::new(move |peer_device_id| {
                let listener = listener_disconnected.clone();
                let server = server_for_count.clone();
                tokio::spawn(async move {
                    if let Some(l) = listener.as_ref() {
                        let remaining = server.connected_peer_count().await as u32;
                        l.on_peer_disconnected(peer_device_id, remaining);
                    }
                });
            }))
            .await;

        client.start();
        clients.lock().await.insert(peer.device_id.clone(), client);
    }

    #[allow(clippy::too_many_arguments)]
    async fn spawn_seed_client(
        &self,
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
            device_id: temp_id,
            device_name: "Bootstrap".to_string(),
            addresses: reachable,
            last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            last_address: None,
        };
        self.spawn_client(
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

    #[allow(clippy::too_many_arguments)]
    async fn wire_beacon(
        &self,
        beacon: &Arc<BroadcastDiscovery>,
        server: &Arc<SyncServer>,
        storage: &Arc<SqliteStorageBackend>,
        clients: &Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
        device_id: String,
        device_name: String,
        space_id: String,
        own_addresses: Vec<String>,
        auth_secret: Option<String>,
    ) {
        let server = server.clone();
        let storage = storage.clone();
        let clients = clients.clone();
        let listener = self.listener.read().await.clone();
        beacon
            .set_on_peer_discovered(Arc::new(move |peer: BeaconPeer| {
                let server = server.clone();
                let storage = storage.clone();
                let clients = clients.clone();
                let device_id = device_id.clone();
                let device_name = device_name.clone();
                let space_id = space_id.clone();
                let own = own_addresses.clone();
                let auth_secret = auth_secret.clone();
                let listener = listener.clone();
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
                        .register_external_peer(
                            &peer.device_id,
                            &peer.device_name,
                            reachable.clone(),
                        )
                        .await;

                    if server.is_connected_to(&peer.device_id).await {
                        return;
                    }

                    let guard = clients.lock().await;
                    if let Some(existing) = guard.get(&peer.device_id) {
                        existing
                            .update_peer(PeerRecord {
                                device_id: peer.device_id.clone(),
                                device_name: peer.device_name.clone(),
                                addresses: reachable,
                                last_seen: chrono::Utc::now()
                                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                                last_address: None,
                            })
                            .await;
                        return;
                    }
                    drop(guard);

                    // Instantiate a fresh SyncClient.
                    let client = Arc::new(SyncClient::new(
                        storage.clone() as Arc<dyn StorageBackend>,
                        PeerRecord {
                            device_id: peer.device_id.clone(),
                            device_name: peer.device_name.clone(),
                            addresses: reachable,
                            last_seen: chrono::Utc::now()
                                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                            last_address: None,
                        },
                        device_id.clone(),
                        device_name.clone(),
                        space_id.clone(),
                        own.clone(),
                        auth_secret.clone(),
                    ));

                    let listener_change = listener.clone();
                    client
                        .set_on_change(Arc::new(move |entity| {
                            if let Some(l) = listener_change.as_ref() {
                                if let Ok(json) = serde_json::to_string(&entity) {
                                    l.on_entity_changed(json);
                                }
                            }
                        }))
                        .await;
                    let listener_connect = listener.clone();
                    client
                        .set_on_connected(Arc::new(move |peer_device_id, peer_name| {
                            if let Some(l) = listener_connect.as_ref() {
                                l.on_peer_connected(peer_device_id, peer_name);
                            }
                        }))
                        .await;
                    let listener_disconnect = listener.clone();
                    client
                        .set_on_disconnected(Arc::new(move |peer_device_id| {
                            if let Some(l) = listener_disconnect.as_ref() {
                                l.on_peer_disconnected(peer_device_id, 0);
                            }
                        }))
                        .await;
                    client.start();
                    clients.lock().await.insert(peer.device_id, client);
                });
            }))
            .await;
    }
}

// Helper so async methods can take an owning snapshot of the runtime
// without holding the TokioMutex guard.
impl SyncRuntime {
    fn clone_refs(&self) -> CloneSyncRuntime {
        CloneSyncRuntime {
            server: self.server.clone(),
            storage: self.storage.clone(),
            clients: self.clients.clone(),
            relay: self.relay.clone(),
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            space_id: self.space_id.clone(),
            auth_secret: self.auth_secret.clone(),
            own_addresses: self.own_addresses.clone(),
        }
    }
}

struct CloneSyncRuntime {
    server: Arc<SyncServer>,
    storage: Arc<SqliteStorageBackend>,
    clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    relay: Option<Arc<RelaySync>>,
    device_id: String,
    device_name: String,
    space_id: String,
    auth_secret: Option<String>,
    own_addresses: Vec<String>,
}

// Silence unused warnings: the same CloneSyncRuntime is used only for the
// public helpers above; add a dummy accessor so rustc doesn't complain about
// `storage`/`clients` being unreachable through the field syntax.
#[allow(dead_code)]
fn _touch(r: &CloneSyncRuntime) -> Value {
    Value::String(format!("{} {} {}", r.device_id, r.device_name, r.space_id))
}

// ---------------------------------------------------------------------------
// UniFFI-exposed free functions (mirror main.rs convenience helpers)
// ---------------------------------------------------------------------------

#[uniffi::export]
pub fn ffi_host_device_name() -> String {
    get_host_device_name()
}

#[uniffi::export]
pub fn ffi_own_addresses(port: u32) -> Vec<String> {
    get_own_addresses(port as u16)
}
