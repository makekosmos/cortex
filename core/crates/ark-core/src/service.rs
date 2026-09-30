//! In-process ARK service — the request dispatcher that used to live inside
//! the `ark-core-rpc` sidecar binary, now hosted by the Engine in the same
//! process.
//!
//! ## Threading model
//!
//! The sidecar processed stdin requests strictly sequentially on its own
//! tokio runtime. `ArkService` mirrors that contract with a **dedicated
//! worker thread** running a private multi_thread tokio runtime (4 workers —
//! the sidecar used `#[tokio::main]` = multi_thread; iroh and the sync
//! client loops rely on background-task parallelism):
//!
//!   * every `call()` is delivered through a FIFO channel and executed by
//!     `runtime::handle_request` on the worker — one request at a time, so
//!     SQLite never runs on the Engine's runtime threads and the
//!     single-writer / transaction ordering is identical to the sidecar's;
//!   * background work spawned by handlers (`broadcast_local_change`, sync
//!     client loops, beacon discovery) runs on the same worker runtime;
//!   * events go out through the process-wide `crate::events` broadcast bus
//!     (`ArkService::subscribe` returns a receiver on it — note the bus is
//!     shared by every `ArkService` instance in the process; the Engine
//!     hosts exactly one).
//!
//! ## Fault isolation
//!
//! `handle_request` is awaited under `catch_unwind`: a panic inside a handler
//! returns an error for that request, is logged, and the service then reopens
//! the database (the sync runtime is stopped and the shared connection is
//! dropped, so SQLite rolls back any transaction the panic left open). The
//! Engine survives an ARK panic and the next request is served on a fresh
//! connection — the respawn the sidecar never had. Requires `panic =
//! "unwind"` (the default; no workspace profile overrides it).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex as TokioMutex};

use crate::beacon::{BeaconPeer, BroadcastDiscovery, BroadcastDiscoveryOptions};
use crate::db::{self, SqliteStorageBackend};
use crate::events::emit_event;
use crate::hlc::HLC;
use crate::host::{get_host_device_name, get_own_addresses};
use crate::integration_replication::{
    AuthorizedNode, IntegrationCredentialEnvelope, IntegrationNodeGrant, SignedSyncEnvelope,
};
use crate::net::is_address_routable;
use crate::protocol::LAN_SYNC_PORT;
use crate::relay_sync::{RelaySync, RelaySyncConfig};
use crate::sync_bind::SyncBind;
use crate::sync_client::SyncClient;
use crate::sync_server::{StorageBackend, SyncServer};
use crate::transport_select::{select_transport, TransportChoice};
use crate::types::*;

mod definitions;
mod runtime;
mod sync;

#[cfg(test)]
mod tests;

pub(crate) use self::definitions::{
    ObjectWriteSnapshot, Request, StartSyncParams, SyncRuntime, SyncStartParams,
};
use self::runtime::handle_request;
#[cfg(test)]
pub(crate) use crate::db::RefreshLeaseAcquireParams;
// The runtime::system handlers reach the sync handlers through the shared
// `use super::*` chain, so the whole set is imported at this scope.
#[cfg(test)]
use self::sync::build_pairing_restart_params;
use self::sync::{
    handle_add_seed_peer, handle_broadcast_change, handle_connect_with_pairing_code,
    handle_disconnect_peer, handle_get_connected_peers, handle_get_own_iroh_ticket,
    handle_get_sync_snapshot, handle_start_sync, handle_stop_sync,
};

/// Per-service state — what used to be the sidecar's process-wide globals
/// (`DB`, `DB_PATH`, `SYNC`, `BACKUP_GATE`), scoped to one `ArkService`
/// instance so a single process can host several services (tests do;
/// production hosts exactly one).
pub(crate) struct ServiceState {
    /// Shared SQLite connection behind a mutex — the single writer for every
    /// op executed by this service's worker.
    db: StdMutex<Option<Arc<StdMutex<rusqlite::Connection>>>>,
    /// Path to the live ARK DB (set by `init`). `db_backup` opens a separate
    /// read connection so it does not hold the DB mutex for the whole copy.
    db_path: StdMutex<Option<String>>,
    sync: TokioMutex<Option<Arc<SyncRuntime>>>,
    /// KOS-51: serializes background `db_backup` copying and
    /// `db_backup_restore` — backup and restore never overlap. Always taken
    /// before the DB mutex (deadlock-safe: the backup thread works on its own
    /// read connection and never takes the DB mutex).
    backup_gate: StdMutex<()>,
}

impl ServiceState {
    fn new() -> Self {
        Self {
            db: StdMutex::new(None),
            db_path: StdMutex::new(None),
            sync: TokioMutex::new(None),
            backup_gate: StdMutex::new(()),
        }
    }
}

/// Error channel for [`ArkService::call`].
#[derive(Debug, thiserror::Error)]
pub enum ArkServiceError {
    /// The handler rejected the request — equivalent to the sidecar's
    /// `{"ok": false, "error": ...}` response line.
    #[error("{0}")]
    Request(String),
    /// The request never reached the worker (service dropped or worker dead).
    #[error("ark service unavailable: {0}")]
    Unavailable(String),
}

struct ServiceJob {
    request: Request,
    reply: oneshot::Sender<Result<Value, ArkServiceError>>,
}

/// In-process handle to the ARK runtime. Cheap to keep in an `Arc`; dropping
/// it closes the job channel so the worker finishes its in-flight request,
/// stops sync, and exits.
pub struct ArkService {
    jobs: Option<mpsc::UnboundedSender<ServiceJob>>,
    worker: Option<std::thread::JoinHandle<()>>,
    panic_count: Arc<AtomicU64>,
    db_path: String,
}

impl ArkService {
    /// Spawn the worker and open (create if needed) the ARK database at
    /// `db_path`. Same semantics as the sidecar's `init` op.
    pub async fn open(db_path: impl Into<String>) -> Result<Self, ArkServiceError> {
        let db_path = db_path.into();
        let state = Arc::new(ServiceState::new());
        let panic_count = Arc::new(AtomicU64::new(0));
        let (jobs, worker) = spawn_worker(state.clone(), db_path.clone(), panic_count.clone())
            .map_err(|e| ArkServiceError::Unavailable(format!("worker spawn failed: {e}")))?;
        let service = Self {
            jobs: Some(jobs),
            worker: Some(worker),
            panic_count,
            db_path,
        };
        service
            .call("init", json!({ "dbPath": service.db_path }))
            .await?;
        Ok(service)
    }

    /// Execute one request — same ops and semantics as the sidecar's
    /// `{"operation": "...", ...params}` envelope, without the JSON framing.
    /// Requests run strictly sequentially on the worker (FIFO).
    pub async fn call(&self, operation: &str, params: Value) -> Result<Value, ArkServiceError> {
        let mut envelope = match params {
            Value::Object(map) => map,
            Value::Null => serde_json::Map::new(),
            other => {
                let mut map = serde_json::Map::new();
                map.insert("params".into(), other);
                map
            }
        };
        envelope.insert("operation".into(), Value::String(operation.to_owned()));
        let request: Request = serde_json::from_value(Value::Object(envelope))
            .map_err(|e| ArkServiceError::Request(e.to_string()))?;
        let (tx, rx) = oneshot::channel();
        self.jobs
            .as_ref()
            .ok_or_else(|| ArkServiceError::Unavailable("service shut down".into()))?
            .send(ServiceJob { request, reply: tx })
            .map_err(|_| ArkServiceError::Unavailable("worker channel closed".into()))?;
        rx.await
            .map_err(|_| ArkServiceError::Unavailable("worker dropped reply".into()))?
    }

    /// Subscribe to the ARK event stream (`entity_changed`, `peer_connected`,
    /// `object_upserted`, `db_backup_result`, ...). The bus is process-wide —
    /// every `ArkService` in the process shares it.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Value> {
        crate::events::subscribe()
    }

    /// Number of handler panics the worker has caught (each followed by a DB
    /// reopen). Exposed for Engine diagnostics.
    pub fn panic_count(&self) -> u64 {
        self.panic_count.load(Ordering::SeqCst)
    }
}

impl Drop for ArkService {
    fn drop(&mut self) {
        // Close the job channel first — the worker drains pending requests,
        // runs stop_sync, and exits; join() would deadlock with it open.
        drop(self.jobs.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn spawn_worker(
    state: Arc<ServiceState>,
    db_path: String,
    panic_count: Arc<AtomicU64>,
) -> std::io::Result<(
    mpsc::UnboundedSender<ServiceJob>,
    std::thread::JoinHandle<()>,
)> {
    let (tx, rx) = mpsc::unbounded_channel::<ServiceJob>();
    let worker = std::thread::Builder::new()
        .name("ark-service".to_string())
        .spawn(move || {
            // A private multi_thread runtime — mirrors the sidecar's
            // `#[tokio::main]`. Requests still run one-at-a-time (the loop
            // consumes the channel FIFO), but handler-spawned tasks (iroh
            // transport, sync clients, beacon) get real parallelism, and no
            // work ever lands on the Engine's own executor.
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(error) => {
                    eprintln!("[ark-service] failed to build worker runtime: {error}");
                    return;
                }
            };
            rt.block_on(worker_loop(state, db_path, rx, panic_count));
        })?;
    Ok((tx, worker))
}

async fn worker_loop(
    state: Arc<ServiceState>,
    db_path: String,
    mut rx: mpsc::UnboundedReceiver<ServiceJob>,
    panic_count: Arc<AtomicU64>,
) {
    use futures_util::FutureExt;
    while let Some(job) = rx.recv().await {
        let outcome =
            std::panic::AssertUnwindSafe(async { handle_request(&state, job.request).await })
                .catch_unwind()
                .await;
        let reply = match outcome {
            Ok(result) => result.map_err(ArkServiceError::Request),
            Err(panic) => {
                let count = panic_count.fetch_add(1, Ordering::SeqCst) + 1;
                let message = panic_message(&*panic);
                eprintln!(
                    "[ark-service] handler panicked: {message}; reopening database (panic #{count})"
                );
                // Reopen unconditionally: after a panic inside `with_write_tx`
                // the shared connection may hold a half-open transaction.
                let _ = std::panic::AssertUnwindSafe(reopen(&state, &db_path))
                    .catch_unwind()
                    .await;
                Err(ArkServiceError::Request(format!(
                    "ark handler panicked: {message}"
                )))
            }
        };
        let _ = job.reply.send(reply);
    }
    // Channel closed — the service was dropped. Stop sync so beacon/relay/
    // server sockets and client loops die with the worker.
    let _ = std::panic::AssertUnwindSafe(handle_stop_sync(&state))
        .catch_unwind()
        .await;
}

/// Re-open the database after a handler panic: stop sync, drop the shared
/// connection (SQLite rolls back a half-open transaction on drop) and run
/// `init` again. Best-effort — failures are logged; subsequent requests then
/// see "Database not initialized".
async fn reopen(state: &Arc<ServiceState>, db_path: &str) {
    handle_stop_sync(state).await;
    *state.db.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *state.db_path.lock().unwrap_or_else(|e| e.into_inner()) = None;
    match handle_request(
        state,
        Request::Init {
            db_path: db_path.to_string(),
        },
    )
    .await
    {
        Ok(_) => eprintln!("[ark-service] database reopened after panic"),
        Err(e) => eprintln!("[ark-service] reopen after panic failed: {e}"),
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}
