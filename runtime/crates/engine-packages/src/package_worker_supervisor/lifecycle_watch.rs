use super::*;

pub(super) async fn heartbeat_watch(
    inner: Arc<SupervisorInner>,
    key: (String, String),
    generation: u64,
) {
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

pub(super) async fn join_task_until(
    mut task: tokio::task::JoinHandle<()>,
    deadline: Instant,
) -> bool {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if time::timeout(remaining, &mut task).await.is_err() {
        task.abort();
        let _ = task.await;
        return false;
    }
    true
}
