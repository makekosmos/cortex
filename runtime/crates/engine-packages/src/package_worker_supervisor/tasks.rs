use super::*;

#[cfg(any(windows, target_os = "macos"))]
pub(super) struct SpawnedTasks {
    pub(super) stdin_tx: mpsc::UnboundedSender<Vec<u8>>,
    pub(super) lifecycle_tx: mpsc::UnboundedSender<WorkerLifecycleEvent>,
    pub(super) bootstrap_ack_rx: oneshot::Receiver<Result<(), ()>>,
    pub(super) hello_tx: oneshot::Sender<Result<(), &'static str>>,
    pub(super) hello_rx: oneshot::Receiver<Result<(), &'static str>>,
    pub(super) stdout_tail: Arc<Mutex<BoundedTextTail>>,
    pub(super) stderr_tail: Arc<Mutex<BoundedTextTail>>,
}

impl PackageWorkerSupervisor {
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) async fn spawn_worker_tasks<I, O, E>(
        &self,
        key: &(String, String),
        generation: u64,
        deadline: Instant,
        lifecycle_task_key: TaskKey,
        worker_io_keys: Vec<TaskKey>,
        mut io_gates: Vec<(oneshot::Receiver<bool>, oneshot::Receiver<()>)>,
        cancel_rx: &mut oneshot::Receiver<()>,
        mut stdin: I,
        stdout: O,
        stderr: E,
        line: Vec<u8>,
    ) -> Result<SpawnedTasks, &'static str>
    where
        I: AsyncWrite + Unpin + Send + 'static,
        O: AsyncRead + Unpin + Send + 'static,
        E: AsyncRead + Unpin + Send + 'static,
    {
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
            return Err("worker-unavailable");
        }
        let (hello_tx, hello_rx) = oneshot::channel();
        let (lifecycle_tx, mut lifecycle_rx) = mpsc::unbounded_channel::<WorkerLifecycleEvent>();
        self.inner.lifecycles.reap_completed().await;
        if cancellation_pending(cancel_rx).await {
            return Err("worker-unavailable");
        }
        let Some((lifecycle_start, lifecycle_cancel)) =
            self.inner.lifecycles.reserve(lifecycle_task_key.clone())
        else {
            return Err("worker-unavailable");
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
            return Err("worker-unavailable");
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
            return Err("worker-unavailable");
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
            return Err("worker-unavailable");
        }
        Ok(SpawnedTasks {
            stdin_tx,
            lifecycle_tx,
            bootstrap_ack_rx,
            hello_tx,
            hello_rx,
            stdout_tail,
            stderr_tail,
        })
    }
}
