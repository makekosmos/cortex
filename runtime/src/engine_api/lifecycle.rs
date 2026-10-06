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
    Reserved,
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
        let (start_sender, start_receiver) = oneshot::channel();
        self.continuations
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(operation_id, HttpOperationSlot::Reserved);
        let handle = tokio::spawn(async move {
            if start_receiver.await.is_err() {
                return;
            }
            let _permit = permit;
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
        debug_assert!(matches!(old, Some(HttpOperationSlot::Reserved)));
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
            tracing::error!(
                ?deadline,
                elapsed = ?started.elapsed(),
                "HTTP shutdown exceeded its bounded cleanup lifecycle",
            );
            Err("HTTP shutdown exceeded its deadline")
        } else {
            Ok(())
        }
    }
}
