pub(crate) fn track_task(
    tasks: &std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
    task: tokio::task::JoinHandle<()>,
) {
    let mut tasks = tasks.lock().unwrap_or_else(|e| e.into_inner());
    tasks.retain(|task| !task.is_finished());
    tasks.push(task);
}

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
