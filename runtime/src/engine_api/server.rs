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
    user_data: Arc<crate::user_data::UserDataRoots>,
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
    ) -> Result<(tempfile::TempDir, Self), EngineApiError> {
        Self::bind_with_test_dispatcher_and_deadline(
            auth_token,
            dispatcher,
            request_timeout,
            Duration::from_secs(5),
        )
        .await
    }

    /// The TempDir is returned to the caller: the server's stores keep open
    /// file handles inside it, so the directory must outlive the server —
    /// dropping it at helper return leaks it (KOS-270).
    #[cfg(test)]
    async fn bind_with_test_dispatcher_and_deadline(
        auth_token: String,
        dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
        request_timeout: Duration,
        continuation_deadline: Duration,
    ) -> Result<(tempfile::TempDir, Self), EngineApiError> {
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
        Ok((dir, server))
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
            user_data: Arc::new(crate::user_data::UserDataRoots::new()),
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
            let user_data = self.user_data.clone();
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
                        user_data.clone(),
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
