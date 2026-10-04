use super::*;

impl PackageWorkerSupervisor {
    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub fn test_registry_counts(&self) -> (usize, usize) {
        (self.inner.calls.len(), self.inner.lifecycles.len())
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub async fn test_start_once(
        &self,
        manifest: &PackageManifest,
        executable: PathBuf,
        state_root: PathBuf,
        hash: String,
        roots: &[PathBuf],
        correlation_id: String,
    ) -> Result<(), &'static str> {
        let key = (manifest.id.clone(), manifest.version.clone());
        let startup_key = TaskKey::Startup {
            package: key.0.clone(),
            version: key.1.clone(),
            generation: 1,
        };
        if self.inner.startups.is_finished(&startup_key)
            || self
                .inner
                .startups
                .contains_quarantined_startup(&key.0, &key.1)
        {
            return Err("already-running");
        }
        self.insert_failed(key.clone());
        self.start_windows(
            key,
            LaunchSpec {
                manifest: manifest.clone(),
                state_root,
                executable,
                hash,
                roots: roots.to_vec(),
                correlation_id,
                bridge_config: None,
                integration: None,
            },
            1,
            0,
            0,
            false,
        )
        .await
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub fn test_registry_snapshot(&self) -> (usize, usize, usize, usize) {
        (
            self.inner.calls.len(),
            self.inner.startups.len(),
            self.inner.lifecycles.len(),
            self.inner.worker_io.len(),
        )
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub fn test_startup_reservation_snapshot(
        &self,
        id: &str,
        version: &str,
        generation: u64,
    ) -> Option<(bool, bool)> {
        self.inner
            .startups
            .startup_reservation_snapshot(&TaskKey::Startup {
                package: id.to_owned(),
                version: version.to_owned(),
                generation,
            })
    }
    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub async fn test_stop_all_until(&self, deadline: Instant) -> Result<(), &'static str> {
        self.stop_all_until(deadline).await
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub async fn test_reap_worker_startup(&self, id: &str, version: &str) {
        let deadline = Instant::now() + STOP_DEADLINE;
        let _ = self
            .inner
            .startups
            .cancel_generation_until(id, version, 1, deadline)
            .await;
        let _ = self
            .inner
            .lifecycles
            .cancel_generation_until(id, version, 1, deadline)
            .await;
        let _ = self
            .inner
            .worker_io
            .cancel_generation_until(id, version, 1, deadline)
            .await;
        self.inner.startups.reap_completed().await;
        self.inner.worker_io.reap_completed().await;
        self.inner.lifecycles.reap_completed().await;
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub async fn test_worker_snapshot(
        &self,
        id: &str,
        version: &str,
    ) -> (WorkerState, Option<String>, bool, bool) {
        let (state, reason, restart_allowed, holder) = {
            let workers = lock(&self.inner.workers);
            let Some(worker) = workers.get(&(id.to_owned(), version.to_owned())) else {
                return (WorkerState::Stopped, None, false, false);
            };
            (
                worker.health.state,
                worker.lifecycle_reason.clone(),
                worker.restart_allowed,
                worker.process_holder.clone(),
            )
        };
        let holder_nonempty = lock_holder_until(&holder, Instant::now() + STOP_DEADLINE)
            .await
            .is_some_and(|process| process.is_some());
        (state, reason, restart_allowed, holder_nonempty)
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub fn test_pause_after_launch(&self) -> AfterLaunchGate {
        let (ready_tx, ready_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        *NEXT_AFTER_LAUNCH_GATE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(AfterLaunchGateParts {
            ready: ready_tx,
            release: release_rx,
        });
        AfterLaunchGate {
            ready: ready_rx,
            release: Some(release_tx),
        }
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub fn test_hold_process_holder(&self, id: &str, version: &str) -> Option<HolderLockGate> {
        let holder = lock(&self.inner.workers)
            .get(&(id.to_owned(), version.to_owned()))
            .map(|worker| worker.process_holder.clone())?;
        let (ready_tx, ready_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        tokio::spawn(async move {
            let _guard = holder.lock().await;
            let _ = ready_tx.send(());
            let _ = release_rx.await;
        });
        Some(HolderLockGate {
            ready: ready_rx,
            release: Some(release_tx),
        })
    }
}
