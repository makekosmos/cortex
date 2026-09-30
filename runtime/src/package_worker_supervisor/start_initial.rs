use super::*;

impl PackageWorkerSupervisor {
    #[cfg(windows)]
    pub(super) async fn start_initial_windows(
        &self,
        key: (String, String),
        spec: LaunchSpec,
    ) -> Result<(), &'static str> {
        // Initial failures are retried synchronously so PackageService only
        // commits enablement after a worker has actually completed hello.
        self.insert_failed(key.clone());
        for attempt in 0..=3u8 {
            if lock(&self.inner.workers).get(&key).is_some_and(|worker| {
                matches!(
                    worker.health.state,
                    WorkerState::Stopping | WorkerState::Stopped
                )
            }) {
                return Err("worker-unavailable");
            }
            if let Some(delay) = (attempt > 0)
                .then(|| restart_delay(&self.inner.restart_delays, attempt))
                .flatten()
            {
                time::sleep(delay).await;
            }
            self.inner.startups.reap_completed().await;
            let generation = attempt as u64 + 1;
            if let Some(worker) = lock(&self.inner.workers).get_mut(&key) {
                if matches!(
                    worker.health.state,
                    WorkerState::Failed | WorkerState::Starting
                ) {
                    worker.generation = generation;
                }
            }
            let result = self
                .start_windows(
                    key.clone(),
                    spec.clone(),
                    generation,
                    attempt as u32,
                    attempt,
                    false,
                )
                .await;
            if result.is_ok() {
                return Ok(());
            }
            if let Err(reason) = result {
                if let Some(worker) = lock(&self.inner.workers).get_mut(&key) {
                    worker.lifecycle_reason = Some(reason.into());
                }
            }
        }
        if let Some(store) = lock(&self.inner.store).clone() {
            let _ = store.disable(&key.0, &key.1);
        }
        Err("worker-unavailable")
    }
}
