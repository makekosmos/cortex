use super::*;

impl PackageWorkerSupervisor {
    pub(super) async fn shutdown_retry_tasks_until(&self, deadline: Instant) -> bool {
        self.inner.retry_tasks.shutdown_until(deadline).await
    }
}
