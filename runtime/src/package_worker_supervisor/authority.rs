use super::*;
#[path = "authority/data_request.rs"]
mod data_request;
use data_request::execute_data_request;

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
    pub(super) grants: Mutex<Option<Arc<GrantAuthorityRegistry>>>,
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
    pub fn new(api_major: u32) -> Self {
        Self {
            inner: Arc::new(SupervisorInner {
                workers: Mutex::new(HashMap::new()),
                calls: TaskRegistry::owned(MAX_IN_FLIGHT as usize * 256),
                invocations: Mutex::new(HashMap::new()),
                startups: TaskRegistry::owned(256),
                lifecycles: TaskRegistry::owned(256),
                api_major,
                ark: None,
                ark_executor: Arc::new(UnavailableArk),
                typed_launches: Mutex::new(HashMap::new()),
                store: Mutex::new(None),
                retry_tasks: TaskRegistry::owned(256),
                worker_io: TaskRegistry::owned(256 * 3),
                secrets: PackageWorkerSecretRegistry::new(),
                grants: Mutex::new(None),
                #[cfg(test)]
                fail_next_start: std::sync::atomic::AtomicBool::new(false),
            }),
        }
    }
    pub fn with_ark(api_major: u32, ark: Arc<ArkHost>) -> Self {
        let executor: Arc<dyn ArkRequestExecutor> = ark.clone();
        Self {
            inner: Arc::new(SupervisorInner {
                workers: Mutex::new(HashMap::new()),
                calls: TaskRegistry::owned(MAX_IN_FLIGHT as usize * 256),
                invocations: Mutex::new(HashMap::new()),
                startups: TaskRegistry::owned(256),
                lifecycles: TaskRegistry::owned(256),
                api_major,
                ark: Some(ark),
                ark_executor: executor,
                typed_launches: Mutex::new(HashMap::new()),
                store: Mutex::new(None),
                retry_tasks: TaskRegistry::owned(256),
                worker_io: TaskRegistry::owned(256 * 3),
                secrets: PackageWorkerSecretRegistry::new(),
                grants: Mutex::new(None),
                #[cfg(test)]
                fail_next_start: std::sync::atomic::AtomicBool::new(false),
            }),
        }
    }
    pub fn with_ark_executor(api_major: u32, executor: Arc<dyn ArkRequestExecutor>) -> Self {
        Self {
            inner: Arc::new(SupervisorInner {
                workers: Mutex::new(HashMap::new()),
                calls: TaskRegistry::owned(MAX_IN_FLIGHT as usize * 256),
                invocations: Mutex::new(HashMap::new()),
                startups: TaskRegistry::owned(256),
                lifecycles: TaskRegistry::owned(256),
                api_major,
                ark: None,
                ark_executor: executor,
                typed_launches: Mutex::new(HashMap::new()),
                store: Mutex::new(None),
                retry_tasks: TaskRegistry::owned(256),
                worker_io: TaskRegistry::owned(256 * 3),
                secrets: PackageWorkerSecretRegistry::new(),
                grants: Mutex::new(None),
                #[cfg(test)]
                fail_next_start: std::sync::atomic::AtomicBool::new(false),
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

pub(super) async fn dispatch_typed_inner(
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
