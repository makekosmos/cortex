use super::*;

struct PendingInvocation {
    inner: Arc<SupervisorInner>,
    key: (String, String, u64, String),
}

impl Drop for PendingInvocation {
    fn drop(&mut self) {
        lock(&self.inner.invocations).remove(&self.key);
    }
}

impl PackageWorkerSupervisor {
    pub fn validate_manifest(manifest: &PackageManifest) -> Result<(), &'static str> {
        if !matches!(manifest.kind, PackageKind::Source | PackageKind::Bridge)
            || !manifest.entrypoint.to_ascii_lowercase().ends_with(".exe")
        {
            return Err("worker-required");
        }
        Ok(())
    }

    pub fn health(&self, id: &str, version: &str) -> WorkerHealth {
        self.inner
            .workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(id.into(), version.into()))
            .map(|w| w.health.clone())
            .unwrap_or(WorkerHealth {
                state: WorkerState::Stopped,
                restart_count: 0,
            })
    }

    pub fn diagnostics(&self) -> Vec<WorkerDiagnostics> {
        let mut result: Vec<_> = lock(&self.inner.workers)
            .iter()
            .map(|((id, version), worker)| WorkerDiagnostics {
                id: id.clone(),
                version: version.clone(),
                hash: worker.launch_spec.as_ref().map(|spec| spec.hash.clone()),
                correlation_id: worker
                    .launch_spec
                    .as_ref()
                    .map(|spec| spec.correlation_id.clone()),
                state: worker.health.state,
                restart_count: worker.health.restart_count,
                generation: worker.generation,
                lifecycle_reason: worker.lifecycle_reason.clone(),
                stdout_tail: lock(&worker.stdout_tail)
                    .snapshot()
                    .into_iter()
                    .map(|line| redact_worker_tail(&line))
                    .collect(),
                stderr_tail: lock(&worker.stderr_tail)
                    .snapshot()
                    .into_iter()
                    .map(|line| redact_worker_tail(&line))
                    .collect(),
                bridge_status: worker.bridge_status.clone(),
            })
            .collect();
        result.sort_by(|a, b| (&a.id, &a.version).cmp(&(&b.id, &b.version)));
        result.truncate(1024);
        result
    }

    pub fn bind_store(&self, store: Arc<PackageStore>) {
        *lock(&self.inner.store) = Some(store);
    }

    pub fn activate(&self, id: &str, version: &str) -> bool {
        let mut workers = lock(&self.inner.workers);
        let Some(worker) = workers.get_mut(&(id.into(), version.into())) else {
            return false;
        };
        if worker.health.state != WorkerState::Running {
            return false;
        }
        worker.restart_allowed = true;
        if worker.schedule_task.is_none() {
            if let Some((seconds, stdin)) = worker
                .launch_spec
                .as_ref()
                .and_then(|spec| spec.integration.as_ref())
                .and_then(|config| config.manifest.schedule.as_ref())
                .and_then(|schedule| {
                    worker
                        .stdin
                        .clone()
                        .map(|stdin| (schedule.interval_seconds, stdin))
                })
            {
                let generation = worker.generation;
                worker.schedule_task = Some(tokio::spawn(async move {
                    let mut interval = time::interval(Duration::from_secs(seconds));
                    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
                    interval.tick().await;
                    loop {
                        interval.tick().await;
                        let Some(line) = run_line(generation) else {
                            break;
                        };
                        if stdin.send(line).is_err() {
                            break;
                        }
                    }
                }));
            }
        }
        worker.lifecycle_reason = Some("activated".into());
        tracing::info!(
            target: "package_worker",
            package_id = %id,
            version = %version,
            generation = worker.generation,
            "worker activated",
        );
        true
    }

    pub fn run_now(&self, id: &str, version: &str) -> Result<(), &'static str> {
        let workers = lock(&self.inner.workers);
        let worker = workers
            .get(&(id.to_owned(), version.to_owned()))
            .filter(|worker| worker.health.state == WorkerState::Running)
            .ok_or("unavailable")?;
        let stdin = worker.stdin.as_ref().ok_or("unavailable")?;
        stdin
            .send(run_line(worker.generation).ok_or("unavailable")?)
            .map_err(|_| "unavailable")
    }

    pub async fn invoke(
        &self,
        id: &str,
        version: &str,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, &'static str> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (generation, stdin) = {
            let workers = lock(&self.inner.workers);
            let worker = workers
                .get(&(id.to_owned(), version.to_owned()))
                .filter(|worker| worker.health.state == WorkerState::Running)
                .ok_or("unavailable")?;
            (
                worker.generation,
                worker.stdin.clone().ok_or("unavailable")?,
            )
        };
        let message = InvokeMessage {
            method: "worker.invoke".into(),
            id: request_id.clone(),
            generation,
            operation: operation.to_owned(),
            params,
        };
        let mut line = serde_json::to_vec(&message).map_err(|_| "invalid-request")?;
        if line.len() >= MAX_LINE_BYTES
            || crate::package_worker_protocol::parse_json_line(&line).is_err()
        {
            return Err("invalid-request");
        }
        line.push(b'\n');
        let pending_key = (id.to_owned(), version.to_owned(), generation, request_id);
        let (sender, receiver) = oneshot::channel();
        {
            let mut invocations = lock(&self.inner.invocations);
            let active = invocations
                .keys()
                .filter(|(package, worker_version, worker_generation, _)| {
                    package == id && worker_version == version && *worker_generation == generation
                })
                .count();
            if active >= MAX_IN_FLIGHT as usize {
                return Err("unavailable");
            }
            invocations.insert(pending_key.clone(), sender);
        }
        let _pending = PendingInvocation {
            inner: self.inner.clone(),
            key: pending_key.clone(),
        };
        if stdin.send(line).is_err() {
            return Err("unavailable");
        }
        let response = time::timeout(Self::timeout(), receiver)
            .await
            .map_err(|_| "timeout")?
            .map_err(|_| "unavailable");
        let response = response?;
        if response.ok {
            response.result.ok_or("invalid-response")
        } else {
            let error_class = worker_error_class(response.error.as_deref());
            tracing::warn!(
                target: "package_worker",
                package_id = %id,
                version,
                operation,
                error = error_class,
                "worker invocation rejected"
            );
            Err(error_class)
        }
    }

    pub async fn start(
        &self,
        manifest: &PackageManifest,
        executable: PathBuf,
        state_root: PathBuf,
        hash: String,
        roots: &[PathBuf],
        correlation_id: String,
        bridge_config: Option<BridgeWorkerConfig>,
        integration: Option<IntegrationLaunchConfig>,
    ) -> Result<(), &'static str> {
        Self::validate_manifest(manifest)?;
        #[cfg(any(test, feature = "test-support"))]
        if self.inner.fail_next_start.swap(false, Ordering::AcqRel) {
            return Err("unavailable");
        }
        self.inner.calls.reap_completed().await;
        self.inner.startups.reap_completed().await;
        self.inner.lifecycles.reap_completed().await;
        let key = (manifest.id.clone(), manifest.version.clone());
        if self
            .inner
            .startups
            .contains_quarantined_startup(&key.0, &key.1)
        {
            return Err("cleanup-failed");
        }
        let existing_state = lock(&self.inner.workers)
            .get(&key)
            .map(|worker| worker.health.state);
        if existing_state.is_some_and(|state| {
            matches!(
                state,
                WorkerState::Starting | WorkerState::Running | WorkerState::Stopping
            )
        }) {
            return Err("already-running");
        }
        if existing_state == Some(WorkerState::Failed)
            && lock(&self.inner.workers)
                .get(&key)
                .is_some_and(|worker| worker.cleanup_started)
        {
            return Err("already-running");
        }
        if existing_state.is_some() {
            self.stop(&key.0, &key.1).await?;
            lock(&self.inner.workers).remove(&key);
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = (
                executable,
                state_root,
                hash,
                roots,
                correlation_id,
                bridge_config,
                integration,
            );
            self.insert_failed(key);
            return Err("unsupported-platform");
        }
        #[cfg(any(windows, target_os = "macos"))]
        {
            self.start_initial_windows(
                key,
                LaunchSpec {
                    manifest: manifest.clone(),
                    executable,
                    state_root,
                    hash,
                    roots: roots.to_vec(),
                    correlation_id,
                    bridge_config,
                    integration,
                },
            )
            .await
        }
    }
}

fn run_line(generation: u64) -> Option<Vec<u8>> {
    let mut line = serde_json::to_vec(&RunMessage {
        method: "worker.run".into(),
        generation,
        run_id: uuid::Uuid::new_v4().to_string(),
    })
    .ok()?;
    line.push(b'\n');
    Some(line)
}
