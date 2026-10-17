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
//! `handle_request` is awaited under `catch_unwind`. A handler panic makes the
//! active request outcome unknown and stops this service instance. The worker
//! cancels queued requests, stops sync, and closes the database owner. The
//! Engine must explicitly open a new service before it can serve requests
//! again. This requires `panic = "unwind"` (the default; no workspace profile
//! overrides it).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, watch, Mutex as TokioMutex};

use crate::db::{self, DbHandle, SqliteStorageBackend};
use crate::events::emit_event;
use crate::hlc::HLC;
use crate::integration_replication::{
    AuthorizedNode, IntegrationCredentialEnvelope, IntegrationNodeGrant, SignedSyncEnvelope,
};
use crate::protocol::LAN_SYNC_PORT;
use crate::relay_sync::{RelaySync, RelaySyncConfig};
use crate::replication_storage::SyncRuntimeStorage;
use crate::sync_client::SyncClient;
use crate::sync_server::{PeerEntry, SyncServer};
use crate::types::*;
use ark_net::beacon::{BeaconPeer, BroadcastDiscovery, BroadcastDiscoveryOptions};
use ark_net::host::{get_host_device_name, get_own_addresses};
use ark_net::net::is_address_routable;
use ark_net::sync_bind::SyncBind;
use ark_sync::transport_select::{select_transport, TransportChoice};

mod admission;
mod definitions;
mod runtime;
mod sync;

#[cfg(test)]
mod tests;

use self::admission::{AdmissionPermit, RequestAdmission};
pub(crate) use self::definitions::{
    DataVolumeScope, ObjectWriteSnapshot, Request, StartSyncParams, SyncPolicyRuleInput,
    SyncRuntime, SyncStartParams,
};
use self::runtime::handle_request;
#[cfg(test)]
pub(crate) use crate::db::RefreshLeaseAcquireParams;
// The runtime::system handlers reach the sync handlers through the shared
// `use super::*` chain, so the whole set is imported at this scope.
#[cfg(test)]
use self::sync::build_pairing_restart_params;
use self::sync::{
    handle_accept_pairing, handle_add_seed_peer, handle_broadcast_change, handle_cancel_pairing,
    handle_connect_with_pairing_code, handle_decline_pairing, handle_disconnect_peer,
    handle_get_connected_peers, handle_get_own_iroh_ticket, handle_get_sync_snapshot,
    handle_show_pairing_code, handle_start_sync, handle_start_sync_with_params, handle_stop_sync,
};

/// Per-service state — what used to be the sidecar's process-wide globals
/// (`DB`, `DB_PATH`, `SYNC`, `BACKUP_GATE`), scoped to one `ArkService`
/// instance so a single process can host several services (tests do;
/// production hosts exactly one).
/// Host-provided environment facts the service must not discover on its own
/// — ark-core never guesses OS paths. `windows_dir` is the real %SystemRoot%
/// the Engine resolved via `GetSystemWindowsDirectoryW`; usage analytics
/// marks rows under it as system apps (KOS-287). `None` marks nothing.
#[derive(Clone, Debug, Default)]
pub struct ArkHostConfig {
    pub windows_dir: Option<String>,
}

pub(crate) struct ServiceState {
    /// Cloneable producer handle. It never exposes the writer connection.
    db: StdMutex<Option<DbHandle>>,
    /// Unique owner of the writer actor; service lifecycle alone shuts it down.
    db_owner: StdMutex<Option<db::DbOwner>>,
    /// Path to the live ARK DB (set by `init`). `db_backup` opens a separate
    /// read connection so it does not hold the DB mutex for the whole copy.
    db_path: StdMutex<Option<String>>,
    sync: TokioMutex<Option<Arc<SyncRuntime>>>,
    /// KOS-51: serializes background `db_backup` copying and
    /// `db_backup_restore` — backup and restore never overlap. Always taken
    /// before the DB mutex (deadlock-safe: the backup thread works on its own
    /// read connection and never takes the DB mutex).
    backup_gate: TokioMutex<()>,
    /// Host config resolved by the Engine at construction (KOS-287).
    windows_dir: Option<String>,
}

impl ServiceState {
    fn new(host_config: ArkHostConfig) -> Self {
        Self {
            db: StdMutex::new(None),
            db_owner: StdMutex::new(None),
            db_path: StdMutex::new(None),
            sync: TokioMutex::new(None),
            backup_gate: TokioMutex::new(()),
            windows_dir: host_config.windows_dir,
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
    /// Admission is bounded; callers may retry after pressure subsides.
    #[error("ark service busy: {0}")]
    Busy(String),
}

struct ServiceJob {
    request: Request,
    reply: oneshot::Sender<Result<Value, ArkServiceError>>,
    _admission: AdmissionPermit,
}

/// In-process handle to the ARK runtime. Cheap to keep in an `Arc`; dropping
/// it closes the job channel so the worker finishes its in-flight request,
/// stops sync, and exits.
pub struct ArkService {
    jobs: Option<mpsc::Sender<ServiceJob>>,
    admission: RequestAdmission,
    worker: Option<std::thread::JoinHandle<()>>,
    panic_count: Arc<AtomicU64>,
    db_path: String,
    shutdown_deadline: Arc<StdMutex<Option<Instant>>>,
}

impl ArkService {
    /// Spawn the worker and open (create if needed) the ARK database at
    /// `db_path`. Same semantics as the sidecar's `init` op.
    pub async fn open(db_path: impl Into<String>) -> Result<Self, ArkServiceError> {
        Self::open_with_host_config(db_path, ArkHostConfig::default()).await
    }

    /// `open` + host-provided environment facts (see [`ArkHostConfig`]).
    pub async fn open_with_host_config(
        db_path: impl Into<String>,
        host_config: ArkHostConfig,
    ) -> Result<Self, ArkServiceError> {
        let db_path = db_path.into();
        let state = Arc::new(ServiceState::new(host_config));
        let panic_count = Arc::new(AtomicU64::new(0));
        let shutdown_deadline = Arc::new(StdMutex::new(None));
        let (jobs, worker) = spawn_worker(
            state.clone(),
            panic_count.clone(),
            shutdown_deadline.clone(),
        )
        .map_err(|e| ArkServiceError::Unavailable(format!("worker spawn failed: {e}")))?;
        let service = Self {
            jobs: Some(jobs),
            admission: RequestAdmission::new(),
            worker: Some(worker),
            panic_count,
            db_path,
            shutdown_deadline,
        };
        service
            .call("init", json!({ "dbPath": service.db_path }))
            .await?;
        Ok(service)
    }

    /// Execute one request — same ops and semantics as the sidecar's
    /// `{"operation": "...", ...params}` envelope, without the JSON framing.
    /// Requests run strictly sequentially on the worker (FIFO). Admission is
    /// immediate and bounded by both retained request count and estimated size.
    pub async fn call(&self, operation: &str, params: Value) -> Result<Value, ArkServiceError> {
        let admission = self
            .admission
            .try_admit(operation, &params)
            .map_err(|error| ArkServiceError::Busy(error.to_string()))?;
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
        let job = ServiceJob {
            request,
            reply: tx,
            _admission: admission,
        };
        let jobs = self
            .jobs
            .as_ref()
            .ok_or_else(|| ArkServiceError::Unavailable("service shut down".into()))?;
        match jobs.try_send(job) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_job)) => {
                return Err(ArkServiceError::Busy("request queue is full".into()));
            }
            Err(mpsc::error::TrySendError::Closed(_job)) => {
                return Err(ArkServiceError::Unavailable("worker channel closed".into()));
            }
        }
        rx.await
            .map_err(|_| ArkServiceError::Unavailable("worker dropped reply".into()))?
    }

    /// Subscribe to the ARK event stream (`entity_changed`, `peer_connected`,
    /// `object_upserted`, `db_backup_result`, ...). The bus is process-wide —
    /// every `ArkService` in the process shares it.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Value> {
        crate::events::subscribe()
    }

    /// Number of handler panics that stopped this service worker.
    /// Exposed for Engine diagnostics.
    pub fn panic_count(&self) -> u64 {
        self.panic_count.load(Ordering::SeqCst)
    }

    /// Stop ingress, finish sync producers, drain accepted DB work, and close
    /// the unique writer owner before the deadline.
    pub async fn shutdown(mut self, deadline: Instant) -> Result<(), ArkServiceError> {
        *self
            .shutdown_deadline
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(deadline);
        drop(self.jobs.take());
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        match tokio::time::timeout(
            remaining,
            tokio::task::spawn_blocking(move || worker.join()),
        )
        .await
        {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(panic))) => Err(ArkServiceError::Unavailable(panic_message(&*panic))),
            Ok(Err(error)) => Err(ArkServiceError::Unavailable(error.to_string())),
            Err(_) => Err(ArkServiceError::Unavailable(
                "service shutdown deadline elapsed".into(),
            )),
        }
    }
}

impl Drop for ArkService {
    fn drop(&mut self) {
        if self.worker.is_some() {
            *self
                .shutdown_deadline
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(Instant::now());
        }
        // Close the job channel first — the worker drains pending requests,
        // runs stop_sync, and exits; join() would deadlock with it open.
        drop(self.jobs.take());
        // A dropped service never blocks indefinitely. The worker drains and
        // closes its DB owner after its current callback and sync producers end.
        self.worker.take();
    }
}

fn spawn_worker(
    state: Arc<ServiceState>,
    panic_count: Arc<AtomicU64>,
    shutdown_deadline: Arc<StdMutex<Option<Instant>>>,
) -> std::io::Result<(mpsc::Sender<ServiceJob>, std::thread::JoinHandle<()>)> {
    let (tx, rx) = mpsc::channel::<ServiceJob>(admission::REQUEST_QUEUE_CAPACITY);
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
            rt.block_on(worker_loop(state, rx, panic_count, shutdown_deadline));
        })?;
    Ok((tx, worker))
}

async fn worker_loop(
    state: Arc<ServiceState>,
    mut rx: mpsc::Receiver<ServiceJob>,
    panic_count: Arc<AtomicU64>,
    shutdown_deadline: Arc<StdMutex<Option<Instant>>>,
) {
    use futures_util::FutureExt;
    while let Some(job) = rx.recv().await {
        let outcome =
            std::panic::AssertUnwindSafe(async { handle_request(&state, job.request).await })
                .catch_unwind()
                .await;
        match outcome {
            Ok(result) => {
                let _ = job.reply.send(result.map_err(ArkServiceError::Request));
            }
            Err(panic) => {
                let count = panic_count.fetch_add(1, Ordering::SeqCst) + 1;
                let message = panic_message(&*panic);
                eprintln!(
                    "[ark-service] handler panicked: {message}; stopping service (panic #{count})"
                );
                let _ = job.reply.send(Err(ArkServiceError::Unavailable(format!(
                    "request outcome unknown after handler panic; service stopped: {message}"
                ))));
                rx.close();
                while let Ok(queued) = rx.try_recv() {
                    let _ = queued.reply.send(Err(ArkServiceError::Request(
                        "request cancelled before start after service handler panic".into(),
                    )));
                }
                break;
            }
        }
    }
    // Channel closure or a handler panic stops sync producers before the
    // writer owner closes.
    let _ = std::panic::AssertUnwindSafe(handle_stop_sync(&state))
        .catch_unwind()
        .await;
    let deadline = shutdown_deadline
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take()
        .unwrap_or_else(|| Instant::now() + Duration::from_secs(30));
    let owner = {
        state
            .db_owner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    };
    if let Some(mut owner) = owner {
        if let Err(error) = owner.shutdown(deadline).await {
            eprintln!("[ark-service] database owner shutdown failed: {error}");
        }
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
