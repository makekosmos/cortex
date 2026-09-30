use super::*;

pub(super) const HELLO_DEADLINE: Duration = Duration::from_secs(10);
pub(super) const BOOTSTRAP_DEADLINE: Duration = Duration::from_secs(10);
pub(super) const HEARTBEAT_DEADLINE: Duration = Duration::from_secs(60);
#[cfg(not(test))]
pub(super) const STOP_DEADLINE: Duration = Duration::from_secs(10);

#[cfg(windows)]
#[path = "types/windows.rs"]
mod windows;
#[cfg(windows)]
pub(super) use windows::*;
#[cfg(test)]
pub(super) const STOP_DEADLINE: Duration = Duration::from_secs(2);
pub(super) const PROCESS_LAUNCH_DEADLINE: Duration = Duration::from_secs(10);
pub(super) const MAX_IN_FLIGHT: u32 = 4;
pub(super) const RESTART_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(5),
    Duration::from_secs(30),
];

pub(super) fn worker_error_class(error: Option<&str>) -> &'static str {
    let Some(value) = error.filter(|value| {
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    }) else {
        return "unavailable";
    };
    match value {
        "forbidden" => "forbidden",
        "invalid-request" => "invalid-request",
        "not-found" => "not-found",
        "conflict" => "conflict",
        "timeout" => "timeout",
        _ => "unavailable",
    }
}

#[derive(Debug, Clone)]
pub(super) struct LaunchSpec {
    pub(super) manifest: PackageManifest,
    pub(super) executable: PathBuf,
    pub(super) state_root: PathBuf,
    pub(super) hash: String,
    pub(super) roots: Vec<PathBuf>,
    pub(super) correlation_id: String,
    pub(super) bridge_config: Option<BridgeWorkerConfig>,
    pub(super) integration: Option<IntegrationLaunchConfig>,
}

#[derive(Clone)]
pub struct IntegrationLaunchConfig {
    pub manifest: IntegrationManifest,
    pub values: HashMap<String, String>,
    pub secrets: HashMap<String, String>,
}

impl std::fmt::Debug for IntegrationLaunchConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IntegrationLaunchConfig")
            .field("manifest", &self.manifest)
            .field("values", &self.values)
            .field("secret_keys", &self.secrets.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Drop for IntegrationLaunchConfig {
    fn drop(&mut self) {
        for secret in self.secrets.values_mut() {
            crate::package_worker_secrets::zeroize_secret(secret);
        }
    }
}

pub(super) fn restart_delay(delays: &[Duration], failures: u8) -> Option<Duration> {
    delays.get(failures.saturating_sub(1) as usize).copied()
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

pub(super) struct LiveWorker {
    pub(super) health: WorkerHealth,
    pub(super) generation: u64,
    pub(super) started: Instant,
    pub(super) last_heartbeat: Instant,
    pub(super) grant: Option<Grant>,
    pub(super) bootstrap_token_hash: Option<[u8; 32]>,
    #[cfg(windows)]
    pub(super) process_holder: WorkerProcessHolder,
    pub(super) stdin: Option<mpsc::UnboundedSender<Vec<u8>>>,
    pub(super) io_keys: [TaskKey; 3],
    pub(super) lifecycle_tx: Option<mpsc::UnboundedSender<WorkerLifecycleEvent>>,
    pub(super) heartbeat_task: Option<tokio::task::JoinHandle<()>>,
    pub(super) schedule_task: Option<tokio::task::JoinHandle<()>>,
    pub(super) hello: Option<oneshot::Sender<Result<(), &'static str>>>,
    pub(super) bootstrap_complete: bool,
    pub(super) cleanup_started: bool,
    pub(super) broker: BrokerConfig,
    pub(super) stdout_tail: Arc<Mutex<BoundedTextTail>>,
    pub(super) stderr_tail: Arc<Mutex<BoundedTextTail>>,
    pub(super) bridge_status: Option<BridgeStatus>,
    pub(super) in_flight: u32,
    pub(super) restart_allowed: bool,
    pub(super) launch_spec: Option<LaunchSpec>,
    pub(super) failure_streak: u8,
    pub(super) lifecycle_reason: Option<String>,
}

#[cfg(windows)]
pub(super) struct LaunchTransaction {
    pub(super) inner: Arc<SupervisorInner>,
    pub(super) lifecycle_key: TaskKey,
    pub(super) worker_io_keys: Vec<TaskKey>,
    pub(super) process_holder: WorkerProcessHolder,
}

#[cfg(windows)]
pub(super) type WorkerProcessHolder = Arc<AsyncMutex<Option<WorkerProcess>>>;

#[cfg(windows)]
pub(super) async fn cancellation_pending(cancel: &mut oneshot::Receiver<()>) -> bool {
    tokio::select! {
        _ = cancel => true,
        _ = tokio::task::yield_now() => false,
    }
}

#[cfg(windows)]
pub(super) async fn startup_wait<T, F>(
    cancel: &mut oneshot::Receiver<()>,
    future: F,
) -> Result<T, ()>
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
    pub(super) async fn rollback_error(
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

    pub(super) async fn rollback(self, deadline: Instant) -> bool {
        if let TaskKey::Lifecycle {
            package,
            generation,
            ..
        } = &self.lifecycle_key
        {
            self.inner.secrets.revoke_generation(package, *generation);
        }
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
pub(super) enum WorkerLifecycleEvent {
    Finish { generation: u64, state: WorkerState },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum TaskKey {
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

pub(super) struct TaskSlot {
    pub(super) start: Option<oneshot::Sender<bool>>,
    pub(super) cancel: Option<oneshot::Sender<()>>,
    pub(super) task: Option<tokio::task::JoinHandle<()>>,
    #[cfg(windows)]
    pub(super) owner: Option<Arc<LaunchCleanupOwner>>,
    #[cfg(windows)]
    pub(super) process_holder: Option<WorkerProcessHolder>,
}
