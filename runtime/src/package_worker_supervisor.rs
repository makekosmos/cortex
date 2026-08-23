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

include!("package_worker_supervisor_core.rs");
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

include!("package_worker_supervisor_runtime.rs");
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
#[cfg(test)]
mod tests {
    include!("package_worker_supervisor_tests.rs");
}
