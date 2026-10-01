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
        let launch_leases = Arc::new(Mutex::new(LaunchLeaseRegistry::with_limits(ttl, capacity)));
        // The dispatch-side `packages.open` mints through this same registry —
        // wire the surface before serving so the op is never half-configured.
        package_service.configure_launch_surface(crate::package_launch::LaunchSurface {
            leases: launch_leases.clone(),
            http_port: listener.local_addr()?.port(),
        });
        Ok(Self {
            listener,
            dispatcher,
            auth_token: Arc::new(auth_token),
            ws_port,
            protocol_usage,
            correlation_id: Arc::new(correlation_id),
            package_service,
            launch_leases,
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
