use super::*;

pub(super) async fn finish_inner_until(
    inner: &Arc<SupervisorInner>,
    key: &(String, String),
    generation: u64,
    state: WorkerState,
    deadline: Instant,
) {
    inner.secrets.revoke_generation(&key.0, generation);
    let session_id = lock(&inner.workers)
        .get(key)
        .filter(|worker| worker.generation == generation)
        .and_then(|worker| worker.launch_spec.as_ref())
        .map(|spec| format!("{}:{}", spec.correlation_id, key.0));
    if let (Some(grants), Some(session_id)) = (lock(&inner.grants).clone(), session_id) {
        grants.close_generation(&session_id, generation);
    }
    lock(&inner.invocations).retain(|(id, version, pending_generation, _), _| {
        id != &key.0 || version != &key.1 || *pending_generation != generation
    });
    if deadline <= Instant::now() {
        return;
    }
    #[cfg(any(windows, target_os = "macos"))]
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
    #[cfg(any(windows, target_os = "macos"))]
    let retained_cleanup_retry = retained_holder.is_some();
    #[cfg(not(any(windows, target_os = "macos")))]
    let retained_cleanup_retry = false;
    #[cfg(any(windows, target_os = "macos"))]
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
    let (hello, terminal_disable, retry, heartbeat_task, schedule_task, io_keys) = {
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
            worker.schedule_task.take(),
            worker.io_keys.clone(),
        )
    };
    #[cfg(any(windows, target_os = "macos"))]
    let process_holder = lock(&inner.workers)
        .get(key)
        .filter(|worker| worker.generation == generation)
        .map(|worker| worker.process_holder.clone());
    let mut cleanup_ok = true;
    let cancel_ok = inner
        .calls
        .cancel_generation_until(&key.0, &key.1, generation, deadline)
        .await;
    inner
        .network_responses
        .close_owner(&calls_dispatch::network_owner(&key.0, &key.1, generation));
    if !cancel_ok {
        cleanup_ok = false;
    }
    if let Some(hello) = hello {
        let _ = hello.send(Err("worker-unavailable"));
    }
    if !inner.worker_io.cancel_keys_until(&io_keys, deadline).await {
        cleanup_ok = false;
    }
    #[cfg(any(windows, target_os = "macos"))]
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
    if let Some(task) = schedule_task {
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
        tracing::error!(
            target: "package_worker",
            package_id = %key.0,
            version = %key.1,
            generation,
            "worker cleanup failed",
        );
        return;
    }
    let still_exact = lock(&inner.workers)
        .get(key)
        .is_some_and(|worker| worker.generation == generation && worker.cleanup_started);
    if !still_exact {
        return;
    }
    tracing::info!(
        target: "package_worker",
        package_id = %key.0,
        version = %key.1,
        generation,
        state = ?state,
        "worker lifecycle transition",
    );
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
        schedule_retry(inner, key, spec, generation, failures).await;
    }
}
