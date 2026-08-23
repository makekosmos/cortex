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

