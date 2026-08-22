//! Engine-owned, fail-closed stdio supervision for first-party package workers.

#[cfg(windows)]
use crate::package_worker_process::WorkerProcess;
#[cfg(windows)]
use crate::package_worker_process::{LaunchCleanupOwner, WorkerProcessError};
use crate::{
    ark_host::ArkHost,
    observability::{redact_text, BoundedTextTail},
    package_manifest::{PackageKind, PackageManifest},
    package_store::PackageStore,
    package_worker_broker::{self, BrokerConfig},
    package_worker_protocol::{
        BootstrapMessage, BridgeStatus, BridgeWorkerConfig, CallMessage, Grant, HeartbeatMessage,
        HelloMessage, ResultMessage, WorkerMessage, WorkerMethod, MAX_LINE_BYTES,
    },
    runtime_grants::{DataRequest, LaunchGrant},
};
use async_trait::async_trait;
use base64::Engine as _;
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::future::Future;
#[cfg(windows)]
use std::pin::Pin;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
#[cfg(windows)]
use tokio::sync::Mutex as AsyncMutex;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::{mpsc, oneshot},
    time,
};

const HELLO_DEADLINE: Duration = Duration::from_secs(10);
const BOOTSTRAP_DEADLINE: Duration = Duration::from_secs(10);
const HEARTBEAT_DEADLINE: Duration = Duration::from_secs(60);
#[cfg(not(test))]
const STOP_DEADLINE: Duration = Duration::from_secs(10);

#[cfg(windows)]
async fn cleanup_owner_async(
    owner: Arc<LaunchCleanupOwner>,
    deadline: Instant,
) -> Result<(), WorkerProcessError> {
    owner.cleanup_until(deadline).await
}

#[cfg(windows)]
async fn cleanup_process_holder_until(holder: WorkerProcessHolder, deadline: Instant) -> bool {
    let Some(mut process_guard) = lock_holder_until(&holder, deadline).await else {
        return false;
    };
    let Some(process) = process_guard.as_mut() else {
        return true;
    };
    if process.stop_until(deadline).await.is_ok() {
        *process_guard = None;
        true
    } else {
        false
    }
}

#[cfg(windows)]
async fn lock_holder_until<'a>(
    holder: &'a WorkerProcessHolder,
    deadline: Instant,
) -> Option<tokio::sync::MutexGuard<'a, Option<WorkerProcess>>> {
    if deadline <= Instant::now() {
        return None;
    }
    time::timeout(
        deadline.saturating_duration_since(Instant::now()),
        holder.lock(),
    )
    .await
    .ok()
}

#[cfg(windows)]
async fn holder_empty_until(holder: WorkerProcessHolder, deadline: Instant) -> bool {
    lock_holder_until(&holder, deadline)
        .await
        .is_some_and(|process| process.is_none())
}
#[cfg(test)]
const STOP_DEADLINE: Duration = Duration::from_millis(25);
const PROCESS_LAUNCH_DEADLINE: Duration = Duration::from_secs(10);
const MAX_IN_FLIGHT: u32 = 4;
const RESTART_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(5),
    Duration::from_secs(30),
];

#[derive(Debug, Clone)]
struct LaunchSpec {
    manifest: PackageManifest,
    executable: PathBuf,
    hash: String,
    roots: Vec<PathBuf>,
    correlation_id: String,
    bridge_config: Option<BridgeWorkerConfig>,
}

fn restart_delay(failures: u8) -> Option<Duration> {
    RESTART_DELAYS
        .get(failures.saturating_sub(1) as usize)
        .copied()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerState {
    Starting,
    Running,
    Stopping,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerHealth {
    pub state: WorkerState,
    pub restart_count: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkerDiagnostics {
    pub id: String,
    pub version: String,
    pub hash: Option<String>,
    pub correlation_id: Option<String>,
    pub state: WorkerState,
    pub restart_count: u32,
    pub generation: u64,
    pub lifecycle_reason: Option<String>,
    pub stdout_tail: Vec<String>,
    pub stderr_tail: Vec<String>,
    pub bridge_status: Option<BridgeStatus>,
}

struct LiveWorker {
    health: WorkerHealth,
    generation: u64,
    started: Instant,
    last_heartbeat: Instant,
    grant: Option<Grant>,
    bootstrap_token_hash: Option<[u8; 32]>,
    #[cfg(windows)]
    process_holder: WorkerProcessHolder,
    stdin: Option<mpsc::UnboundedSender<Vec<u8>>>,
    io_keys: [TaskKey; 3],
    lifecycle_tx: Option<mpsc::UnboundedSender<WorkerLifecycleEvent>>,
    heartbeat_task: Option<tokio::task::JoinHandle<()>>,
    hello: Option<oneshot::Sender<Result<(), &'static str>>>,
    bootstrap_complete: bool,
    cleanup_started: bool,
    broker: BrokerConfig,
    stdout_tail: Arc<Mutex<BoundedTextTail>>,
    stderr_tail: Arc<Mutex<BoundedTextTail>>,
    bridge_status: Option<BridgeStatus>,
    in_flight: u32,
    restart_allowed: bool,
    launch_spec: Option<LaunchSpec>,
    failure_streak: u8,
    lifecycle_reason: Option<String>,
}

#[cfg(windows)]
struct LaunchTransaction {
    inner: Arc<SupervisorInner>,
    lifecycle_key: TaskKey,
    worker_io_keys: Vec<TaskKey>,
    process_holder: WorkerProcessHolder,
}

#[cfg(windows)]
type WorkerProcessHolder = Arc<AsyncMutex<Option<WorkerProcess>>>;

#[cfg(windows)]
async fn cancellation_pending(cancel: &mut oneshot::Receiver<()>) -> bool {
    tokio::select! {
        _ = cancel => true,
        _ = tokio::task::yield_now() => false,
    }
}

#[cfg(windows)]
async fn startup_wait<T, F>(cancel: &mut oneshot::Receiver<()>, future: F) -> Result<T, ()>
where
    F: std::future::Future<Output = T>,
{
    tokio::select! {
        _ = cancel => Err(()),
        value = future => Ok(value),
    }
}

#[cfg(windows)]
impl LaunchTransaction {
    async fn rollback_error(
        self,
        original: &'static str,
        _deadline: Instant,
    ) -> Result<(), &'static str> {
        let deadline = Instant::now() + STOP_DEADLINE;
        if self.rollback(deadline).await {
            Err(original)
        } else {
            Err("cleanup-failed")
        }
    }

    async fn rollback(self, deadline: Instant) -> bool {
        let mut ok = self
            .inner
            .worker_io
            .cancel_keys_until(&self.worker_io_keys, deadline)
            .await;
        if self.inner.lifecycles.contains(&self.lifecycle_key)
            && !self
                .inner
                .lifecycles
                .cancel_keys_until(std::slice::from_ref(&self.lifecycle_key), deadline)
                .await
        {
            ok = false;
        }
        let Some(mut process) = lock_holder_until(&self.process_holder, deadline).await else {
            return false;
        };
        if let Some(worker) = process.as_mut() {
            if deadline <= Instant::now() || worker.stop_until(deadline).await.is_err() {
                ok = false;
            } else {
                *process = None;
            }
        }
        ok
    }
}

#[derive(Debug, Clone, Copy)]
enum WorkerLifecycleEvent {
    Finish { generation: u64, state: WorkerState },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TaskKey {
    Startup {
        package: String,
        version: String,
        generation: u64,
    },
    Call {
        package: String,
        version: String,
        generation: u64,
        id: String,
    },
    Lifecycle {
        package: String,
        version: String,
        generation: u64,
    },
    Retry {
        package: String,
        version: String,
        generation: u64,
    },
    WorkerStdin {
        package: String,
        version: String,
        generation: u64,
    },
    WorkerStdout {
        package: String,
        version: String,
        generation: u64,
    },
    WorkerStderr {
        package: String,
        version: String,
        generation: u64,
    },
}

struct TaskSlot {
    start: Option<oneshot::Sender<bool>>,
    cancel: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
    #[cfg(windows)]
    owner: Option<Arc<LaunchCleanupOwner>>,
    #[cfg(windows)]
    process_holder: Option<WorkerProcessHolder>,
}

/// Server-owned task admission. A reservation is published before spawn; the
/// start gate prevents work from beginning until its JoinHandle is installed.
struct TaskRegistry {
    admissions_open: AtomicBool,
    capacity: usize,
    slots: Mutex<HashMap<TaskKey, TaskSlot>>,
    quarantine: Mutex<HashMap<TaskKey, TaskSlot>>,
}

impl TaskRegistry {
    fn owned(capacity: usize) -> Arc<Self> {
        Arc::new(Self::new(capacity))
    }

    fn new(capacity: usize) -> Self {
        Self {
            admissions_open: AtomicBool::new(true),
            capacity,
            slots: Mutex::new(HashMap::new()),
            quarantine: Mutex::new(HashMap::new()),
        }
    }

    fn reserve(&self, key: TaskKey) -> Option<(oneshot::Receiver<bool>, oneshot::Receiver<()>)> {
        let mut slots = lock(&self.slots);
        let quarantine = lock(&self.quarantine);
        if !self.admissions_open.load(Ordering::Acquire)
            || slots.len() + quarantine.len() >= self.capacity
            || slots.contains_key(&key)
            || quarantine.contains_key(&key)
        {
            return None;
        }
        let (start_tx, start_rx) = oneshot::channel();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        slots.insert(
            key,
            TaskSlot {
                start: Some(start_tx),
                cancel: Some(cancel_tx),
                task: None,
                #[cfg(windows)]
                owner: None,
                #[cfg(windows)]
                process_holder: None,
            },
        );
        Some((start_rx, cancel_rx))
    }

    #[cfg(windows)]
    fn reserve_startup(
        &self,
        key: TaskKey,
    ) -> Option<(
        oneshot::Receiver<bool>,
        oneshot::Receiver<()>,
        Arc<LaunchCleanupOwner>,
        WorkerProcessHolder,
    )> {
        let owner = LaunchCleanupOwner::new();
        let process_holder = Arc::new(AsyncMutex::new(None));
        let mut slots = lock(&self.slots);
        let quarantine = lock(&self.quarantine);
        if !self.admissions_open.load(Ordering::Acquire)
            || slots.len() + quarantine.len() >= self.capacity
            || slots.contains_key(&key)
            || quarantine.contains_key(&key)
        {
            return None;
        }
        let (start_tx, start_rx) = oneshot::channel();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        slots.insert(
            key,
            TaskSlot {
                start: Some(start_tx),
                cancel: Some(cancel_tx),
                task: None,
                owner: Some(owner.clone()),
                process_holder: Some(process_holder.clone()),
            },
        );
        Some((start_rx, cancel_rx, owner, process_holder))
    }

    #[cfg(windows)]
    fn owner(&self, key: &TaskKey) -> Option<Arc<LaunchCleanupOwner>> {
        lock(&self.slots)
            .get(key)
            .and_then(|slot| slot.owner.clone())
            .or_else(|| {
                lock(&self.quarantine)
                    .get(key)
                    .and_then(|slot| slot.owner.clone())
            })
    }

    fn install_pending(
        &self,
        key: &TaskKey,
        task: tokio::task::JoinHandle<()>,
    ) -> Result<(), tokio::task::JoinHandle<()>> {
        let mut slots = lock(&self.slots);
        let Some(slot) = slots.get_mut(key) else {
            return Err(task);
        };
        slot.task = Some(task);
        Ok(())
    }

    #[cfg(windows)]
    fn detach_process_holder(&self, key: &TaskKey) -> bool {
        let mut slots = lock(&self.slots);
        let Some(slot) = slots.get_mut(key) else {
            return false;
        };
        slot.process_holder = None;
        true
    }

    #[cfg(windows)]
    async fn release_clean_startup(&self, key: &TaskKey) -> bool {
        let (owner, holder) = {
            let slots = lock(&self.slots);
            let Some(slot) = slots.get(key) else {
                return false;
            };
            (slot.owner.clone(), slot.process_holder.clone())
        };
        if owner.is_some_and(|owner| owner.is_armed()) {
            return false;
        }
        if let Some(holder) = holder {
            if !holder_empty_until(holder, Instant::now() + STOP_DEADLINE).await {
                return false;
            }
        }
        let Some(mut slot) = self.remove(key) else {
            return false;
        };
        if let Some(task) = slot.task.take() {
            let _ = task.await;
        }
        true
    }

    fn insert_quarantine(
        quarantine: &mut HashMap<TaskKey, TaskSlot>,
        key: TaskKey,
        slot: TaskSlot,
    ) -> Result<(), TaskSlot> {
        if quarantine.contains_key(&key) {
            Err(slot)
        } else {
            quarantine.insert(key, slot);
            Ok(())
        }
    }

    fn open(&self, key: &TaskKey) -> bool {
        lock(&self.slots)
            .get_mut(key)
            .and_then(|slot| slot.start.take())
            .is_some_and(|start| start.send(true).is_ok())
    }

    fn install(
        &self,
        key: &TaskKey,
        task: tokio::task::JoinHandle<()>,
    ) -> Result<(), tokio::task::JoinHandle<()>> {
        if let Err(task) = self.install_pending(key, task) {
            return Err(task);
        }
        self.open(key);
        Ok(())
    }

    fn remove(&self, key: &TaskKey) -> Option<TaskSlot> {
        lock(&self.slots).remove(key)
    }

    fn contains(&self, key: &TaskKey) -> bool {
        lock(&self.slots).contains_key(key)
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    fn is_finished(&self, key: &TaskKey) -> bool {
        lock(&self.slots)
            .get(key)
            .and_then(|slot| slot.task.as_ref())
            .is_some_and(tokio::task::JoinHandle::is_finished)
    }

    fn has_generation(&self, package: &str, version: &str, generation: u64) -> bool {
        let matches = |key: &TaskKey| match key {
            TaskKey::WorkerStdin {
                package: p,
                version: v,
                generation: g,
            }
            | TaskKey::WorkerStdout {
                package: p,
                version: v,
                generation: g,
            }
            | TaskKey::WorkerStderr {
                package: p,
                version: v,
                generation: g,
            } => p == package && v == version && *g == generation,
            _ => false,
        };
        lock(&self.slots).keys().any(matches) || lock(&self.quarantine).keys().any(matches)
    }

    fn close(&self) {
        self.admissions_open.store(false, Ordering::Release);
    }

    async fn reap_completed(&self) {
        self.reap_completed_until(Instant::now() + STOP_DEADLINE)
            .await;
    }

    async fn reap_completed_until(&self, deadline: Instant) {
        if deadline <= Instant::now() {
            return;
        }
        // Move completed Startup slots into quarantine while holding both locks.
        // The exact key is therefore continuously reserved throughout cleanup.
        let (ordinary, startup_keys) = {
            let mut slots = lock(&self.slots);
            let mut quarantine = lock(&self.quarantine);
            let keys: Vec<_> = slots
                .iter()
                .filter_map(|(key, slot)| {
                    slot.task
                        .as_ref()
                        .filter(|task| task.is_finished())
                        .map(|_| key.clone())
                })
                .collect();
            let mut ordinary = Vec::new();
            let mut startup_keys = Vec::new();
            for key in keys {
                let Some(slot) = slots.remove(&key) else {
                    continue;
                };
                if matches!(key, TaskKey::Startup { .. }) {
                    match Self::insert_quarantine(&mut quarantine, key.clone(), slot) {
                        Ok(()) => startup_keys.push(key),
                        Err(slot) => {
                            // Never overwrite or drop an armed incoming slot.
                            slots.insert(key, slot);
                        }
                    }
                } else {
                    ordinary.push((key, slot));
                }
            }
            (ordinary, startup_keys)
        };

        for (_key, mut slot) in ordinary {
            if let Some(start) = slot.start.take() {
                let _ = start.send(false);
            }
            if let Some(task) = slot.task.take() {
                let _ = task.await;
            }
        }

        for key in &startup_keys {
            #[cfg(windows)]
            let owner = {
                let quarantine = lock(&self.quarantine);
                quarantine.get(&key).and_then(|slot| slot.owner.clone())
            };
            #[cfg(windows)]
            let owner_ok = match owner {
                Some(owner) if !owner.is_armed() => true,
                Some(owner) => cleanup_owner_async(owner, deadline).await.is_ok(),
                None => true,
            };
            #[cfg(windows)]
            let holder = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.process_holder.clone());
            #[cfg(windows)]
            let holder_ok = match holder {
                Some(holder) => cleanup_process_holder_until(holder, deadline).await,
                None => true,
            };
            #[cfg(windows)]
            let cleanup_ok = owner_ok && holder_ok;
            #[cfg(not(windows))]
            let cleanup_ok = true;
            #[cfg(windows)]
            let holder = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.process_holder.clone());
            #[cfg(windows)]
            let holder_empty = match holder {
                Some(holder) => holder_empty_until(holder, deadline).await,
                None => true,
            };
            #[cfg(not(windows))]
            let holder_empty = true;
            let task_done = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.task.as_ref())
                .is_none_or(|task| task.is_finished());
            if cleanup_ok && holder_empty && task_done {
                let task = lock(&self.quarantine).remove(&key).and_then(|mut slot| {
                    if let Some(start) = slot.start.take() {
                        let _ = start.send(false);
                    }
                    slot.task.take()
                });
                if let Some(task) = task {
                    let _ = task.await;
                }
            }
        }

        // A previously quarantined Startup is inspected in place. Its key is
        // never absent while cleanup or joining is in progress.
        let newly_quarantined = startup_keys
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let keys = lock(&self.quarantine)
            .keys()
            .filter(|key| !newly_quarantined.contains(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            #[cfg(windows)]
            let owner = {
                let quarantine = lock(&self.quarantine);
                quarantine.get(&key).and_then(|slot| slot.owner.clone())
            };
            #[cfg(windows)]
            let owner_ok = match owner {
                Some(owner) if !owner.is_armed() => true,
                Some(owner) => cleanup_owner_async(owner, deadline).await.is_ok(),
                None => true,
            };
            #[cfg(windows)]
            let holder = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.process_holder.clone());
            #[cfg(windows)]
            let holder_ok = match holder {
                Some(holder) => cleanup_process_holder_until(holder, deadline).await,
                None => true,
            };
            #[cfg(windows)]
            let cleanup_ok = owner_ok && holder_ok;
            #[cfg(not(windows))]
            let cleanup_ok = true;
            #[cfg(windows)]
            let holder = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.process_holder.clone());
            #[cfg(windows)]
            let holder_empty = match holder {
                Some(holder) => holder_empty_until(holder, deadline).await,
                None => true,
            };
            #[cfg(not(windows))]
            let holder_empty = true;
            let task_done = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.task.as_ref())
                .is_none_or(|task| task.is_finished());
            if cleanup_ok && holder_empty && task_done {
                let task = lock(&self.quarantine)
                    .remove(&key)
                    .and_then(|mut slot| slot.task.take());
                if let Some(task) = task {
                    let _ = task.await;
                }
            }
        }
    }

    async fn cancel_generation_until(
        &self,
        package: &str,
        version: &str,
        generation: u64,
        deadline: Instant,
    ) -> bool {
        self.cancel_matching_until(
            |key| match key {
                TaskKey::Call {
                    package: p,
                    version: v,
                    generation: g,
                    ..
                }
                | TaskKey::Startup {
                    package: p,
                    version: v,
                    generation: g,
                }
                | TaskKey::Lifecycle {
                    package: p,
                    version: v,
                    generation: g,
                }
                | TaskKey::Retry {
                    package: p,
                    version: v,
                    generation: g,
                }
                | TaskKey::WorkerStdin {
                    package: p,
                    version: v,
                    generation: g,
                }
                | TaskKey::WorkerStdout {
                    package: p,
                    version: v,
                    generation: g,
                }
                | TaskKey::WorkerStderr {
                    package: p,
                    version: v,
                    generation: g,
                } => p == package && v == version && *g == generation,
            },
            deadline,
        )
        .await
    }

    async fn cancel_generation(&self, package: &str, version: &str, generation: u64) -> bool {
        self.cancel_generation_until(package, version, generation, Instant::now() + STOP_DEADLINE)
            .await
    }

    async fn shutdown_until(&self, deadline: Instant) -> bool {
        if deadline <= Instant::now() {
            return false;
        }
        self.close();
        let ok = self.cancel_matching_until(|_| true, deadline).await;
        loop {
            self.reap_completed_until(deadline).await;
            if self.len() == 0 {
                break;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::task::yield_now().await;
        }
        ok
    }

    async fn shutdown(&self) -> bool {
        self.shutdown_until(Instant::now() + STOP_DEADLINE).await
    }

    async fn cancel_matching<F>(&self, predicate: F) -> bool
    where
        F: Fn(&TaskKey) -> bool,
    {
        self.cancel_matching_until(predicate, Instant::now() + STOP_DEADLINE)
            .await
    }

    async fn cancel_matching_until<F>(&self, predicate: F, deadline: Instant) -> bool
    where
        F: Fn(&TaskKey) -> bool,
    {
        if deadline <= Instant::now() {
            return false;
        }
        self.reap_completed_until(deadline).await;
        let already_quarantined = {
            let _slots = lock(&self.slots);
            let quarantine = lock(&self.quarantine);
            quarantine.keys().any(|key| predicate(key))
        };
        let (entries, startup_keys, collision) = {
            let mut slots = lock(&self.slots);
            let keys: Vec<_> = slots.keys().filter(|key| predicate(key)).cloned().collect();
            let mut entries = Vec::new();
            let mut startup_keys = Vec::new();
            let mut collision = false;
            let mut quarantine = lock(&self.quarantine);
            for key in keys {
                let Some(slot) = slots.remove(&key) else {
                    continue;
                };
                if matches!(key, TaskKey::Startup { .. }) {
                    match Self::insert_quarantine(&mut quarantine, key.clone(), slot) {
                        Ok(()) => startup_keys.push(key),
                        Err(slot) => {
                            // A Startup slot is never routed through generic
                            // abort handling. Preserve it in active ownership.
                            slots.insert(key, slot);
                            collision = true;
                        }
                    }
                } else {
                    entries.push((key, slot));
                }
            }
            (entries, startup_keys, collision)
        };
        let mut ok = !already_quarantined && !collision;
        for key in &startup_keys {
            let mut quarantine = lock(&self.quarantine);
            if let Some(slot) = quarantine.get_mut(key) {
                if let Some(start) = slot.start.take() {
                    let _ = start.send(false);
                }
                if let Some(cancel) = slot.cancel.take() {
                    let _ = cancel.send(());
                }
            }
        }
        while !startup_keys.is_empty() {
            self.reap_completed_until(deadline).await;
            if startup_keys
                .iter()
                .all(|key| !self.contains_quarantined(key))
            {
                break;
            }
            if Instant::now() >= deadline {
                ok = false;
                break;
            }
            tokio::task::yield_now().await;
        }
        for (_key, mut slot) in entries {
            if let Some(start) = slot.start.take() {
                let _ = start.send(false);
            }
            if let Some(cancel) = slot.cancel.take() {
                let _ = cancel.send(());
            }
            if let Some(mut task) = slot.task.take() {
                if time::timeout(
                    deadline.saturating_duration_since(Instant::now()),
                    &mut task,
                )
                .await
                .is_err()
                {
                    ok = false;
                    task.abort();
                    let _ = task.await;
                }
            }
        }
        ok
    }

    async fn cancel_keys_until(&self, keys: &[TaskKey], deadline: Instant) -> bool {
        let wanted = keys
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        self.cancel_matching_until(|key| wanted.contains(key), deadline)
            .await
    }

    async fn cancel_keys(&self, keys: &[TaskKey]) -> bool {
        self.cancel_keys_until(keys, Instant::now() + STOP_DEADLINE)
            .await
    }

    async fn join_key_until(&self, key: &TaskKey, deadline: Instant) -> bool {
        self.reap_completed_until(deadline).await;
        if lock(&self.quarantine).contains_key(key) {
            return false;
        }
        let Some(mut slot) = self.remove(key) else {
            return true;
        };
        if let Some(start) = slot.start.take() {
            let _ = start.send(true);
        }
        let Some(mut task) = slot.task.take() else {
            return true;
        };
        if time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            &mut task,
        )
        .await
        .is_err()
        {
            task.abort();
            let _ = task.await;
            return false;
        }
        true
    }

    async fn join_key(&self, key: &TaskKey) -> bool {
        self.join_key_until(key, Instant::now() + STOP_DEADLINE)
            .await
    }

    fn len(&self) -> usize {
        lock(&self.slots).len() + lock(&self.quarantine).len()
    }

    fn contains_quarantined(&self, key: &TaskKey) -> bool {
        lock(&self.quarantine).contains_key(key)
    }

    fn contains_quarantined_startup(&self, package: &str, version: &str) -> bool {
        lock(&self.quarantine).keys().any(|key| {
            matches!(
                key,
                TaskKey::Startup {
                    package: p,
                    version: v,
                    ..
                } if p == package && v == version
            )
        })
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    fn startup_reservation_snapshot(&self, key: &TaskKey) -> Option<(bool, bool)> {
        let slots = lock(&self.slots);
        let quarantine = lock(&self.quarantine);
        slots
            .get(key)
            .or_else(|| quarantine.get(key))
            .map(|slot| (slot.owner.is_some(), slot.process_holder.is_some()))
    }
}

struct SupervisorInner {
    workers: Mutex<HashMap<(String, String), LiveWorker>>,
    calls: Arc<TaskRegistry>,
    startups: Arc<TaskRegistry>,
    lifecycles: Arc<TaskRegistry>,
    api_major: u32,
    ark: Option<Arc<ArkHost>>,
    ark_executor: Arc<dyn ArkRequestExecutor>,
    typed_launches: Mutex<HashMap<(String, String), TypedLaunch>>,
    store: Mutex<Option<Arc<PackageStore>>>,
    retry_tasks: Arc<TaskRegistry>,
    worker_io: Arc<TaskRegistry>,
}

#[async_trait]
pub trait ArkRequestExecutor: Send + Sync {
    async fn request(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, &'static str>;
}

#[async_trait]
impl ArkRequestExecutor for ArkHost {
    async fn request(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, &'static str> {
        let response = ArkHost::request(self, operation, params)
            .await
            .map_err(|_| "unavailable")?;
        if response.ok {
            Ok(response.data)
        } else {
            Err("unavailable")
        }
    }
}

struct UnavailableArk;
#[async_trait]
impl ArkRequestExecutor for UnavailableArk {
    async fn request(
        &self,
        _operation: &str,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, &'static str> {
        Err("unavailable")
    }
}

#[derive(Clone)]
struct TypedLaunch {
    session_id: String,
    generation: u64,
    grant: LaunchGrant,
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
static NEXT_AFTER_LAUNCH_GATE: std::sync::OnceLock<Mutex<Option<AfterLaunchGateParts>>> =
    std::sync::OnceLock::new();

#[cfg(all(windows, feature = "package-worker-fixture"))]
struct AfterLaunchGateParts {
    ready: oneshot::Sender<()>,
    release: oneshot::Receiver<()>,
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
pub struct AfterLaunchGate {
    ready: oneshot::Receiver<()>,
    release: Option<oneshot::Sender<()>>,
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
impl AfterLaunchGate {
    pub async fn ready(&mut self) {
        let _ = (&mut self.ready).await;
    }

    pub fn release(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
impl Drop for AfterLaunchGate {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
pub struct HolderLockGate {
    ready: oneshot::Receiver<()>,
    release: Option<oneshot::Sender<()>>,
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
impl HolderLockGate {
    pub async fn ready(&mut self) {
        let _ = (&mut self.ready).await;
    }

    pub fn release(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
impl Drop for HolderLockGate {
    fn drop(&mut self) {
        self.release();
    }
}

#[derive(Clone)]
pub struct PackageWorkerSupervisor {
    inner: Arc<SupervisorInner>,
}

impl PackageWorkerSupervisor {
    pub fn new(api_major: u32) -> Self {
        Self {
            inner: Arc::new(SupervisorInner {
                workers: Mutex::new(HashMap::new()),
                calls: TaskRegistry::owned(MAX_IN_FLIGHT as usize * 256),
                startups: TaskRegistry::owned(256),
                lifecycles: TaskRegistry::owned(256),
                api_major,
                ark: None,
                ark_executor: Arc::new(UnavailableArk),
                typed_launches: Mutex::new(HashMap::new()),
                store: Mutex::new(None),
                retry_tasks: TaskRegistry::owned(256),
                worker_io: TaskRegistry::owned(256 * 3),
            }),
        }
    }

    pub fn with_ark(api_major: u32, ark: Arc<ArkHost>) -> Self {
        let executor: Arc<dyn ArkRequestExecutor> = ark.clone();
        Self {
            inner: Arc::new(SupervisorInner {
                workers: Mutex::new(HashMap::new()),
                calls: TaskRegistry::owned(MAX_IN_FLIGHT as usize * 256),
                startups: TaskRegistry::owned(256),
                lifecycles: TaskRegistry::owned(256),
                api_major,
                ark: Some(ark),
                ark_executor: executor,
                typed_launches: Mutex::new(HashMap::new()),
                store: Mutex::new(None),
                retry_tasks: TaskRegistry::owned(256),
                worker_io: TaskRegistry::owned(256 * 3),
            }),
        }
    }

    pub fn with_ark_executor(api_major: u32, executor: Arc<dyn ArkRequestExecutor>) -> Self {
        Self {
            inner: Arc::new(SupervisorInner {
                workers: Mutex::new(HashMap::new()),
                calls: TaskRegistry::owned(MAX_IN_FLIGHT as usize * 256),
                startups: TaskRegistry::owned(256),
                lifecycles: TaskRegistry::owned(256),
                api_major,
                ark: None,
                ark_executor: executor,
                typed_launches: Mutex::new(HashMap::new()),
                store: Mutex::new(None),
                retry_tasks: TaskRegistry::owned(256),
                worker_io: TaskRegistry::owned(256 * 3),
            }),
        }
    }

    /// Binds the host-compiled typed grant to one authenticated launch.
    /// Replacement is explicit and keyed by package/version; stopping a
    /// worker revokes the binding before its generation can be reused.
    pub fn bind_typed_launch(
        &self,
        id: &str,
        version: &str,
        session_id: &str,
        generation: u64,
        grant: LaunchGrant,
    ) -> Result<(), &'static str> {
        if grant.package_id != id || grant.package_version != version || session_id.is_empty() {
            return Err("forbidden");
        }
        let mut launches = lock(&self.inner.typed_launches);
        if launches
            .get(&(id.to_owned(), version.to_owned()))
            .is_some_and(|current| current.generation >= generation)
        {
            return Err("stale-generation");
        }
        launches.insert(
            (id.to_owned(), version.to_owned()),
            TypedLaunch {
                session_id: session_id.to_owned(),
                generation,
                grant,
            },
        );
        Ok(())
    }

    pub fn revoke_typed_launch(&self, id: &str, version: &str) {
        lock(&self.inner.typed_launches).remove(&(id.to_owned(), version.to_owned()));
    }

    /// The cross-platform authority seam used by worker dispatch. It parses
    /// the strict typed envelope, checks launch identity/generation, and only
    /// then invokes the injected authoritative ARK executor.
    pub async fn dispatch_typed_request(
        &self,
        id: &str,
        version: &str,
        session_id: &str,
        generation: u64,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, &'static str> {
        dispatch_typed_inner(&self.inner, id, version, session_id, generation, request).await
    }
}

async fn dispatch_typed_inner(
    inner: &SupervisorInner,
    id: &str,
    version: &str,
    session_id: &str,
    generation: u64,
    request: serde_json::Value,
) -> Result<serde_json::Value, &'static str> {
    let launch = lock(&inner.typed_launches)
        .get(&(id.to_owned(), version.to_owned()))
        .cloned()
        .ok_or("forbidden")?;
    if launch.session_id != session_id {
        return Err("forbidden");
    }
    if launch.generation != generation {
        return Err("stale-generation");
    }
    let request = serde_json::from_value::<DataRequest>(request).map_err(|_| "invalid-request")?;
    launch
        .grant
        .authorize_request(&request)
        .map_err(|_| "forbidden")?;
    let params = serde_json::to_value(&request).map_err(|_| "invalid-request")?;
    inner.ark_executor.request("data.request", params).await
}

impl PackageWorkerSupervisor {
    pub fn validate_manifest(manifest: &PackageManifest) -> Result<(), &'static str> {
        if !matches!(manifest.kind, PackageKind::Source | PackageKind::Bridge)
            || !manifest.entrypoint.to_ascii_lowercase().ends_with(".exe")
        {
            return Err("worker-required");
        }
        Ok(())
    }

    pub fn health(&self, id: &str, version: &str) -> WorkerHealth {
        self.inner
            .workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(id.into(), version.into()))
            .map(|w| w.health.clone())
            .unwrap_or(WorkerHealth {
                state: WorkerState::Stopped,
                restart_count: 0,
            })
    }

    pub fn diagnostics(&self) -> Vec<WorkerDiagnostics> {
        let mut result: Vec<_> = lock(&self.inner.workers)
            .iter()
            .map(|((id, version), worker)| WorkerDiagnostics {
                id: id.clone(),
                version: version.clone(),
                hash: worker.launch_spec.as_ref().map(|spec| spec.hash.clone()),
                correlation_id: worker
                    .launch_spec
                    .as_ref()
                    .map(|spec| spec.correlation_id.clone()),
                state: worker.health.state,
                restart_count: worker.health.restart_count,
                generation: worker.generation,
                lifecycle_reason: worker.lifecycle_reason.clone(),
                stdout_tail: lock(&worker.stdout_tail)
                    .snapshot()
                    .into_iter()
                    .map(|line| redact_worker_tail(&line))
                    .collect(),
                stderr_tail: lock(&worker.stderr_tail)
                    .snapshot()
                    .into_iter()
                    .map(|line| redact_worker_tail(&line))
                    .collect(),
                bridge_status: worker.bridge_status.clone(),
            })
            .collect();
        result.sort_by(|a, b| (&a.id, &a.version).cmp(&(&b.id, &b.version)));
        result.truncate(1024);
        result
    }

    pub fn bind_store(&self, store: Arc<PackageStore>) {
        *lock(&self.inner.store) = Some(store);
    }

    pub fn activate(&self, id: &str, version: &str) -> bool {
        let mut workers = lock(&self.inner.workers);
        let Some(worker) = workers.get_mut(&(id.into(), version.into())) else {
            return false;
        };
        if worker.health.state != WorkerState::Running {
            return false;
        }
        worker.restart_allowed = true;
        worker.lifecycle_reason = Some("activated".into());
        tracing::info!(target: "package_worker", package_id = %id, version = %version, generation = worker.generation, "worker activated");
        true
    }

    pub async fn start(
        &self,
        manifest: &PackageManifest,
        executable: PathBuf,
        hash: String,
        roots: &[PathBuf],
        correlation_id: String,
        bridge_config: Option<BridgeWorkerConfig>,
    ) -> Result<(), &'static str> {
        Self::validate_manifest(manifest)?;
        self.inner.calls.reap_completed().await;
        self.inner.startups.reap_completed().await;
        self.inner.lifecycles.reap_completed().await;
        let key = (manifest.id.clone(), manifest.version.clone());
        if self
            .inner
            .startups
            .contains_quarantined_startup(&key.0, &key.1)
        {
            return Err("cleanup-failed");
        }
        let existing_state = lock(&self.inner.workers)
            .get(&key)
            .map(|worker| worker.health.state);
        if existing_state.is_some_and(|state| {
            matches!(
                state,
                WorkerState::Starting | WorkerState::Running | WorkerState::Stopping
            )
        }) {
            return Err("already-running");
        }
        if existing_state == Some(WorkerState::Failed)
            && lock(&self.inner.workers)
                .get(&key)
                .is_some_and(|worker| worker.cleanup_started)
        {
            return Err("already-running");
        }
        if existing_state.is_some() {
            self.stop(&key.0, &key.1).await?;
            lock(&self.inner.workers).remove(&key);
        }
        #[cfg(not(windows))]
        {
            let _ = (executable, hash, roots, correlation_id, bridge_config);
            self.insert_failed(key);
            return Err("unsupported-platform");
        }
        #[cfg(windows)]
        {
            self.start_initial_windows(
                key,
                LaunchSpec {
                    manifest: manifest.clone(),
                    executable,
                    hash,
                    roots: roots.to_vec(),
                    correlation_id,
                    bridge_config,
                },
            )
            .await
        }
    }

    #[cfg(windows)]
    async fn start_initial_windows(
        &self,
        key: (String, String),
        spec: LaunchSpec,
    ) -> Result<(), &'static str> {
        // Initial failures are retried synchronously so PackageService only
        // commits enablement after a worker has actually completed hello.
        self.insert_failed(key.clone());
        for attempt in 0..=3u8 {
            if lock(&self.inner.workers).get(&key).is_some_and(|worker| {
                matches!(
                    worker.health.state,
                    WorkerState::Stopping | WorkerState::Stopped
                )
            }) {
                return Err("worker-unavailable");
            }
            if let Some(delay) = (attempt > 0).then(|| restart_delay(attempt)).flatten() {
                time::sleep(delay).await;
            }
            self.inner.startups.reap_completed().await;
            let generation = attempt as u64 + 1;
            if let Some(worker) = lock(&self.inner.workers).get_mut(&key) {
                if matches!(
                    worker.health.state,
                    WorkerState::Failed | WorkerState::Starting
                ) {
                    worker.generation = generation;
                }
            }
            let result = self
                .start_windows(
                    key.clone(),
                    spec.clone(),
                    generation,
                    attempt as u32,
                    attempt,
                    false,
                )
                .await;
            if result.is_ok() {
                return Ok(());
            }
            if let Err(reason) = result {
                if let Some(worker) = lock(&self.inner.workers).get_mut(&key) {
                    worker.lifecycle_reason = Some(reason.into());
                }
            }
        }
        if let Some(store) = lock(&self.inner.store).clone() {
            let _ = store.disable(&key.0, &key.1);
        }
        Err("worker-unavailable")
    }

    #[cfg(windows)]
    async fn start_windows(
        &self,
        key: (String, String),
        spec: LaunchSpec,
        generation: u64,
        restart_count: u32,
        failure_streak: u8,
        restart_allowed: bool,
    ) -> Result<(), &'static str> {
        let startup_key = TaskKey::Startup {
            package: key.0.clone(),
            version: key.1.clone(),
            generation,
        };
        let Some((start_rx, cancel_rx, owner, process_holder)) =
            self.inner.startups.reserve_startup(startup_key.clone())
        else {
            return Err("worker-unavailable");
        };
        let (result_tx, result_rx) = oneshot::channel();
        let supervisor = self.clone();
        let task = tokio::spawn(async move {
            let mut cancel_rx = cancel_rx;
            let started = tokio::select! {
                _ = &mut cancel_rx => false,
                result = start_rx => result.unwrap_or(false),
            };
            if !started {
                let _ = result_tx.send(Err("worker-unavailable"));
                return;
            }
            let result = supervisor
                .start_windows_transaction(
                    key,
                    spec,
                    generation,
                    restart_count,
                    failure_streak,
                    restart_allowed,
                    cancel_rx,
                    owner,
                    process_holder,
                )
                .await;
            let _ = result_tx.send(result);
        });
        if let Err(task) = self.inner.startups.install_pending(&startup_key, task) {
            task.abort();
            let _ = task.await;
            return Err("worker-unavailable");
        }
        self.inner.startups.open(&startup_key);
        let result = result_rx.await.unwrap_or(Err("worker-unavailable"));
        self.inner
            .startups
            .release_clean_startup(&startup_key)
            .await;
        result
    }

    #[cfg(windows)]
    async fn start_windows_transaction(
        &self,
        key: (String, String),
        spec: LaunchSpec,
        generation: u64,
        restart_count: u32,
        failure_streak: u8,
        restart_allowed: bool,
        mut cancel_rx: oneshot::Receiver<()>,
        owner: Arc<LaunchCleanupOwner>,
        process_holder: WorkerProcessHolder,
    ) -> Result<(), &'static str> {
        let deadline = Instant::now() + PROCESS_LAUNCH_DEADLINE;
        tracing::info!(target: "package_worker", package_id = %key.0, version = %key.1, generation, restart_count, "worker start");
        let LaunchSpec {
            manifest,
            executable,
            hash,
            roots,
            correlation_id,
            bridge_config,
        } = spec.clone();
        if let Some(config) = bridge_config.as_ref() {
            crate::lock_file::ensure_owner_only_directory(std::path::Path::new(&config.state_root))
                .map_err(|_| "grant-failed")?;
        }
        let lifecycle_task_key = TaskKey::Lifecycle {
            package: key.0.clone(),
            version: key.1.clone(),
            generation,
        };
        let worker_io_keys = vec![
            TaskKey::WorkerStdin {
                package: key.0.clone(),
                version: key.1.clone(),
                generation,
            },
            TaskKey::WorkerStdout {
                package: key.0.clone(),
                version: key.1.clone(),
                generation,
            },
            TaskKey::WorkerStderr {
                package: key.0.clone(),
                version: key.1.clone(),
                generation,
            },
        ];
        let launch_transaction = LaunchTransaction {
            inner: self.inner.clone(),
            lifecycle_key: lifecycle_task_key.clone(),
            worker_io_keys: worker_io_keys.clone(),
            process_holder: process_holder.clone(),
        };
        self.inner.worker_io.reap_completed().await;
        if cancellation_pending(&mut cancel_rx).await {
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        }
        let mut io_gates = Vec::with_capacity(worker_io_keys.len());
        for io_key in &worker_io_keys {
            let Some(gate) = self.inner.worker_io.reserve(io_key.clone()) else {
                return launch_transaction
                    .rollback_error("worker-unavailable", deadline)
                    .await;
            };
            io_gates.push(gate);
        }
        let Some(mut process_guard) = lock_holder_until(&process_holder, deadline).await else {
            return launch_transaction
                .rollback_error("initial-process-holder-timeout", deadline)
                .await;
        };
        // The guard is acquired before CreateProcessW and held through the
        // server-owned launch. Publication is synchronous while it is held.
        match WorkerProcess::launch_with_owner_until(executable, owner.clone(), deadline).await {
            Ok(process) => *process_guard = Some(process),
            Err(error) => {
                drop(process_guard);
                let reason = match error {
                    WorkerProcessError::InvalidExecutable => "invalid-executable",
                    WorkerProcessError::UnsupportedPlatform => "unsupported-platform",
                    WorkerProcessError::Setup => "process-setup-failed",
                    WorkerProcessError::Cleanup => "process-cleanup-failed",
                };
                return launch_transaction.rollback_error(reason, deadline).await;
            }
        }
        #[cfg(all(windows, feature = "package-worker-fixture"))]
        let after_launch_gate = NEXT_AFTER_LAUNCH_GATE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        #[cfg(all(windows, feature = "package-worker-fixture"))]
        if let Some(gate) = after_launch_gate {
            let _ = gate.ready.send(());
            let _ = gate.release.await;
        }
        let pid = match process_guard.as_ref().and_then(WorkerProcess::id) {
            Some(pid) => pid,
            None => {
                drop(process_guard);
                return launch_transaction
                    .rollback_error("pid-unavailable", deadline)
                    .await;
            }
        };
        drop(process_guard);
        if cancellation_pending(&mut cancel_rx).await {
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        }
        let (grant, token) = match Grant::derive(
            &manifest,
            hash.clone(),
            pid,
            generation,
            correlation_id.clone(),
            &roots,
        ) {
            Ok(value) => value,
            Err(_) => {
                return launch_transaction
                    .rollback_error("grant-failed", deadline)
                    .await
            }
        };
        let typed_launch_invalid =
            lock(&self.inner.typed_launches)
                .get(&key)
                .is_some_and(|typed| {
                    typed.generation != generation || typed.session_id != correlation_id
                });
        if typed_launch_invalid {
            return launch_transaction
                .rollback_error("grant-failed", deadline)
                .await;
        }
        let broker = match BrokerConfig::new(
            grant.scopes.get("network").into_iter().flatten(),
            grant
                .scopes
                .get("filesystem.read")
                .into_iter()
                .flatten()
                .chain(grant.scopes.get("filesystem.write").into_iter().flatten())
                .map(PathBuf::from)
                .collect(),
        ) {
            Ok(broker) => broker,
            Err(_) => {
                return launch_transaction
                    .rollback_error("grant-failed", deadline)
                    .await
            }
        };
        let broker = match bridge_config.as_ref() {
            Some(config) => {
                match broker.with_private_state_root(std::path::Path::new(&config.state_root)) {
                    Ok(broker) => broker,
                    Err(_) => {
                        return launch_transaction
                            .rollback_error("grant-failed", deadline)
                            .await
                    }
                }
            }
            None => broker,
        };
        let (mut stdin, stdout, stderr) = {
            let Some(mut holder) = lock_holder_until(&process_holder, deadline).await else {
                return launch_transaction
                    .rollback_error("published-process-holder-timeout", deadline)
                    .await;
            };
            let Some(process) = holder.as_mut() else {
                drop(holder);
                return launch_transaction
                    .rollback_error("process-missing", deadline)
                    .await;
            };
            let Some((stdin, stdout, stderr)) = process.take_all_pipes() else {
                drop(holder);
                return launch_transaction
                    .rollback_error("worker-pipes-missing", deadline)
                    .await;
            };
            (stdin, stdout, stderr)
        };
        let bootstrap = BootstrapMessage {
            method: "worker.bootstrap".into(),
            package_id: manifest.id.clone(),
            version: manifest.version.clone(),
            hash,
            pid,
            api_version: self.inner.api_major,
            generation,
            correlation_id,
            token: token.clone(),
            bridge_config,
        };
        let mut line = match serde_json::to_vec(&bootstrap) {
            Ok(line) => line,
            Err(_) => {
                return launch_transaction
                    .rollback_error("bootstrap-serialize-failed", deadline)
                    .await
            }
        };
        line.push(b'\n');
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (bootstrap_ack_tx, bootstrap_ack_rx) = oneshot::channel::<Result<(), ()>>();
        let (stdin_start, mut stdin_cancel) = io_gates.remove(0);
        let stdin_task = tokio::spawn(async move {
            let started = tokio::select! {
                _ = &mut stdin_cancel => false,
                started = stdin_start => started.unwrap_or(false),
            };
            if !started {
                return;
            }
            if stdin.write_all(&line).await.is_err() || stdin.flush().await.is_err() {
                let _ = bootstrap_ack_tx.send(Err(()));
                return;
            }
            if bootstrap_ack_tx.send(Ok(())).is_err() {
                return;
            }
            loop {
                tokio::select! {
                    _ = &mut stdin_cancel => break,
                    line = stdin_rx.recv() => {
                        let Some(line) = line else { break };
                        if stdin.write_all(&line).await.is_err() || stdin.flush().await.is_err() {
                            break;
                        }
                    }
                }
            }
        });
        if let Err(task) = self
            .inner
            .worker_io
            .install_pending(&worker_io_keys[0], stdin_task)
        {
            task.abort();
            let _ = task.await;
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        }
        let (hello_tx, hello_rx) = oneshot::channel();
        let (lifecycle_tx, mut lifecycle_rx) = mpsc::unbounded_channel::<WorkerLifecycleEvent>();
        self.inner.lifecycles.reap_completed().await;
        if cancellation_pending(&mut cancel_rx).await {
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        }
        let Some((lifecycle_start, lifecycle_cancel)) =
            self.inner.lifecycles.reserve(lifecycle_task_key.clone())
        else {
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        };
        let lifecycle_inner = self.inner.clone();
        let lifecycle_key = key.clone();
        let lifecycle_task = tokio::spawn(async move {
            if !lifecycle_start.await.unwrap_or(false) {
                return;
            }
            if let Some(WorkerLifecycleEvent::Finish { generation, state }) = tokio::select! {
                _ = lifecycle_cancel => None,
                event = lifecycle_rx.recv() => event,
            } {
                let lifecycle_deadline = if state == WorkerState::Stopped {
                    Instant::now() + STOP_DEADLINE
                } else {
                    deadline
                };
                finish_inner_until(
                    &lifecycle_inner,
                    &lifecycle_key,
                    generation,
                    state,
                    lifecycle_deadline,
                )
                .await;
            }
        });
        if let Err(task) = self
            .inner
            .lifecycles
            .install(&lifecycle_task_key, lifecycle_task)
        {
            task.abort();
            let _ = task.await;
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        }
        let stdout_tail = Arc::new(Mutex::new(BoundedTextTail::new(200, 64 * 1024)));
        let stderr_tail = Arc::new(Mutex::new(BoundedTextTail::new(200, 64 * 1024)));
        let (stdout_start, mut stdout_cancel) = io_gates.remove(0);
        let inner = self.inner.clone();
        let reader_key = key.clone();
        let stdout_tail_for_task = stdout_tail.clone();
        let stdout_event_tx = lifecycle_tx.clone();
        let stdout_task = tokio::spawn(async move {
            let started = tokio::select! {
                _ = &mut stdout_cancel => false,
                started = stdout_start => started.unwrap_or(false),
            };
            if !started {
                return;
            }
            tokio::select! {
                _ = &mut stdout_cancel => {}
                _ = read_stdout(
                    inner,
                    reader_key,
                    generation,
                    BufReader::new(stdout),
                    stdout_tail_for_task,
                    stdout_event_tx,
                ) => {}
            }
        });
        if let Err(task) = self
            .inner
            .worker_io
            .install_pending(&worker_io_keys[1], stdout_task)
        {
            task.abort();
            let _ = task.await;
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        }
        let (stderr_start, mut stderr_cancel) = io_gates.remove(0);
        let stderr_tail_for_task = stderr_tail.clone();
        let stderr_event_tx = lifecycle_tx.clone();
        let stderr_task = tokio::spawn(async move {
            let started = tokio::select! {
                _ = &mut stderr_cancel => false,
                started = stderr_start => started.unwrap_or(false),
            };
            if !started {
                return;
            }
            tokio::select! {
                _ = &mut stderr_cancel => {}
                _ = drain_stderr(
                    BufReader::new(stderr),
                    generation,
                    stderr_tail_for_task,
                    stderr_event_tx,
                ) => {}
            }
        });
        if let Err(task) = self
            .inner
            .worker_io
            .install_pending(&worker_io_keys[2], stderr_task)
        {
            task.abort();
            let _ = task.await;
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        }
        let stopping = {
            let workers = lock(&self.inner.workers);
            workers.get(&key).is_some_and(|worker| {
                matches!(
                    worker.health.state,
                    WorkerState::Stopping | WorkerState::Stopped
                )
            })
        };
        if stopping {
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        }
        let worker_has_resources = lock(&self.inner.workers)
            .get(&key)
            .is_some_and(worker_has_live_resources);
        if worker_has_resources {
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        }
        {
            let mut workers = lock(&self.inner.workers);
            workers.insert(
                key.clone(),
                LiveWorker {
                    health: WorkerHealth {
                        state: WorkerState::Starting,
                        restart_count,
                    },
                    generation,
                    started: Instant::now(),
                    last_heartbeat: Instant::now(),
                    grant: Some(grant),
                    bootstrap_token_hash: Some(hash_token(&token)),
                    process_holder,
                    stdin: Some(stdin_tx.clone()),
                    io_keys: [
                        worker_io_keys[0].clone(),
                        worker_io_keys[1].clone(),
                        worker_io_keys[2].clone(),
                    ],
                    lifecycle_tx: Some(lifecycle_tx.clone()),
                    heartbeat_task: None,
                    hello: Some(hello_tx),
                    bootstrap_complete: false,
                    cleanup_started: false,
                    broker,
                    stdout_tail: stdout_tail.clone(),
                    stderr_tail: stderr_tail.clone(),
                    bridge_status: None,
                    in_flight: 0,
                    restart_allowed,
                    launch_spec: Some(spec),
                    failure_streak,
                    lifecycle_reason: Some("starting".into()),
                },
            );
        }
        // Publication precedes every pipe operation. The stdin task owns the
        // bootstrap frame and acknowledges write+flush before any reader gate
        // can process child output.
        self.inner.worker_io.open(&worker_io_keys[0]);
        let bootstrap_ok = matches!(
            startup_wait(
                &mut cancel_rx,
                time::timeout(BOOTSTRAP_DEADLINE, bootstrap_ack_rx)
            )
            .await,
            Ok(Ok(Ok(Ok(()))))
        );
        if !bootstrap_ok {
            return self
                .cleanup_published_generation(
                    &key,
                    generation,
                    Some(lifecycle_tx.clone()),
                    "bootstrap-write-failed",
                    deadline,
                )
                .await;
        }
        let bootstrap_generation_is_current = {
            let mut workers = lock(&self.inner.workers);
            workers.get_mut(&key).is_some_and(|worker| {
                if worker.generation == generation && worker.health.state == WorkerState::Starting {
                    worker.bootstrap_complete = true;
                    true
                } else {
                    false
                }
            })
        };
        if !bootstrap_generation_is_current {
            return self
                .cleanup_published_generation(
                    &key,
                    generation,
                    Some(lifecycle_tx.clone()),
                    "worker-unavailable",
                    deadline,
                )
                .await;
        }
        self.inner.worker_io.open(&worker_io_keys[1]);
        self.inner.worker_io.open(&worker_io_keys[2]);
        let hello_result =
            startup_wait(&mut cancel_rx, time::timeout(HELLO_DEADLINE, hello_rx)).await;
        match hello_result {
            Ok(Ok(Ok(Ok(())))) => {
                tracing::info!(target: "package_worker", package_id = %key.0, version = %key.1, generation, "worker hello");
                let inner = self.inner.clone();
                let heartbeat_key = key.clone();
                let heartbeat_generation = generation;
                let heartbeat_task = tokio::spawn(async move {
                    heartbeat_watch(inner, heartbeat_key, heartbeat_generation).await;
                });
                let mut heartbeat_task = Some(heartbeat_task);
                let installed = {
                    let mut workers = lock(&self.inner.workers);
                    workers.get_mut(&key).is_some_and(|worker| {
                        if worker.generation == generation
                            && worker.health.state == WorkerState::Running
                            && worker.heartbeat_task.is_none()
                        {
                            worker.heartbeat_task = heartbeat_task.take();
                            true
                        } else {
                            false
                        }
                    })
                };
                if !installed {
                    let task = heartbeat_task.expect("heartbeat task not installed");
                    task.abort();
                    let _ = task.await;
                    return self
                        .cleanup_published_generation(
                            &key,
                            generation,
                            Some(lifecycle_tx.clone()),
                            "worker-unavailable",
                            deadline,
                        )
                        .await;
                }
                let startup_key = TaskKey::Startup {
                    package: key.0.clone(),
                    version: key.1.clone(),
                    generation,
                };
                if !self.inner.startups.detach_process_holder(&startup_key) {
                    return self
                        .cleanup_published_generation(
                            &key,
                            generation,
                            Some(lifecycle_tx.clone()),
                            "worker-unavailable",
                            deadline,
                        )
                        .await;
                }
                Ok(())
            }
            _ => {
                self.cleanup_published_generation(
                    &key,
                    generation,
                    Some(lifecycle_tx.clone()),
                    "worker-unavailable",
                    deadline,
                )
                .await
            }
        }
    }

    #[cfg(windows)]
    async fn cleanup_published_generation(
        &self,
        key: &(String, String),
        generation: u64,
        lifecycle_tx: Option<mpsc::UnboundedSender<WorkerLifecycleEvent>>,
        original: &'static str,
        _deadline: Instant,
    ) -> Result<(), &'static str> {
        let deadline = Instant::now() + STOP_DEADLINE;
        let lifecycle_key = TaskKey::Lifecycle {
            package: key.0.clone(),
            version: key.1.clone(),
            generation,
        };
        let signaled = lifecycle_tx.is_some_and(|tx| {
            tx.send(WorkerLifecycleEvent::Finish {
                generation,
                state: WorkerState::Failed,
            })
            .is_ok()
        });
        let joined = if self.inner.lifecycles.contains(&lifecycle_key) {
            self.inner.lifecycles.join_key(&lifecycle_key).await
        } else {
            true
        };
        if !signaled
            || !joined
            || self
                .exact_generation_has_live_resources(key, generation, deadline)
                .await
        {
            finish_inner_until(&self.inner, key, generation, WorkerState::Failed, deadline).await;
        }
        if !joined
            || !self
                .exact_generation_is_clean(key, generation, deadline)
                .await
        {
            return Err("cleanup-failed");
        }
        Err(original)
    }

    #[cfg(windows)]
    async fn exact_generation_has_live_resources(
        &self,
        key: &(String, String),
        generation: u64,
        deadline: Instant,
    ) -> bool {
        let holder = {
            let workers = lock(&self.inner.workers);
            let Some(worker) = workers
                .get(key)
                .filter(|worker| worker.generation == generation)
            else {
                return false;
            };
            if worker_has_live_resources(worker) {
                return true;
            }
            worker.process_holder.clone()
        };
        !holder_empty_until(holder, deadline).await
    }

    #[cfg(windows)]
    async fn exact_generation_is_clean(
        &self,
        key: &(String, String),
        generation: u64,
        deadline: Instant,
    ) -> bool {
        let worker_clean = lock(&self.inner.workers)
            .get(key)
            .filter(|worker| worker.generation == generation)
            .is_none_or(|worker| !worker_has_live_resources(worker));
        let io_clean = !self
            .inner
            .worker_io
            .has_generation(&key.0, &key.1, generation);
        let worker_holder = {
            let workers = lock(&self.inner.workers);
            workers
                .get(key)
                .filter(|worker| worker.generation == generation)
                .map(|worker| worker.process_holder.clone())
        };
        let holder_clean = match worker_holder {
            Some(holder) => holder_empty_until(holder, deadline).await,
            None => true,
        };
        worker_clean && io_clean && holder_clean
    }

    pub async fn stop(&self, id: &str, version: &str) -> Result<(), &'static str> {
        self.stop_until(id, version, Instant::now() + STOP_DEADLINE)
            .await
    }

    async fn stop_until(
        &self,
        id: &str,
        version: &str,
        deadline: Instant,
    ) -> Result<(), &'static str> {
        if deadline <= Instant::now() {
            return Err("cleanup-failed");
        }
        let key = (id.to_owned(), version.to_owned());
        self.inner.startups.reap_completed_until(deadline).await;
        if self
            .inner
            .startups
            .contains_quarantined_startup(id, version)
        {
            return Err("cleanup-failed");
        }
        let (generation, io_keys, stopped) = {
            let mut workers = lock(&self.inner.workers);
            let Some(worker) = workers.get_mut(&key) else {
                return Ok(());
            };
            let stopped = worker.health.state == WorkerState::Stopped;
            if !stopped {
                worker.health.state = WorkerState::Stopping;
                worker.lifecycle_reason = Some("stopping".into());
                worker.restart_allowed = false;
                worker.grant = None;
                worker.bootstrap_token_hash = None;
            }
            (worker.generation, worker.io_keys.clone(), stopped)
        };
        #[cfg(windows)]
        if stopped
            && self
                .exact_generation_is_clean(&key, generation, deadline)
                .await
        {
            return Ok(());
        }
        #[cfg(not(windows))]
        if stopped {
            return Ok(());
        }
        if stopped {
            if let Some(worker) = lock(&self.inner.workers)
                .get_mut(&key)
                .filter(|worker| worker.generation == generation)
            {
                worker.health.state = WorkerState::Stopping;
                worker.lifecycle_reason = Some("stopping".into());
            }
        }
        self.revoke_typed_launch(id, version);
        let mut cleanup_ok = true;
        if !self
            .inner
            .startups
            .cancel_generation_until(id, version, generation, deadline)
            .await
        {
            cleanup_ok = false;
        }
        if !self
            .inner
            .calls
            .cancel_generation_until(id, version, generation, deadline)
            .await
        {
            cleanup_ok = false;
        }
        if !self
            .inner
            .retry_tasks
            .cancel_generation_until(id, version, generation, deadline)
            .await
        {
            cleanup_ok = false;
        }
        if let Some(stop_tx) = lock(&self.inner.workers)
            .get(&key)
            .and_then(|worker| worker.lifecycle_tx.clone())
        {
            if stop_tx
                .send(WorkerLifecycleEvent::Finish {
                    generation,
                    state: WorkerState::Stopped,
                })
                .is_err()
            {
                cleanup_ok = false;
            }
        }
        if !self
            .inner
            .worker_io
            .cancel_keys_until(&io_keys, deadline)
            .await
        {
            cleanup_ok = false;
        }
        let monitor_key = TaskKey::Lifecycle {
            package: id.to_owned(),
            version: version.to_owned(),
            generation,
        };
        let monitor_present = self.inner.lifecycles.contains(&monitor_key);
        let monitor_joined = self
            .inner
            .lifecycles
            .join_key_until(&monitor_key, deadline)
            .await;
        if !monitor_joined {
            return Err("cleanup-failed");
        }
        #[cfg(windows)]
        if !monitor_present
            && !self
                .exact_generation_is_clean(&key, generation, deadline)
                .await
        {
            finish_inner_until(
                &self.inner,
                &key,
                generation,
                WorkerState::Stopped,
                deadline,
            )
            .await;
        }
        #[cfg(windows)]
        if !self
            .exact_generation_is_clean(&key, generation, deadline)
            .await
        {
            cleanup_ok = false;
        }
        if cleanup_ok {
            let mut workers = lock(&self.inner.workers);
            if let Some(worker) = workers
                .get_mut(&key)
                .filter(|worker| worker.generation == generation)
            {
                worker.health.state = WorkerState::Stopped;
                worker.lifecycle_reason = Some("stopped".into());
            }
            Ok(())
        } else {
            Err("cleanup-failed")
        }
    }

    pub async fn stop_all(&self) -> Result<(), &'static str> {
        self.stop_all_until(Instant::now() + STOP_DEADLINE).await
    }

    async fn stop_all_until(&self, deadline: Instant) -> Result<(), &'static str> {
        if deadline <= Instant::now() {
            return Err("cleanup-failed");
        }
        self.inner.calls.close();
        self.inner.startups.close();
        self.inner.lifecycles.close();
        let keys: Vec<_> = lock(&self.inner.workers).keys().cloned().collect();
        let mut error = false;
        for (id, version) in keys {
            if self.stop_until(&id, &version, deadline).await.is_err() {
                error = true;
            }
        }
        let startup_ok = self.inner.startups.shutdown_until(deadline).await;
        let retry_ok = self.shutdown_retry_tasks_until(deadline).await;
        let worker_io_ok = self.inner.worker_io.shutdown_until(deadline).await;
        if !startup_ok
            || !self.inner.calls.shutdown_until(deadline).await
            || !self.inner.lifecycles.shutdown_until(deadline).await
            || !retry_ok
            || !worker_io_ok
        {
            error = true;
        }
        if error {
            Err("cleanup-failed")
        } else {
            Ok(())
        }
    }

    pub fn authorize(
        &self,
        id: &str,
        version: &str,
        token: &str,
        pid: u32,
        generation: u64,
        method: &WorkerMethod,
        scope: Option<&str>,
    ) -> bool {
        lock(&self.inner.workers)
            .get(&(id.into(), version.into()))
            .and_then(|w| w.grant.as_ref())
            .is_some_and(|g| {
                g.authorize(
                    token,
                    pid,
                    generation,
                    self.inner.api_major,
                    self.inner.api_major,
                    method,
                    scope,
                )
            })
    }

    pub fn timeout() -> Duration {
        Duration::from_secs(30)
    }

    fn insert_failed(&self, key: (String, String)) {
        lock(&self.inner.workers).insert(key, failed_worker());
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub fn test_registry_counts(&self) -> (usize, usize) {
        (self.inner.calls.len(), self.inner.lifecycles.len())
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub async fn test_start_once(
        &self,
        manifest: &PackageManifest,
        executable: PathBuf,
        hash: String,
        roots: &[PathBuf],
        correlation_id: String,
    ) -> Result<(), &'static str> {
        let key = (manifest.id.clone(), manifest.version.clone());
        let startup_key = TaskKey::Startup {
            package: key.0.clone(),
            version: key.1.clone(),
            generation: 1,
        };
        if self.inner.startups.is_finished(&startup_key)
            || self
                .inner
                .startups
                .contains_quarantined_startup(&key.0, &key.1)
        {
            return Err("already-running");
        }
        self.insert_failed(key.clone());
        self.start_windows(
            key,
            LaunchSpec {
                manifest: manifest.clone(),
                executable,
                hash,
                roots: roots.to_vec(),
                correlation_id,
                bridge_config: None,
            },
            1,
            0,
            0,
            false,
        )
        .await
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub fn test_registry_snapshot(&self) -> (usize, usize, usize, usize) {
        (
            self.inner.calls.len(),
            self.inner.startups.len(),
            self.inner.lifecycles.len(),
            self.inner.worker_io.len(),
        )
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub fn test_startup_reservation_snapshot(
        &self,
        id: &str,
        version: &str,
        generation: u64,
    ) -> Option<(bool, bool)> {
        self.inner
            .startups
            .startup_reservation_snapshot(&TaskKey::Startup {
                package: id.to_owned(),
                version: version.to_owned(),
                generation,
            })
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub async fn test_stop_all_until(&self, deadline: Instant) -> Result<(), &'static str> {
        self.stop_all_until(deadline).await
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub async fn test_reap_worker_startup(&self, id: &str, version: &str) {
        let deadline = Instant::now() + STOP_DEADLINE;
        let _ = self
            .inner
            .startups
            .cancel_generation_until(id, version, 1, deadline)
            .await;
        let _ = self
            .inner
            .lifecycles
            .cancel_generation_until(id, version, 1, deadline)
            .await;
        let _ = self
            .inner
            .worker_io
            .cancel_generation_until(id, version, 1, deadline)
            .await;
        self.inner.startups.reap_completed().await;
        self.inner.worker_io.reap_completed().await;
        self.inner.lifecycles.reap_completed().await;
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub async fn test_worker_snapshot(
        &self,
        id: &str,
        version: &str,
    ) -> (WorkerState, Option<String>, bool, bool) {
        let (state, reason, restart_allowed, holder) = {
            let workers = lock(&self.inner.workers);
            let Some(worker) = workers.get(&(id.to_owned(), version.to_owned())) else {
                return (WorkerState::Stopped, None, false, false);
            };
            (
                worker.health.state,
                worker.lifecycle_reason.clone(),
                worker.restart_allowed,
                worker.process_holder.clone(),
            )
        };
        let holder_nonempty = lock_holder_until(&holder, Instant::now() + STOP_DEADLINE)
            .await
            .is_some_and(|process| process.is_some());
        (state, reason, restart_allowed, holder_nonempty)
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub fn test_pause_after_launch(&self) -> AfterLaunchGate {
        let (ready_tx, ready_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        *NEXT_AFTER_LAUNCH_GATE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(AfterLaunchGateParts {
            ready: ready_tx,
            release: release_rx,
        });
        AfterLaunchGate {
            ready: ready_rx,
            release: Some(release_tx),
        }
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub fn test_hold_process_holder(&self, id: &str, version: &str) -> Option<HolderLockGate> {
        let holder = lock(&self.inner.workers)
            .get(&(id.to_owned(), version.to_owned()))
            .map(|worker| worker.process_holder.clone())?;
        let (ready_tx, ready_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        tokio::spawn(async move {
            let _guard = holder.lock().await;
            let _ = ready_tx.send(());
            let _ = release_rx.await;
        });
        Some(HolderLockGate {
            ready: ready_rx,
            release: Some(release_tx),
        })
    }

    async fn shutdown_retry_tasks_until(&self, deadline: Instant) -> bool {
        self.inner.retry_tasks.shutdown_until(deadline).await
    }

    async fn shutdown_retry_tasks(&self) -> bool {
        self.shutdown_retry_tasks_until(Instant::now() + STOP_DEADLINE)
            .await
    }
}

fn worker_has_live_resources(worker: &LiveWorker) -> bool {
    worker.stdin.is_some()
        || worker.lifecycle_tx.is_some()
        || worker.heartbeat_task.is_some()
        || worker.hello.is_some()
        || worker.grant.is_some()
}

fn failed_worker() -> LiveWorker {
    LiveWorker {
        health: WorkerHealth {
            state: WorkerState::Failed,
            restart_count: 0,
        },
        generation: 0,
        started: Instant::now(),
        last_heartbeat: Instant::now(),
        grant: None,
        bootstrap_token_hash: None,
        #[cfg(windows)]
        process_holder: Arc::new(AsyncMutex::new(None)),
        stdin: None,
        io_keys: [
            TaskKey::WorkerStdin {
                package: String::new(),
                version: String::new(),
                generation: 0,
            },
            TaskKey::WorkerStdout {
                package: String::new(),
                version: String::new(),
                generation: 0,
            },
            TaskKey::WorkerStderr {
                package: String::new(),
                version: String::new(),
                generation: 0,
            },
        ],
        lifecycle_tx: None,
        heartbeat_task: None,
        hello: None,
        bootstrap_complete: false,
        cleanup_started: false,
        broker: BrokerConfig {
            allowed_origins: Default::default(),
            filesystem_roots: vec![],
            private_state_roots: vec![],
        },
        stdout_tail: Arc::new(Mutex::new(BoundedTextTail::new(200, 64 * 1024))),
        stderr_tail: Arc::new(Mutex::new(BoundedTextTail::new(200, 64 * 1024))),
        bridge_status: None,
        in_flight: 0,
        restart_allowed: false,
        launch_spec: None,
        failure_streak: 0,
        lifecycle_reason: Some("failed".into()),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn redact_worker_tail(line: &str) -> String {
    ["filesystem.read", "filesystem.write", "network"]
        .into_iter()
        .fold(redact_text(line), |line, scope| {
            line.replace(scope, "[REDACTED]")
        })
}
fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}
fn token_matches(expected: &[u8; 32], token: &str) -> bool {
    let actual = hash_token(token);
    expected
        .iter()
        .zip(actual)
        .fold(0u8, |out, (a, b)| out | (a ^ b))
        == 0
}

async fn read_bounded_line<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>, ()> {
    let mut line = Vec::new();
    loop {
        let mut byte = [0];
        match reader.read(&mut byte).await {
            Ok(0) => return (!line.is_empty()).then_some(line).ok_or(()).map(Some),
            Ok(_) => {}
            Err(_) => return Err(()),
        }
        if byte[0] == b'\n' {
            return Ok(Some(line));
        }
        if line.len() >= MAX_LINE_BYTES {
            return Err(());
        }
        line.push(byte[0]);
    }
}

async fn drain_stderr<R: AsyncRead + Unpin + Send + 'static>(
    mut reader: BufReader<R>,
    generation: u64,
    tail: Arc<Mutex<BoundedTextTail>>,
    lifecycle_tx: mpsc::UnboundedSender<WorkerLifecycleEvent>,
) {
    while let Ok(Some(line)) = read_bounded_line(&mut reader).await {
        lock(&tail).push(&String::from_utf8_lossy(&line));
    }
    let _ = lifecycle_tx.send(WorkerLifecycleEvent::Finish {
        generation,
        state: WorkerState::Failed,
    });
}

async fn read_stdout<R: AsyncRead + Unpin + Send + 'static>(
    inner: Arc<SupervisorInner>,
    key: (String, String),
    generation: u64,
    mut reader: BufReader<R>,
    tail: Arc<Mutex<BoundedTextTail>>,
    lifecycle_tx: mpsc::UnboundedSender<WorkerLifecycleEvent>,
) {
    loop {
        let line = match read_bounded_line(&mut reader).await {
            Ok(Some(line)) => line,
            _ => {
                let _ = lifecycle_tx.send(WorkerLifecycleEvent::Finish {
                    generation,
                    state: WorkerState::Failed,
                });
                return;
            }
        };
        let message = match crate::package_worker_protocol::parse_json_line(&line) {
            Ok(message) => message,
            Err(_) => {
                lock(&tail).push(&String::from_utf8_lossy(&line));
                let _ = lifecycle_tx.send(WorkerLifecycleEvent::Finish {
                    generation,
                    state: WorkerState::Failed,
                });
                return;
            }
        };
        match message {
            WorkerMessage::Hello(hello) => handle_hello(&inner, &key, generation, hello),
            WorkerMessage::Heartbeat(heartbeat) => handle_heartbeat(&inner, &key, heartbeat),
            WorkerMessage::Call(call) => {
                spawn_call(inner.clone(), key.clone(), call).await;
            }
            _ => {
                let _ = lifecycle_tx.send(WorkerLifecycleEvent::Finish {
                    generation,
                    state: WorkerState::Failed,
                });
                return;
            }
        }
    }
}

fn handle_hello(
    inner: &Arc<SupervisorInner>,
    key: &(String, String),
    generation: u64,
    hello: HelloMessage,
) {
    let mut workers = lock(&inner.workers);
    let Some(worker) = workers.get_mut(key) else {
        return;
    };
    let valid = worker.generation == generation
        && worker.health.state == WorkerState::Starting
        && worker.bootstrap_complete
        && worker.grant.as_ref().is_some_and(|grant| {
            grant.package_id == hello.package_id
                && grant.version == hello.version
                && grant.hash == hello.hash
                && grant.pid == hello.pid
        })
        && worker
            .bootstrap_token_hash
            .as_ref()
            .is_some_and(|hash| token_matches(hash, &hello.token))
        && hello.api_version == inner.api_major;
    if !valid {
        if let Some(tx) = worker.lifecycle_tx.clone() {
            let _ = tx.send(WorkerLifecycleEvent::Finish {
                generation,
                state: WorkerState::Failed,
            });
        }
        return;
    }
    worker.bootstrap_token_hash = None;
    worker.health.state = WorkerState::Running;
    worker.lifecycle_reason = Some("hello".into());
    worker.last_heartbeat = Instant::now();
    if let Some(hello_tx) = worker.hello.take() {
        let _ = hello_tx.send(Ok(()));
    }
}

fn handle_heartbeat(
    inner: &Arc<SupervisorInner>,
    key: &(String, String),
    heartbeat: HeartbeatMessage,
) {
    let mut workers = lock(&inner.workers);
    let Some(worker) = workers.get_mut(key) else {
        return;
    };
    let valid = worker.health.state == WorkerState::Running
        && worker.grant.as_ref().is_some_and(|grant| {
            grant.authenticate(
                &heartbeat.token,
                grant.pid,
                heartbeat.generation,
                inner.api_major,
                inner.api_major,
            )
        });
    if valid {
        worker.last_heartbeat = Instant::now();
        worker.bridge_status = heartbeat.bridge_status.map(sanitize_bridge_status);
    } else if let Some(tx) = worker.lifecycle_tx.clone() {
        let _ = tx.send(WorkerLifecycleEvent::Finish {
            generation: heartbeat.generation,
            state: WorkerState::Failed,
        });
    }
}

fn sanitize_bridge_status(status: BridgeStatus) -> BridgeStatus {
    fn safe_time(value: Option<String>) -> Option<String> {
        value.filter(|value| {
            value.len() <= 64
                && !value
                    .chars()
                    .any(|ch| ch.is_control() || matches!(ch, '/' | '\\'))
        })
    }
    BridgeStatus {
        last_sync: safe_time(status.last_sync),
        conflict_count: status.conflict_count.min(1_000_000),
        last_conflict_at: safe_time(status.last_conflict_at),
    }
}

async fn spawn_call(inner: Arc<SupervisorInner>, key: (String, String), call: CallMessage) {
    let task_key = TaskKey::Call {
        package: key.0.clone(),
        version: key.1.clone(),
        generation: call.generation,
        id: call.id.clone(),
    };
    {
        let mut workers = lock(&inner.workers);
        let Some(worker) = workers.get_mut(&key) else {
            return;
        };
        if worker.health.state != WorkerState::Running
            || worker.generation != call.generation
            || worker.in_flight >= MAX_IN_FLIGHT
        {
            return;
        }
    }
    inner.calls.reap_completed().await;
    let Some((start_rx, cancel_rx)) = inner.calls.reserve(task_key.clone()) else {
        if let Some(worker) = lock(&inner.workers).get_mut(&key) {
            send_result(worker.stdin.as_ref(), &call.id, false, None, "unavailable");
        }
        return;
    };
    let task_inner = inner.clone();
    let task_key_for_task = task_key.clone();
    let task = tokio::spawn(async move {
        if !start_rx.await.unwrap_or(false) {
            return;
        }
        tokio::select! {
            _ = cancel_rx => {},
            _ = handle_call(task_inner, key, call) => {},
        }
    });
    if let Err(task) = inner.calls.install(&task_key_for_task, task) {
        task.abort();
        let _ = task.await;
    }
}

async fn handle_call(inner: Arc<SupervisorInner>, key: (String, String), call: CallMessage) {
    let (grant, broker, stdin) = {
        let mut workers = lock(&inner.workers);
        let Some(worker) = workers.get_mut(&key) else {
            return;
        };
        if worker.health.state != WorkerState::Running
            || worker.generation != call.generation
            || worker.in_flight >= MAX_IN_FLIGHT
        {
            send_result(worker.stdin.as_ref(), &call.id, false, None, "unavailable");
            return;
        }
        worker.in_flight += 1;
        (
            worker.grant.clone(),
            worker.broker.clone(),
            worker.stdin.clone(),
        )
    };
    let result = match grant {
        Some(grant) => time::timeout(
            PackageWorkerSupervisor::timeout(),
            dispatch(&inner, &grant, &broker, &call),
        )
        .await
        .unwrap_or_else(|_| Err("timeout")),
        None => Err("forbidden"),
    };
    if let Err(error) = result.as_ref() {
        tracing::warn!(
            target: "package_worker",
            package_id = %key.0,
            version = %key.1,
            operation = ?call.operation,
            error,
            "worker call failed"
        );
    }
    if let Some(stdin) = stdin {
        let mut response = match result {
            Ok(value) => result_line(&call.id, true, Some(value), None),
            Err(error) => result_line(&call.id, false, None, Some(error)),
        };
        if response.len() > MAX_LINE_BYTES {
            response = result_line(&call.id, false, None, Some("unavailable"));
        }
        let current = lock(&inner.workers).get(&key).is_some_and(|worker| {
            worker.health.state == WorkerState::Running && worker.generation == call.generation
        });
        if current {
            let _ = stdin.send(response);
        }
    }
    if let Some(worker) = lock(&inner.workers).get_mut(&key) {
        worker.in_flight = worker.in_flight.saturating_sub(1);
    }
}

async fn dispatch(
    inner: &SupervisorInner,
    grant: &Grant,
    broker: &BrokerConfig,
    call: &CallMessage,
) -> Result<serde_json::Value, &'static str> {
    let store = lock(&inner.store).clone().ok_or("unavailable")?;
    let installed = store
        .installed(&grant.package_id, &grant.version)
        .map_err(|_| "forbidden")?;
    if !installed.enabled || installed.revoked || !installed.hash.eq_ignore_ascii_case(&grant.hash)
    {
        return Err("forbidden");
    }
    if matches!(
        call.operation,
        WorkerMethod::ArkRead | WorkerMethod::ArkWrite
    ) {
        let typed_key = (grant.package_id.clone(), grant.version.clone());
        let typed_bound = lock(&inner.typed_launches).contains_key(&typed_key);
        let typed_envelope = call
            .params
            .get("request")
            .is_some_and(|value| value.get("kind").is_some());
        if typed_envelope {
            if !typed_bound {
                return Err("forbidden");
            }
            if !grant.authenticate(
                &call.token,
                grant.pid,
                call.generation,
                inner.api_major,
                inner.api_major,
            ) {
                return Err("forbidden");
            }
            return dispatch_typed_inner(
                inner,
                &grant.package_id,
                &grant.version,
                &grant.correlation_id,
                call.generation,
                call.params
                    .get("request")
                    .cloned()
                    .unwrap_or_else(|| call.params.clone()),
            )
            .await;
        }
    }
    let scope = match call.operation {
        WorkerMethod::ArkRead | WorkerMethod::ArkWrite => call
            .params
            .get("operation")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        WorkerMethod::NetworkFetch => call
            .params
            .get("url")
            .and_then(serde_json::Value::as_str)
            .and_then(|url| reqwest::Url::parse(url).ok())
            .map(|url| url.origin().ascii_serialization()),
        WorkerMethod::FilesystemRead
        | WorkerMethod::FilesystemWrite
        | WorkerMethod::FilesystemList
        | WorkerMethod::FilesystemPoll
        | WorkerMethod::FilesystemDelete => call
            .params
            .get("path")
            .and_then(serde_json::Value::as_str)
            .and_then(|path| granted_path_scope(grant, &call.operation, Path::new(path))),
    };
    if !grant.authorize(
        &call.token,
        grant.pid,
        call.generation,
        inner.api_major,
        inner.api_major,
        &call.operation,
        scope.as_deref(),
    ) {
        return Err("forbidden");
    }
    match call.operation {
        WorkerMethod::ArkRead | WorkerMethod::ArkWrite => {
            let operation = scope.ok_or("invalid-request")?;
            let params = call
                .params
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let ark = inner.ark.as_ref().ok_or("unavailable")?;
            let response = ark
                .request(&operation, params)
                .await
                .map_err(|_| "unavailable")?;
            if response.ok {
                Ok(response.data)
            } else {
                Err("unavailable")
            }
        }
        WorkerMethod::NetworkFetch => {
            let url = call
                .params
                .get("url")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let bytes = package_worker_broker::fetch(broker, url)
                .await
                .map_err(|_| "unavailable")?;
            if bytes.len() > 700 * 1024 {
                return Err("unavailable");
            }
            Ok(
                serde_json::json!({ "bytes": base64::engine::general_purpose::STANDARD.encode(bytes) }),
            )
        }
        WorkerMethod::FilesystemRead => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let bytes = package_worker_broker::read_file(broker, Path::new(path)).map_err(|error| {
                let class = match error {
                    package_worker_broker::BrokerError::Invalid(_) => "invalid",
                    package_worker_broker::BrokerError::Io(_) => "io",
                    package_worker_broker::BrokerError::Http(_) => "http",
                };
                tracing::warn!(target: "package_worker", error_class = class, "worker filesystem read failed");
                "unavailable"
            })?;
            if bytes.len() > 700 * 1024 {
                return Err("unavailable");
            }
            Ok(
                serde_json::json!({ "bytes": base64::engine::general_purpose::STANDARD.encode(bytes) }),
            )
        }
        WorkerMethod::FilesystemWrite => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let bytes = call
                .params
                .get("bytes")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| base64::engine::general_purpose::STANDARD.decode(value).ok())
                .ok_or("invalid-request")?;
            package_worker_broker::write_file(broker, Path::new(path), &bytes)
                .map_err(|_| "unavailable")?;
            Ok(serde_json::Value::Null)
        }
        WorkerMethod::FilesystemDelete => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            package_worker_broker::delete_file(broker, Path::new(path))
                .map_err(|_| "unavailable")?;
            Ok(serde_json::Value::Null)
        }
        WorkerMethod::FilesystemList => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let entries = package_worker_broker::list_directory(broker, Path::new(path))
                .map_err(|_| "unavailable")?;
            serde_json::to_value(entries).map_err(|_| "unavailable")
        }
        WorkerMethod::FilesystemPoll => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let entries = package_worker_broker::poll_metadata(broker, Path::new(path))
                .map_err(|_| "unavailable")?;
            serde_json::to_value(entries).map_err(|_| "unavailable")
        }
    }
}

fn granted_path_scope(grant: &Grant, method: &WorkerMethod, path: &Path) -> Option<String> {
    let capability = match method {
        WorkerMethod::FilesystemRead
        | WorkerMethod::FilesystemList
        | WorkerMethod::FilesystemPoll => "filesystem.read",
        WorkerMethod::FilesystemWrite | WorkerMethod::FilesystemDelete => "filesystem.write",
        _ => return None,
    };
    grant
        .scopes
        .get(capability)?
        .iter()
        .find(|root| path.starts_with(root))
        .cloned()
}
fn result_line(
    id: &str,
    ok: bool,
    result: Option<serde_json::Value>,
    error: Option<&str>,
) -> Vec<u8> {
    let mut line = serde_json::to_vec(&ResultMessage {
        method: "worker.result".into(),
        id: id.into(),
        ok,
        result,
        error: error.map(str::to_owned),
    })
    .unwrap_or_default();
    line.push(b'\n');
    line
}
fn send_result(
    stdin: Option<&mpsc::UnboundedSender<Vec<u8>>>,
    id: &str,
    ok: bool,
    result: Option<serde_json::Value>,
    error: &str,
) {
    if let Some(stdin) = stdin {
        let _ = stdin.send(result_line(id, ok, result, Some(error)));
    }
}

async fn heartbeat_watch(inner: Arc<SupervisorInner>, key: (String, String), generation: u64) {
    loop {
        time::sleep(Duration::from_secs(30)).await;
        let stale = {
            let mut workers = lock(&inner.workers);
            let Some(worker) = workers.get_mut(&key) else {
                return;
            };
            if worker.generation != generation {
                return;
            }
            if worker.health.state == WorkerState::Running
                && worker.started.elapsed() >= Duration::from_secs(60)
            {
                worker.failure_streak = 0;
            }
            worker.health.state == WorkerState::Running
                && worker.last_heartbeat.elapsed() > HEARTBEAT_DEADLINE
        };
        if stale {
            if let Some(tx) = lock(&inner.workers)
                .get(&key)
                .and_then(|worker| worker.lifecycle_tx.clone())
            {
                let _ = tx.send(WorkerLifecycleEvent::Finish {
                    generation,
                    state: WorkerState::Failed,
                });
            }
            return;
        }
        if !lock(&inner.workers).contains_key(&key) {
            return;
        }
    }
}

async fn join_task_until(mut task: tokio::task::JoinHandle<()>, deadline: Instant) -> bool {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if time::timeout(remaining, &mut task).await.is_err() {
        task.abort();
        let _ = task.await;
        return false;
    }
    true
}

async fn finish_inner_until(
    inner: &Arc<SupervisorInner>,
    key: &(String, String),
    generation: u64,
    state: WorkerState,
    deadline: Instant,
) {
    if deadline <= Instant::now() {
        return;
    }
    #[cfg(windows)]
    let retained_holder = {
        let workers = lock(&inner.workers);
        let Some(worker) = workers.get(key) else {
            return;
        };
        if worker.generation != generation {
            return;
        }
        worker
            .cleanup_started
            .then(|| worker.process_holder.clone())
    };
    #[cfg(windows)]
    let retained_cleanup_retry = retained_holder.is_some();
    #[cfg(not(windows))]
    let retained_cleanup_retry = false;
    #[cfg(windows)]
    if let Some(holder) = retained_holder {
        if !cleanup_process_holder_until(holder, deadline).await {
            if let Some(worker) = lock(&inner.workers)
                .get_mut(key)
                .filter(|worker| worker.generation == generation)
            {
                worker.health.state = WorkerState::Failed;
                worker.lifecycle_reason = Some("cleanup-failed".into());
                worker.restart_allowed = false;
            }
            return;
        }
        let mut workers = lock(&inner.workers);
        let Some(worker) = workers
            .get_mut(key)
            .filter(|worker| worker.generation == generation)
        else {
            return;
        };
        worker.cleanup_started = false;
    }
    let (hello, terminal_disable, retry, heartbeat_task, io_keys) = {
        let mut workers = lock(&inner.workers);
        let Some(worker) = workers.get_mut(key) else {
            return;
        };
        if worker.generation != generation || worker.cleanup_started {
            return;
        }
        worker.cleanup_started = true;
        if state == WorkerState::Failed
            && worker.health.state == WorkerState::Failed
            && !retained_cleanup_retry
        {
            return;
        }
        let retry = if state == WorkerState::Failed && worker.restart_allowed {
            worker.failure_streak = worker.failure_streak.saturating_add(1);
            (worker.failure_streak <= 3)
                .then(|| {
                    worker
                        .launch_spec
                        .clone()
                        .map(|spec| (spec, worker.generation, worker.failure_streak))
                })
                .flatten()
        } else {
            None
        };
        let terminal_disable =
            state == WorkerState::Failed && worker.restart_allowed && worker.failure_streak > 3;
        worker.health.state = state;
        worker.lifecycle_reason = Some(
            match state {
                WorkerState::Failed => "failed",
                WorkerState::Stopped => "stopped",
                WorkerState::Stopping => "stopping",
                WorkerState::Starting => "starting",
                WorkerState::Running => "running",
            }
            .into(),
        );
        worker.grant = None;
        worker.bootstrap_token_hash = None;
        worker.bootstrap_complete = false;
        worker.stdin = None;
        worker.lifecycle_tx = None;
        (
            worker.hello.take(),
            terminal_disable,
            retry,
            worker.heartbeat_task.take(),
            worker.io_keys.clone(),
        )
    };
    #[cfg(windows)]
    let process_holder = lock(&inner.workers)
        .get(key)
        .filter(|worker| worker.generation == generation)
        .map(|worker| worker.process_holder.clone());
    let mut cleanup_ok = true;
    let cancel_ok = inner
        .calls
        .cancel_generation_until(&key.0, &key.1, generation, deadline)
        .await;
    if !cancel_ok {
        cleanup_ok = false;
    }
    if let Some(hello) = hello {
        let _ = hello.send(Err("worker-unavailable"));
    }
    if !inner.worker_io.cancel_keys_until(&io_keys, deadline).await {
        cleanup_ok = false;
    }
    #[cfg(windows)]
    if let Some(holder) = process_holder {
        let Some(mut process) = lock_holder_until(&holder, deadline).await else {
            return;
        };
        if let Some(worker_process) = process.as_mut() {
            if worker_process.stop_until(deadline).await.is_err() {
                cleanup_ok = false;
            } else {
                *process = None;
            }
        }
    }
    if let Some(task) = heartbeat_task {
        task.abort();
        if !join_task_until(task, deadline).await {
            cleanup_ok = false;
        }
    }
    if !cleanup_ok {
        if let Some(worker) = lock(&inner.workers)
            .get_mut(key)
            .filter(|worker| worker.generation == generation)
        {
            worker.health.state = WorkerState::Failed;
            worker.restart_allowed = false;
            worker.lifecycle_reason = Some("cleanup-failed".into());
        }
        tracing::error!(target: "package_worker", package_id = %key.0, version = %key.1, generation, "worker cleanup failed");
        return;
    }
    let still_exact = lock(&inner.workers)
        .get(key)
        .is_some_and(|worker| worker.generation == generation && worker.cleanup_started);
    if !still_exact {
        return;
    }
    tracing::info!(target: "package_worker", package_id = %key.0, version = %key.1, generation, state = ?state, "worker lifecycle transition");
    if terminal_disable {
        let still_exact = lock(&inner.workers)
            .get(key)
            .is_some_and(|worker| worker.generation == generation);
        if still_exact {
            if let Some(store) = lock(&inner.store).clone() {
                let _ = store.disable(&key.0, &key.1);
            }
        }
    }
    if let Some((spec, generation, failures)) = retry {
        #[cfg(not(windows))]
        {
            // Package workers are intentionally unsupported on Unix; leave the
            // failed state recorded rather than attempting a Windows launch.
            let _ = (spec, generation, failures);
            return;
        }
        #[cfg(windows)]
        {
            if restart_delay(failures).is_some() {
                let retry_inner = inner.clone();
                let retry_key = key.clone();
                let old_lifecycle_key = TaskKey::Lifecycle {
                    package: key.0.clone(),
                    version: key.1.clone(),
                    generation,
                };
                inner.retry_tasks.reap_completed().await;
                let retry_registry_key = TaskKey::Retry {
                    package: key.0.clone(),
                    version: key.1.clone(),
                    generation,
                };
                let Some((retry_start, mut retry_cancel)) =
                    inner.retry_tasks.reserve(retry_registry_key.clone())
                else {
                    return;
                };
                let retry_task = tokio::spawn(async move {
                    let retry_started = tokio::select! {
                        _ = &mut retry_cancel => return,
                        started = retry_start => started.unwrap_or(false),
                    };
                    if !retry_started {
                        return;
                    }
                    let joined = tokio::select! {
                        _ = &mut retry_cancel => return,
                        joined = retry_inner.lifecycles.join_key(&old_lifecycle_key) => joined,
                    };
                    if !joined {
                        return;
                    }
                    let inner = retry_inner;
                    let key = retry_key;
                    let delay = restart_delay(failures).expect("retry delay");
                    let next_generation = generation.saturating_add(1);
                    tokio::select! {
                        _ = &mut retry_cancel => return,
                        _ = time::sleep(delay) => {}
                    }
                    let supervisor = PackageWorkerSupervisor {
                        inner: inner.clone(),
                    };
                    let valid = {
                        let workers = lock(&inner.workers);
                        workers.get(&key).is_some_and(|worker| {
                            worker.health.state == WorkerState::Failed
                                && worker.restart_allowed
                                && worker.generation == generation
                        })
                    };
                    if !valid {
                        tracing::info!(target: "package_worker", package_id = %key.0, version = %key.1, generation, "worker retry canceled");
                        return;
                    }
                    if let Some(store) = lock(&inner.store).clone() {
                        let Ok(installed) = store.installed(&key.0, &key.1) else {
                            tracing::warn!(target: "package_worker", package_id = %key.0, version = %key.1, "worker retry package missing");
                            return;
                        };
                        let Ok(entrypoint) = store.immutable_entrypoint(&installed) else {
                            tracing::warn!(target: "package_worker", package_id = %key.0, version = %key.1, "worker retry immutable entrypoint invalid");
                            return;
                        };
                        if !installed.enabled
                            || installed.revoked
                            || !installed.hash.eq_ignore_ascii_case(&spec.hash)
                            || entrypoint != spec.executable
                        {
                            tracing::warn!(target: "package_worker", package_id = %key.0, version = %key.1, "worker retry package state invalid");
                            return;
                        }
                    }
                    {
                        let mut workers = lock(&inner.workers);
                        let Some(worker) = workers.get_mut(&key) else {
                            return;
                        };
                        if worker.health.state != WorkerState::Failed
                            || !worker.restart_allowed
                            || worker.generation != generation
                        {
                            return;
                        }
                        worker.health.state = WorkerState::Starting;
                        worker.health.restart_count = failures as u32;
                        worker.generation = next_generation;
                        worker.lifecycle_reason = Some("restarting".into());
                    }
                    tracing::info!(target: "package_worker", package_id = %key.0, version = %key.1, generation = next_generation, "worker retry");
                    inner.startups.reap_completed().await;
                    let start_result = tokio::select! {
                        _ = &mut retry_cancel => return,
                        result = supervisor.start_windows_boxed(
                            key.clone(),
                            spec,
                            next_generation,
                            failures as u32,
                            failures,
                            true,
                        ) => result,
                    };
                    if start_result.is_err() {
                        let should_finish = {
                            let workers = lock(&inner.workers);
                            workers.get(&key).is_some_and(|worker| {
                                worker.generation == next_generation
                                    && worker.health.state == WorkerState::Starting
                            })
                        };
                        if should_finish {
                            if let Some(tx) = lock(&inner.workers)
                                .get(&key)
                                .and_then(|worker| worker.lifecycle_tx.clone())
                            {
                                let _ = tx.send(WorkerLifecycleEvent::Finish {
                                    generation: next_generation,
                                    state: WorkerState::Failed,
                                });
                            }
                        }
                    }
                });
                if let Err(task) = inner.retry_tasks.install(&retry_registry_key, retry_task) {
                    let _ = task.await;
                }
            }
        }
    }
}

#[cfg(windows)]
impl PackageWorkerSupervisor {
    fn start_windows_boxed<'a>(
        &'a self,
        key: (String, String),
        spec: LaunchSpec,
        generation: u64,
        restart_count: u32,
        failure_streak: u8,
        restart_allowed: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send + 'a>> {
        Box::pin(self.start_windows(
            key,
            spec,
            generation,
            restart_count,
            failure_streak,
            restart_allowed,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn windows_start_transaction_signature_is_compile_checked() {
        #[allow(clippy::too_many_arguments)]
        fn call_chain(
            supervisor: &PackageWorkerSupervisor,
            key: (String, String),
            spec: LaunchSpec,
            generation: u64,
            restart_count: u32,
            failure_streak: u8,
            restart_allowed: bool,
            cancel_rx: oneshot::Receiver<()>,
            owner: Arc<LaunchCleanupOwner>,
            process_holder: WorkerProcessHolder,
        ) -> impl std::future::Future<Output = Result<(), &'static str>> + '_ {
            supervisor.start_windows_transaction(
                key,
                spec,
                generation,
                restart_count,
                failure_streak,
                restart_allowed,
                cancel_rx,
                owner,
                process_holder,
            )
        }
        let _ = call_chain;
    }

    #[tokio::test]
    async fn task_registry_publishes_before_start_and_drains_cancelled_tasks() {
        let registry = TaskRegistry::new(1);
        let key = TaskKey::Call {
            package: "pkg".into(),
            version: "1".into(),
            generation: 7,
            id: "call-1".into(),
        };
        let (start_rx, _cancel_rx) = registry.reserve(key.clone()).expect("reservation");
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started_task = started.clone();
        let task = tokio::spawn(async move {
            if !start_rx.await.unwrap_or(false) {
                return;
            }
            started_task.store(true, Ordering::SeqCst);
            std::future::pending::<()>().await;
        });
        assert!(!started.load(Ordering::SeqCst));
        assert!(registry.install(&key, task).is_ok());
        tokio::task::yield_now().await;
        assert!(started.load(Ordering::SeqCst));
        assert_eq!(registry.len(), 1);
        assert!(!registry.shutdown().await);
        assert_eq!(registry.len(), 0);
    }

    #[tokio::test]
    async fn worker_pipe_tasks_are_installed_before_any_gate_opens() {
        let registry = TaskRegistry::new(3);
        let keys = [
            TaskKey::WorkerStdin {
                package: "pkg".into(),
                version: "1".into(),
                generation: 1,
            },
            TaskKey::WorkerStdout {
                package: "pkg".into(),
                version: "1".into(),
                generation: 1,
            },
            TaskKey::WorkerStderr {
                package: "pkg".into(),
                version: "1".into(),
                generation: 1,
            },
        ];
        let mut gates = Vec::new();
        for key in &keys {
            gates.push((
                key.clone(),
                registry.reserve(key.clone()).expect("reservation").0,
            ));
        }
        let started = (0..3)
            .map(|_| Arc::new(std::sync::atomic::AtomicBool::new(false)))
            .collect::<Vec<_>>();
        let mut tasks = Vec::new();
        for (index, (key, gate)) in gates.into_iter().enumerate() {
            let marker = started[index].clone();
            let task = tokio::spawn(async move {
                if gate.await.unwrap_or(false) {
                    marker.store(true, Ordering::SeqCst);
                }
            });
            registry.install_pending(&key, task).expect("install");
            tasks.push(key);
        }
        tokio::task::yield_now().await;
        assert!(started.iter().all(|marker| !marker.load(Ordering::SeqCst)));
        assert!(registry.open(&tasks[0]));
        tokio::task::yield_now().await;
        assert!(started[0].load(Ordering::SeqCst));
        assert!(!started[1].load(Ordering::SeqCst));
        assert!(!started[2].load(Ordering::SeqCst));
        assert!(registry.open(&tasks[1]));
        assert!(registry.open(&tasks[2]));
        tokio::task::yield_now().await;
        assert!(started.iter().all(|marker| marker.load(Ordering::SeqCst)));
        registry.reap_completed().await;
        assert_eq!(registry.len(), 0);
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn owned_registry_keeps_completed_calls_until_explicit_reap() {
        let registry = TaskRegistry::owned(1);
        let key = TaskKey::Call {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
            id: "call-1".into(),
        };
        let (start_rx, _cancel_rx) = registry.reserve(key.clone()).expect("reservation");
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("start gate"));
        });
        registry.install(&key, task).expect("install");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(registry.len(), 1);
        registry.reap_completed().await;
        assert_eq!(registry.len(), 0);
        assert!(registry.reserve(key).is_some());
    }

    #[tokio::test]
    async fn task_registry_reaps_10000_completed_tasks_without_background_work() {
        let registry = TaskRegistry::owned(1);
        for id in 0..10_000 {
            let key = TaskKey::Call {
                package: "pkg".into(),
                version: "1".into(),
                generation: 1,
                id: id.to_string(),
            };
            let (start_rx, _cancel_rx) = registry.reserve(key.clone()).expect("reservation");
            let task = tokio::spawn(async move {
                assert!(start_rx.await.expect("start gate"));
            });
            registry.install(&key, task).expect("install");
            tokio::task::yield_now().await;
            registry.reap_completed().await;
        }
        assert_eq!(registry.len(), 0);
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn shutdown_aborts_pending_task_and_joins_it() {
        let registry = TaskRegistry::owned(1);
        let key = TaskKey::Call {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
            id: "pending".into(),
        };
        let (start_rx, _cancel_rx) = registry.reserve(key.clone()).expect("reservation");
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("start gate"));
            std::future::pending::<()>().await;
        });
        registry.install(&key, task).expect("install");

        assert!(!registry.shutdown().await);
        assert_eq!(registry.len(), 0);
    }

    #[tokio::test]
    async fn startup_timeout_quarantines_without_aborting_and_reaps_after_release() {
        let registry = TaskRegistry::owned(1);
        let key = TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        };
        let (start_rx, cancel_rx) = registry.reserve(key.clone()).expect("reservation");
        let (release_tx, release_rx) = oneshot::channel();
        let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let completed_task = completed.clone();
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("start gate"));
            let _ = cancel_rx.await;
            let _ = release_rx.await;
            completed_task.store(true, Ordering::SeqCst);
        });
        registry.install(&key, task).expect("install");

        assert!(
            !registry
                .cancel_matching(|candidate| candidate == &key)
                .await
        );
        assert!(!completed.load(Ordering::SeqCst));
        assert!(registry.contains_quarantined(&key));
        assert_eq!(registry.len(), 1);
        assert!(registry
            .reserve(TaskKey::Startup {
                package: "pkg".into(),
                version: "1".into(),
                generation: 2,
            })
            .is_none());
        assert!(
            !registry
                .cancel_matching(|candidate| candidate == &key)
                .await
        );
        assert!(!registry.shutdown().await);
        assert!(registry.contains_quarantined(&key));

        release_tx.send(()).expect("release startup");
        tokio::task::yield_now().await;
        registry.reap_completed().await;
        assert!(completed.load(Ordering::SeqCst));
        assert_eq!(registry.len(), 0);
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn quarantined_startup_rejects_exact_replacement_and_preserves_original_handle() {
        let registry = TaskRegistry::owned(2);
        let key = TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        };
        let (start_rx, cancel_rx) = registry.reserve(key.clone()).expect("reservation");
        let (release_tx, release_rx) = oneshot::channel();
        let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let completed_task = completed.clone();
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("start gate"));
            let _ = cancel_rx.await;
            let _ = release_rx.await;
            completed_task.store(true, Ordering::SeqCst);
        });
        registry.install(&key, task).expect("install");

        assert!(
            !registry
                .cancel_matching(|candidate| candidate == &key)
                .await
        );
        assert!(registry.contains_quarantined(&key));
        assert!(registry.reserve(key.clone()).is_none());
        assert!(registry
            .reserve(TaskKey::Startup {
                package: "pkg".into(),
                version: "1".into(),
                generation: 2,
            })
            .is_some());

        assert!(
            !registry
                .cancel_matching(|candidate| candidate == &key)
                .await
        );
        assert!(registry.contains_quarantined(&key));
        assert!(!completed.load(Ordering::SeqCst));

        release_tx.send(()).expect("release startup");
        tokio::task::yield_now().await;
        registry.reap_completed().await;
        assert!(completed.load(Ordering::SeqCst));
        assert!(!registry.contains_quarantined(&key));
        assert!(registry.reserve(key.clone()).is_some());
        registry.remove(&key);
        registry.remove(&TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 2,
        });
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn absent_worker_stop_reports_live_matching_startup_quarantine() {
        let supervisor = PackageWorkerSupervisor::new(1);
        let key = TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 4,
        };
        let (start_rx, cancel_rx) = supervisor
            .inner
            .startups
            .reserve(key.clone())
            .expect("startup reservation");
        let (release_tx, release_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("start gate"));
            let _ = cancel_rx.await;
            let _ = release_rx.await;
        });
        supervisor
            .inner
            .startups
            .install(&key, task)
            .expect("startup install");
        assert!(
            !supervisor
                .inner
                .startups
                .cancel_matching(|candidate| candidate == &key)
                .await
        );

        assert_eq!(supervisor.stop("pkg", "1").await, Err("cleanup-failed"));

        release_tx.send(()).expect("release startup");
        tokio::task::yield_now().await;
        supervisor.inner.startups.reap_completed().await;
        assert_eq!(supervisor.stop("pkg", "1").await, Ok(()));
        assert!(supervisor.stop_all().await.is_ok());
    }

    #[tokio::test]
    async fn absent_worker_stop_is_ok_after_completed_startup_is_reaped() {
        let supervisor = PackageWorkerSupervisor::new(1);
        let key = TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 5,
        };
        let (start_rx, _cancel_rx) = supervisor
            .inner
            .startups
            .reserve(key.clone())
            .expect("startup reservation");
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("start gate"));
        });
        supervisor
            .inner
            .startups
            .install_pending(&key, task)
            .expect("startup install");
        assert!(supervisor.inner.startups.open(&key));
        tokio::task::yield_now().await;
        assert_eq!(supervisor.stop("pkg", "1").await, Ok(()));
        assert_eq!(supervisor.inner.startups.len(), 0);
    }

    #[tokio::test]
    async fn join_key_reports_a_running_quarantined_task_without_dropping_it() {
        let registry = TaskRegistry::owned(1);
        let key = TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        };
        let (start_rx, cancel_rx) = registry.reserve(key.clone()).expect("reservation");
        let (release_tx, release_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("start gate"));
            let _ = cancel_rx.await;
            let _ = release_rx.await;
        });
        registry.install(&key, task).expect("install");
        assert!(
            !registry
                .cancel_matching(|candidate| candidate == &key)
                .await
        );

        assert!(!registry.join_key(&key).await);
        assert!(registry.contains_quarantined(&key));

        release_tx.send(()).expect("release startup");
        tokio::task::yield_now().await;
        assert!(registry.join_key(&key).await);
        assert_eq!(registry.len(), 0);
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn quarantine_insert_is_non_overwriting() {
        let key = TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        };
        let (old_start, _old_cancel) = oneshot::channel();
        let (new_start, _new_cancel) = oneshot::channel();
        let old = TaskSlot {
            start: Some(old_start),
            cancel: None,
            task: Some(tokio::spawn(async {})),
            #[cfg(windows)]
            owner: None,
            #[cfg(windows)]
            process_holder: None,
        };
        let incoming = TaskSlot {
            start: Some(new_start),
            cancel: None,
            task: Some(tokio::spawn(async {})),
            #[cfg(windows)]
            owner: None,
            #[cfg(windows)]
            process_holder: None,
        };
        let mut quarantine = HashMap::new();
        assert!(TaskRegistry::insert_quarantine(&mut quarantine, key.clone(), old).is_ok());
        let incoming = TaskRegistry::insert_quarantine(&mut quarantine, key.clone(), incoming)
            .expect_err("duplicate quarantine key must not overwrite");
        assert!(quarantine.contains_key(&key));
        let incoming_task = incoming.task.expect("incoming handle preserved");
        incoming_task.await.expect("incoming task joined");
    }

    #[tokio::test]
    async fn startup_completion_is_explicitly_reaped_for_large_batches() {
        let registry = TaskRegistry::owned(10_000);
        for generation in 0..10_000 {
            let key = TaskKey::Startup {
                package: "pkg".into(),
                version: "1".into(),
                generation,
            };
            let (start_rx, _cancel_rx) = registry.reserve(key.clone()).expect("reservation");
            let task = tokio::spawn(async move {
                assert!(start_rx.await.expect("start gate"));
            });
            registry.install(&key, task).expect("install");
        }
        assert_eq!(registry.len(), 10_000);
        while registry.len() != 0 {
            tokio::task::yield_now().await;
            registry.reap_completed().await;
        }
        assert_eq!(registry.len(), 0);
    }

    #[tokio::test]
    async fn task_registry_rejects_duplicate_and_capacity_overflow() {
        let registry = TaskRegistry::new(1);
        let first = TaskKey::Lifecycle {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        };
        let second = TaskKey::Lifecycle {
            package: "pkg".into(),
            version: "1".into(),
            generation: 2,
        };
        assert!(registry.reserve(first.clone()).is_some());
        assert!(registry.reserve(first).is_none());
        assert!(registry.reserve(second).is_none());
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn startup_key_is_generation_specific_and_registry_owned() {
        let registry = TaskRegistry::new(2);
        let key = TaskKey::Startup {
            package: "pkg".into(),
            version: "1".into(),
            generation: 9,
        };
        let (start_rx, cancel_rx) = registry.reserve(key.clone()).expect("startup reservation");
        let task = tokio::spawn(async move {
            assert!(start_rx.await.expect("startup gate"));
            let _ = cancel_rx.await;
        });
        registry.install(&key, task).expect("startup install");
        assert!(registry.contains(&key));
        assert!(registry.cancel_generation("pkg", "1", 9).await);
        assert!(!registry.contains(&key));
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn new_generation_has_an_exact_distinct_lifecycle_key() {
        let registry = TaskRegistry::new(2);
        let old = TaskKey::Lifecycle {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        };
        let new = TaskKey::Lifecycle {
            package: "pkg".into(),
            version: "1".into(),
            generation: 2,
        };
        let (_old_start, _old_cancel) = registry.reserve(old).expect("old lifecycle");
        let (_new_start, _new_cancel) = registry.reserve(new).expect("new lifecycle");
        assert_eq!(registry.len(), 2);
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn generation_cleanup_is_exact_and_preserves_replacement_slots() {
        let registry = TaskRegistry::new(6);
        let old_keys = [
            TaskKey::WorkerStdin {
                package: "pkg".into(),
                version: "1".into(),
                generation: 1,
            },
            TaskKey::WorkerStdout {
                package: "pkg".into(),
                version: "1".into(),
                generation: 1,
            },
            TaskKey::WorkerStderr {
                package: "pkg".into(),
                version: "1".into(),
                generation: 1,
            },
        ];
        let replacement = TaskKey::WorkerStdin {
            package: "pkg".into(),
            version: "1".into(),
            generation: 2,
        };
        for key in old_keys.iter().chain(std::iter::once(&replacement)) {
            let (start, _cancel) = registry.reserve(key.clone()).expect("reservation");
            drop(start);
        }
        assert!(registry.has_generation("pkg", "1", 1));
        assert!(registry.has_generation("pkg", "1", 2));
        assert!(registry.cancel_generation("pkg", "1", 1).await);
        assert!(!registry.has_generation("pkg", "1", 1));
        assert!(registry.has_generation("pkg", "1", 2));
        assert!(registry.shutdown().await);
    }

    #[tokio::test]
    async fn stale_finish_cannot_claim_replacement_generation() {
        let supervisor = PackageWorkerSupervisor::new(1);
        let key = ("pkg".into(), "1".into());
        supervisor.insert_failed(key.clone());
        {
            let mut workers = lock(&supervisor.inner.workers);
            let worker = workers.get_mut(&key).expect("worker");
            worker.generation = 2;
            worker.lifecycle_reason = Some("replacement".into());
        }
        finish_inner_until(
            &supervisor.inner,
            &key,
            1,
            WorkerState::Failed,
            Instant::now() + STOP_DEADLINE,
        )
        .await;
        let workers = lock(&supervisor.inner.workers);
        let worker = workers.get(&key).expect("replacement");
        assert_eq!(worker.generation, 2);
        assert_eq!(worker.lifecycle_reason.as_deref(), Some("replacement"));
    }
    #[test]
    fn cleanup_failure_is_a_distinct_diagnostic_reason() {
        let supervisor = PackageWorkerSupervisor::new(1);
        let key = ("cleanup".into(), "1".into());
        supervisor.insert_failed(key.clone());
        if let Some(worker) = lock(&supervisor.inner.workers).get_mut(&key) {
            worker.lifecycle_reason = Some("cleanup-failed".into());
        }
        assert_eq!(
            supervisor.diagnostics()[0].lifecycle_reason.as_deref(),
            Some("cleanup-failed")
        );
    }
    #[test]
    fn app_and_non_exe_fail_closed() {
        let mut m = crate::package_manifest::PackageManifest {
            schema_version: 1,
            id: "x".into(),
            name: "x".into(),
            version: "1.0.0".into(),
            kind: PackageKind::App,
            engine_api: ">=1".into(),
            entrypoint: "x.exe".into(),
            publisher: "kosmos".into(),
            permissions: vec![],
        };
        assert!(PackageWorkerSupervisor::validate_manifest(&m).is_err());
        m.kind = PackageKind::Source;
        m.entrypoint = "x.js".into();
        assert!(PackageWorkerSupervisor::validate_manifest(&m).is_err());
    }
    #[test]
    fn token_hash_does_not_accept_wrong_hello_token() {
        assert!(!token_matches(&hash_token("right"), "wrong"));
    }

    #[test]
    fn restart_policy_uses_bounded_backoff() {
        assert_eq!(restart_delay(1), Some(Duration::from_secs(1)));
        assert_eq!(restart_delay(2), Some(Duration::from_secs(5)));
        assert_eq!(restart_delay(3), Some(Duration::from_secs(30)));
        assert_eq!(restart_delay(4), None);
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn start_is_unsupported_on_non_windows_and_records_failure() {
        let supervisor = PackageWorkerSupervisor::new(1);
        let manifest = PackageManifest {
            schema_version: 1,
            id: "linux-test".into(),
            name: "linux-test".into(),
            version: "1.0.0".into(),
            kind: PackageKind::Source,
            engine_api: ">=1".into(),
            entrypoint: "worker.exe".into(),
            publisher: "kosmos".into(),
            permissions: vec![],
        };
        let result = supervisor
            .start(
                &manifest,
                PathBuf::from("/tmp/worker.exe"),
                "hash".into(),
                &[],
                "correlation".into(),
                None,
            )
            .await;
        assert_eq!(result, Err("unsupported-platform"));
        assert_eq!(
            supervisor.health("linux-test", "1.0.0").state,
            WorkerState::Failed
        );
    }

    #[test]
    fn diagnostics_are_sorted_bounded_and_redacted() {
        let supervisor = PackageWorkerSupervisor::new(1);
        supervisor.insert_failed(("b".into(), "1".into()));
        supervisor.insert_failed(("a".into(), "1".into()));
        let key = ("a".to_string(), "1".to_string());
        let workers = lock(&supervisor.inner.workers);
        let worker = workers.get(&key).expect("worker");
        lock(&worker.stdout_tail).push("token=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa alice@example.com C:\\Users\\alice\\note.txt");
        lock(&worker.stderr_tail).push("scope=filesystem.read /home/alice/private");
        drop(workers);
        let diagnostics = supervisor.diagnostics();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].id, "a");
        let json = serde_json::to_string(&diagnostics).expect("json");
        for forbidden in [
            "secret",
            "alice@example.com",
            "note.txt",
            "filesystem.read",
            "/home/alice",
        ] {
            assert!(!json.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn bridge_status_is_bounded_and_path_free() {
        let status = sanitize_bridge_status(BridgeStatus {
            last_sync: Some("C:\\vault\\body".into()),
            conflict_count: u32::MAX,
            last_conflict_at: Some("2026-07-29T12:00:00Z".into()),
        });
        assert_eq!(status.last_sync, None);
        assert_eq!(status.conflict_count, 1_000_000);
        assert_eq!(
            status.last_conflict_at.as_deref(),
            Some("2026-07-29T12:00:00Z")
        );
    }
}
