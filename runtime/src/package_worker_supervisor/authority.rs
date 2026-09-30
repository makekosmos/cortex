use super::*;
#[path = "authority/data_request.rs"]
mod data_request;
#[path = "authority/typed_request.rs"]
mod typed_request;
use data_request::execute_data_request;
pub(super) use typed_request::dispatch_typed_inner;

pub(super) struct SupervisorInner {
    pub(super) workers: Mutex<HashMap<(String, String), LiveWorker>>,
    pub(super) calls: Arc<TaskRegistry>,
    pub(super) invocations:
        Mutex<HashMap<(String, String, u64, String), oneshot::Sender<ResultMessage>>>,
    pub(super) startups: Arc<TaskRegistry>,
    pub(super) lifecycles: Arc<TaskRegistry>,
    pub(super) api_major: u32,
    pub(super) ark: Option<Arc<ArkHost>>,
    pub(super) ark_executor: Arc<dyn ArkRequestExecutor>,
    pub(super) typed_launches: Mutex<HashMap<(String, String), TypedLaunch>>,
    pub(super) store: Mutex<Option<Arc<PackageStore>>>,
    pub(super) retry_tasks: Arc<TaskRegistry>,
    pub(super) worker_io: Arc<TaskRegistry>,
    pub(super) secrets: PackageWorkerSecretRegistry,
    pub(super) network_responses: package_worker_broker::SnapshotRegistry,
    pub(super) network_slots: tokio::sync::Semaphore,
    pub(super) grants: Mutex<Option<Arc<GrantAuthorityRegistry>>>,
    /// Backoff schedule between automatic worker restarts. Production
    /// supervisors use [`RESTART_DELAYS`]; tests inject a shorter schedule via
    /// [`PackageWorkerSupervisor::with_restart_delays`] so retry exhaustion
    /// does not wait on real backoff.
    // Package workers only launch on Windows; on other targets the field is
    // stored but never read.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(super) restart_delays: Vec<Duration>,
    #[cfg(test)]
    pub(super) fail_next_start: std::sync::atomic::AtomicBool,
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
        if operation == "data.request" {
            let request =
                serde_json::from_value::<DataRequest>(params).map_err(|_| "invalid-request")?;
            return execute_data_request(self, request).await;
        }
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
pub(super) struct TypedLaunch {
    pub(super) session_id: String,
    pub(super) generation: u64,
    pub(super) grant: LaunchGrant,
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
pub(super) static NEXT_AFTER_LAUNCH_GATE: std::sync::OnceLock<Mutex<Option<AfterLaunchGateParts>>> =
    std::sync::OnceLock::new();

#[cfg(all(windows, feature = "package-worker-fixture"))]
pub(super) struct AfterLaunchGateParts {
    pub(super) ready: oneshot::Sender<()>,
    pub(super) release: oneshot::Receiver<()>,
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
pub struct AfterLaunchGate {
    pub(super) ready: oneshot::Receiver<()>,
    pub(super) release: Option<oneshot::Sender<()>>,
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
    pub(super) ready: oneshot::Receiver<()>,
    pub(super) release: Option<oneshot::Sender<()>>,
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
    pub(super) inner: Arc<SupervisorInner>,
}

impl PackageWorkerSupervisor {
    #[cfg(test)]
    pub fn test_fail_next_start(&self) {
        self.inner
            .fail_next_start
            .store(true, std::sync::atomic::Ordering::Release);
    }
    fn build(
        api_major: u32,
        ark: Option<Arc<ArkHost>>,
        ark_executor: Arc<dyn ArkRequestExecutor>,
        restart_delays: Vec<Duration>,
    ) -> Self {
        Self {
            inner: Arc::new(SupervisorInner {
                workers: Mutex::new(HashMap::new()),
                calls: TaskRegistry::owned(MAX_IN_FLIGHT as usize * 256),
                invocations: Mutex::new(HashMap::new()),
                startups: TaskRegistry::owned(256),
                lifecycles: TaskRegistry::owned(256),
                api_major,
                ark,
                ark_executor,
                typed_launches: Mutex::new(HashMap::new()),
                store: Mutex::new(None),
                retry_tasks: TaskRegistry::owned(256),
                worker_io: TaskRegistry::owned(256 * 3),
                secrets: PackageWorkerSecretRegistry::new(),
                network_responses: package_worker_broker::SnapshotRegistry::network_responses(),
                network_slots: tokio::sync::Semaphore::new(4),
                grants: Mutex::new(None),
                restart_delays,
                #[cfg(test)]
                fail_next_start: std::sync::atomic::AtomicBool::new(false),
            }),
        }
    }
    pub fn new(api_major: u32) -> Self {
        Self::build(
            api_major,
            None,
            Arc::new(UnavailableArk),
            RESTART_DELAYS.to_vec(),
        )
    }
    /// Same as [`Self::new`], with a caller-provided retry backoff schedule.
    /// Integration tests inject short delays so exhaust-retry cases finish
    /// without sleeping through the production backoff.
    pub fn with_restart_delays(api_major: u32, restart_delays: Vec<Duration>) -> Self {
        Self::build(api_major, None, Arc::new(UnavailableArk), restart_delays)
    }
    pub fn with_ark(api_major: u32, ark: Arc<ArkHost>) -> Self {
        let executor: Arc<dyn ArkRequestExecutor> = ark.clone();
        Self::build(api_major, Some(ark), executor, RESTART_DELAYS.to_vec())
    }
    pub fn with_ark_executor(api_major: u32, executor: Arc<dyn ArkRequestExecutor>) -> Self {
        Self::build(api_major, None, executor, RESTART_DELAYS.to_vec())
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

    pub fn revoke_package_secrets(&self, id: &str) {
        self.inner.secrets.revoke_package(id);
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
