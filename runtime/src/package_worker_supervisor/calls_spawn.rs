use super::*;

pub(super) async fn spawn_call(
    inner: Arc<SupervisorInner>,
    key: (String, String),
    call: CallMessage,
) {
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
