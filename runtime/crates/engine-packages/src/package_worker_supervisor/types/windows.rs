use super::*;

pub(in crate::package_worker_supervisor) async fn cleanup_owner_async(
    owner: Arc<LaunchCleanupOwner>,
    deadline: Instant,
) -> Result<(), WorkerProcessError> {
    owner.cleanup_until(deadline).await
}

pub(in crate::package_worker_supervisor) async fn cleanup_process_holder_until(
    holder: WorkerProcessHolder,
    deadline: Instant,
) -> bool {
    let Some(mut process_guard) = lock_holder_until(&holder, deadline).await else {
        return false;
    };
    let Some(process) = process_guard.as_mut() else {
        return true;
    };
    if process.stop_until(deadline).await.is_ok() {
        *process_guard = None;
        true
    } else {
        false
    }
}

pub(in crate::package_worker_supervisor) async fn lock_holder_until<'a>(
    holder: &'a WorkerProcessHolder,
    deadline: Instant,
) -> Option<tokio::sync::MutexGuard<'a, Option<WorkerProcess>>> {
    if deadline <= Instant::now() {
        return None;
    }
    time::timeout(
        deadline.saturating_duration_since(Instant::now()),
        holder.lock(),
    )
    .await
    .ok()
}

pub(in crate::package_worker_supervisor) async fn holder_empty_until(
    holder: WorkerProcessHolder,
    deadline: Instant,
) -> bool {
    lock_holder_until(&holder, deadline)
        .await
        .is_some_and(|process| process.is_none())
}
