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

pub(crate) fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    #[cfg(debug_assertions)]
    let future: std::pin::Pin<Box<dyn std::future::Future<Output = F::Output> + Send>> =
        Box::pin(future);
    tokio::spawn(future)
}
