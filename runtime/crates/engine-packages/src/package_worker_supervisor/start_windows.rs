use super::*;

impl PackageWorkerSupervisor {
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) async fn start_windows(
        &self,
        key: (String, String),
        spec: LaunchSpec,
        generation: u64,
        restart_count: u32,
        failure_streak: u8,
        restart_allowed: bool,
    ) -> Result<(), &'static str> {
        let startup_key = TaskKey::Startup {
            package: key.0.clone(),
            version: key.1.clone(),
            generation,
        };
        let Some((start_rx, cancel_rx, owner, process_holder)) =
            self.inner.startups.reserve_startup(startup_key.clone())
        else {
            return Err("worker-unavailable");
        };
        let (result_tx, result_rx) = oneshot::channel();
        let supervisor = self.clone();
        let task = tokio::spawn(async move {
            let mut cancel_rx = cancel_rx;
            let started = tokio::select! {
                _ = &mut cancel_rx => false,
                result = start_rx => result.unwrap_or(false),
            };
            if !started {
                let _ = result_tx.send(Err("worker-unavailable"));
                return;
            }
            let result = supervisor
                .start_windows_transaction(
                    key,
                    spec,
                    generation,
                    restart_count,
                    failure_streak,
                    restart_allowed,
                    cancel_rx,
                    owner,
                    process_holder,
                )
                .await;
            let _ = result_tx.send(result);
        });
        if let Err(task) = self.inner.startups.install_pending(&startup_key, task) {
            task.abort();
            let _ = task.await;
            return Err("worker-unavailable");
        }
        self.inner.startups.open(&startup_key);
        let result = result_rx.await.unwrap_or(Err("worker-unavailable"));
        self.inner
            .startups
            .release_clean_startup(&startup_key)
            .await;
        result
    }
}
