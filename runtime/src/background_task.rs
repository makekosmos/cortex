//! Shared teardown for spawned tasks that hold a service `Arc`.
//!
//! A task's acknowledgement or last observable effect always precedes its
//! teardown: a spawned task holding `Arc<Service>` releases it — and every
//! file handle that Arc keeps open — only when the task returns. Joining the
//! `JoinHandle` is the only point where the task is provably gone; dropping
//! the handle silently detaches it, which is the exact leak class the tmp
//! gate (KOS-270/KOS-314) hunts.

use std::sync::Mutex;
use std::time::Duration;
use tokio::task::JoinHandle;

/// `(label, task)` registry — the label only feeds the wedged-task warning.
pub(crate) type TaskRegistry = Mutex<Vec<(String, JoinHandle<()>)>>;

/// Join a spawned task; on `timeout` abort it and await the abort so nothing
/// is ever left detached. `abort()` is bounded and `kill_on_drop` children
/// are reaped by the abort's teardown.
pub(crate) async fn join_task(label: &str, mut task: JoinHandle<()>, timeout: Duration) {
    if tokio::time::timeout(timeout, &mut task).await.is_err() {
        tracing::warn!(
            task = label,
            "background task did not stop in time; aborting"
        );
        task.abort();
        let _ = task.await;
    }
}

/// Register a spawned task. Finished handles are reaped on push so the
/// registry stays bounded by the number of *live* tasks, not total spawns.
pub(crate) fn track_task(tasks: &TaskRegistry, label: impl Into<String>, task: JoinHandle<()>) {
    let mut tasks = tasks.lock().unwrap_or_else(|p| p.into_inner());
    tasks.retain(|(_, task)| !task.is_finished());
    tasks.push((label.into(), task));
}

/// Join every tracked task, in reverse spawn order.
pub(crate) async fn drain_tasks(tasks: &TaskRegistry, timeout: Duration) {
    loop {
        let next = tasks.lock().unwrap_or_else(|p| p.into_inner()).pop();
        let Some((label, task)) = next else { return };
        join_task(&label, task, timeout).await;
    }
}
