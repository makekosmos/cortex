use super::*;

pub(super) struct PreparedLaunch {
    pub(super) deadline: Instant,
    pub(super) manifest: PackageManifest,
    pub(super) hash: String,
    pub(super) roots: Vec<PathBuf>,
    pub(super) state_root: PathBuf,
    pub(super) correlation_id: String,
    pub(super) bridge_config: Option<BridgeWorkerConfig>,
    pub(super) integration: Option<IntegrationLaunchConfig>,
    pub(super) lifecycle_task_key: TaskKey,
    pub(super) worker_io_keys: Vec<TaskKey>,
    pub(super) launch_transaction: LaunchTransaction,
    pub(super) io_gates: Vec<(oneshot::Receiver<bool>, oneshot::Receiver<()>)>,
    pub(super) process_holder: WorkerProcessHolder,
    pub(super) pid: u32,
}

impl PackageWorkerSupervisor {
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) async fn prepare_launch(
        &self,
        key: &(String, String),
        spec: LaunchSpec,
        generation: u64,
        restart_count: u32,
        cancel_rx: &mut oneshot::Receiver<()>,
        owner: Arc<LaunchCleanupOwner>,
        process_holder: WorkerProcessHolder,
    ) -> Result<PreparedLaunch, &'static str> {
        let deadline = Instant::now() + PROCESS_LAUNCH_DEADLINE;
        tracing::info!(
            target: "package_worker",
            package_id = %key.0,
            version = %key.1,
            generation,
            restart_count,
            "worker start",
        );
        let LaunchSpec {
            manifest,
            executable,
            state_root,
            hash,
            roots,
            correlation_id,
            bridge_config,
            integration,
        } = spec.clone();
        crate::lock_file::ensure_owner_only_directory(&state_root).map_err(|_| "grant-failed")?;
        if let Some(config) = bridge_config.as_ref() {
            crate::lock_file::ensure_owner_only_directory(std::path::Path::new(&config.state_root))
                .map_err(|_| "grant-failed")?;
        }
        let lifecycle_task_key = TaskKey::Lifecycle {
            package: key.0.clone(),
            version: key.1.clone(),
            generation,
        };
        let worker_io_keys = vec![
            TaskKey::WorkerStdin {
                package: key.0.clone(),
                version: key.1.clone(),
                generation,
            },
            TaskKey::WorkerStdout {
                package: key.0.clone(),
                version: key.1.clone(),
                generation,
            },
            TaskKey::WorkerStderr {
                package: key.0.clone(),
                version: key.1.clone(),
                generation,
            },
        ];
        let launch_transaction = LaunchTransaction {
            inner: self.inner.clone(),
            lifecycle_key: lifecycle_task_key.clone(),
            worker_io_keys: worker_io_keys.clone(),
            process_holder: process_holder.clone(),
        };
        self.inner.worker_io.reap_completed().await;
        if cancellation_pending(cancel_rx).await {
            return match launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await
            {
                Ok(()) => Err("worker-unavailable"),
                Err(error) => Err(error),
            };
        }
        let mut io_gates = Vec::with_capacity(worker_io_keys.len());
        for io_key in &worker_io_keys {
            let Some(gate) = self.inner.worker_io.reserve(io_key.clone()) else {
                return match launch_transaction
                    .rollback_error("worker-unavailable", deadline)
                    .await
                {
                    Ok(()) => Err("worker-unavailable"),
                    Err(error) => Err(error),
                };
            };
            io_gates.push(gate);
        }
        let Some(mut process_guard) = lock_holder_until(&process_holder, deadline).await else {
            return match launch_transaction
                .rollback_error("initial-process-holder-timeout", deadline)
                .await
            {
                Ok(()) => Err("initial-process-holder-timeout"),
                Err(error) => Err(error),
            };
        };
        // The guard is acquired before CreateProcessW and held through the
        // server-owned launch. Publication is synchronous while it is held.
        match WorkerProcess::launch_with_owner_until_in_state_root(
            executable,
            owner.clone(),
            state_root.clone(),
            deadline,
        )
        .await
        {
            Ok(process) => *process_guard = Some(process),
            Err(error) => {
                drop(process_guard);
                let reason = match error {
                    WorkerProcessError::InvalidExecutable => "invalid-executable",
                    WorkerProcessError::UnsupportedPlatform => "unsupported-platform",
                    WorkerProcessError::Setup => "process-setup-failed",
                    WorkerProcessError::Cleanup => "process-cleanup-failed",
                };
                return match launch_transaction.rollback_error(reason, deadline).await {
                    Ok(()) => Err(reason),
                    Err(error) => Err(error),
                };
            }
        }
        #[cfg(all(windows, feature = "package-worker-fixture"))]
        let after_launch_gate = NEXT_AFTER_LAUNCH_GATE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        #[cfg(all(windows, feature = "package-worker-fixture"))]
        if let Some(gate) = after_launch_gate {
            let _ = gate.ready.send(());
            let _ = gate.release.await;
        }
        let pid = match process_guard.as_ref().and_then(WorkerProcess::id) {
            Some(pid) => pid,
            None => {
                drop(process_guard);
                return match launch_transaction
                    .rollback_error("pid-unavailable", deadline)
                    .await
                {
                    Ok(()) => Err("pid-unavailable"),
                    Err(error) => Err(error),
                };
            }
        };
        drop(process_guard);
        if cancellation_pending(cancel_rx).await {
            return match launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await
            {
                Ok(()) => Err("worker-unavailable"),
                Err(error) => Err(error),
            };
        }
        Ok(PreparedLaunch {
            deadline,
            manifest,
            hash,
            roots,
            state_root,
            correlation_id,
            bridge_config,
            integration,
            lifecycle_task_key,
            worker_io_keys,
            launch_transaction,
            io_gates,
            process_holder,
            pid,
        })
    }
}
