use super::*;

pub(super) fn worker_has_live_resources(worker: &LiveWorker) -> bool {
    worker.stdin.is_some()
        || worker.lifecycle_tx.is_some()
        || worker.heartbeat_task.is_some()
        || worker.hello.is_some()
        || worker.grant.is_some()
}

pub(super) fn failed_worker() -> LiveWorker {
    LiveWorker {
        health: WorkerHealth {
            state: WorkerState::Failed,
            restart_count: 0,
        },
        generation: 0,
        started: Instant::now(),
        last_heartbeat: Instant::now(),
        grant: None,
        bootstrap_token_hash: None,
        #[cfg(any(windows, target_os = "macos"))]
        process_holder: Arc::new(AsyncMutex::new(None)),
        stdin: None,
        io_keys: [
            TaskKey::WorkerStdin {
                package: String::new(),
                version: String::new(),
                generation: 0,
            },
            TaskKey::WorkerStdout {
                package: String::new(),
                version: String::new(),
                generation: 0,
            },
            TaskKey::WorkerStderr {
                package: String::new(),
                version: String::new(),
                generation: 0,
            },
        ],
        lifecycle_tx: None,
        heartbeat_task: None,
        schedule_task: None,
        hello: None,
        bootstrap_complete: false,
        cleanup_started: false,
        broker: BrokerConfig {
            allowed_origins: Default::default(),
            filesystem_roots: vec![],
            private_state_roots: vec![],
            #[cfg(feature = "package-worker-fixture")]
            allow_local_test_origin: false,
        },
        stdout_tail: Arc::new(Mutex::new(BoundedTextTail::new(200, 64 * 1024))),
        stderr_tail: Arc::new(Mutex::new(BoundedTextTail::new(200, 64 * 1024))),
        bridge_status: None,
        in_flight: 0,
        restart_allowed: false,
        launch_spec: None,
        failure_streak: 0,
        lifecycle_reason: Some("failed".into()),
    }
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

pub(super) fn redact_worker_tail(line: &str) -> String {
    ["filesystem.read", "filesystem.write", "network"]
        .into_iter()
        .fold(redact_text(line), |line, scope| {
            line.replace(scope, "[REDACTED]")
        })
}
pub(super) fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}
pub(super) fn token_matches(expected: &[u8; 32], token: &str) -> bool {
    let actual = hash_token(token);
    expected
        .iter()
        .zip(actual)
        .fold(0u8, |out, (a, b)| out | (a ^ b))
        == 0
}

pub(super) fn result_line(
    id: &str,
    ok: bool,
    result: Option<serde_json::Value>,
    error: Option<&str>,
) -> Vec<u8> {
    let mut line = serde_json::to_vec(&ResultMessage {
        method: "worker.result".into(),
        id: id.into(),
        ok,
        result,
        error: error.map(str::to_owned),
    })
    .unwrap_or_default();
    line.push(b'\n');
    line
}

pub(super) fn send_result(
    stdin: Option<&mpsc::UnboundedSender<Vec<u8>>>,
    id: &str,
    ok: bool,
    result: Option<serde_json::Value>,
    error: &str,
) {
    if let Some(stdin) = stdin {
        let _ = stdin.send(result_line(id, ok, result, Some(error)));
    }
}

pub(super) async fn read_bounded_line<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<Option<Vec<u8>>, ()> {
    let mut line = Vec::new();
    loop {
        let mut byte = [0];
        match reader.read(&mut byte).await {
            Ok(0) => return (!line.is_empty()).then_some(line).ok_or(()).map(Some),
            Ok(_) => {}
            Err(_) => return Err(()),
        }
        if byte[0] == b'\n' {
            return Ok(Some(line));
        }
        if line.len() >= MAX_LINE_BYTES {
            return Err(());
        }
        line.push(byte[0]);
    }
}

pub(super) async fn drain_stderr<R: AsyncRead + Unpin + Send + 'static>(
    mut reader: BufReader<R>,
    generation: u64,
    tail: Arc<Mutex<BoundedTextTail>>,
    lifecycle_tx: mpsc::UnboundedSender<WorkerLifecycleEvent>,
) {
    while let Ok(Some(line)) = read_bounded_line(&mut reader).await {
        lock(&tail).push(&String::from_utf8_lossy(&line));
    }
    let _ = lifecycle_tx.send(WorkerLifecycleEvent::Finish {
        generation,
        state: WorkerState::Failed,
    });
}

pub(super) async fn read_stdout<R: AsyncRead + Unpin + Send + 'static>(
    inner: Arc<SupervisorInner>,
    key: (String, String),
    generation: u64,
    mut reader: BufReader<R>,
    tail: Arc<Mutex<BoundedTextTail>>,
    lifecycle_tx: mpsc::UnboundedSender<WorkerLifecycleEvent>,
) {
    loop {
        let line = match read_bounded_line(&mut reader).await {
            Ok(Some(line)) => line,
            _ => {
                let _ = lifecycle_tx.send(WorkerLifecycleEvent::Finish {
                    generation,
                    state: WorkerState::Failed,
                });
                return;
            }
        };
        let message = match crate::package_worker_protocol::parse_json_line(&line) {
            Ok(message) => message,
            Err(_) => {
                lock(&tail).push(&String::from_utf8_lossy(&line));
                let _ = lifecycle_tx.send(WorkerLifecycleEvent::Finish {
                    generation,
                    state: WorkerState::Failed,
                });
                return;
            }
        };
        match message {
            WorkerMessage::Hello(hello) => handle_hello(&inner, &key, generation, hello),
            WorkerMessage::Heartbeat(heartbeat) => handle_heartbeat(&inner, &key, heartbeat),
            WorkerMessage::Call(call) => {
                spawn_call(inner.clone(), key.clone(), call).await;
            }
            WorkerMessage::Result(result) => {
                let pending_key = (key.0.clone(), key.1.clone(), generation, result.id.clone());
                let Some(sender) = lock(&inner.invocations).remove(&pending_key) else {
                    continue;
                };
                let _ = sender.send(result);
            }
            // Worker diagnostics are advisory; keep the process alive and let
            // the correlated worker.result carry the operation failure.
            WorkerMessage::Event(_) | WorkerMessage::Error(_) => {}
            _ => {
                let _ = lifecycle_tx.send(WorkerLifecycleEvent::Finish {
                    generation,
                    state: WorkerState::Failed,
                });
                return;
            }
        }
    }
}

pub(super) fn handle_hello(
    inner: &Arc<SupervisorInner>,
    key: &(String, String),
    generation: u64,
    hello: HelloMessage,
) {
    let mut workers = lock(&inner.workers);
    let Some(worker) = workers.get_mut(key) else {
        return;
    };
    let valid = worker.generation == generation
        && worker.health.state == WorkerState::Starting
        && worker.bootstrap_complete
        && worker.grant.as_ref().is_some_and(|grant| {
            grant.package_id == hello.package_id
                && grant.version == hello.version
                && grant.hash == hello.hash
                && grant.pid == hello.pid
        })
        && worker
            .bootstrap_token_hash
            .as_ref()
            .is_some_and(|hash| token_matches(hash, &hello.token))
        && hello.api_version == inner.api_major;
    if !valid {
        if let Some(tx) = worker.lifecycle_tx.clone() {
            let _ = tx.send(WorkerLifecycleEvent::Finish {
                generation,
                state: WorkerState::Failed,
            });
        }
        return;
    }
    worker.bootstrap_token_hash = None;
    worker.health.state = WorkerState::Running;
    worker.lifecycle_reason = Some("hello".into());
    worker.last_heartbeat = Instant::now();
    if let Some(hello_tx) = worker.hello.take() {
        let _ = hello_tx.send(Ok(()));
    }
}

pub(super) fn handle_heartbeat(
    inner: &Arc<SupervisorInner>,
    key: &(String, String),
    heartbeat: HeartbeatMessage,
) {
    let mut workers = lock(&inner.workers);
    let Some(worker) = workers.get_mut(key) else {
        return;
    };
    let valid = worker.health.state == WorkerState::Running
        && worker.grant.as_ref().is_some_and(|grant| {
            grant.authenticate(
                &heartbeat.token,
                grant.pid,
                heartbeat.generation,
                inner.api_major,
                inner.api_major,
            )
        });
    if valid {
        worker.last_heartbeat = Instant::now();
        worker.bridge_status = heartbeat.bridge_status.map(sanitize_bridge_status);
    } else if let Some(tx) = worker.lifecycle_tx.clone() {
        let _ = tx.send(WorkerLifecycleEvent::Finish {
            generation: heartbeat.generation,
            state: WorkerState::Failed,
        });
    }
}

pub(super) fn sanitize_bridge_status(status: BridgeStatus) -> BridgeStatus {
    fn safe_time(value: Option<String>) -> Option<String> {
        value.filter(|value| {
            value.len() <= 64
                && !value
                    .chars()
                    .any(|ch| ch.is_control() || matches!(ch, '/' | '\\'))
        })
    }
    BridgeStatus {
        last_sync: safe_time(status.last_sync),
        conflict_count: status.conflict_count.min(1_000_000),
        last_conflict_at: safe_time(status.last_conflict_at),
    }
}
