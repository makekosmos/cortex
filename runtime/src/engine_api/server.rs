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
}

pub struct EngineApiServer {
    listener: TcpListener,
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    auth_token: Arc<String>,
    ws_port: u16,
    protocol_usage: Arc<ProtocolUsageStore>,
    correlation_id: Arc<String>,
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
        dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    ) -> Result<Self, EngineApiError> {
        Self::bind_with_limits(
            auth_token,
            ws_port,
            protocol_usage,
            correlation_id,
            dispatcher,
            Duration::from_secs(1),
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

    /// The TempDir is returned to the caller: the server keeps open file
    /// handles inside it, so the directory must outlive the server —
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
            dispatcher,
            Duration::from_secs(1),
            request_timeout,
        )
        .await?;
        server.operations.continuation_deadline = continuation_deadline;
        Ok((dir, server))
    }

    async fn bind_with_limits(
        auth_token: String,
        ws_port: u16,
        protocol_usage: Arc<ProtocolUsageStore>,
        correlation_id: String,
        dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
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
                accept = self.listener.accept() => match accept {
                    Ok(accepted) => Some(accepted),
                    // A burst can leave aborted entries in the accept queue
                    // (BSD/macOS report them, Linux does not) — a dropped
                    // socket must not kill the whole listener.
                    Err(_) => {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                        continue;
                    }
                },
                _ = self.connections.cancelled() => return Ok(()),
                _ = cleanup.tick() => {
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
            let dispatcher = self.dispatcher.clone();
            let request_timeout = self.request_timeout;
            let operations = self.operations.clone();
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
            let (start_sender, start_receiver) = oneshot::channel();
            connections
                .tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(
                    task_id,
                    HttpConnectionSlot::Reserved {
                        start: start_sender,
                    },
                );
            let task = tokio::spawn(async move {
                let _permit = permit;
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
                        dispatcher.clone(),
                        request_timeout,
                        operations.clone(),
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
