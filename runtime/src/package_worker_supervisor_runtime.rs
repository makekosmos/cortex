fn worker_has_live_resources(worker: &LiveWorker) -> bool {
    worker.stdin.is_some()
        || worker.lifecycle_tx.is_some()
        || worker.heartbeat_task.is_some()
        || worker.hello.is_some()
        || worker.grant.is_some()
}

fn failed_worker() -> LiveWorker {
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
        #[cfg(windows)]
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
        hello: None,
        bootstrap_complete: false,
        cleanup_started: false,
        broker: BrokerConfig {
            allowed_origins: Default::default(),
            filesystem_roots: vec![],
            private_state_roots: vec![],
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

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn redact_worker_tail(line: &str) -> String {
    ["filesystem.read", "filesystem.write", "network"]
        .into_iter()
        .fold(redact_text(line), |line, scope| {
            line.replace(scope, "[REDACTED]")
        })
}
fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}
fn token_matches(expected: &[u8; 32], token: &str) -> bool {
    let actual = hash_token(token);
    expected
        .iter()
        .zip(actual)
        .fold(0u8, |out, (a, b)| out | (a ^ b))
        == 0
}

async fn read_bounded_line<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>, ()> {
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

async fn drain_stderr<R: AsyncRead + Unpin + Send + 'static>(
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

async fn read_stdout<R: AsyncRead + Unpin + Send + 'static>(
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

fn handle_hello(
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

fn handle_heartbeat(
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

fn sanitize_bridge_status(status: BridgeStatus) -> BridgeStatus {
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

async fn spawn_call(inner: Arc<SupervisorInner>, key: (String, String), call: CallMessage) {
    let task_key = TaskKey::Call {
        package: key.0.clone(),
        version: key.1.clone(),
        generation: call.generation,
        id: call.id.clone(),
    };
    {
        let mut workers = lock(&inner.workers);
        let Some(worker) = workers.get_mut(&key) else {
            return;
        };
        if worker.health.state != WorkerState::Running
            || worker.generation != call.generation
            || worker.in_flight >= MAX_IN_FLIGHT
        {
            return;
        }
    }
    inner.calls.reap_completed().await;
    let Some((start_rx, cancel_rx)) = inner.calls.reserve(task_key.clone()) else {
        if let Some(worker) = lock(&inner.workers).get_mut(&key) {
            send_result(worker.stdin.as_ref(), &call.id, false, None, "unavailable");
        }
        return;
    };
    let task_inner = inner.clone();
    let task_key_for_task = task_key.clone();
    let task = tokio::spawn(async move {
        if !start_rx.await.unwrap_or(false) {
            return;
        }
        tokio::select! {
            _ = cancel_rx => {},
            _ = handle_call(task_inner, key, call) => {},
        }
    });
    if let Err(task) = inner.calls.install(&task_key_for_task, task) {
        task.abort();
        let _ = task.await;
    }
}

async fn handle_call(inner: Arc<SupervisorInner>, key: (String, String), call: CallMessage) {
    let (grant, broker, stdin) = {
        let mut workers = lock(&inner.workers);
        let Some(worker) = workers.get_mut(&key) else {
            return;
        };
        if worker.health.state != WorkerState::Running
            || worker.generation != call.generation
            || worker.in_flight >= MAX_IN_FLIGHT
        {
            send_result(worker.stdin.as_ref(), &call.id, false, None, "unavailable");
            return;
        }
        worker.in_flight += 1;
        (
            worker.grant.clone(),
            worker.broker.clone(),
            worker.stdin.clone(),
        )
    };
    let result = match grant {
        Some(grant) => time::timeout(
            PackageWorkerSupervisor::timeout(),
            dispatch(&inner, &grant, &broker, &call),
        )
        .await
        .unwrap_or_else(|_| Err("timeout")),
        None => Err("forbidden"),
    };
    if let Err(error) = result.as_ref() {
        tracing::warn!(
            target: "package_worker",
            package_id = %key.0,
            version = %key.1,
            operation = ?call.operation,
            error,
            "worker call failed"
        );
    }
    if let Some(stdin) = stdin {
        let mut response = match result {
            Ok(value) => result_line(&call.id, true, Some(value), None),
            Err(error) => result_line(&call.id, false, None, Some(error)),
        };
        if response.len() > MAX_LINE_BYTES {
            response = result_line(&call.id, false, None, Some("unavailable"));
        }
        let current = lock(&inner.workers).get(&key).is_some_and(|worker| {
            worker.health.state == WorkerState::Running && worker.generation == call.generation
        });
        if current {
            let _ = stdin.send(response);
        }
    }
    if let Some(worker) = lock(&inner.workers).get_mut(&key) {
        worker.in_flight = worker.in_flight.saturating_sub(1);
    }
}

async fn dispatch(
    inner: &SupervisorInner,
    grant: &Grant,
    broker: &BrokerConfig,
    call: &CallMessage,
) -> Result<serde_json::Value, &'static str> {
    let store = lock(&inner.store).clone().ok_or("unavailable")?;
    let installed = store
        .installed(&grant.package_id, &grant.version)
        .map_err(|_| "forbidden")?;
    if !installed.enabled || installed.revoked || !installed.hash.eq_ignore_ascii_case(&grant.hash)
    {
        return Err("forbidden");
    }
    if matches!(
        call.operation,
        WorkerMethod::ArkRead | WorkerMethod::ArkWrite
    ) {
        let typed_key = (grant.package_id.clone(), grant.version.clone());
        let typed_bound = lock(&inner.typed_launches).contains_key(&typed_key);
        let typed_envelope = call
            .params
            .get("request")
            .is_some_and(|value| value.get("kind").is_some());
        if typed_envelope {
            if !typed_bound {
                return Err("forbidden");
            }
            if !grant.authenticate(
                &call.token,
                grant.pid,
                call.generation,
                inner.api_major,
                inner.api_major,
            ) {
                return Err("forbidden");
            }
            return dispatch_typed_inner(
                inner,
                &grant.package_id,
                &grant.version,
                &grant.correlation_id,
                call.generation,
                call.params
                    .get("request")
                    .cloned()
                    .unwrap_or_else(|| call.params.clone()),
            )
            .await;
        }
    }
    let scope = match call.operation {
        WorkerMethod::ArkRead | WorkerMethod::ArkWrite => call
            .params
            .get("operation")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        WorkerMethod::NetworkFetch => call
            .params
            .get("url")
            .and_then(serde_json::Value::as_str)
            .and_then(|url| reqwest::Url::parse(url).ok())
            .map(|url| url.origin().ascii_serialization()),
        WorkerMethod::FilesystemRead
        | WorkerMethod::FilesystemWrite
        | WorkerMethod::FilesystemList
        | WorkerMethod::FilesystemPoll
        | WorkerMethod::FilesystemDelete => call
            .params
            .get("path")
            .and_then(serde_json::Value::as_str)
            .and_then(|path| granted_path_scope(grant, &call.operation, Path::new(path))),
    };
    if !grant.authorize(
        &call.token,
        grant.pid,
        call.generation,
        inner.api_major,
        inner.api_major,
        &call.operation,
        scope.as_deref(),
    ) {
        return Err("forbidden");
    }
    match call.operation {
        WorkerMethod::ArkRead | WorkerMethod::ArkWrite => {
            let operation = scope.ok_or("invalid-request")?;
            let params = call
                .params
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let ark = inner.ark.as_ref().ok_or("unavailable")?;
            let response = ark
                .request(&operation, params)
                .await
                .map_err(|_| "unavailable")?;
            if response.ok {
                Ok(response.data)
            } else {
                Err("unavailable")
            }
        }
        WorkerMethod::NetworkFetch => {
            let url = call
                .params
                .get("url")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let bytes = package_worker_broker::fetch(broker, url)
                .await
                .map_err(|_| "unavailable")?;
            if bytes.len() > 700 * 1024 {
                return Err("unavailable");
            }
            Ok(
                serde_json::json!({ "bytes": base64::engine::general_purpose::STANDARD.encode(bytes) }),
            )
        }
        WorkerMethod::FilesystemRead => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let bytes = package_worker_broker::read_file(broker, Path::new(path)).map_err(|error| {
                let class = match error {
                    package_worker_broker::BrokerError::Invalid(_) => "invalid",
                    package_worker_broker::BrokerError::Io(_) => "io",
                    package_worker_broker::BrokerError::Http(_) => "http",
                };
                tracing::warn!(target: "package_worker", error_class = class, "worker filesystem read failed");
                "unavailable"
            })?;
            if bytes.len() > 700 * 1024 {
                return Err("unavailable");
            }
            Ok(
                serde_json::json!({ "bytes": base64::engine::general_purpose::STANDARD.encode(bytes) }),
            )
        }
        WorkerMethod::FilesystemWrite => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let bytes = call
                .params
                .get("bytes")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| base64::engine::general_purpose::STANDARD.decode(value).ok())
                .ok_or("invalid-request")?;
            package_worker_broker::write_file(broker, Path::new(path), &bytes)
                .map_err(|_| "unavailable")?;
            Ok(serde_json::Value::Null)
        }
        WorkerMethod::FilesystemDelete => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            package_worker_broker::delete_file(broker, Path::new(path))
                .map_err(|_| "unavailable")?;
            Ok(serde_json::Value::Null)
        }
        WorkerMethod::FilesystemList => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let entries = package_worker_broker::list_directory(broker, Path::new(path))
                .map_err(|_| "unavailable")?;
            serde_json::to_value(entries).map_err(|_| "unavailable")
        }
        WorkerMethod::FilesystemPoll => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let entries = package_worker_broker::poll_metadata(broker, Path::new(path))
                .map_err(|_| "unavailable")?;
            serde_json::to_value(entries).map_err(|_| "unavailable")
        }
    }
}

fn granted_path_scope(grant: &Grant, method: &WorkerMethod, path: &Path) -> Option<String> {
    let capability = match method {
        WorkerMethod::FilesystemRead
        | WorkerMethod::FilesystemList
        | WorkerMethod::FilesystemPoll => "filesystem.read",
        WorkerMethod::FilesystemWrite | WorkerMethod::FilesystemDelete => "filesystem.write",
        _ => return None,
    };
    grant
        .scopes
        .get(capability)?
        .iter()
        .find(|root| path.starts_with(root))
        .cloned()
}
fn result_line(
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
fn send_result(
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

async fn heartbeat_watch(inner: Arc<SupervisorInner>, key: (String, String), generation: u64) {
    loop {
        time::sleep(Duration::from_secs(30)).await;
        let stale = {
            let mut workers = lock(&inner.workers);
            let Some(worker) = workers.get_mut(&key) else {
                return;
            };
            if worker.generation != generation {
                return;
            }
            if worker.health.state == WorkerState::Running
                && worker.started.elapsed() >= Duration::from_secs(60)
            {
                worker.failure_streak = 0;
            }
            worker.health.state == WorkerState::Running
                && worker.last_heartbeat.elapsed() > HEARTBEAT_DEADLINE
        };
        if stale {
            if let Some(tx) = lock(&inner.workers)
                .get(&key)
                .and_then(|worker| worker.lifecycle_tx.clone())
            {
                let _ = tx.send(WorkerLifecycleEvent::Finish {
                    generation,
                    state: WorkerState::Failed,
                });
            }
            return;
        }
        if !lock(&inner.workers).contains_key(&key) {
            return;
        }
    }
}

async fn join_task_until(mut task: tokio::task::JoinHandle<()>, deadline: Instant) -> bool {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if time::timeout(remaining, &mut task).await.is_err() {
        task.abort();
        let _ = task.await;
        return false;
    }
    true
}

async fn finish_inner_until(
    inner: &Arc<SupervisorInner>,
    key: &(String, String),
    generation: u64,
    state: WorkerState,
    deadline: Instant,
) {
    if deadline <= Instant::now() {
        return;
    }
    #[cfg(windows)]
    let retained_holder = {
        let workers = lock(&inner.workers);
        let Some(worker) = workers.get(key) else {
            return;
        };
        if worker.generation != generation {
            return;
        }
        worker
            .cleanup_started
            .then(|| worker.process_holder.clone())
    };
    #[cfg(windows)]
    let retained_cleanup_retry = retained_holder.is_some();
    #[cfg(not(windows))]
    let retained_cleanup_retry = false;
    #[cfg(windows)]
    if let Some(holder) = retained_holder {
        if !cleanup_process_holder_until(holder, deadline).await {
            if let Some(worker) = lock(&inner.workers)
                .get_mut(key)
                .filter(|worker| worker.generation == generation)
            {
                worker.health.state = WorkerState::Failed;
                worker.lifecycle_reason = Some("cleanup-failed".into());
                worker.restart_allowed = false;
            }
            return;
        }
        let mut workers = lock(&inner.workers);
        let Some(worker) = workers
            .get_mut(key)
            .filter(|worker| worker.generation == generation)
        else {
            return;
        };
        worker.cleanup_started = false;
    }
    let (hello, terminal_disable, retry, heartbeat_task, io_keys) = {
        let mut workers = lock(&inner.workers);
        let Some(worker) = workers.get_mut(key) else {
            return;
        };
        if worker.generation != generation || worker.cleanup_started {
            return;
        }
        worker.cleanup_started = true;
        if state == WorkerState::Failed
            && worker.health.state == WorkerState::Failed
            && !retained_cleanup_retry
        {
            return;
        }
        let retry = if state == WorkerState::Failed && worker.restart_allowed {
            worker.failure_streak = worker.failure_streak.saturating_add(1);
            (worker.failure_streak <= 3)
                .then(|| {
                    worker
                        .launch_spec
                        .clone()
                        .map(|spec| (spec, worker.generation, worker.failure_streak))
                })
                .flatten()
        } else {
            None
        };
        let terminal_disable =
            state == WorkerState::Failed && worker.restart_allowed && worker.failure_streak > 3;
        worker.health.state = state;
        worker.lifecycle_reason = Some(
            match state {
                WorkerState::Failed => "failed",
                WorkerState::Stopped => "stopped",
                WorkerState::Stopping => "stopping",
                WorkerState::Starting => "starting",
                WorkerState::Running => "running",
            }
            .into(),
        );
        worker.grant = None;
        worker.bootstrap_token_hash = None;
        worker.bootstrap_complete = false;
        worker.stdin = None;
        worker.lifecycle_tx = None;
        (
            worker.hello.take(),
            terminal_disable,
            retry,
            worker.heartbeat_task.take(),
            worker.io_keys.clone(),
        )
    };
    #[cfg(windows)]
    let process_holder = lock(&inner.workers)
        .get(key)
        .filter(|worker| worker.generation == generation)
        .map(|worker| worker.process_holder.clone());
    let mut cleanup_ok = true;
    let cancel_ok = inner
        .calls
        .cancel_generation_until(&key.0, &key.1, generation, deadline)
        .await;
    if !cancel_ok {
        cleanup_ok = false;
    }
    if let Some(hello) = hello {
        let _ = hello.send(Err("worker-unavailable"));
    }
    if !inner.worker_io.cancel_keys_until(&io_keys, deadline).await {
        cleanup_ok = false;
    }
    #[cfg(windows)]
    if let Some(holder) = process_holder {
        let Some(mut process) = lock_holder_until(&holder, deadline).await else {
            return;
        };
        if let Some(worker_process) = process.as_mut() {
            if worker_process.stop_until(deadline).await.is_err() {
                cleanup_ok = false;
            } else {
                *process = None;
            }
        }
    }
    if let Some(task) = heartbeat_task {
        task.abort();
        if !join_task_until(task, deadline).await {
            cleanup_ok = false;
        }
    }
    if !cleanup_ok {
        if let Some(worker) = lock(&inner.workers)
            .get_mut(key)
            .filter(|worker| worker.generation == generation)
        {
            worker.health.state = WorkerState::Failed;
            worker.restart_allowed = false;
            worker.lifecycle_reason = Some("cleanup-failed".into());
        }
        tracing::error!(target: "package_worker", package_id = %key.0, version = %key.1, generation, "worker cleanup failed");
        return;
    }
    let still_exact = lock(&inner.workers)
        .get(key)
        .is_some_and(|worker| worker.generation == generation && worker.cleanup_started);
    if !still_exact {
        return;
    }
    tracing::info!(target: "package_worker", package_id = %key.0, version = %key.1, generation, state = ?state, "worker lifecycle transition");
    if terminal_disable {
        let still_exact = lock(&inner.workers)
            .get(key)
            .is_some_and(|worker| worker.generation == generation);
        if still_exact {
            if let Some(store) = lock(&inner.store).clone() {
                let _ = store.disable(&key.0, &key.1);
            }
        }
    }
    if let Some((spec, generation, failures)) = retry {
        #[cfg(not(windows))]
        {
            // Package workers are intentionally unsupported on Unix; leave the
            // failed state recorded rather than attempting a Windows launch.
            let _ = (spec, generation, failures);
            return;
        }
        #[cfg(windows)]
        {
            if restart_delay(failures).is_some() {
                let retry_inner = inner.clone();
                let retry_key = key.clone();
                let old_lifecycle_key = TaskKey::Lifecycle {
                    package: key.0.clone(),
                    version: key.1.clone(),
                    generation,
                };
                inner.retry_tasks.reap_completed().await;
                let retry_registry_key = TaskKey::Retry {
                    package: key.0.clone(),
                    version: key.1.clone(),
                    generation,
                };
                let Some((retry_start, mut retry_cancel)) =
                    inner.retry_tasks.reserve(retry_registry_key.clone())
                else {
                    return;
                };
                let retry_task = tokio::spawn(async move {
                    let retry_started = tokio::select! {
                        _ = &mut retry_cancel => return,
                        started = retry_start => started.unwrap_or(false),
                    };
                    if !retry_started {
                        return;
                    }
                    let joined = tokio::select! {
                        _ = &mut retry_cancel => return,
                        joined = retry_inner.lifecycles.join_key(&old_lifecycle_key) => joined,
                    };
                    if !joined {
                        return;
                    }
                    let inner = retry_inner;
                    let key = retry_key;
                    let delay = restart_delay(failures).expect("retry delay");
                    let next_generation = generation.saturating_add(1);
                    tokio::select! {
                        _ = &mut retry_cancel => return,
                        _ = time::sleep(delay) => {}
                    }
                    let supervisor = PackageWorkerSupervisor {
                        inner: inner.clone(),
                    };
                    let valid = {
                        let workers = lock(&inner.workers);
                        workers.get(&key).is_some_and(|worker| {
                            worker.health.state == WorkerState::Failed
                                && worker.restart_allowed
                                && worker.generation == generation
                        })
                    };
                    if !valid {
                        tracing::info!(target: "package_worker", package_id = %key.0, version = %key.1, generation, "worker retry canceled");
                        return;
                    }
                    if let Some(store) = lock(&inner.store).clone() {
                        let Ok(installed) = store.installed(&key.0, &key.1) else {
                            tracing::warn!(target: "package_worker", package_id = %key.0, version = %key.1, "worker retry package missing");
                            return;
                        };
                        let Ok(entrypoint) = store.immutable_entrypoint(&installed) else {
                            tracing::warn!(target: "package_worker", package_id = %key.0, version = %key.1, "worker retry immutable entrypoint invalid");
                            return;
                        };
                        if !installed.enabled
                            || installed.revoked
                            || !installed.hash.eq_ignore_ascii_case(&spec.hash)
                            || entrypoint != spec.executable
                        {
                            tracing::warn!(target: "package_worker", package_id = %key.0, version = %key.1, "worker retry package state invalid");
                            return;
                        }
                    }
                    {
                        let mut workers = lock(&inner.workers);
                        let Some(worker) = workers.get_mut(&key) else {
                            return;
                        };
                        if worker.health.state != WorkerState::Failed
                            || !worker.restart_allowed
                            || worker.generation != generation
                        {
                            return;
                        }
                        worker.health.state = WorkerState::Starting;
                        worker.health.restart_count = failures as u32;
                        worker.generation = next_generation;
                        worker.lifecycle_reason = Some("restarting".into());
                    }
                    tracing::info!(target: "package_worker", package_id = %key.0, version = %key.1, generation = next_generation, "worker retry");
                    inner.startups.reap_completed().await;
                    let start_result = tokio::select! {
                        _ = &mut retry_cancel => return,
                        result = supervisor.start_windows_boxed(
                            key.clone(),
                            spec,
                            next_generation,
                            failures as u32,
                            failures,
                            true,
                        ) => result,
                    };
                    if start_result.is_err() {
                        let should_finish = {
                            let workers = lock(&inner.workers);
                            workers.get(&key).is_some_and(|worker| {
                                worker.generation == next_generation
                                    && worker.health.state == WorkerState::Starting
                            })
                        };
                        if should_finish {
                            if let Some(tx) = lock(&inner.workers)
                                .get(&key)
                                .and_then(|worker| worker.lifecycle_tx.clone())
                            {
                                let _ = tx.send(WorkerLifecycleEvent::Finish {
                                    generation: next_generation,
                                    state: WorkerState::Failed,
                                });
                            }
                        }
                    }
                });
                if let Err(task) = inner.retry_tasks.install(&retry_registry_key, retry_task) {
                    let _ = task.await;
                }
            }
        }
    }
}
