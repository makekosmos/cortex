use super::*;

impl PackageWorkerSupervisor {
    pub async fn stop(&self, id: &str, version: &str) -> Result<(), &'static str> {
        self.stop_until(id, version, Instant::now() + STOP_DEADLINE)
            .await
    }

    pub(super) async fn stop_until(
        &self,
        id: &str,
        version: &str,
        deadline: Instant,
    ) -> Result<(), &'static str> {
        if deadline <= Instant::now() {
            return Err("cleanup-failed");
        }
        let key = (id.to_owned(), version.to_owned());
        self.inner.startups.reap_completed_until(deadline).await;
        if self
            .inner
            .startups
            .contains_quarantined_startup(id, version)
        {
            return Err("cleanup-failed");
        }
        let (generation, io_keys, stopped) = {
            let mut workers = lock(&self.inner.workers);
            let Some(worker) = workers.get_mut(&key) else {
                return Ok(());
            };
            let stopped = worker.health.state == WorkerState::Stopped;
            if !stopped {
                worker.health.state = WorkerState::Stopping;
                worker.lifecycle_reason = Some("stopping".into());
                worker.restart_allowed = false;
                worker.grant = None;
                worker.bootstrap_token_hash = None;
            }
            self.inner
                .network_responses
                .close_owner(&calls_dispatch::network_owner(
                    id,
                    version,
                    worker.generation,
                ));
            (worker.generation, worker.io_keys.clone(), stopped)
        };
        #[cfg(any(windows, target_os = "macos"))]
        if stopped
            && self
                .exact_generation_is_clean(&key, generation, deadline)
                .await
        {
            return Ok(());
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        if stopped {
            return Ok(());
        }
        if stopped {
            if let Some(worker) = lock(&self.inner.workers)
                .get_mut(&key)
                .filter(|worker| worker.generation == generation)
            {
                worker.health.state = WorkerState::Stopping;
                worker.lifecycle_reason = Some("stopping".into());
            }
        }
        self.revoke_typed_launch(id, version);
        let mut cleanup_ok = true;
        if !self
            .inner
            .startups
            .cancel_generation_until(id, version, generation, deadline)
            .await
        {
            cleanup_ok = false;
        }
        if !self
            .inner
            .calls
            .cancel_generation_until(id, version, generation, deadline)
            .await
        {
            cleanup_ok = false;
        }
        if !self
            .inner
            .retry_tasks
            .cancel_generation_until(id, version, generation, deadline)
            .await
        {
            cleanup_ok = false;
        }
        if let Some(stop_tx) = lock(&self.inner.workers)
            .get(&key)
            .and_then(|worker| worker.lifecycle_tx.clone())
        {
            if stop_tx
                .send(WorkerLifecycleEvent::Finish {
                    generation,
                    state: WorkerState::Stopped,
                })
                .is_err()
            {
                cleanup_ok = false;
            }
        }
        if !self
            .inner
            .worker_io
            .cancel_keys_until(&io_keys, deadline)
            .await
        {
            cleanup_ok = false;
        }
        let monitor_key = TaskKey::Lifecycle {
            package: id.to_owned(),
            version: version.to_owned(),
            generation,
        };
        #[cfg(any(windows, target_os = "macos"))]
        let monitor_present = self.inner.lifecycles.contains(&monitor_key);
        let monitor_joined = self
            .inner
            .lifecycles
            .join_key_until(&monitor_key, deadline)
            .await;
        if !monitor_joined {
            return Err("cleanup-failed");
        }
        #[cfg(any(windows, target_os = "macos"))]
        if !monitor_present
            && !self
                .exact_generation_is_clean(&key, generation, deadline)
                .await
        {
            finish_inner_until(
                &self.inner,
                &key,
                generation,
                WorkerState::Stopped,
                deadline,
            )
            .await;
        }
        #[cfg(any(windows, target_os = "macos"))]
        if !self
            .exact_generation_is_clean(&key, generation, deadline)
            .await
        {
            cleanup_ok = false;
        }
        if cleanup_ok {
            let mut workers = lock(&self.inner.workers);
            if let Some(worker) = workers
                .get_mut(&key)
                .filter(|worker| worker.generation == generation)
            {
                worker.health.state = WorkerState::Stopped;
                worker.lifecycle_reason = Some("stopped".into());
            }
            Ok(())
        } else {
            Err("cleanup-failed")
        }
    }

    pub async fn stop_all(&self) -> Result<(), &'static str> {
        self.stop_all_until(Instant::now() + STOP_DEADLINE).await
    }

    pub(super) async fn stop_all_until(&self, deadline: Instant) -> Result<(), &'static str> {
        if deadline <= Instant::now() {
            return Err("cleanup-failed");
        }
        self.inner.calls.close();
        self.inner.startups.close();
        self.inner.lifecycles.close();
        let keys: Vec<_> = lock(&self.inner.workers).keys().cloned().collect();
        let mut error = false;
        for (id, version) in keys {
            if self.stop_until(&id, &version, deadline).await.is_err() {
                error = true;
            }
        }
        let startup_ok = self.inner.startups.shutdown_until(deadline).await;
        let retry_ok = self.shutdown_retry_tasks_until(deadline).await;
        let worker_io_ok = self.inner.worker_io.shutdown_until(deadline).await;
        if !startup_ok
            || !self.inner.calls.shutdown_until(deadline).await
            || !self.inner.lifecycles.shutdown_until(deadline).await
            || !retry_ok
            || !worker_io_ok
        {
            error = true;
        }
        if error {
            Err("cleanup-failed")
        } else {
            Ok(())
        }
    }
}
