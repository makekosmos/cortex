use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::header::{AUTHORIZATION, CONTENT_TYPE};
use hyper::service::service_fn;
use hyper::HeaderMap;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde_json::{json, Value};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::sync::Notify;

use crate::auth;
use crate::engine_dispatch::{DispatchClient, DispatchRequest, Operation};
use crate::package_service::PackageService;
use crate::protocol_usage::ProtocolUsageStore;
use crate::protocol_version::{
    Compatibility, ProtocolVersion, API_VERSION, API_VERSION_CURRENT, PROTOCOL_VERSION,
};
use crate::runtime_grants::{DataRequest, FieldInput, LaunchGrant};

const MAX_HTTP_BODY_BYTES: usize = 1024 * 1024;
const LAUNCH_LEASE_TTL: Duration = Duration::from_secs(300);
const DATA_GRANT_TTL: Duration = Duration::from_secs(900);
const MAX_ACTIVE_LAUNCH_LEASES: usize = 2_048;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CLIENT_PID_HEADER: &str = "x-kosmos-client-pid";
const API_VERSION_HEADER: &str = "x-kosmos-api-version";
const CLIENT_CLASS_HEADER: &str = "x-kosmos-client-class";
const CLIENT_VERSION_HEADER: &str = "x-kosmos-client-version";
const APP_LAUNCH_TOKEN_HEADER: &str = "x-kosmos-launch-token";

type HttpResponse = Response<Full<Bytes>>;

const MAX_IN_FLIGHT_HTTP_OPERATIONS: usize = 128;
const MAX_ACTIVE_HTTP_CONNECTIONS: usize = 128;
const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(5);

struct HttpConnectionLifecycle {
    admission: Mutex<()>,
    closed: AtomicBool,
    shutdown: Notify,
    capacity: Arc<tokio::sync::Semaphore>,
    tasks: Mutex<HashMap<u64, HttpConnectionSlot>>,
    next_task: std::sync::atomic::AtomicU64,
}

enum HttpConnectionSlot {
    Reserved {
        permit: Arc<Mutex<Option<tokio::sync::OwnedSemaphorePermit>>>,
        start: oneshot::Sender<()>,
    },
    Installed(tokio::task::JoinHandle<()>),
}

impl Default for HttpConnectionLifecycle {
    fn default() -> Self {
        Self {
            admission: Mutex::new(()),
            closed: AtomicBool::new(false),
            shutdown: Notify::new(),
            capacity: Arc::new(tokio::sync::Semaphore::new(MAX_ACTIVE_HTTP_CONNECTIONS)),
            tasks: Mutex::new(HashMap::new()),
            next_task: std::sync::atomic::AtomicU64::new(1),
        }
    }
}

impl HttpConnectionLifecycle {
    async fn begin_shutdown(&self) {
        let _admission = self.admission.lock().unwrap_or_else(|p| p.into_inner());
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.shutdown.notify_waiters();
        }
    }

    async fn cancelled(&self) {
        let notified = self.shutdown.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        notified.await;
    }

    async fn reap(&self) {
        let finished = {
            let mut tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
            let ids = tasks
                .iter()
                .filter_map(|(id, slot)| match slot {
                    HttpConnectionSlot::Installed(task) if task.is_finished() => Some(*id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            ids.into_iter()
                .filter_map(|id| tasks.remove(&id))
                .collect::<Vec<_>>()
        };
        for slot in finished {
            if let HttpConnectionSlot::Installed(task) = slot {
                let _ = task.await;
            }
        }
    }

    fn task_count(&self) -> usize {
        self.tasks.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    async fn drain(&self, deadline: Duration) -> Result<(), &'static str> {
        self.begin_shutdown().await;
        let started = Instant::now();
        let tasks = {
            let mut registry = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
            registry.drain().map(|(_, slot)| slot).collect::<Vec<_>>()
        };
        let mut timed_out = false;
        for slot in tasks {
            let HttpConnectionSlot::Installed(mut task) = slot else {
                continue;
            };
            let remaining = deadline.saturating_sub(started.elapsed());
            match tokio::time::timeout(remaining, &mut task).await {
                Ok(Ok(())) | Ok(Err(_)) => {}
                Err(_) => {
                    timed_out = true;
                    task.abort();
                    let _ = task.await;
                }
            }
        }
        self.reap().await;
        if timed_out || self.task_count() != 0 {
            tracing::error!(
                ?deadline,
                remaining = self.task_count(),
                "HTTP connection shutdown exceeded its bounded cleanup lifecycle"
            );
            Err("HTTP connection shutdown exceeded its deadline")
        } else {
            Ok(())
        }
    }
}

struct OwnedHttpOperation {
    task: tokio::task::JoinHandle<()>,
    cleanup: Arc<HttpOperationCleanup>,
    cancel: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

enum HttpOperationSlot {
    Reserved {
        cleanup: Arc<HttpOperationCleanup>,
        permit: Arc<Mutex<Option<tokio::sync::OwnedSemaphorePermit>>>,
    },
    Installed(OwnedHttpOperation),
}

struct ResponseWaitGuard {
    abandon: Option<oneshot::Sender<()>>,
}

impl ResponseWaitGuard {
    fn disarm(&mut self) {
        self.abandon.take();
    }
}

impl Drop for ResponseWaitGuard {
    fn drop(&mut self) {
        if let Some(abandon) = self.abandon.take() {
            let _ = abandon.send(());
        }
    }
}

struct HttpOperationCleanup {
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    owner: crate::engine_dispatch::OwnerLease,
    completed: AtomicBool,
}

impl HttpOperationCleanup {
    fn run(&self) {
        if !self.completed.swap(true, Ordering::AcqRel) {
            self.dispatcher.cleanup_connection_sync(&self.owner);
        }
    }
}

impl Drop for HttpOperationCleanup {
    fn drop(&mut self) {
        self.run();
    }
}

#[derive(Clone)]
struct HttpOperationRegistry {
    permits: Arc<tokio::sync::Semaphore>,
    continuations: Arc<Mutex<HashMap<u64, HttpOperationSlot>>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    admission: Arc<Mutex<()>>,
    closed: Arc<AtomicBool>,
    serve_shutdown: Arc<Notify>,
    continuation_deadline: Duration,
}

impl Default for HttpOperationRegistry {
    fn default() -> Self {
        Self {
            permits: Arc::new(tokio::sync::Semaphore::new(MAX_IN_FLIGHT_HTTP_OPERATIONS)),
            continuations: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            admission: Arc::new(Mutex::new(())),
            closed: Arc::new(AtomicBool::new(false)),
            serve_shutdown: Arc::new(Notify::new()),
            continuation_deadline: Duration::from_secs(5),
        }
    }
}

impl HttpOperationRegistry {
    async fn start(
        &self,
        request: crate::engine_dispatch::DispatchRequest,
        dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
        owner: crate::engine_dispatch::OwnerLease,
    ) -> Option<(
        u64,
        oneshot::Receiver<crate::engine_dispatch::DispatchResult>,
        ResponseWaitGuard,
    )> {
        self.reap().await;
        let permit = self.permits.clone().try_acquire_owned().ok()?;
        let (sender, receiver) = oneshot::channel();
        let (cancel_sender, cancel_receiver) = oneshot::channel();
        let cancel = Arc::new(Mutex::new(Some(cancel_sender)));
        let (abandon_sender, abandon_receiver) = oneshot::channel();
        let operation_id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let cleanup = Arc::new(HttpOperationCleanup {
            dispatcher: dispatcher.clone(),
            owner,
            completed: AtomicBool::new(false),
        });
        let task_cleanup = cleanup.clone();
        let continuation_deadline = self.continuation_deadline;
        let _admission = self.admission.lock().unwrap_or_else(|p| p.into_inner());
        if self.closed.load(Ordering::Acquire) {
            cleanup.run();
            return None;
        }
        let permit_cell = Arc::new(Mutex::new(Some(permit)));
        let (start_sender, start_receiver) = oneshot::channel();
        self.continuations
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                operation_id,
                HttpOperationSlot::Reserved {
                    cleanup: cleanup.clone(),
                    permit: permit_cell.clone(),
                },
            );
        let handle = tokio::spawn(async move {
            if start_receiver.await.is_err() {
                return;
            }
            let _permit = permit_cell.lock().unwrap_or_else(|p| p.into_inner()).take();
            let dispatch = dispatcher.dispatch(request);
            tokio::pin!(dispatch);
            let result = tokio::select! {
                result = &mut dispatch => result,
                _ = cancel_receiver => Err(crate::engine_dispatch::DispatchError::Cancelled),
                _ = abandon_receiver => {
                    match tokio::time::timeout(continuation_deadline, &mut dispatch).await {
                        Ok(result) => result,
                        Err(_) => Err(crate::engine_dispatch::DispatchError::Timeout),
                    }
                }
            };
            task_cleanup.run();
            let _ = sender.send(result);
        });
        let old = self
            .continuations
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                operation_id,
                HttpOperationSlot::Installed(OwnedHttpOperation {
                    task: handle,
                    cleanup,
                    cancel,
                }),
            );
        debug_assert!(matches!(old, Some(HttpOperationSlot::Reserved { .. })));
        let _ = start_sender.send(());
        Some((
            operation_id,
            receiver,
            ResponseWaitGuard {
                abandon: Some(abandon_sender),
            },
        ))
    }

    async fn reap(&self) {
        let finished = {
            let mut guard = self.continuations.lock().unwrap_or_else(|p| p.into_inner());
            let ids = guard
                .iter()
                .filter_map(|(id, slot)| match slot {
                    HttpOperationSlot::Installed(operation) if operation.task.is_finished() => {
                        Some(*id)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            ids.into_iter()
                .filter_map(|id| guard.remove(&id))
                .collect::<Vec<_>>()
        };
        for slot in finished {
            if let HttpOperationSlot::Installed(operation) = slot {
                let _ = operation.task.await;
            }
        }
    }

    async fn begin_shutdown(&self) {
        let _admission = self.admission.lock().unwrap_or_else(|p| p.into_inner());
        if !self.closed.swap(true, Ordering::AcqRel) {
            // Retain the signal if the accept loop is between select polls.
            self.serve_shutdown.notify_one();
        }
    }

    pub async fn shutdown(&self) -> Result<(), &'static str> {
        self.begin_shutdown().await;
        self.shutdown_with_deadline(SHUTDOWN_DEADLINE).await
    }

    async fn shutdown_with_deadline(&self, deadline: Duration) -> Result<(), &'static str> {
        let started = Instant::now();
        let operations = {
            let mut guard = self.continuations.lock().unwrap_or_else(|p| p.into_inner());
            guard
                .drain()
                .map(|(_, operation)| operation)
                .collect::<Vec<_>>()
        };
        let mut timed_out = false;
        for slot in operations {
            let HttpOperationSlot::Installed(mut operation) = slot else {
                continue;
            };
            if let Some(cancel) = operation
                .cancel
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take()
            {
                let _ = cancel.send(());
            }
            let remaining = deadline.saturating_sub(started.elapsed());
            match tokio::time::timeout(remaining, &mut operation.task).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) | Err(_) => {
                    timed_out = true;
                    operation.task.abort();
                    let _ = operation.task.await;
                }
            }
            operation.cleanup.run();
        }
        if timed_out {
            tracing::error!(?deadline, elapsed = ?started.elapsed(), "HTTP shutdown exceeded its bounded cleanup lifecycle");
            Err("HTTP shutdown exceeded its deadline")
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
pub struct EngineApiShutdownHandle {
    operations: HttpOperationRegistry,
    connections: Arc<HttpConnectionLifecycle>,
}

impl EngineApiShutdownHandle {
    pub async fn begin_shutdown(&self) {
        self.connections.begin_shutdown().await;
        self.operations.begin_shutdown().await;
    }

    pub async fn shutdown(&self) -> Result<(), &'static str> {
        let connections = self.connections.drain(SHUTDOWN_DEADLINE).await;
        let operations = self.operations.shutdown().await;
        connections.and(operations)
    }
}

#[derive(Debug, Error)]
pub enum EngineApiError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("package definition registration failed: {0}")]
    PackageDefinitionRegistration(String),
}

pub(crate) async fn register_package_definitions(
    package_service: &PackageService,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
) -> Result<(), String> {
    package_service
        .register_package_definitions(dispatcher)
        .await
        .map_err(|error| error.to_string())
}

pub struct EngineApiServer {
    listener: TcpListener,
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    auth_token: Arc<String>,
    ws_port: u16,
    protocol_usage: Arc<ProtocolUsageStore>,
    correlation_id: Arc<String>,
    package_service: Arc<PackageService>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    cleanup_interval: Duration,
    operations: HttpOperationRegistry,
    connections: Arc<HttpConnectionLifecycle>,
    request_timeout: Duration,
}

#[derive(Debug, Clone)]
struct AssetGrant {
    id: String,
    version: String,
    hash: String,
}

#[derive(Debug, Clone)]
struct LaunchLease {
    launch_id: String,
    asset_token: String,
    launch_token: Option<String>,
    grant: AssetGrant,
    typed_grant: Option<LaunchGrant>,
    expires_at: Instant,
    expires_at_rfc3339: String,
    grant_expires_at: Option<Instant>,
    grant_expires_at_rfc3339: Option<String>,
}

#[derive(Debug)]
struct LaunchLeaseRegistry {
    leases: HashMap<String, LaunchLease>,
    asset_tokens: HashMap<String, String>,
    ttl: Duration,
    capacity: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LeaseCapacityError;

impl Default for LaunchLeaseRegistry {
    fn default() -> Self {
        Self::with_limits(LAUNCH_LEASE_TTL, MAX_ACTIVE_LAUNCH_LEASES)
    }
}

impl LaunchLeaseRegistry {
    fn with_limits(ttl: Duration, capacity: usize) -> Self {
        Self {
            leases: HashMap::new(),
            asset_tokens: HashMap::new(),
            ttl,
            capacity,
        }
    }

    fn create(&mut self, grant: AssetGrant) -> Result<LaunchLease, LeaseCapacityError> {
        self.try_create_with_grant_at(grant, None, Instant::now())
    }

    fn create_with_typed_grant(
        &mut self,
        grant: AssetGrant,
        typed_grant: LaunchGrant,
    ) -> Result<LaunchLease, LeaseCapacityError> {
        self.try_create_with_grant_at(grant, Some(typed_grant), Instant::now())
    }

    fn try_create_at(
        &mut self,
        grant: AssetGrant,
        now: Instant,
    ) -> Result<LaunchLease, LeaseCapacityError> {
        self.try_create_with_grant_at(grant, None, now)
    }

    fn try_create_with_grant_at(
        &mut self,
        grant: AssetGrant,
        typed_grant: Option<LaunchGrant>,
        now: Instant,
    ) -> Result<LaunchLease, LeaseCapacityError> {
        self.purge_expired_at(now);
        if self.leases.len() >= self.capacity {
            return Err(LeaseCapacityError);
        }
        let launch_token = typed_grant.as_ref().map(|_| new_asset_token());
        let lease_ttl = if typed_grant.is_some() {
            DATA_GRANT_TTL
        } else {
            self.ttl
        };
        let grant_expires_at = typed_grant.as_ref().map(|_| now + lease_ttl);
        let grant_expires_at_rfc3339 = typed_grant.as_ref().map(|_| {
            (chrono::Utc::now() + chrono::Duration::seconds(lease_ttl.as_secs() as i64))
                .to_rfc3339()
        });
        let lease = LaunchLease {
            launch_id: uuid::Uuid::new_v4().to_string(),
            asset_token: new_asset_token(),
            launch_token,
            grant,
            typed_grant,
            expires_at: now + lease_ttl,
            expires_at_rfc3339: (chrono::Utc::now()
                + chrono::Duration::seconds(lease_ttl.as_secs() as i64))
            .to_rfc3339(),
            grant_expires_at,
            grant_expires_at_rfc3339,
        };
        self.asset_tokens
            .insert(lease.asset_token.clone(), lease.launch_id.clone());
        self.leases.insert(lease.launch_id.clone(), lease.clone());
        Ok(lease)
    }

    fn asset(&mut self, asset_token: &str) -> Option<AssetGrant> {
        self.purge_expired();
        let launch_id = self.asset_tokens.get(asset_token)?;
        self.leases
            .get(launch_id)
            .filter(|lease| lease.expires_at > Instant::now())
            .map(|lease| lease.grant.clone())
    }

    fn revoke(&mut self, launch_id: &str) -> bool {
        self.purge_expired();
        let Some(lease) = self.leases.remove(launch_id) else {
            return false;
        };
        self.asset_tokens.remove(&lease.asset_token);
        true
    }

    fn typed_grant(
        &mut self,
        launch_id: &str,
        launch_token: &str,
    ) -> Option<(AssetGrant, LaunchGrant)> {
        self.purge_expired();
        let lease = self.leases.get(launch_id)?;
        let token = lease.launch_token.as_deref()?;
        if !auth::validate_token(launch_token, token)
            || lease
                .grant_expires_at
                .is_some_and(|expires_at| expires_at <= Instant::now())
        {
            return None;
        }
        Some((lease.grant.clone(), lease.typed_grant.clone()?))
    }

    fn renew(&mut self, launch_id: &str, launch_token: &str) -> Option<LaunchLease> {
        self.purge_expired();
        let lease = self.leases.get_mut(launch_id)?;
        let token = lease.launch_token.as_deref()?;
        if !auth::validate_token(launch_token, token) || lease.typed_grant.is_none() {
            return None;
        }
        let now = Instant::now();
        lease.expires_at = now + DATA_GRANT_TTL;
        lease.expires_at_rfc3339 = (chrono::Utc::now()
            + chrono::Duration::seconds(DATA_GRANT_TTL.as_secs() as i64))
        .to_rfc3339();
        lease.grant_expires_at = Some(now + DATA_GRANT_TTL);
        lease.grant_expires_at_rfc3339 = Some(
            (chrono::Utc::now() + chrono::Duration::seconds(DATA_GRANT_TTL.as_secs() as i64))
                .to_rfc3339(),
        );
        Some(lease.clone())
    }

    fn purge_expired(&mut self) {
        self.purge_expired_at(Instant::now());
    }

    fn purge_expired_at(&mut self, now: Instant) {
        let expired = self
            .leases
            .iter()
            .filter_map(|(id, lease)| {
                if lease.expires_at <= now {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for id in expired {
            self.revoke_expired(&id);
        }
    }

    fn revoke_expired(&mut self, launch_id: &str) {
        if let Some(lease) = self.leases.remove(launch_id) {
            self.asset_tokens.remove(&lease.asset_token);
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.leases.len()
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LaunchRequest {
    id: String,
    version: Option<String>,
}

impl EngineApiServer {
    pub async fn bind(
        auth_token: String,
        ws_port: u16,
        protocol_usage: Arc<ProtocolUsageStore>,
        correlation_id: String,
        package_service: Arc<PackageService>,
        dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    ) -> Result<Self, EngineApiError> {
        let server = Self::bind_with_limits(
            auth_token,
            ws_port,
            protocol_usage,
            correlation_id,
            package_service,
            dispatcher,
            LAUNCH_LEASE_TTL,
            MAX_ACTIVE_LAUNCH_LEASES,
            Duration::from_secs(1),
            REQUEST_TIMEOUT,
        )
        .await?;
        server
            .package_service
            .configure_package_definition_dispatcher(server.dispatcher.clone());
        register_package_definitions(&server.package_service, &server.dispatcher)
            .await
            .map_err(EngineApiError::PackageDefinitionRegistration)?;
        Ok(server)
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    async fn bind_with_test_limits(
        auth_token: String,
        ws_port: u16,
        protocol_usage: Arc<ProtocolUsageStore>,
        correlation_id: String,
        package_service: Arc<PackageService>,
        ttl: Duration,
        capacity: usize,
        cleanup_interval: Duration,
    ) -> Result<Self, EngineApiError> {
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
            |_| {
                Box::pin(async {
                    Err::<serde_json::Value, crate::engine_dispatch::DispatchError>(
                        crate::engine_dispatch::DispatchError::Unavailable,
                    )
                })
            },
        )));
        Self::bind_with_limits(
            auth_token,
            ws_port,
            protocol_usage,
            correlation_id,
            package_service,
            dispatcher,
            ttl,
            capacity,
            cleanup_interval,
            REQUEST_TIMEOUT,
        )
        .await
    }

    #[cfg(test)]
    async fn bind_with_test_dispatcher(
        auth_token: String,
        dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
        request_timeout: Duration,
    ) -> Result<Self, EngineApiError> {
        Self::bind_with_test_dispatcher_and_deadline(
            auth_token,
            dispatcher,
            request_timeout,
            Duration::from_secs(5),
        )
        .await
    }

    #[cfg(test)]
    async fn bind_with_test_dispatcher_and_deadline(
        auth_token: String,
        dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
        request_timeout: Duration,
        continuation_deadline: Duration,
    ) -> Result<Self, EngineApiError> {
        let dir = tempfile::tempdir().expect("test data dir");
        let mut server = Self::bind_with_limits(
            auth_token,
            9,
            Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
            "00000000-0000-4000-8000-000000000001".into(),
            Arc::new(PackageService::open(dir.path()).expect("packages")),
            dispatcher,
            LAUNCH_LEASE_TTL,
            MAX_ACTIVE_LAUNCH_LEASES,
            Duration::from_secs(1),
            request_timeout,
        )
        .await?;
        server.operations.continuation_deadline = continuation_deadline;
        Ok(server)
    }

    #[allow(clippy::too_many_arguments)]
    async fn bind_with_limits(
        auth_token: String,
        ws_port: u16,
        protocol_usage: Arc<ProtocolUsageStore>,
        correlation_id: String,
        package_service: Arc<PackageService>,
        dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
        ttl: Duration,
        capacity: usize,
        cleanup_interval: Duration,
        request_timeout: Duration,
    ) -> Result<Self, EngineApiError> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self {
            listener,
            dispatcher,
            auth_token: Arc::new(auth_token),
            ws_port,
            protocol_usage,
            correlation_id: Arc::new(correlation_id),
            package_service,
            launch_leases: Arc::new(Mutex::new(LaunchLeaseRegistry::with_limits(ttl, capacity))),
            cleanup_interval,
            operations: HttpOperationRegistry::default(),
            connections: Arc::new(HttpConnectionLifecycle::default()),
            request_timeout,
        })
    }

    pub async fn shutdown(&self) -> Result<(), &'static str> {
        self.shutdown_handle().shutdown().await
    }

    pub fn shutdown_handle(&self) -> EngineApiShutdownHandle {
        EngineApiShutdownHandle {
            operations: self.operations.clone(),
            connections: self.connections.clone(),
        }
    }

    pub fn port(&self) -> u16 {
        self.listener
            .local_addr()
            .map(|address| address.port())
            .unwrap_or_default()
    }

    pub async fn run(self) -> Result<(), EngineApiError> {
        let mut cleanup = tokio::time::interval(self.cleanup_interval);
        cleanup.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let stream = tokio::select! {
                accept = self.listener.accept() => Some(accept?),
                _ = self.connections.cancelled() => return Ok(()),
                _ = cleanup.tick() => {
                    self.launch_leases
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .purge_expired();
                    self.operations.reap().await;
                    None
                }
            };
            let Some((stream, _)) = stream else {
                continue;
            };
            let token = self.auth_token.clone();
            let usage = self.protocol_usage.clone();
            let ws_port = self.ws_port;
            let correlation_id = self.correlation_id.clone();
            let package_service = self.package_service.clone();
            let dispatcher = self.dispatcher.clone();
            let request_timeout = self.request_timeout;
            let operations = self.operations.clone();
            let launch_leases = self.launch_leases.clone();
            let http_port = self.port();
            let connections = self.connections.clone();
            let permit = match connections.capacity.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    drop(stream);
                    continue;
                }
            };
            let _admission = connections
                .admission
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if connections.closed.load(Ordering::Acquire) {
                drop(permit);
                drop(stream);
                continue;
            }
            let task_id = connections.next_task.fetch_add(1, Ordering::Relaxed);
            let task_shutdown = connections.clone();
            let permit = Arc::new(Mutex::new(Some(permit)));
            let (start_sender, start_receiver) = oneshot::channel();
            connections
                .tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(
                    task_id,
                    HttpConnectionSlot::Reserved {
                        permit: permit.clone(),
                        start: start_sender,
                    },
                );
            let task = tokio::spawn(async move {
                let _permit = permit.lock().unwrap_or_else(|p| p.into_inner()).take();
                if start_receiver.await.is_err() {
                    return;
                }
                let service = service_fn(move |request| {
                    handle_request(
                        request,
                        token.clone(),
                        ws_port,
                        usage.clone(),
                        correlation_id.clone(),
                        package_service.clone(),
                        dispatcher.clone(),
                        request_timeout,
                        operations.clone(),
                        launch_leases.clone(),
                        http_port,
                    )
                });
                let result = tokio::select! {
                    _ = task_shutdown.cancelled() => Ok(()),
                    result = hyper::server::conn::http1::Builder::new()
                        .max_buf_size(32 * 1024)
                        .serve_connection(TokioIo::new(stream), service) => result,
                };
                if let Err(error) = result {
                    tracing::debug!(error = %error, "Engine HTTP connection closed");
                }
            });
            let old = connections
                .tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(task_id, HttpConnectionSlot::Installed(task));
            debug_assert!(matches!(old, Some(HttpConnectionSlot::Reserved { .. })));
            if let Some(HttpConnectionSlot::Reserved { start, .. }) = old {
                let _ = start.send(());
            }
            drop(_admission);
            connections.reap().await;
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::result_large_err)]
async fn handle_request(
    request: Request<Incoming>,
    expected_token: Arc<String>,
    ws_port: u16,
    protocol_usage: Arc<ProtocolUsageStore>,
    correlation_id: Arc<String>,
    package_service: Arc<PackageService>,
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    request_timeout: Duration,
    operations: HttpOperationRegistry,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    http_port: u16,
) -> Result<HttpResponse, Infallible> {
    let is_asset =
        request.method() == Method::GET && request.uri().path().starts_with("/v1/apps/assets/");
    let response = if is_asset {
        serve_asset(request.uri().path(), package_service, launch_leases).await
    } else {
        match authenticate(request.headers(), &expected_token) {
            Ok(client) => match (request.method(), request.uri().path()) {
                (&Method::GET, "/v1/health") => json_response(
                    StatusCode::OK,
                    json!({ "ok": true, "status": "ready", "api_version": API_VERSION }),
                ),
                (&Method::GET, "/v1/info") => json_response(
                    StatusCode::OK,
                    json!({
                        "ok": true,
                        "api_version": API_VERSION,
                        "legacy_protocol_version": PROTOCOL_VERSION,
                        "pid": std::process::id(),
                        "ws_port": ws_port,
                        "correlation_id": correlation_id.as_str(),
                        "protocol_usage": protocol_usage.snapshot(),
                    }),
                ),
                (&Method::POST, "/v1/rpc") => {
                    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
                    match body.collect().await {
                        Ok(collected) => {
                            match serde_json::from_slice::<Value>(&collected.to_bytes()) {
                                Ok(value)
                                    if value.get("operation").and_then(Value::as_str).is_some()
                                        && value.is_object() =>
                                {
                                    let mut request =
                                        match crate::engine_dispatch::DispatchRequest::from_wire(
                                            value,
                                        ) {
                                            Ok(request) => request,
                                            Err(error) => {
                                                return Ok(json_response(
                                                    StatusCode::BAD_REQUEST,
                                                    json!({ "ok": false, "error": error.to_string() }),
                                                ));
                                            }
                                        };
                                    if request.request_id.is_none() {
                                        request = request.with_request_id(request_id());
                                    }
                                    let owner = match dispatcher.allocate_owner() {
                                        Ok(owner) => owner,
                                        Err(error) => {
                                            return Ok(json_response(
                                                StatusCode::SERVICE_UNAVAILABLE,
                                                json!({ "ok": false, "error": error.to_string() }),
                                            ));
                                        }
                                    };
                                    let owner_id = owner.id();
                                    let request = request.with_client(
                                        crate::engine_dispatch::DispatchClient {
                                            pid: Some(client.pid),
                                            class: Some(client.class.clone()),
                                            version: Some(client.version.clone()),
                                            correlation_id: Some(correlation_id.as_ref().clone()),
                                            connection_id: Some(owner_id),
                                            desktop_authorized: false,
                                        },
                                    );
                                    let Some((_operation_id, receiver, mut response_guard)) =
                                        operations.start(request, dispatcher.clone(), owner).await
                                    else {
                                        return Ok(json_response(
                                            StatusCode::SERVICE_UNAVAILABLE,
                                            json!({ "ok": false, "error": "HTTP operation capacity exhausted or server shutting down" }),
                                        ));
                                    };
                                    if let Err(error) = protocol_usage.record(
                                        crate::protocol_usage::TransportKind::ApiV1,
                                        Some(&client.class),
                                        Some(&client.version),
                                    ) {
                                        tracing::warn!(error = %error, "protocol usage persistence failed");
                                    }
                                    let response = match tokio::time::timeout(
                                        request_timeout,
                                        receiver,
                                    )
                                    .await
                                    {
                                        Ok(Ok(Ok(value))) => {
                                            response_guard.disarm();
                                            json_response(StatusCode::OK, value)
                                        }
                                        Ok(Ok(Err(error))) => {
                                            response_guard.disarm();
                                            json_response(
                                                StatusCode::BAD_GATEWAY,
                                                json!({ "ok": false, "error": error.to_string() }),
                                            )
                                        }
                                        Ok(Err(_)) => {
                                            response_guard.disarm();
                                            json_response(
                                                StatusCode::BAD_GATEWAY,
                                                json!({ "ok": false, "error": "Engine RPC task failed" }),
                                            )
                                        }
                                        Err(_) => json_response(
                                            StatusCode::BAD_GATEWAY,
                                            json!({ "ok": false, "error": "Engine RPC timed out" }),
                                        ),
                                    };
                                    response
                                }
                                Ok(_) => json_response(
                                    StatusCode::BAD_REQUEST,
                                    json!({ "ok": false, "error": "body must contain string operation" }),
                                ),
                                Err(_) => json_response(
                                    StatusCode::BAD_REQUEST,
                                    json!({ "ok": false, "error": "malformed JSON body" }),
                                ),
                            }
                        }
                        Err(_) => json_response(
                            StatusCode::PAYLOAD_TOO_LARGE,
                            json!({ "ok": false, "error": "request body exceeds 1 MiB" }),
                        ),
                    }
                }
                (&Method::POST, "/v1/apps/resolve") => {
                    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
                    match body.collect().await {
                        Ok(collected) => {
                            match serde_json::from_slice::<LaunchRequest>(&collected.to_bytes()) {
                                Ok(resolve) => match package_service
                                    .resolve_app(&resolve.id, resolve.version.as_deref())
                                {
                                    Ok(resolved) => json_response(
                                        StatusCode::OK,
                                        resolve_payload(&resolved.package),
                                    ),
                                    Err(_) => json_response(
                                        StatusCode::NOT_FOUND,
                                        json!({ "ok": false, "error": "app not found" }),
                                    ),
                                },
                                Err(_) => json_response(
                                    StatusCode::BAD_REQUEST,
                                    json!({ "ok": false, "error": "malformed resolve request" }),
                                ),
                            }
                        }
                        Err(_) => json_response(
                            StatusCode::PAYLOAD_TOO_LARGE,
                            json!({ "ok": false, "error": "request body exceeds 1 MiB" }),
                        ),
                    }
                }
                (&Method::POST, "/v1/apps/launch") => {
                    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
                    match body.collect().await {
                        Ok(collected) => {
                            match serde_json::from_slice::<LaunchRequest>(&collected.to_bytes()) {
                                Ok(launch) => match register_package_definitions(
                                    &package_service,
                                    &dispatcher,
                                )
                                .await
                                {
                                    Err(_) => json_response(
                                        StatusCode::SERVICE_UNAVAILABLE,
                                        json!({ "ok": false, "error": "package definition registration failed" }),
                                    ),
                                    Ok(()) => match package_service
                                        .resolve_app(&launch.id, launch.version.as_deref())
                                    {
                                        Ok(resolved) => {
                                            let package = resolved.package;
                                            let mut leases = launch_leases
                                                .lock()
                                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                                            let ttl = leases.ttl;
                                            let grant = resolved.grant;
                                            let lease_result = leases.create_with_typed_grant(
                                                AssetGrant {
                                                    id: package.id.clone(),
                                                    version: package.version.clone(),
                                                    hash: package.hash.clone(),
                                                },
                                                grant,
                                            );
                                            match lease_result {
                                                Ok(lease) => json_response(
                                                    StatusCode::OK,
                                                    launch_payload(
                                                        http_port, &lease, &package, ttl,
                                                    ),
                                                ),
                                                Err(LeaseCapacityError) => json_response(
                                                    StatusCode::TOO_MANY_REQUESTS,
                                                    json!({ "ok": false, "error": "launch capacity reached" }),
                                                ),
                                            }
                                        }
                                        Err(_) => json_response(
                                            StatusCode::NOT_FOUND,
                                            json!({ "ok": false, "error": "app not found" }),
                                        ),
                                    },
                                },
                                Err(_) => json_response(
                                    StatusCode::BAD_REQUEST,
                                    json!({ "ok": false, "error": "malformed launch request" }),
                                ),
                            }
                        }
                        Err(_) => json_response(
                            StatusCode::PAYLOAD_TOO_LARGE,
                            json!({ "ok": false, "error": "request body exceeds 1 MiB" }),
                        ),
                    }
                }
                (&Method::POST, path)
                    if path.starts_with("/v1/apps/launch/") && path.ends_with("/ark") =>
                {
                    let launch_id = path
                        .strip_prefix("/v1/apps/launch/")
                        .and_then(|value| value.strip_suffix("/ark"))
                        .filter(|value| uuid::Uuid::parse_str(value).is_ok());
                    let Some(launch_id) = launch_id else {
                        return Ok(json_response(
                            StatusCode::NOT_FOUND,
                            json!({ "ok": false, "error": "launch not found" }),
                        ));
                    };
                    let Some(launch_token) = request
                        .headers()
                        .get(APP_LAUNCH_TOKEN_HEADER)
                        .and_then(|value| value.to_str().ok())
                    else {
                        return Ok(json_response(
                            StatusCode::UNAUTHORIZED,
                            json!({ "ok": false, "error": "missing launch token" }),
                        ));
                    };
                    let Some((asset_grant, typed_grant)) = ({
                        let mut leases = launch_leases
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        leases.typed_grant(launch_id, launch_token)
                    }) else {
                        return Ok(json_response(
                            StatusCode::FORBIDDEN,
                            json!({ "ok": false, "error": "launch authority denied" }),
                        ));
                    };
                    if typed_grant.package_id != asset_grant.id
                        || typed_grant.package_version != asset_grant.version
                        || !launch_binding_current(&package_service, &asset_grant, &typed_grant)
                    {
                        return Ok(json_response(
                            StatusCode::FORBIDDEN,
                            json!({ "ok": false, "error": "launch authority denied" }),
                        ));
                    }
                    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
                    let collected = match body.collect().await {
                        Ok(collected) => collected,
                        Err(_) => {
                            return Ok(json_response(
                                StatusCode::PAYLOAD_TOO_LARGE,
                                json!({ "ok": false, "error": "request body exceeds 1 MiB" }),
                            ));
                        }
                    };
                    let (wire_request_id, operation, params) = match parse_app_rpc(
                        serde_json::from_slice(&collected.to_bytes()).unwrap_or(Value::Null),
                        &typed_grant,
                    ) {
                        Ok(value) => value,
                        Err(error) => {
                            return Ok(json_response(
                                StatusCode::BAD_REQUEST,
                                json!({ "ok": false, "error": error }),
                            ));
                        }
                    };
                    let app_client = DispatchClient {
                        pid: Some(client.pid),
                        class: Some(client.class.clone()),
                        version: Some(client.version.clone()),
                        correlation_id: Some(correlation_id.as_ref().clone()),
                        connection_id: None,
                        desktop_authorized: false,
                    };
                    let params = match authorize_app_request(
                        &operation,
                        params,
                        &typed_grant,
                        &dispatcher,
                        &app_client,
                    )
                    .await
                    {
                        Ok(params) => params,
                        Err(error) => {
                            return Ok(json_response(
                                StatusCode::FORBIDDEN,
                                json!({ "ok": false, "error": error }),
                            ));
                        }
                    };
                    let owner = match dispatcher.allocate_owner() {
                        Ok(owner) => owner,
                        Err(error) => {
                            return Ok(json_response(
                                StatusCode::SERVICE_UNAVAILABLE,
                                json!({ "ok": false, "error": error.to_string() }),
                            ));
                        }
                    };
                    let owner_id = owner.id();
                    let request = DispatchRequest {
                        request_id: wire_request_id.or_else(|| Some(request_id())),
                        operation: Operation::Named(operation.clone()),
                        params,
                        client: DispatchClient {
                            connection_id: Some(owner_id),
                            ..app_client.clone()
                        },
                    };
                    let Some((_operation_id, receiver, mut response_guard)) =
                        operations.start(request, dispatcher.clone(), owner).await
                    else {
                        return Ok(json_response(
                            StatusCode::SERVICE_UNAVAILABLE,
                            json!({ "ok": false, "error": "HTTP operation capacity exhausted or server shutting down" }),
                        ));
                    };
                    let response = match tokio::time::timeout(request_timeout, receiver).await {
                        Ok(Ok(Ok(value))) => {
                            response_guard.disarm();
                            filter_app_response(
                                &operation,
                                value,
                                &typed_grant,
                                &dispatcher,
                                &app_client,
                            )
                            .await
                        }
                        Ok(Ok(Err(error))) => {
                            response_guard.disarm();
                            json!({ "ok": false, "error": error.to_string() })
                        }
                        Ok(Err(_)) => {
                            response_guard.disarm();
                            json!({ "ok": false, "error": "Engine RPC task failed" })
                        }
                        Err(_) => json!({ "ok": false, "error": "Engine RPC timed out" }),
                    };
                    if let Err(error) = protocol_usage.record(
                        crate::protocol_usage::TransportKind::ApiV1,
                        Some(&client.class),
                        Some(&client.version),
                    ) {
                        tracing::warn!(error = %error, "protocol usage persistence failed");
                    }
                    json_response(StatusCode::OK, response)
                }
                (&Method::POST, path)
                    if path.starts_with("/v1/apps/launch/") && path.ends_with("/renew") =>
                {
                    let launch_id = path
                        .strip_prefix("/v1/apps/launch/")
                        .and_then(|value| value.strip_suffix("/renew"))
                        .filter(|value| uuid::Uuid::parse_str(value).is_ok());
                    let Some(launch_id) = launch_id else {
                        return Ok(json_response(
                            StatusCode::NOT_FOUND,
                            json!({ "ok": false, "error": "launch not found" }),
                        ));
                    };
                    let Some(launch_token) = request
                        .headers()
                        .get(APP_LAUNCH_TOKEN_HEADER)
                        .and_then(|value| value.to_str().ok())
                    else {
                        return Ok(json_response(
                            StatusCode::UNAUTHORIZED,
                            json!({ "ok": false, "error": "missing launch token" }),
                        ));
                    };
                    let current = launch_leases
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .typed_grant(launch_id, launch_token)
                        .filter(|(asset, grant)| {
                            launch_binding_current(&package_service, asset, grant)
                        });
                    let renewed = current.and_then(|_| {
                        launch_leases
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .renew(launch_id, launch_token)
                    });
                    match renewed {
                        Some(lease) => json_response(
                            StatusCode::OK,
                            json!({
                                "ok": true,
                                "data": {
                                    "launch_id": launch_id,
                                    "ttl_seconds": DATA_GRANT_TTL.as_secs(),
                                    "expires_at": lease.grant_expires_at_rfc3339,
                                }
                            }),
                        ),
                        None => json_response(
                            StatusCode::FORBIDDEN,
                            json!({ "ok": false, "error": "launch authority denied" }),
                        ),
                    }
                }
                (&Method::DELETE, path) if path.starts_with("/v1/apps/launch/") => {
                    if let Some(launch_id) = path
                        .strip_prefix("/v1/apps/launch/")
                        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
                    {
                        let revoked = launch_leases
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .revoke(launch_id);
                        if revoked {
                            json_response(
                                StatusCode::OK,
                                json!({ "ok": true, "data": { "launch_id": launch_id, "revoked": true } }),
                            )
                        } else {
                            json_response(
                                StatusCode::NOT_FOUND,
                                json!({ "ok": false, "error": "launch not found" }),
                            )
                        }
                    } else {
                        json_response(
                            StatusCode::NOT_FOUND,
                            json!({ "ok": false, "error": "launch not found" }),
                        )
                    }
                }
                (&Method::GET, "/v1/rpc")
                | (&Method::POST, "/v1/health")
                | (&Method::POST, "/v1/info") => json_response(
                    StatusCode::METHOD_NOT_ALLOWED,
                    json!({ "ok": false, "error": "method not allowed" }),
                ),
                _ => json_response(
                    StatusCode::NOT_FOUND,
                    json!({ "ok": false, "error": "not found" }),
                ),
            },
            Err(response) => response,
        }
    };
    Ok(response)
}

async fn serve_asset(
    path: &str,
    package_service: Arc<PackageService>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
) -> HttpResponse {
    let Some(rest) = path.strip_prefix("/v1/apps/assets/") else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let Some((token, raw_asset)) = rest.split_once('/') else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    if token.is_empty()
        || token.len() > 128
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return asset_error(StatusCode::NOT_FOUND);
    }
    let Some(asset) = percent_decode(raw_asset) else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let grant = launch_leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .asset(token);
    let Some(grant) = grant else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let bytes = match package_service.read_app_asset(&grant.id, &grant.version, &grant.hash, &asset)
    {
        Ok(bytes) => bytes,
        Err(_) => return asset_error(StatusCode::NOT_FOUND),
    };
    asset_response(&asset, bytes)
}

fn asset_response(asset: &str, bytes: Vec<u8>) -> HttpResponse {
    let html = asset.rsplit_once('.').is_some_and(|(_, ext)| {
        ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm")
    });
    let content_type = mime_type(asset);
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header("cache-control", "no-store")
        .header("referrer-policy", "no-referrer")
        .header("x-content-type-options", "nosniff");
    if html {
        response = response.header("content-security-policy", "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'");
    }
    response
        .body(Full::new(Bytes::from(bytes)))
        .unwrap_or_else(|_| asset_error(StatusCode::INTERNAL_SERVER_ERROR))
}

fn asset_error(status: StatusCode) -> HttpResponse {
    Response::builder()
        .status(status)
        .header("cache-control", "no-store")
        .header("referrer-policy", "no-referrer")
        .body(Full::new(Bytes::new()))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
}

#[derive(Debug, Deserialize)]
struct AppRpcEnvelope {
    #[serde(rename = "_req_id", alias = "id", default)]
    request_id: Option<String>,
    operation: String,
    #[serde(default)]
    params: Value,
}

const APP_GRAPH_READS: &[&str] = &[
    "list_object_types",
    "list_objects",
    "list_objects_by_type",
    "list_object_summaries",
    "list_object_summaries_by_type",
    "get_object",
    "get_object_type",
    "search_objects",
    "list_object_links",
];
const APP_GRAPH_WRITES: &[&str] = &[
    "upsert_object",
    "delete_object",
    "upsert_object_link",
    "delete_object_link",
];
const APP_DICTATION_OPERATIONS: &[&str] = &[
    "dictation.get_state",
    "dictation.get_config",
    "dictation.start_recording",
    "dictation.cancel",
];

fn parse_app_rpc(
    value: Value,
    grant: &LaunchGrant,
) -> Result<(Option<String>, String, Value), &'static str> {
    let envelope: AppRpcEnvelope = serde_json::from_value(value)
        .map_err(|_| "app request must contain operation and params")?;
    if !APP_GRAPH_READS.contains(&envelope.operation.as_str())
        && !APP_GRAPH_WRITES.contains(&envelope.operation.as_str())
        && !(APP_DICTATION_OPERATIONS.contains(&envelope.operation.as_str())
            && grant.allows_dictation_operation(&envelope.operation))
    {
        return Err("unsupported app operation");
    }
    let params = if envelope.params.is_null() {
        Value::Object(serde_json::Map::new())
    } else if envelope.params.is_object() {
        envelope.params
    } else {
        return Err("app params must be an object");
    };
    Ok((envelope.request_id, envelope.operation, params))
}

fn canonical_type_id(type_id: &str) -> String {
    ark_core::canonical_types::definitions::canonical_type_registrations()
        .ok()
        .and_then(|registrations| {
            registrations.into_iter().find_map(|registration| {
                (registration.type_id == type_id
                    || registration
                        .aliases
                        .iter()
                        .any(|alias| alias.alias == type_id))
                .then_some(registration.type_id)
            })
        })
        .unwrap_or_else(|| type_id.to_owned())
}

fn param_str<'a>(params: &'a Value, snake: &str, camel: &str) -> Option<&'a str> {
    params
        .get(snake)
        .or_else(|| params.get(camel))
        .and_then(Value::as_str)
}

fn grant_rule_matches<'a>(
    grant: &'a LaunchGrant,
    type_id: &str,
    type_version: Option<&str>,
    action: &str,
) -> Option<&'a crate::runtime_grants::GrantRule> {
    grant.rules.iter().find(|rule| {
        rule.type_id == type_id
            && rule.actions.contains(action)
            && type_version.is_none_or(|version| {
                semver::Version::parse(version).ok().is_some_and(|version| {
                    rule.versions.iter().any(|requirement| {
                        semver::VersionReq::parse(requirement)
                            .ok()
                            .is_some_and(|requirement| requirement.matches(&version))
                    })
                })
            })
    })
}

fn grant_authorizes(
    grant: &LaunchGrant,
    type_id: &str,
    type_version: Option<&str>,
    action: &str,
    fields: &[String],
    relations: &[String],
) -> Result<(), &'static str> {
    let rule =
        grant_rule_matches(grant, type_id, type_version, action).ok_or("data grant denied")?;
    let allowed_fields = if matches!(action, "read" | "subscribe") {
        &rule.fields_read
    } else {
        &rule.fields_write
    };
    let allowed_relations = if matches!(action, "read" | "subscribe") {
        &rule.relations_read
    } else {
        &rule.relations_write
    };
    if fields.iter().any(|field| !allowed_fields.contains(field))
        || relations
            .iter()
            .any(|relation| !allowed_relations.contains(relation))
    {
        return Err("data grant denied");
    }
    Ok(())
}

fn broad_read_authorized(grant: &LaunchGrant) -> bool {
    grant.rules.iter().any(|rule| rule.actions.contains("read"))
}

fn object_field_inputs(object: &Value) -> Result<(Vec<FieldInput>, Vec<String>), &'static str> {
    let map = object.as_object().ok_or("object must be an object")?;
    let mut fields = Vec::new();
    let mut names = Vec::new();
    for (key, field_id) in [
        ("title", "title"),
        ("contentJson", "content"),
        ("content_json", "content"),
    ] {
        if let Some(value) = map.get(key) {
            if key == "content_json" && map.contains_key("contentJson") {
                continue;
            }
            fields.push(FieldInput {
                field_id: field_id.to_owned(),
                value: serde_json::from_value(value.clone()).map_err(|_| "invalid object field")?,
            });
            names.push(field_id.to_owned());
        }
    }
    let props = map.get("propsJson").or_else(|| map.get("props_json"));
    if let Some(Value::Object(props)) = props {
        for (key, value) in props {
            let field_id = format!("props.{key}");
            fields.push(FieldInput {
                field_id: field_id.clone(),
                value: serde_json::from_value(value.clone()).map_err(|_| "invalid object field")?,
            });
            names.push(field_id);
        }
    } else if props.is_some() {
        return Err("propsJson must be an object");
    }
    Ok((fields, names))
}

fn object_type_and_version(object: &Value) -> Result<(String, String), &'static str> {
    let type_id = param_str(object, "type_id", "typeId").ok_or("object type is required")?;
    let canonical = canonical_type_id(type_id);
    let type_version = param_str(object, "type_version", "typeVersion")
        .map(str::to_owned)
        .or_else(|| {
            ark_core::canonical_types::definitions::canonical_type_registrations()
                .ok()
                .and_then(|registrations| {
                    registrations
                        .into_iter()
                        .find(|registration| registration.type_id == canonical)
                        .map(|registration| registration.version)
                })
        })
        .ok_or("object type version is required")?;
    Ok((canonical, type_version))
}

fn object_type_and_version_for_write(
    object: &Value,
    grant: &LaunchGrant,
) -> Result<(String, String), &'static str> {
    let type_id =
        canonical_type_id(param_str(object, "type_id", "typeId").ok_or("object type is required")?);
    if let Some(version) = param_str(object, "type_version", "typeVersion") {
        return Ok((type_id, version.to_owned()));
    }
    let version = grant
        .rules
        .iter()
        .find(|rule| rule.type_id == type_id)
        .and_then(|rule| (rule.versions.len() == 1).then(|| &rule.versions[0]))
        .and_then(|requirement| requirement.strip_prefix('='))
        .filter(|version| semver::Version::parse(version).is_ok())
        .ok_or("object type version is required")?;
    Ok((type_id, version.to_owned()))
}

fn data_request_allowed(grant: &LaunchGrant, request: DataRequest) -> Result<(), &'static str> {
    grant
        .authorize_request(&request)
        .map_err(|_| "data grant denied")
}

async fn internal_app_lookup(
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
    operation: &str,
    params: Value,
) -> Option<Value> {
    let request = DispatchRequest {
        request_id: Some(request_id()),
        operation: Operation::Named(operation.to_owned()),
        params,
        client: client.clone(),
    };
    let response = dispatcher.dispatch(request).await.ok()?;
    if response.get("ok").is_some() {
        response
            .get("ok")
            .and_then(Value::as_bool)
            .filter(|ok| *ok)
            .and_then(|_| response.get("data").cloned())
    } else {
        Some(response)
    }
}

fn launch_binding_current(
    package_service: &PackageService,
    asset: &AssetGrant,
    grant: &LaunchGrant,
) -> bool {
    package_service
        .resolve_app(&asset.id, Some(&asset.version))
        .ok()
        .and_then(|resolved| {
            (resolved.package.hash.eq_ignore_ascii_case(&asset.hash)
                && resolved.grant.package_id == grant.package_id
                && resolved.grant.package_version == grant.package_version
                && resolved.grant.manifest_digest == grant.manifest_digest)
                .then_some(())
        })
        .is_some()
}

async fn authorize_app_request(
    operation: &str,
    mut params: Value,
    grant: &LaunchGrant,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
) -> Result<Value, &'static str> {
    let map = params
        .as_object_mut()
        .ok_or("app params must be an object")?;
    match operation {
        "dictation.get_state"
        | "dictation.get_config"
        | "dictation.start_recording"
        | "dictation.cancel" => {
            if !grant.allows_dictation_operation(operation) {
                return Err("dictation grant denied");
            }
        }
        "list_objects_by_type" | "list_object_summaries_by_type" => {
            let raw_type = map
                .get("type_id")
                .or_else(|| map.get("typeId"))
                .and_then(Value::as_str)
                .ok_or("type_id is required")?;
            let canonical = canonical_type_id(raw_type);
            grant_authorizes(grant, &canonical, None, "read", &[], &[])?;
            map.insert("type_id".into(), Value::String(canonical));
            map.remove("typeId");
        }
        "list_objects" | "list_object_summaries" | "list_object_types" => {
            if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "get_object" => {
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object id is required")?;
            let object = internal_app_lookup(dispatcher, client, "get_object", json!({ "id": id }))
                .await
                .ok_or("data grant denied")?;
            if let Some(object) = object.as_object() {
                let (type_id, version) = object_type_and_version(&Value::Object(object.clone()))?;
                grant_authorizes(grant, &type_id, Some(&version), "read", &[], &[])?;
            } else if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "get_object_type" => {
            let raw_type = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object type id is required")?;
            let canonical = canonical_type_id(raw_type);
            grant_authorizes(grant, &canonical, None, "read", &[], &[])?;
            map.insert("id".into(), Value::String(canonical));
        }
        "search_objects" => {
            if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "list_object_links" => {
            if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "upsert_object" => {
            let object = map.get("object").ok_or("object is required")?;
            let (type_id, type_version) = object_type_and_version_for_write(object, grant)?;
            let (fields, _) = object_field_inputs(object)?;
            let object_id = param_str(object, "id", "id").map(str::to_owned);
            let create_request = || DataRequest::CreateObject {
                type_id: type_id.clone(),
                type_version: type_version.clone(),
                object_id: object_id.clone(),
                fields: fields.clone(),
                links: vec![],
            };
            let update_request = |object_id: String| DataRequest::UpdateObject {
                type_id: type_id.clone(),
                type_version: type_version.clone(),
                object_id,
                expected_hlc: None,
                fields: fields.clone(),
                links: vec![],
            };
            let (create, update) = match object_id.as_deref() {
                None => (data_request_allowed(grant, create_request()).is_ok(), false),
                Some(object_id) => {
                    let existing = internal_app_lookup(
                        dispatcher,
                        client,
                        "get_object",
                        json!({ "id": object_id }),
                    )
                    .await
                    .ok_or("data grant denied")?;
                    if existing.is_null() {
                        (data_request_allowed(grant, create_request()).is_ok(), false)
                    } else {
                        let (existing_type, existing_version) = object_type_and_version(&existing)?;
                        if existing_type != type_id || existing_version != type_version {
                            return Err("data grant denied");
                        }
                        (
                            false,
                            data_request_allowed(grant, update_request(object_id.to_owned()))
                                .is_ok(),
                        )
                    }
                }
            };
            if !create && !update {
                return Err("data grant denied");
            }
            if let Some(object) = map.get_mut("object").and_then(Value::as_object_mut) {
                object.retain(|key, _| {
                    matches!(
                        key.as_str(),
                        "id" | "typeId"
                            | "type_id"
                            | "typeVersion"
                            | "type_version"
                            | "title"
                            | "contentJson"
                            | "content_json"
                            | "propsJson"
                            | "props_json"
                    )
                });
                object.insert("typeId".into(), Value::String(type_id));
                object.insert("typeVersion".into(), Value::String(type_version));
                object.remove("type_id");
                object.remove("type_version");
            }
        }
        "delete_object" => {
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object id is required")?;
            let object = internal_app_lookup(dispatcher, client, "get_object", json!({ "id": id }))
                .await
                .ok_or("data grant denied")?;
            let (type_id, version) = object_type_and_version(&object)?;
            data_request_allowed(
                grant,
                DataRequest::DeleteObject {
                    type_id,
                    type_version: version,
                    object_id: id.to_owned(),
                    expected_hlc: None,
                },
            )?;
        }
        "upsert_object_link" => {
            let link = map
                .get("object_link")
                .or_else(|| map.get("objectLink"))
                .ok_or("object_link is required")?;
            let (source_id, target_id, relation) = link_parts(link)?;
            let source =
                internal_app_lookup(dispatcher, client, "get_object", json!({ "id": source_id }))
                    .await
                    .ok_or("data grant denied")?;
            let target =
                internal_app_lookup(dispatcher, client, "get_object", json!({ "id": target_id }))
                    .await
                    .ok_or("data grant denied")?;
            authorize_link(grant, &source, &target, relation, true)?;
        }
        "delete_object_link" => {
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object link id is required")?;
            let links = internal_app_lookup(dispatcher, client, "list_object_links", json!({}))
                .await
                .ok_or("data grant denied")?;
            let link = links
                .as_array()
                .and_then(|links| {
                    links
                        .iter()
                        .find(|link| link.get("id").and_then(Value::as_str) == Some(id))
                })
                .ok_or("data grant denied")?;
            let (source_id, target_id, relation) = link_parts(link)?;
            let source =
                internal_app_lookup(dispatcher, client, "get_object", json!({ "id": source_id }))
                    .await
                    .ok_or("data grant denied")?;
            let target =
                internal_app_lookup(dispatcher, client, "get_object", json!({ "id": target_id }))
                    .await
                    .ok_or("data grant denied")?;
            authorize_link(grant, &source, &target, relation, true)?;
        }
        _ => return Err("unsupported app operation"),
    }
    Ok(params)
}

fn link_parts(link: &Value) -> Result<(&str, &str, &str), &'static str> {
    let source = param_str(link, "source_object_id", "sourceObjectId")
        .ok_or("source_object_id is required")?;
    let target = param_str(link, "target_object_id", "targetObjectId")
        .ok_or("target_object_id is required")?;
    let relation = param_str(link, "link_type", "linkType").ok_or("link_type is required")?;
    Ok((source, target, relation))
}

fn authorize_link(
    grant: &LaunchGrant,
    source: &Value,
    target: &Value,
    relation: &str,
    write: bool,
) -> Result<(), &'static str> {
    let (source_type, source_version) = object_type_and_version(source)?;
    let (target_type, target_version) = object_type_and_version(target)?;
    let source_rule = grant_rule_matches(
        grant,
        &source_type,
        Some(&source_version),
        if write { "link" } else { "read" },
    )
    .ok_or("data grant denied")?;
    let relations = if write {
        &source_rule.relations_write
    } else {
        &source_rule.relations_read
    };
    if !relations.iter().any(|value| value == relation) {
        return Err("data grant denied");
    }
    grant_authorizes(grant, &target_type, Some(&target_version), "read", &[], &[])
}

fn filter_object_value(value: &Value, grant: &LaunchGrant) -> Option<Value> {
    let mut object = value.as_object()?.clone();
    let (type_id, version) = object_type_and_version(value).ok()?;
    let rule = grant_rule_matches(grant, &type_id, Some(&version), "read")?;
    if !rule.fields_read.iter().any(|field| field == "title") {
        object.remove("title");
    }
    if !rule.fields_read.iter().any(|field| field == "content") {
        object.remove("contentJson");
        object.remove("content_json");
    }
    let props_key = if object.contains_key("propsJson") {
        Some("propsJson")
    } else if object.contains_key("props_json") {
        Some("props_json")
    } else {
        None
    };
    if let Some(props) = props_key
        .and_then(|key| object.get_mut(key))
        .and_then(Value::as_object_mut)
    {
        props.retain(|key, _| {
            rule.fields_read
                .iter()
                .any(|field| field == &format!("props.{key}"))
        });
    }
    Some(Value::Object(object))
}

fn filter_object_array(value: &mut Value, grant: &LaunchGrant) {
    let Some(objects) = value.as_array_mut() else {
        return;
    };
    objects.retain_mut(|object| {
        let Some(filtered) = filter_object_value(object, grant) else {
            return false;
        };
        *object = filtered;
        true
    });
}

fn filter_type_value(value: &Value, grant: &LaunchGrant) -> Option<Value> {
    let mut object = value.as_object()?.clone();
    let id = object
        .get("id")
        .or_else(|| object.get("typeId"))
        .and_then(Value::as_str)?;
    let canonical = canonical_type_id(id);
    let rule = grant_rule_matches(grant, &canonical, None, "read")?;
    for key in ["schemaJson", "uiSchemaJson"] {
        let Some(document) = object.get_mut(key) else {
            continue;
        };
        let Some(mut document_json) = document
            .as_str()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        else {
            continue;
        };
        let allowed_props = rule
            .fields_read
            .iter()
            .filter_map(|field| field.strip_prefix("props."))
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(properties) = document_json
            .get_mut("properties")
            .and_then(Value::as_object_mut)
        {
            properties.retain(|field, _| allowed_props.contains(field.as_str()));
        }
        if let Some(required) = document_json
            .get_mut("required")
            .and_then(Value::as_array_mut)
        {
            required.retain(|field| {
                field
                    .as_str()
                    .is_some_and(|field| allowed_props.contains(field))
            });
        }
        for field in [
            "visibleFields",
            "hiddenFields",
            "featuredFields",
            "readOnlyFields",
            "fieldOrder",
        ] {
            if let Some(values) = document_json.get_mut(field).and_then(Value::as_array_mut) {
                values.retain(|value| {
                    value
                        .as_str()
                        .is_some_and(|value| allowed_props.contains(value))
                });
            }
        }
        if let Ok(raw) = serde_json::to_string(&document_json) {
            *document = Value::String(raw);
        }
    }
    Some(Value::Object(object))
}

async fn filter_link_array(
    value: &mut Value,
    grant: &LaunchGrant,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
) {
    let Some(links) = value.as_array_mut() else {
        return;
    };
    let mut filtered = Vec::with_capacity(links.len());
    for link in links.iter() {
        let Ok((source_id, target_id, relation)) = link_parts(link) else {
            continue;
        };
        let Some(source) =
            internal_app_lookup(dispatcher, client, "get_object", json!({ "id": source_id })).await
        else {
            continue;
        };
        let Some(target) =
            internal_app_lookup(dispatcher, client, "get_object", json!({ "id": target_id })).await
        else {
            continue;
        };
        if authorize_link(grant, &source, &target, relation, false).is_ok() {
            filtered.push(link.clone());
        }
    }
    *links = filtered;
}

async fn filter_app_response(
    operation: &str,
    response: Value,
    grant: &LaunchGrant,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
) -> Value {
    if response.get("ok").and_then(Value::as_bool) == Some(false) {
        return response;
    }
    let mut response = response;
    let data_is_wrapped = response.get("data").is_some();
    let data = if data_is_wrapped {
        response.get_mut("data").expect("data exists")
    } else {
        &mut response
    };
    match operation {
        "list_objects"
        | "list_objects_by_type"
        | "list_object_summaries"
        | "list_object_summaries_by_type" => {
            filter_object_array(data, grant);
        }
        "get_object" => {
            if let Some(filtered) = filter_object_value(data, grant) {
                *data = filtered;
            } else {
                *data = Value::Null;
            }
        }
        "list_object_types" => {
            if let Some(types) = data.as_array_mut() {
                types.retain_mut(|item| {
                    let Some(filtered) = filter_type_value(item, grant) else {
                        return false;
                    };
                    *item = filtered;
                    true
                });
            }
        }
        "get_object_type" => {
            if let Some(filtered) = filter_type_value(data, grant) {
                *data = filtered;
            } else {
                *data = Value::Null;
            }
        }
        "search_objects" => {
            if let Some(results) = data.as_array_mut() {
                let mut filtered = Vec::with_capacity(results.len());
                for result in results.iter() {
                    let Some(id) = result.get("entryId").and_then(Value::as_str) else {
                        continue;
                    };
                    let Some(object) =
                        internal_app_lookup(dispatcher, client, "get_object", json!({ "id": id }))
                            .await
                    else {
                        continue;
                    };
                    if filter_object_value(&object, grant).is_some() {
                        let mut result = result.clone();
                        // ARK search snippets can combine multiple indexed fields, so their
                        // provenance cannot be filtered safely at this compatibility boundary.
                        if let Some(map) = result.as_object_mut() {
                            map.insert("text".into(), Value::String(String::new()));
                        }
                        filtered.push(result);
                    }
                }
                *results = filtered;
            }
        }
        "list_object_links" => filter_link_array(data, grant, dispatcher, client).await,
        _ => {}
    }
    response
}

fn resolve_payload(package: &crate::package_store::InstalledPackage) -> Value {
    json!({
        "ok": true,
        "data": {
            "id": package.id,
            "version": package.version,
            "name": package.manifest.name(),
            "permissions": package.manifest.permissions(),
            "enabled": package.enabled,
            "revoked": package.revoked,
        }
    })
}

fn launch_payload(
    http_port: u16,
    lease: &LaunchLease,
    package: &crate::package_store::InstalledPackage,
    ttl: Duration,
) -> Value {
    let mut data = json!({
        "id": package.id,
        "version": package.version,
        "name": package.manifest.name(),
        "launch_url": format!("http://127.0.0.1:{http_port}/v1/apps/assets/{}/{}", lease.asset_token, package.manifest.entrypoint()),
        "permissions": package.manifest.permissions(),
        "launch_id": lease.launch_id,
        "asset_token": lease.asset_token,
        "ttl_seconds": ttl.as_secs(),
        "expires_at": lease.expires_at_rfc3339,
    });
    if let Some(token) = lease.launch_token.as_ref() {
        data["broker_token"] = Value::String(token.clone());
        data["data_api"] = Value::String(format!(
            "http://127.0.0.1:{http_port}/v1/apps/launch/{}/ark",
            lease.launch_id
        ));
        data["ttl_seconds"] = Value::from(DATA_GRANT_TTL.as_secs());
        if let Some(expires_at) = lease.grant_expires_at_rfc3339.as_ref() {
            data["expires_at"] = Value::String(expires_at.clone());
        }
        data["manifest_schema_version"] = Value::from(2);
        let effective_read_types = lease
            .typed_grant
            .as_ref()
            .map(|grant| {
                let mut ids = std::collections::BTreeSet::new();
                for rule in grant
                    .rules
                    .iter()
                    .filter(|rule| rule.actions.contains("subscribe"))
                {
                    ids.insert(rule.type_id.clone());
                    if let Ok(registrations) =
                        ark_core::canonical_types::definitions::canonical_type_registrations()
                    {
                        if let Some(registration) = registrations
                            .into_iter()
                            .find(|registration| registration.type_id == rule.type_id)
                        {
                            ids.extend(registration.aliases.into_iter().map(|alias| alias.alias));
                        }
                    }
                }
                ids.into_iter().map(Value::String).collect::<Vec<_>>()
            })
            .unwrap_or_default();
        data["effective_read_types"] = Value::Array(effective_read_types);
    }
    json!({
        "ok": true,
        "data": data
    })
}

fn new_asset_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn percent_decode(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let input = value.as_bytes();
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' {
            if index + 2 >= input.len() {
                return None;
            }
            let high = (input[index + 1] as char).to_digit(16)? as u8;
            let low = (input[index + 2] as char).to_digit(16)? as u8;
            bytes.push((high << 4) | low);
            index += 3;
        } else {
            bytes.push(input[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).ok()
}

fn mime_type(path: &str) -> &'static str {
    match path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[derive(Debug)]
struct AuthenticatedClient {
    pid: u32,
    class: String,
    version: String,
}

fn authenticate(
    headers: &HeaderMap,
    expected_token: &str,
) -> Result<AuthenticatedClient, HttpResponse> {
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if !bearer.is_some_and(|token| auth::validate_token(token, expected_token)) {
        return Err(json_response(
            StatusCode::UNAUTHORIZED,
            json!({ "ok": false, "error": "invalid bearer token" }),
        ));
    }

    let pid = headers
        .get(CLIENT_PID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u32>().ok());
    if pid.is_none_or(|pid| auth::validate_pid_belongs_to_current_user(pid).is_err()) {
        return Err(json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "invalid client PID" }),
        ));
    }

    let compatible = headers
        .get(API_VERSION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| ProtocolVersion::parse(value).ok())
        .is_some_and(|version| {
            !matches!(
                version.is_compatible_with_server(&API_VERSION_CURRENT),
                Compatibility::Incompatible
            )
        });
    if !compatible {
        return Err(json_response(
            StatusCode::UPGRADE_REQUIRED,
            json!({ "ok": false, "error": "missing or incompatible API version" }),
        ));
    }

    Ok(AuthenticatedClient {
        pid: pid.expect("validated client PID"),
        class: header_string(headers, CLIENT_CLASS_HEADER)
            .unwrap_or_else(|| "engine-http".to_string()),
        version: header_string(headers, CLIENT_VERSION_HEADER)
            .unwrap_or_else(|| API_VERSION.to_string()),
    })
}

fn header_string(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn request_id() -> String {
    format!("engine-http-{}", uuid::Uuid::new_v4())
}

fn json_response(status: StatusCode, value: Value) -> HttpResponse {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .header("cache-control", "no-store")
        .body(Full::new(Bytes::from(value.to_string())))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::from_static(b"{\"ok\":false}"))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn valid_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        headers.insert(
            CLIENT_PID_HEADER,
            std::process::id().to_string().parse().unwrap(),
        );
        headers.insert(API_VERSION_HEADER, API_VERSION.parse().unwrap());
        headers
    }

    #[test]
    fn http_auth_requires_token_pid_and_compatible_version() {
        let token = "a".repeat(64);
        assert!(authenticate(&valid_headers(&token), &token).is_ok());

        let mut missing_pid = valid_headers(&token);
        missing_pid.remove(CLIENT_PID_HEADER);
        assert_eq!(
            authenticate(&missing_pid, &token)
                .expect_err("missing PID must fail")
                .status(),
            StatusCode::FORBIDDEN
        );

        let mut wrong_version = valid_headers(&token);
        wrong_version.insert(API_VERSION_HEADER, "2.0.0".parse().unwrap());
        assert_eq!(
            authenticate(&wrong_version, &token)
                .expect_err("major mismatch must fail")
                .status(),
            StatusCode::UPGRADE_REQUIRED
        );

        assert_eq!(
            authenticate(&valid_headers("b"), &token)
                .expect_err("wrong token must fail")
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }

    fn app_test_grant() -> LaunchGrant {
        LaunchGrant {
            package_id: "com.kosmos.app".into(),
            package_version: "1.0.0".into(),
            manifest_digest: "digest".into(),
            rules: vec![crate::runtime_grants::GrantRule {
                type_id: "com.kosmos.note".into(),
                versions: vec!["1.0.0".into()],
                actions: ["read", "create", "update", "delete", "subscribe", "link"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                fields_read: vec!["title".into(), "props.description".into()],
                fields_write: vec!["title".into(), "props.description".into()],
                relations_read: vec!["related".into()],
                relations_write: vec!["related".into()],
            }],
            capabilities: vec![],
        }
    }

    #[test]
    fn launch_scoped_dictation_rpc_requires_the_exact_grant_operation() {
        let request = || json!({"operation": "dictation.get_state", "params": {}});
        assert!(parse_app_rpc(request(), &app_test_grant()).is_err());

        let mut grant = app_test_grant();
        grant
            .capabilities
            .push(crate::runtime_grants::ScopedCapability::Dictation {
                operations: vec!["dictation.get_state".into()],
            });
        assert_eq!(
            parse_app_rpc(request(), &grant)
                .expect("granted dictation operation")
                .1,
            "dictation.get_state"
        );
        assert!(parse_app_rpc(
            json!({"operation": "dictation.start_recording", "params": {}}),
            &grant,
        )
        .is_err());
        assert!(parse_app_rpc(
            json!({"operation": "dictation.submit_audio", "params": {}}),
            &grant,
        )
        .is_err());
    }

    fn task_test_grant(version: &str) -> LaunchGrant {
        LaunchGrant {
            package_id: "com.kosmos.app".into(),
            package_version: "1.0.0".into(),
            manifest_digest: "digest".into(),
            rules: vec![crate::runtime_grants::GrantRule {
                type_id: "com.kosmos.task".into(),
                versions: vec![version.into()],
                actions: ["create"].into_iter().map(str::to_owned).collect(),
                fields_read: vec![],
                fields_write: vec!["title".into()],
                relations_read: vec![],
                relations_write: vec![],
            }],
            capabilities: vec![],
        }
    }

    #[tokio::test]
    async fn launch_scoped_task_upsert_requires_exact_granted_version() {
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
            Box::pin(async { Ok(json!({"ok": true, "data": null})) })
        }));
        let client = DispatchClient::default();
        let explicit = authorize_app_request(
            "upsert_object",
            json!({"object": {"typeId":"task_obj", "typeVersion":"1.0.0", "title":"ok"}}),
            &task_test_grant("=1.0.0"),
            &dispatcher,
            &client,
        )
        .await
        .expect("exact version is authorized");
        assert_eq!(explicit["object"]["typeId"], "com.kosmos.task");
        assert_eq!(explicit["object"]["typeVersion"], "1.0.0");

        assert!(authorize_app_request(
            "upsert_object",
            json!({"object": {"typeId":"task_obj", "title":"missing version"}}),
            &task_test_grant(">=1.0.0"),
            &dispatcher,
            &client,
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn launch_scoped_task_upsert_rejects_mismatched_version_without_mutation() {
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
            Box::pin(async { Ok(json!({"ok": true, "data": null})) })
        }));
        let client = DispatchClient::default();
        let params = json!({
            "object": {"typeId":"task_obj", "typeVersion":"2.0.0", "title":"wrong"}
        });
        let original = params.clone();
        assert!(authorize_app_request(
            "upsert_object",
            params.clone(),
            &task_test_grant("=1.0.0"),
            &dispatcher,
            &client,
        )
        .await
        .is_err());
        assert_eq!(params, original);
    }

    #[tokio::test]
    async fn launch_scoped_graph_denies_cross_type_and_ungranted_write() {
        let grant = app_test_grant();
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
            Box::pin(async { Ok(json!({"ok": true, "data": null})) })
        }));
        let client = DispatchClient::default();
        assert!(authorize_app_request(
            "list_objects_by_type",
            json!({"type_id":"note_obj"}),
            &grant,
            &dispatcher,
            &client,
        )
        .await
        .is_ok());
        assert!(authorize_app_request(
            "list_objects_by_type",
            json!({"type_id":"com.kosmos.game"}),
            &grant,
            &dispatcher,
            &client,
        )
        .await
        .is_err());
        assert!(authorize_app_request(
            "upsert_object",
            json!({"object": {
                "id":"n1", "typeId":"com.kosmos.note", "typeVersion":"1.0.0",
                "title":"ok", "propsJson":{"secret":"no"}
            }}),
            &grant,
            &dispatcher,
            &client,
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn launch_scoped_upsert_cannot_turn_create_into_update() {
        let mut grant = app_test_grant();
        grant
            .rules
            .first_mut()
            .expect("rule")
            .actions
            .remove("update");
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|request| {
            Box::pin(async move {
                if request.operation.as_str() == "get_object" {
                    Ok(json!({"ok": true, "data": {
                        "id":"n1", "typeId":"com.kosmos.note", "typeVersion":"1.0.0"
                    }}))
                } else {
                    Ok(json!({"ok": true, "data": true}))
                }
            })
        }));
        assert!(authorize_app_request(
            "upsert_object",
            json!({"object": {
                "id":"n1", "typeId":"com.kosmos.note", "typeVersion":"1.0.0",
                "title":"overwrite"
            }}),
            &grant,
            &dispatcher,
            &DispatchClient::default(),
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn launch_scoped_write_strips_untrusted_integrity_fields() {
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
            Box::pin(async { Ok(json!({"ok": true, "data": null})) })
        }));
        let params = authorize_app_request(
            "upsert_object",
            json!({"object": {
                "id":"n2", "typeId":"com.kosmos.note", "typeVersion":"1.0.0",
                "title":"ok", "propsJson":{"description":"allowed"},
                "createdAt":"forged", "deletedAt":"forged", "unexpected":"forged"
            }}),
            &app_test_grant(),
            &dispatcher,
            &DispatchClient::default(),
        )
        .await
        .expect("authorized create");
        let object = params["object"].as_object().expect("object");
        assert!(!object.contains_key("createdAt"));
        assert!(!object.contains_key("deletedAt"));
        assert!(!object.contains_key("unexpected"));
    }

    #[tokio::test]
    async fn launch_scoped_search_never_returns_mixed_field_snippets() {
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
            Box::pin(async {
                Ok(json!({"ok": true, "data": {
                    "id":"n1", "typeId":"com.kosmos.note", "typeVersion":"1.0.0",
                    "title":"allowed", "contentJson":"private"
                }}))
            })
        }));
        let response = filter_app_response(
            "search_objects",
            json!({"ok":true,"data":[{"entryId":"n1","text":"private snippet"}]}),
            &app_test_grant(),
            &dispatcher,
            &DispatchClient::default(),
        )
        .await;
        assert_eq!(response["data"][0]["text"], "");
    }

    #[test]
    fn launch_scoped_filter_removes_ungranted_fields_and_types() {
        let grant = app_test_grant();
        let mut response = json!({"ok":true,"data":[
            {"id":"n1","typeId":"com.kosmos.note","typeVersion":"1.0.0","title":"title","contentJson":{"secret":true},"propsJson":{"description":"ok","secret":"no"}},
            {"id":"g1","typeId":"com.kosmos.game","typeVersion":"1.0.0","title":"private","propsJson":{}}
        ]});
        filter_object_array(response.get_mut("data").unwrap(), &grant);
        assert_eq!(response["data"].as_array().unwrap().len(), 1);
        let object = &response["data"][0];
        assert_eq!(object["title"], "title");
        assert!(object.get("contentJson").is_none());
        assert_eq!(object["propsJson"]["description"], "ok");
        assert!(object["propsJson"].get("secret").is_none());
    }

    #[test]
    fn launch_scoped_token_is_independent_and_revoked_with_lease() {
        let mut leases = LaunchLeaseRegistry::default();
        let lease = leases
            .create_with_typed_grant(
                AssetGrant {
                    id: "com.kosmos.app".into(),
                    version: "1.0.0".into(),
                    hash: "a".repeat(64),
                },
                app_test_grant(),
            )
            .expect("typed lease");
        let token = lease.launch_token.clone().expect("launch token");
        assert_ne!(token, lease.asset_token);
        assert!(leases.typed_grant(&lease.launch_id, &token).is_some());
        assert!(leases.typed_grant(&lease.launch_id, "wrong").is_none());
        assert!(leases.revoke(&lease.launch_id));
        assert!(leases.typed_grant(&lease.launch_id, &token).is_none());
    }

    #[test]
    fn asset_paths_decode_traversal_and_headers_are_deterministic() {
        assert_eq!(
            percent_decode("dist%2Findex.html").as_deref(),
            Some("dist/index.html")
        );
        assert_eq!(
            percent_decode("%2e%2e%2Fsecret").as_deref(),
            Some("../secret")
        );
        assert!(percent_decode("bad%ZZ").is_none());

        let html = asset_response("index.html", b"<html />".to_vec());
        assert_eq!(
            html.headers().get(CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        assert_eq!(html.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            html.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );
        assert!(html.headers().contains_key("content-security-policy"));

        let css = asset_response("styles.css", b"body{}".to_vec());
        assert_eq!(
            css.headers().get(CONTENT_TYPE).unwrap(),
            "text/css; charset=utf-8"
        );
        assert!(!css.headers().contains_key("content-security-policy"));
    }

    #[test]
    fn launch_payload_redacts_store_fields() {
        let package = crate::package_store::InstalledPackage {
            id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            hash: "a".repeat(64),
            manifest: crate::package_manifest::VersionedManifest::V1(
                crate::package_manifest::PackageManifest {
                    schema_version: 1,
                    id: "com.kosmos.demo".into(),
                    name: "Demo".into(),
                    version: "1.0.0".into(),
                    kind: crate::package_manifest::PackageKind::App,
                    engine_api: ">=1.0.0".into(),
                    entrypoint: "index.html".into(),
                    publisher: "kosmos".into(),
                    permissions: vec![],
                },
            ),
            enabled: true,
            revoked: false,
            installed_at: 1,
            catalog_sequence: 1,
        };
        let lease = LaunchLeaseRegistry::default()
            .create(AssetGrant {
                id: package.id.clone(),
                version: package.version.clone(),
                hash: package.hash.clone(),
            })
            .expect("lease");
        let body = launch_payload(1234, &lease, &package, LAUNCH_LEASE_TTL).to_string();
        assert!(body.contains(&lease.asset_token));
        assert!(!body.contains(&package.hash));
        for forbidden in ["bearer", "signature", "public_key", "\\\\blobs\\\\"] {
            assert!(!body.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn launch_leases_are_unique_and_independently_revocable() {
        let mut leases = LaunchLeaseRegistry::default();
        let first = leases
            .create(AssetGrant {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                hash: "a".repeat(64),
            })
            .expect("first lease");
        let second = leases
            .create(AssetGrant {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                hash: "a".repeat(64),
            })
            .expect("second lease");
        assert_ne!(first.launch_id, second.launch_id);
        assert_ne!(first.asset_token, second.asset_token);
        assert!(leases.asset(&first.asset_token).is_some());
        assert!(leases.revoke(&first.launch_id));
        assert!(leases.asset(&first.asset_token).is_none());
        assert!(leases.asset(&second.asset_token).is_some());
        assert!(!leases.revoke("not-a-launch-id"));
    }

    #[test]
    fn lease_registry_purges_expired_before_capacity_and_never_evicts_live_leases() {
        let mut leases = LaunchLeaseRegistry::with_limits(Duration::from_secs(1), 2);
        let first = leases
            .try_create_at(
                AssetGrant {
                    id: "com.kosmos.demo".into(),
                    version: "1.0.0".into(),
                    hash: "a".repeat(64),
                },
                Instant::now(),
            )
            .expect("first live lease");
        let second = leases
            .try_create_at(
                AssetGrant {
                    id: "com.kosmos.demo".into(),
                    version: "1.0.0".into(),
                    hash: "b".repeat(64),
                },
                Instant::now(),
            )
            .expect("second live lease");
        assert!(matches!(
            leases.try_create_at(
                AssetGrant {
                    id: "com.kosmos.demo".into(),
                    version: "1.0.0".into(),
                    hash: "c".repeat(64),
                },
                Instant::now(),
            ),
            Err(LeaseCapacityError)
        ));
        assert!(leases.asset(&first.asset_token).is_some());
        assert!(leases.asset(&second.asset_token).is_some());
        leases.purge_expired_at(Instant::now() + Duration::from_secs(2));
        assert_eq!(leases.len(), 0);
    }

    #[test]
    fn resolve_payload_is_manifest_only_and_never_contains_launch_fields() {
        let package = crate::package_store::InstalledPackage {
            id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            hash: "a".repeat(64),
            manifest: crate::package_manifest::VersionedManifest::V1(
                crate::package_manifest::PackageManifest {
                    schema_version: 1,
                    id: "com.kosmos.demo".into(),
                    name: "Demo".into(),
                    version: "1.0.0".into(),
                    kind: crate::package_manifest::PackageKind::App,
                    engine_api: ">=1.0.0".into(),
                    entrypoint: "index.html".into(),
                    publisher: "kosmos".into(),
                    permissions: vec![],
                },
            ),
            enabled: true,
            revoked: false,
            installed_at: 1,
            catalog_sequence: 1,
        };
        let body = resolve_payload(&package).to_string();
        assert!(body.contains("\"id\":\"com.kosmos.demo\""));
        for launch_only in [
            "launch_url",
            "launch_id",
            "asset_token",
            "expires_at",
            "ttl_seconds",
        ] {
            assert!(!body.contains(launch_only), "resolve leaked {launch_only}");
        }
    }

    #[test]
    fn repeated_resolve_payloads_never_evict_an_active_lease() {
        let mut leases = LaunchLeaseRegistry::default();
        let lease = leases
            .create(AssetGrant {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                hash: "a".repeat(64),
            })
            .expect("lease");
        let package = crate::package_store::InstalledPackage {
            id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            hash: "a".repeat(64),
            manifest: crate::package_manifest::VersionedManifest::V1(
                crate::package_manifest::PackageManifest {
                    schema_version: 1,
                    id: "com.kosmos.demo".into(),
                    name: "Demo".into(),
                    version: "1.0.0".into(),
                    kind: crate::package_manifest::PackageKind::App,
                    engine_api: ">=1.0.0".into(),
                    entrypoint: "index.html".into(),
                    publisher: "kosmos".into(),
                    permissions: vec![],
                },
            ),
            enabled: true,
            revoked: false,
            installed_at: 1,
            catalog_sequence: 1,
        };
        for _ in 0..10_000 {
            assert!(resolve_payload(&package).get("data").is_some());
        }
        assert!(leases.asset(&lease.asset_token).is_some());
    }

    #[tokio::test]
    async fn live_http_rejects_unsupported_malformed_and_oversized_requests() {
        let dir = tempfile::tempdir().unwrap();
        let usage = Arc::new(ProtocolUsageStore::open(dir.path()).unwrap());
        let token = "a".repeat(64);
        let server = EngineApiServer::bind(
            token.clone(),
            9,
            usage,
            "00000000-0000-4000-8000-000000000001".into(),
            Arc::new(PackageService::open(dir.path()).unwrap()),
            test_dispatcher(),
        )
        .await
        .unwrap();
        let port = server.port();
        let task = tokio::spawn(server.run());

        let unauthorized =
            raw_http(port, "GET /v1/health HTTP/1.1\r\nConnection: close\r\n\r\n").await;
        assert!(unauthorized.starts_with("HTTP/1.1 401"));

        let unsupported = raw_http(port, &request(&token, "POST", "/v1/health", "")).await;
        assert!(unsupported.starts_with("HTTP/1.1 405"));
        let unknown = raw_http(port, &request(&token, "GET", "/v1/unknown", "")).await;
        assert!(unknown.starts_with("HTTP/1.1 404"));

        let malformed = raw_http(port, &request(&token, "POST", "/v1/rpc", "{")).await;
        assert!(malformed.starts_with("HTTP/1.1 400"));

        let missing_operation = raw_http(port, &request(&token, "POST", "/v1/rpc", "{}")).await;
        assert!(missing_operation.starts_with("HTTP/1.1 400"));

        let oversized_body = format!(
            r#"{{"operation":"noop","padding":"{}"}}"#,
            "x".repeat(MAX_HTTP_BODY_BYTES)
        );
        let oversized = raw_http(port, &request(&token, "POST", "/v1/rpc", &oversized_body)).await;
        assert!(oversized.starts_with("HTTP/1.1 413"));

        task.abort();
    }

    #[tokio::test]
    async fn loopback_http_launch_lease_lifecycle_uses_signed_installed_package() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
        let server = EngineApiServer::bind(
            token.clone(),
            9,
            Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
            "00000000-0000-4000-8000-000000000001".into(),
            Arc::new(service),
            test_dispatcher(),
        )
        .await
        .expect("server");
        let port = server.port();
        let task = tokio::spawn(server.run());

        // Cover every new route against the real Engine v1 auth boundary;
        // no route is allowed to infer authorization from a launch-only test.
        for (method, path, body) in [
            ("POST", "/v1/apps/resolve", r#"{"id":"com.kosmos.demo"}"#),
            ("POST", "/v1/apps/launch", r#"{"id":"com.kosmos.demo"}"#),
            (
                "DELETE",
                "/v1/apps/launch/00000000-0000-4000-8000-000000000099",
                "",
            ),
        ] {
            let cases = [
                (
                    "missing bearer",
                    format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n\r\n"),
                    "HTTP/1.1 401",
                ),
                (
                    "wrong bearer",
                    request_with_headers(
                        "b",
                        method,
                        path,
                        body,
                        &std::process::id().to_string(),
                        API_VERSION,
                    ),
                    "HTTP/1.1 401",
                ),
                (
                    "dead pid",
                    request_with_headers(&token, method, path, body, "2147483647", API_VERSION),
                    "HTTP/1.1 403",
                ),
                (
                    "incompatible API",
                    request_with_headers(
                        &token,
                        method,
                        path,
                        body,
                        &std::process::id().to_string(),
                        "2.0.0",
                    ),
                    "HTTP/1.1 426",
                ),
            ];
            for (name, invalid, expected) in cases {
                let response = raw_http(port, &invalid).await;
                assert!(
                    response.starts_with(expected),
                    "{name} for {method} {path}: expected {expected}, got {response}"
                );
            }
        }

        let first = response_json(
            &raw_http(
                port,
                &request(
                    &token,
                    "POST",
                    "/v1/apps/launch",
                    r#"{"id":"com.kosmos.demo"}"#,
                ),
            )
            .await,
        );
        let first_data = &first["data"];
        let first_id = first_data["launch_id"]
            .as_str()
            .expect("launch id")
            .to_owned();
        let first_url = first_data["launch_url"]
            .as_str()
            .expect("asset url")
            .to_owned();
        let first_path = first_url
            .split_once(&format!(":{port}"))
            .expect("asset path in loopback URL")
            .1;
        assert!(raw_http(
            port,
            &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));

        for _ in 0..10_000 {
            let resolved = response_json(
                &raw_http(
                    port,
                    &request(
                        &token,
                        "POST",
                        "/v1/apps/resolve",
                        r#"{"id":"com.kosmos.demo"}"#,
                    ),
                )
                .await,
            );
            assert!(resolved["data"].get("launch_id").is_none());
        }
        assert!(raw_http(
            port,
            &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));

        let second = response_json(
            &raw_http(
                port,
                &request(
                    &token,
                    "POST",
                    "/v1/apps/launch",
                    r#"{"id":"com.kosmos.demo"}"#,
                ),
            )
            .await,
        );
        let second_id = second["data"]["launch_id"]
            .as_str()
            .expect("second launch id");
        let malformed = raw_http(
            port,
            &request(&token, "DELETE", "/v1/apps/launch/not-a-uuid", ""),
        )
        .await;
        assert!(malformed.starts_with("HTTP/1.1 404"));
        let unknown = raw_http(
            port,
            &request(
                &token,
                "DELETE",
                "/v1/apps/launch/00000000-0000-4000-8000-000000000099",
                "",
            ),
        )
        .await;
        assert!(unknown.starts_with("HTTP/1.1 404"));
        let second_path = second["data"]["launch_url"]
            .as_str()
            .expect("second url")
            .split_once(&format!(":{port}"))
            .expect("asset path in loopback URL")
            .1;
        assert!(raw_http(
            port,
            &format!("GET {second_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));
        assert_ne!(first_id, second_id);

        assert!(raw_http(
            port,
            &request(&token, "DELETE", &format!("/v1/apps/launch/{first_id}"), "")
        )
        .await
        .starts_with("HTTP/1.1 200"));
        assert!(raw_http(
            port,
            &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 404"));
        task.abort();
    }

    #[tokio::test]
    async fn lifecycle_cleanup_purges_an_idle_lease_without_a_registry_request() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
        let server = EngineApiServer::bind_with_test_limits(
            token.clone(),
            9,
            Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
            "00000000-0000-4000-8000-000000000001".into(),
            Arc::new(service),
            Duration::from_secs(300),
            2,
            Duration::from_millis(5),
        )
        .await
        .expect("server");
        let port = server.port();
        let leases = Arc::clone(&server.launch_leases);
        let task = tokio::spawn(server.run());
        let launched = response_json(
            &raw_http(
                port,
                &request(
                    &token,
                    "POST",
                    "/v1/apps/launch",
                    r#"{"id":"com.kosmos.demo"}"#,
                ),
            )
            .await,
        );
        let path = launched["data"]["launch_url"]
            .as_str()
            .expect("url")
            .split_once(&format!(":{port}"))
            .expect("loopback path")
            .1
            .to_owned();
        let launch_id = launched["data"]["launch_id"].as_str().expect("launch id");

        expire_launch_for_test(&leases, launch_id);
        wait_for_lease_count(&leases, 0).await;
        assert!(raw_http(
            port,
            &format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 404"));
        task.abort();
    }

    #[tokio::test]
    async fn http_capacity_preserves_live_assets_and_accepts_a_launch_after_lifecycle_purge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
        let server = EngineApiServer::bind_with_test_limits(
            token.clone(),
            9,
            Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
            "00000000-0000-4000-8000-000000000001".into(),
            Arc::new(service),
            Duration::from_secs(300),
            2,
            Duration::from_millis(5),
        )
        .await
        .expect("server");
        let port = server.port();
        let leases = Arc::clone(&server.launch_leases);
        let task = tokio::spawn(server.run());
        let launch = || {
            request(
                &token,
                "POST",
                "/v1/apps/launch",
                r#"{"id":"com.kosmos.demo"}"#,
            )
        };
        let first = response_json(&raw_http(port, &launch()).await);
        let first_path = first["data"]["launch_url"]
            .as_str()
            .expect("first url")
            .split_once(&format!(":{port}"))
            .expect("first path")
            .1
            .to_owned();
        let first_id = first["data"]["launch_id"]
            .as_str()
            .expect("first launch id")
            .to_owned();
        let second = response_json(&raw_http(port, &launch()).await);
        let second_path = second["data"]["launch_url"]
            .as_str()
            .expect("second url")
            .split_once(&format!(":{port}"))
            .expect("second path")
            .1
            .to_owned();
        assert!(raw_http(port, &launch()).await.starts_with("HTTP/1.1 429"));
        for path in [&first_path, &second_path] {
            assert!(raw_http(
                port,
                &format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n")
            )
            .await
            .starts_with("HTTP/1.1 200"));
        }

        expire_launch_for_test(&leases, &first_id);
        wait_for_lease_count(&leases, 1).await;
        assert!(raw_http(
            port,
            &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 404"));
        assert!(raw_http(
            port,
            &format!("GET {second_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));
        assert!(raw_http(port, &launch()).await.starts_with("HTTP/1.1 200"));
        assert!(raw_http(
            port,
            &format!("GET {second_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));
        task.abort();
    }

    fn expire_launch_for_test(leases: &Arc<Mutex<LaunchLeaseRegistry>>, launch_id: &str) {
        let mut leases = leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        leases
            .leases
            .get_mut(launch_id)
            .expect("launch lease")
            .expires_at = Instant::now() - Duration::from_secs(1);
    }

    async fn wait_for_lease_count(leases: &Arc<Mutex<LaunchLeaseRegistry>>, expected: usize) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let count = leases
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .len();
                if count == expected {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("lifecycle cleanup deadline");
    }

    #[tokio::test]
    async fn http_preserves_request_identity_and_defaults_client_context() {
        let observed = Arc::new(Mutex::new(None));
        let observed_for_handler = observed.clone();
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
            move |request| {
                let observed = observed_for_handler.clone();
                Box::pin(async move {
                    *observed.lock().unwrap() = Some(request.clone());
                    Ok(json!({
                        "id": request.request_id,
                        "ok": true,
                    }))
                })
            },
        )));
        let token = "a".repeat(64);
        let server =
            EngineApiServer::bind_with_test_dispatcher(token.clone(), dispatcher, REQUEST_TIMEOUT)
                .await
                .expect("server");
        let port = server.port();
        let task = tokio::spawn(server.run());
        let mut child_command = if cfg!(windows) {
            let mut command = std::process::Command::new(
                std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into()),
            );
            command.args(["/C", "timeout", "/T", "5", "/NOBREAK"]);
            #[cfg(windows)]
            std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
            command
        } else {
            let mut command = std::process::Command::new("sleep");
            command.arg("5");
            command
        };
        let mut child = child_command.spawn().expect("same-user child");
        let child_pid = child.id();
        assert_ne!(child_pid, std::process::id());
        let response = raw_http(
            port,
            &request_with_pid(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"preserved-id","operation":"test.read"}"#,
                &child_pid.to_string(),
            ),
        )
        .await;
        let body = response_json(&response);
        assert_eq!(body["id"], "preserved-id");
        let request = observed.lock().unwrap().clone().expect("dispatch request");
        assert_eq!(request.request_id.as_deref(), Some("preserved-id"));
        assert_eq!(request.client.pid, Some(child_pid));
        assert_eq!(request.client.class.as_deref(), Some("engine-http"));
        assert_eq!(request.client.version.as_deref(), Some(API_VERSION));
        assert!(request.client.connection_id.is_some());
        task.abort();
        let _ = child.kill();
        let _ = child.wait();
    }

    #[tokio::test]
    async fn direct_http_rpc_records_only_authenticated_requests_in_bounded_api_v1_buckets() {
        let dir = tempfile::tempdir().expect("usage dir");
        let usage = Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage"));
        let token = "a".repeat(64);
        let server = EngineApiServer::bind(
            token.clone(),
            9,
            usage.clone(),
            "00000000-0000-4000-8000-000000000001".into(),
            Arc::new(PackageService::open(dir.path()).expect("packages")),
            test_dispatcher(),
        )
        .await
        .expect("server");
        let port = server.port();
        let task = tokio::spawn(server.run());

        let valid = request_with_client(
            &token,
            "POST",
            "/v1/rpc",
            r#"{"operation":"noted"}"#,
            "desktop-host",
            "1.2.3",
        );
        assert!(raw_http(port, &valid).await.starts_with("HTTP/1.1 502"));
        assert!(raw_http(port, &request(&token, "POST", "/v1/rpc", "{"))
            .await
            .starts_with("HTTP/1.1 400"));
        assert!(
            raw_http(port, "POST /v1/rpc HTTP/1.1\r\nConnection: close\r\n\r\n")
                .await
                .starts_with("HTTP/1.1 401")
        );
        assert!(raw_http(
            port,
            &request_with_pid(&token, "POST", "/v1/rpc", "{}", "2147483647")
        )
        .await
        .starts_with("HTTP/1.1 403"));

        let snapshot = usage.snapshot();
        assert_eq!(snapshot.api_v1.connections, 1);
        assert_eq!(snapshot.legacy.connections, 0);
        assert_eq!(
            snapshot
                .clients
                .get("api_v1:desktop-host@1.2.3")
                .map(|counter| counter.connections),
            Some(1)
        );
        assert_eq!(
            snapshot
                .clients
                .get("api_v1:engine-http@1.0.0")
                .map(|counter| counter.connections),
            None
        );
        let raw = std::fs::read_to_string(dir.path().join(crate::protocol_usage::FILE_NAME))
            .expect("usage file");
        assert!(!raw.contains("2147483647"));
        assert!(!raw.contains("noted"));
        task.abort();
    }

    #[tokio::test]
    async fn dropping_response_wait_starts_continuation_and_restores_capacity() {
        let cleanup_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
            Arc::new(|_| {
                Box::pin(async {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    Ok(json!({"never": "returns"}))
                })
            }),
            Arc::new({
                let cleanup_count = cleanup_count.clone();
                move |_| {
                    let cleanup_count = cleanup_count.clone();
                    cleanup_count.fetch_add(1, Ordering::SeqCst);
                }
            }),
        ));
        let server = EngineApiServer::bind_with_test_dispatcher_and_deadline(
            "a".repeat(64),
            dispatcher.clone(),
            Duration::from_secs(30),
            Duration::from_millis(20),
        )
        .await
        .expect("server");
        let owner = dispatcher.allocate_owner().unwrap();
        let owner_id = owner.id();
        let request =
            crate::engine_dispatch::DispatchRequest::from_wire(json!({"operation":"stalled"}))
                .expect("request")
                .with_client(crate::engine_dispatch::DispatchClient {
                    connection_id: Some(owner_id),
                    desktop_authorized: false,
                    ..Default::default()
                });
        let (_, receiver, guard) = server
            .operations
            .start(request, dispatcher.clone(), owner)
            .await
            .expect("admitted");
        drop(receiver);
        drop(guard);
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
        assert!(server.operations.permits.available_permits() > 0);
    }

    #[tokio::test]
    async fn response_timeout_is_not_replaced_by_secondary_deadline_and_continuation_is_bounded() {
        let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cleanups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
            Arc::new({
                let writes = writes.clone();
                move |request| {
                    let writes = writes.clone();
                    Box::pin(async move {
                        writes.fetch_add(1, Ordering::SeqCst);
                        if request.operation.as_str() == "finishes-before-response-timeout" {
                            tokio::time::sleep(Duration::from_millis(35)).await;
                            Ok(json!({"ok": true}))
                        } else {
                            tokio::time::sleep(Duration::from_millis(200)).await;
                            Ok(json!({"ok": true}))
                        }
                    })
                }
            }),
            Arc::new({
                let cleanups = cleanups.clone();
                move |_| {
                    let cleanups = cleanups.clone();
                    cleanups.fetch_add(1, Ordering::SeqCst);
                }
            }),
        ));
        let token = "a".repeat(64);
        let server = EngineApiServer::bind_with_test_dispatcher_and_deadline(
            token.clone(),
            dispatcher,
            Duration::from_millis(50),
            Duration::from_millis(30),
        )
        .await
        .expect("server");
        let port = server.port();
        let task = tokio::spawn(server.run());

        assert!(raw_http(
            port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"operation":"finishes-before-response-timeout"}"#
            )
        )
        .await
        .starts_with("HTTP/1.1 200"));
        assert!(raw_http(
            port,
            &request(&token, "POST", "/v1/rpc", r#"{"operation":"times-out"}"#)
        )
        .await
        .starts_with("HTTP/1.1 502"));
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(writes.load(Ordering::SeqCst), 2);
        assert_eq!(cleanups.load(Ordering::SeqCst), 2);
        task.abort();
    }

    #[tokio::test]
    async fn http_timeout_keeps_started_write_exactly_once_and_cleans_connection() {
        let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cleanups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let writes_for_handler = writes.clone();
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
            Arc::new(move |request| {
                let writes = writes_for_handler.clone();
                Box::pin(async move {
                    if request.operation.as_str() == "controlled.write" {
                        writes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(40)).await;
                    }
                    Ok(json!({"ok": true, "written": true}))
                })
            }),
            Arc::new({
                let cleanups = cleanups.clone();
                move |_| {
                    let cleanups = cleanups.clone();
                    cleanups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }),
        ));
        let token = "a".repeat(64);
        let server = EngineApiServer::bind_with_test_dispatcher(
            token.clone(),
            dispatcher,
            Duration::from_millis(10),
        )
        .await
        .expect("server");
        let port = server.port();
        let task = tokio::spawn(server.run());

        let response = raw_http(
            port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"write-1","operation":"controlled.write"}"#,
            ),
        )
        .await;
        assert!(response.starts_with("HTTP/1.1 502"), "response: {response}");

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if writes.load(std::sync::atomic::Ordering::SeqCst) == 1
                    && cleanups.load(std::sync::atomic::Ordering::SeqCst) == 1
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("operation and cleanup must complete");
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(cleanups.load(std::sync::atomic::Ordering::SeqCst), 1);
        task.abort();
    }

    #[tokio::test]
    async fn http_disconnect_keeps_owned_operation_and_cleanup_after_response_is_abandoned() {
        let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cleanups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
            Arc::new({
                let writes = writes.clone();
                move |_| {
                    let writes = writes.clone();
                    Box::pin(async move {
                        writes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_secs(60)).await;
                        Ok(json!({"ok": true}))
                    })
                }
            }),
            Arc::new({
                let cleanups = cleanups.clone();
                move |_| {
                    let cleanups = cleanups.clone();
                    cleanups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }),
        ));
        let token = "a".repeat(64);
        let server = EngineApiServer::bind_with_test_dispatcher_and_deadline(
            token.clone(),
            dispatcher.clone(),
            Duration::from_secs(1),
            Duration::from_millis(20),
        )
        .await
        .expect("server");
        let port = server.port();
        let permits = server.operations.permits.clone();
        let task = tokio::spawn(server.run());
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        let request = request(
            &token,
            "POST",
            "/v1/rpc",
            r#"{"operation":"controlled.write"}"#,
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if writes.load(std::sync::atomic::Ordering::SeqCst) == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("dispatch must start before disconnect");
        stream.shutdown().await.unwrap();
        drop(stream);
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if writes.load(std::sync::atomic::Ordering::SeqCst) == 1
                    && cleanups.load(std::sync::atomic::Ordering::SeqCst) == 1
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("owned operation must finish and clean up after disconnect");
        assert!(permits.available_permits() > 0);
        task.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn production_dispatcher_has_real_http_ws_socket_parity_and_owner_isolation() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let binary = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows) {
            "../../target/debug/ark-core-rpc.exe"
        } else {
            "../../target/debug/ark-core-rpc"
        });
        assert!(binary.exists(), "real ark-core-rpc fixture must be built");
        let ark = Arc::new(
            crate::ark_host::ArkHost::spawn(&binary, &dir.path().join("ark.db").to_string_lossy())
                .await
                .expect("ark host fixture"),
        );
        let app_index = Arc::new(
            crate::app_index::AppIndex::new(dir.path(), dir.path().join("icons"))
                .expect("app index"),
        );
        let file_index =
            Arc::new(crate::file_index::FileIndex::new_disabled(dir.path()).expect("file index"));
        let package_service =
            Arc::new(crate::package_service::PackageService::open(dir.path()).expect("packages"));
        let usage =
            Arc::new(crate::protocol_usage::ProtocolUsageStore::open(dir.path()).expect("usage"));
        let token = "a".repeat(64);
        let ws = crate::ws_server::WsServer::bind(
            ark,
            token.clone(),
            dir.path().to_path_buf(),
            app_index,
            file_index,
            Arc::new(crate::usage_tracker::UsageTrackerDiagnosticsState::default()),
            usage.clone(),
            package_service.clone(),
            "00000000-0000-4000-8000-000000000001".into(),
        )
        .await
        .expect("ws bind");
        let ws_port = ws.port();
        let dispatcher = Arc::new(ws.dispatcher());
        struct ObserverState {
            events: Vec<(crate::engine_dispatch::DispatchPhase, u64, String)>,
            active: usize,
            max_active: usize,
        }
        let observed = Arc::new(std::sync::Mutex::new(ObserverState {
            events: Vec::new(),
            active: 0,
            max_active: 0,
        }));
        let observed_for_dispatch = observed.clone();
        dispatcher.set_test_observer(Some(Arc::new(move |phase, owner, operation| {
            let mut state = observed_for_dispatch.lock().unwrap();
            state.events.push((phase, owner, operation.to_string()));
            if phase == crate::engine_dispatch::DispatchPhase::Started {
                state.active += 1;
                state.max_active = state.max_active.max(state.active);
            } else {
                state.active -= 1;
            }
        })));
        let api = EngineApiServer::bind(
            token.clone(),
            ws_port,
            usage,
            "00000000-0000-4000-8000-000000000001".into(),
            package_service,
            dispatcher,
        )
        .await
        .expect("http bind");
        let http_port = api.port();
        let api_task = tokio::spawn(api.run());

        // The HTTP adapter is independently useful: the legacy listener is
        // bound but deliberately not serving for this first real-socket call.
        let http_without_ws = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"http-without-ws","operation":"list_object_types"}"#,
            ),
        )
        .await;
        assert!(
            http_without_ws.starts_with("HTTP/1.1 200"),
            "{http_without_ws}"
        );
        {
            let mut state = observed.lock().unwrap();
            state.events.clear();
            state.active = 0;
            state.max_active = 0;
        }
        let mut keep_alive = tokio::net::TcpStream::connect(("127.0.0.1", http_port))
            .await
            .expect("keep-alive socket");
        for id in ["keep-1", "keep-2"] {
            keep_alive
                .write_all(
                    request(
                        &token,
                        "POST",
                        "/v1/rpc",
                        &format!(r#"{{"_req_id":"{id}","operation":"list_object_types"}}"#),
                    )
                    .replace("Connection: close", "Connection: keep-alive")
                    .as_bytes(),
                )
                .await
                .expect("keep-alive request");
            assert!(read_http_response(&mut keep_alive)
                .await
                .contains(&format!("\"id\":\"{id}\"")));
        }
        let sequential_starts: Vec<u64> = observed
            .lock()
            .unwrap()
            .events
            .iter()
            .filter(|(phase, _, operation)| {
                *phase == crate::engine_dispatch::DispatchPhase::Started
                    && operation == "list_object_types"
            })
            .map(|(_, owner, _)| *owner)
            .collect();
        assert_eq!(sequential_starts.len(), 2);
        assert!(sequential_starts.iter().all(|owner| *owner != 0));
        assert_ne!(sequential_starts[0], sequential_starts[1]);
        assert_eq!(observed.lock().unwrap().max_active, 1);

        let concurrent_request_a = request(
            &token,
            "POST",
            "/v1/rpc",
            r#"{"_req_id":"concurrent-a","operation":"list_object_types"}"#,
        );
        let concurrent_request_b = request(
            &token,
            "POST",
            "/v1/rpc",
            r#"{"_req_id":"concurrent-b","operation":"list_object_types"}"#,
        );
        let (concurrent_a, concurrent_b) = tokio::join!(
            raw_http(http_port, &concurrent_request_a),
            raw_http(http_port, &concurrent_request_b),
        );
        assert!(concurrent_a.contains("\"id\":\"concurrent-a\""));
        assert!(concurrent_b.contains("\"id\":\"concurrent-b\""));
        let concurrent_starts: Vec<u64> = observed
            .lock()
            .unwrap()
            .events
            .iter()
            .filter(|(phase, _, operation)| {
                *phase == crate::engine_dispatch::DispatchPhase::Started
                    && operation == "list_object_types"
            })
            .map(|(_, owner, _)| *owner)
            .skip(2)
            .collect();
        assert_eq!(concurrent_starts.len(), 2);
        assert_ne!(concurrent_starts[0], concurrent_starts[1]);
        assert!(observed.lock().unwrap().max_active >= 1);
        let ws_shutdown = ws.shutdown_handle();
        let ws_task = tokio::spawn(ws.run());

        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{ws_port}"))
            .await
            .expect("ws socket");
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::json!({
                    "kind": "hello",
                    "apiVersion": API_VERSION,
                    "token": token,
                    "pid": std::process::id(),
                })
                .to_string(),
            ))
            .await
            .expect("hello");
        let hello = socket
            .next()
            .await
            .expect("hello response")
            .expect("hello frame");
        assert!(hello.to_text().expect("hello text").contains("hello_ok"));

        let ws_read = r#"{"_req_id":"ws-read","operation":"list_object_types"}"#;
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                ws_read.into(),
            ))
            .await
            .expect("ws read");
        let ws_response = socket
            .next()
            .await
            .expect("ws read response")
            .expect("ws frame");
        let ws_json: Value =
            serde_json::from_str(ws_response.to_text().expect("ws text")).expect("ws json");
        let http_response = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"http-read","operation":"list_object_types"}"#,
            ),
        )
        .await;
        let http_json = response_json(&http_response);
        assert_eq!(ws_json["ok"], http_json["ok"]);
        assert_eq!(ws_json["data"], http_json["data"]);

        let ws_write = serde_json::json!({
            "_req_id": "ws-write-1",
            "operation": "upsert_object_type",
            "object_type": {
                "id": "ws-note",
                "name": "WS Note",
                "schemaJson": "{}",
                "uiSchemaJson": "{}",
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z",
                "systemLocked": false
            },
            "device_id": "socket-test-ws"
        });
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                ws_write.to_string(),
            ))
            .await
            .expect("ws write");
        let ws_write_response: Value = serde_json::from_str(
            socket
                .next()
                .await
                .expect("ws write response")
                .expect("ws write frame")
                .to_text()
                .expect("ws write text"),
        )
        .expect("ws write JSON");
        assert_eq!(ws_write_response["ok"], true);

        let mut ws_malformed = Value::Null;
        let mut ws_unknown = Value::Null;
        for invalid in [
            serde_json::json!({"_req_id":"ws-malformed","operation":"commands.register"}),
            serde_json::json!({"_req_id":"unknown-op","operation":"definitely.unknown"}),
        ] {
            socket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    invalid.to_string(),
                ))
                .await
                .expect("ws invalid request");
            let response: Value = serde_json::from_str(
                socket
                    .next()
                    .await
                    .expect("ws invalid response")
                    .expect("ws invalid frame")
                    .to_text()
                    .expect("ws invalid text"),
            )
            .expect("ws invalid JSON");
            if response["id"] == "ws-malformed" {
                ws_malformed = response.clone();
            } else {
                ws_unknown = response.clone();
            }
            assert_eq!(response["ok"], false);
        }

        let manifest = r#"[{"id":"ws.command","title":"WS","category":"open"}]"#;
        let register = format!(r#"{{"operation":"commands.register","commands":{manifest}}}"#);
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(register))
            .await
            .expect("ws register");
        let _ = socket.next().await.expect("ws register response");
        let http_register = raw_http(http_port, &request(&token, "POST", "/v1/rpc", r#"{"operation":"commands.register","commands":[{"id":"http.command","title":"HTTP","category":"action"}]}"#)).await;
        assert!(response_json(&http_register)["ok"]
            .as_bool()
            .unwrap_or(false));
        let list = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"operation":"commands.list"}"#,
            ),
        )
        .await;
        let commands = &response_json(&list)["data"]["commands"];
        assert!(commands
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "ws.command"));
        assert!(!commands
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "http.command"));

        let malformed = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"malformed-params","operation":"commands.register"}"#,
            ),
        )
        .await;
        assert!(response_json(&malformed)["ok"] == false);
        let unknown = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"unknown-op","operation":"definitely.unknown"}"#,
            ),
        )
        .await;
        let unknown_json = response_json(&unknown);
        assert_eq!(unknown_json, ws_unknown);
        assert_eq!(unknown_json["id"], "unknown-op");
        assert_eq!(unknown_json["ok"], false);
        assert!(unknown_json["error"]
            .as_str()
            .unwrap_or_default()
            .contains("unknown variant"));
        let malformed_parity = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"ws-malformed","operation":"commands.register"}"#,
            ),
        )
        .await;
        assert_eq!(response_json(&malformed_parity), ws_malformed);

        let type_write = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"type-write-1","operation":"upsert_object_type","object_type":{"id":"note","name":"Note","schemaJson":"{}","uiSchemaJson":"{}","createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","systemLocked":false},"device_id":"socket-test"}"#,
            ),
        )
        .await;
        assert!(
            response_json(&type_write)["ok"].as_bool().unwrap_or(false),
            "{type_write}"
        );
        let write = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"write-1","operation":"upsert_object","object":{"id":"direct-dispatch-write","typeId":"note","title":"socket","contentJson":{},"propsJson":{},"createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","deletedAt":null},"device_id":"socket-test"}"#,
            ),
        )
        .await;
        assert!(
            response_json(&write)["ok"].as_bool().unwrap_or(false),
            "{write}"
        );

        let invoke = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"invoke-1","operation":"commands.invoke","id":"ws.command","params":{"source":"http"}}"#,
            ),
        )
        .await;
        assert!(
            response_json(&invoke)["ok"].as_bool().unwrap_or(false),
            "{invoke}"
        );
        let mut invoked_events = 0;
        let event_window = Instant::now() + Duration::from_millis(500);
        loop {
            let remaining = event_window.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let Some(Ok(frame)) = tokio::time::timeout(remaining, socket.next())
                .await
                .ok()
                .flatten()
            else {
                break;
            };
            let payload: Value =
                serde_json::from_str(frame.to_text().expect("event text")).expect("event JSON");
            if payload["event"] == "command_invoked" && payload["id"] == "ws.command" {
                invoked_events += 1;
                assert_eq!(payload["params"]["source"], "http");
            }
        }
        assert_eq!(invoked_events, 1, "event must be delivered exactly once");

        ws_shutdown.begin_shutdown().await;
        ws_task.await.expect("ws server task").expect("ws server");
        let first_ws_shutdown = ws_shutdown.shutdown().await;
        assert!(
            first_ws_shutdown.is_err(),
            "stalled lifecycle must report deadline breach"
        );
        ws_shutdown
            .shutdown()
            .await
            .expect("idempotent ws shutdown");
        api_task.abort();
    }

    #[tokio::test]
    async fn shutdown_handle_aborts_stalled_operation_and_releases_ownership_once() {
        let cleanups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
            Arc::new(|_| {
                Box::pin(async {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    Ok(json!({"never": "returned"}))
                })
            }),
            Arc::new({
                let cleanups = cleanups.clone();
                move |_| {
                    let cleanups = cleanups.clone();
                    cleanups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }),
        ));
        let token = "a".repeat(64);
        let server = EngineApiServer::bind_with_test_dispatcher(
            token,
            dispatcher.clone(),
            Duration::from_secs(30),
        )
        .await
        .expect("server");
        let owner = dispatcher.allocate_owner().unwrap();
        let owner_id = owner.id();

        let request = crate::engine_dispatch::DispatchRequest::from_wire(
            json!({"operation":"stalled.write"}),
        )
        .expect("request")
        .with_client(crate::engine_dispatch::DispatchClient {
            connection_id: Some(owner_id),
            desktop_authorized: false,
            ..Default::default()
        });
        let _response = server.operations.start(request, dispatcher, owner).await;
        let handle = server.shutdown_handle();
        let _ = tokio::time::timeout(Duration::from_secs(1), handle.shutdown())
            .await
            .expect("shutdown deadline");
        assert_eq!(cleanups.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(server.operations.closed.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn shutdown_closes_admission_and_completes_synchronous_cleanup() {
        let cleanup_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
            Arc::new(|_| Box::pin(async { Ok(json!({"ok": true})) })),
            Arc::new({
                let cleanup_count = cleanup_count.clone();
                move |_| {
                    cleanup_count.fetch_add(1, Ordering::SeqCst);
                }
            }),
        ));
        let server = EngineApiServer::bind_with_test_dispatcher(
            "a".repeat(64),
            dispatcher.clone(),
            Duration::from_secs(1),
        )
        .await
        .expect("server");
        let owner = dispatcher.allocate_owner().unwrap();
        let owner_id = owner.id();
        let request = crate::engine_dispatch::DispatchRequest::from_wire(
            json!({"operation":"controlled.cleanup"}),
        )
        .expect("request")
        .with_client(crate::engine_dispatch::DispatchClient {
            connection_id: Some(owner_id),
            desktop_authorized: false,
            ..Default::default()
        });
        assert!(server
            .operations
            .start(request, dispatcher.clone(), owner)
            .await
            .is_some());
        server.shutdown_handle().shutdown().await.expect("shutdown");
        assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
        assert!(server.operations.closed.load(Ordering::Acquire));
        assert!(server.operations.permits.available_permits() > 0);
    }

    #[tokio::test]
    async fn http_dispatch_starts_only_after_registry_install() {
        let registry = HttpOperationRegistry::default();
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed_registry = registry.clone();
        let observed_started = started.clone();
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
            move |_| {
                let installed = !observed_registry
                    .continuations
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .is_empty();
                observed_started.store(installed, Ordering::SeqCst);
                Box::pin(async { Ok(json!({"ok": true})) })
            },
        )));
        let owner = dispatcher.allocate_owner().unwrap();
        let request = crate::engine_dispatch::DispatchRequest::from_wire(
            json!({"operation":"install-gated"}),
        )
        .unwrap()
        .with_client(crate::engine_dispatch::DispatchClient {
            connection_id: Some(owner.id()),
            ..Default::default()
        });

        let (_, receiver, _guard) = registry
            .start(request, dispatcher, owner)
            .await
            .expect("operation installed");
        assert!(receiver.await.unwrap().is_ok());
        assert!(started.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn admission_racing_shutdown_cannot_insert_after_drain() {
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
            |_| Box::pin(async { Ok(json!({"ok": true})) }),
        )));
        let server = EngineApiServer::bind_with_test_dispatcher(
            "a".repeat(64),
            dispatcher.clone(),
            Duration::from_secs(1),
        )
        .await
        .expect("server");
        let mut admissions = Vec::new();
        for _ in 0..64 {
            let registry = server.operations.clone();
            let dispatcher = dispatcher.clone();
            admissions.push(tokio::spawn(async move {
                let owner = dispatcher.allocate_owner().unwrap();
                let owner_id = owner.id();
                let request =
                    crate::engine_dispatch::DispatchRequest::from_wire(json!({"operation":"race"}))
                        .expect("request")
                        .with_client(crate::engine_dispatch::DispatchClient {
                            connection_id: Some(owner_id),
                            desktop_authorized: false,
                            ..Default::default()
                        });
                registry.start(request, dispatcher, owner).await.is_some()
            }));
        }
        let shutdown_handle = server.shutdown_handle();
        let shutdown = tokio::spawn(async move { shutdown_handle.shutdown().await });
        for admission in admissions {
            let _ = admission.await.expect("admission task");
        }
        let _ = shutdown.await.expect("shutdown task");
        assert!(server.operations.closed.load(Ordering::Acquire));
        assert!(server.operations.continuations.lock().unwrap().is_empty());
        assert_eq!(dispatcher.live_owner_count(), 0);
    }

    fn request(token: &str, method: &str, path: &str, body: &str) -> String {
        request_with_pid(token, method, path, body, &std::process::id().to_string())
    }

    fn request_with_pid(token: &str, method: &str, path: &str, body: &str, pid: &str) -> String {
        request_with_headers(token, method, path, body, pid, API_VERSION)
    }

    fn request_with_client(
        token: &str,
        method: &str,
        path: &str,
        body: &str,
        class: &str,
        version: &str,
    ) -> String {
        request_with_headers(token, method, path, body, &std::process::id().to_string(), API_VERSION)
            .replace(
                "Content-Length:",
                &format!(
                    "X-Kosmos-Client-Class: {class}\r\nX-Kosmos-Client-Version: {version}\r\nContent-Length:"
                ),
            )
    }

    fn request_with_headers(
        token: &str,
        method: &str,
        path: &str,
        body: &str,
        pid: &str,
        api_version: &str,
    ) -> String {
        format!(
            "{method} {path} HTTP/1.1\r\n\
             Authorization: Bearer {token}\r\n\
             X-Kosmos-Client-Pid: {pid}\r\n\
             X-Kosmos-Api-Version: {api_version}\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn response_json(response: &str) -> Value {
        serde_json::from_str(response.split_once("\r\n\r\n").expect("HTTP body").1)
            .expect("JSON response")
    }

    fn test_dispatcher() -> Arc<crate::engine_dispatch::EngineDispatcher> {
        Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
            |_| {
                Box::pin(async {
                    Err::<Value, crate::engine_dispatch::DispatchError>(
                        crate::engine_dispatch::DispatchError::Unavailable,
                    )
                })
            },
        )))
    }

    async fn raw_http(port: u16, request: &str) -> String {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        String::from_utf8(response).unwrap()
    }

    async fn read_http_response(stream: &mut tokio::net::TcpStream) -> String {
        let mut response = Vec::new();
        let header_end = loop {
            let mut chunk = [0_u8; 1024];
            let count = stream.read(&mut chunk).await.expect("HTTP response");
            assert!(count > 0, "HTTP socket closed before response");
            response.extend_from_slice(&chunk[..count]);
            if let Some(end) = response.windows(4).position(|window| window == b"\r\n\r\n") {
                break end + 4;
            }
        };
        let headers = String::from_utf8_lossy(&response[..header_end]);
        let length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .map(str::to_owned)
            })
            .and_then(|value| value.trim().parse::<usize>().ok())
            .expect("content length");
        while response.len() < header_end + length {
            let mut chunk = [0_u8; 1024];
            let count = stream.read(&mut chunk).await.expect("HTTP body");
            assert!(count > 0, "HTTP socket closed before body");
            response.extend_from_slice(&chunk[..count]);
        }
        String::from_utf8(response).expect("HTTP response text")
    }
}
