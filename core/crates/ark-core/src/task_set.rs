//! Task lists shared by the sync client and server. A component records the
//! background tasks it spawns here, and its stop path joins or aborts them, so
//! none keeps the shared db connection alive past teardown (KOS-270, KOS-369).

/// Records a task for a later join or abort. Handles of tasks that already
/// ended are dropped first, so a long-lived component does not accumulate them.
pub(crate) fn track_task(
    tasks: &std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
    task: tokio::task::JoinHandle<()>,
) {
    let mut tasks = tasks.lock().unwrap_or_else(|e| e.into_inner());
    tasks.retain(|task| !task.is_finished());
    tasks.push(task);
}

/// Waits for every recorded task to end on its own, repeating until none is
/// left. Used for short tasks that finish by themselves.
pub(crate) async fn join_all(tasks: &std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>) {
    loop {
        let drained: Vec<tokio::task::JoinHandle<()>> = tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        if drained.is_empty() {
            break;
        }
        for task in drained {
            let _ = task.await;
        }
    }
}

/// Aborts every recorded task and waits for each to end. Repeats until the
/// list stays empty, because a task aborted late may have recorded another.
pub(crate) async fn abort_and_join(tasks: &std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>) {
    loop {
        let drained: Vec<tokio::task::JoinHandle<()>> = tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        if drained.is_empty() {
            break;
        }
        for task in &drained {
            task.abort();
        }
        for task in drained {
            let _ = task.await;
        }
    }
}
