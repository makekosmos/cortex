use super::*;

impl PackageWorkerSupervisor {
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) async fn cleanup_published_generation(
        &self,
        key: &(String, String),
        generation: u64,
        lifecycle_tx: Option<mpsc::UnboundedSender<WorkerLifecycleEvent>>,
        original: &'static str,
        _deadline: Instant,
    ) -> Result<(), &'static str> {
        let deadline = Instant::now() + STOP_DEADLINE;
        let lifecycle_key = TaskKey::Lifecycle {
            package: key.0.clone(),
            version: key.1.clone(),
            generation,
        };
        let signaled = lifecycle_tx.is_some_and(|tx| {
            tx.send(WorkerLifecycleEvent::Finish {
                generation,
                state: WorkerState::Failed,
            })
            .is_ok()
        });
        let joined = if self.inner.lifecycles.contains(&lifecycle_key) {
            self.inner.lifecycles.join_key(&lifecycle_key).await
        } else {
            true
        };
        if !signaled
            || !joined
            || self
                .exact_generation_has_live_resources(key, generation, deadline)
                .await
        {
            finish_inner_until(&self.inner, key, generation, WorkerState::Failed, deadline).await;
        }
        if !joined
            || !self
                .exact_generation_is_clean(key, generation, deadline)
                .await
        {
            return Err("cleanup-failed");
        }
        Err(original)
    }

    #[cfg(any(windows, target_os = "macos"))]
    pub(super) async fn exact_generation_has_live_resources(
        &self,
        key: &(String, String),
        generation: u64,
        deadline: Instant,
    ) -> bool {
        let holder = {
            let workers = lock(&self.inner.workers);
            let Some(worker) = workers
                .get(key)
                .filter(|worker| worker.generation == generation)
            else {
                return false;
            };
            if worker_has_live_resources(worker) {
                return true;
            }
            worker.process_holder.clone()
        };
        !holder_empty_until(holder, deadline).await
    }

    #[cfg(any(windows, target_os = "macos"))]
    pub(super) async fn exact_generation_is_clean(
        &self,
        key: &(String, String),
        generation: u64,
        deadline: Instant,
    ) -> bool {
        let worker_clean = lock(&self.inner.workers)
            .get(key)
            .filter(|worker| worker.generation == generation)
            .is_none_or(|worker| !worker_has_live_resources(worker));
        let io_clean = !self
            .inner
            .worker_io
            .has_generation(&key.0, &key.1, generation);
        let worker_holder = {
            let workers = lock(&self.inner.workers);
            workers
                .get(key)
                .filter(|worker| worker.generation == generation)
                .map(|worker| worker.process_holder.clone())
        };
        let holder_clean = match worker_holder {
            Some(holder) => holder_empty_until(holder, deadline).await,
            None => true,
        };
        worker_clean && io_clean && holder_clean
    }
}
