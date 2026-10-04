use super::*;

impl PackageWorkerSupervisor {
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) async fn start_windows_transaction(
        &self,
        key: (String, String),
        spec: LaunchSpec,
        generation: u64,
        restart_count: u32,
        failure_streak: u8,
        restart_allowed: bool,
        mut cancel_rx: oneshot::Receiver<()>,
        owner: Arc<LaunchCleanupOwner>,
        process_holder: WorkerProcessHolder,
    ) -> Result<(), &'static str> {
        let prepared = self
            .prepare_launch(
                &key,
                spec.clone(),
                generation,
                restart_count,
                &mut cancel_rx,
                owner,
                process_holder,
            )
            .await?;
        let PreparedLaunch {
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
        } = prepared;
        let (grant, token, broker, line) = match self.prepare_grant(
            &key,
            &manifest,
            hash,
            pid,
            generation,
            correlation_id,
            &roots,
            &state_root,
            &bridge_config,
            &integration,
        ) {
            Ok(value) => value,
            Err(reason) => return launch_transaction.rollback_error(reason, deadline).await,
        };
        let (stdin, stdout, stderr) = {
            let Some(mut holder) = lock_holder_until(&process_holder, deadline).await else {
                return launch_transaction
                    .rollback_error("published-process-holder-timeout", deadline)
                    .await;
            };
            let Some(process) = holder.as_mut() else {
                drop(holder);
                return launch_transaction
                    .rollback_error("process-missing", deadline)
                    .await;
            };
            let Some(pipes) = process.take_all_pipes() else {
                drop(holder);
                return launch_transaction
                    .rollback_error("worker-pipes-missing", deadline)
                    .await;
            };
            pipes
        };
        let SpawnedTasks {
            stdin_tx,
            lifecycle_tx,
            bootstrap_ack_rx,
            hello_tx,
            hello_rx,
            stdout_tail,
            stderr_tail,
        } = match self
            .spawn_worker_tasks(
                &key,
                generation,
                deadline,
                lifecycle_task_key.clone(),
                worker_io_keys.clone(),
                io_gates,
                &mut cancel_rx,
                stdin,
                stdout,
                stderr,
                line,
            )
            .await
        {
            Ok(tasks) => tasks,
            Err(reason) => return launch_transaction.rollback_error(reason, deadline).await,
        };
        let stopping = lock(&self.inner.workers).get(&key).is_some_and(|worker| {
            matches!(
                worker.health.state,
                WorkerState::Stopping | WorkerState::Stopped
            )
        });
        let worker_has_resources = lock(&self.inner.workers)
            .get(&key)
            .is_some_and(worker_has_live_resources);
        if stopping || worker_has_resources {
            return launch_transaction
                .rollback_error("worker-unavailable", deadline)
                .await;
        }
        {
            let mut workers = lock(&self.inner.workers);
            workers.insert(
                key.clone(),
                LiveWorker {
                    health: WorkerHealth {
                        state: WorkerState::Starting,
                        restart_count,
                    },
                    generation,
                    started: Instant::now(),
                    last_heartbeat: Instant::now(),
                    grant: Some(grant),
                    bootstrap_token_hash: Some(hash_token(&token)),
                    process_holder,
                    stdin: Some(stdin_tx.clone()),
                    io_keys: [
                        worker_io_keys[0].clone(),
                        worker_io_keys[1].clone(),
                        worker_io_keys[2].clone(),
                    ],
                    lifecycle_tx: Some(lifecycle_tx.clone()),
                    heartbeat_task: None,
                    schedule_task: None,
                    hello: Some(hello_tx),
                    bootstrap_complete: false,
                    cleanup_started: false,
                    broker,
                    stdout_tail: stdout_tail.clone(),
                    stderr_tail: stderr_tail.clone(),
                    bridge_status: None,
                    in_flight: 0,
                    restart_allowed,
                    launch_spec: Some(spec),
                    failure_streak,
                    lifecycle_reason: Some("starting".into()),
                },
            );
        }
        self.inner.worker_io.open(&worker_io_keys[0]);
        let bootstrap_ok = matches!(
            startup_wait(
                &mut cancel_rx,
                time::timeout(BOOTSTRAP_DEADLINE, bootstrap_ack_rx)
            )
            .await,
            Ok(Ok(Ok(Ok(()))))
        );
        if !bootstrap_ok {
            return self
                .cleanup_published_generation(
                    &key,
                    generation,
                    Some(lifecycle_tx.clone()),
                    "bootstrap-write-failed",
                    deadline,
                )
                .await;
        }
        let bootstrap_generation_is_current = {
            let mut workers = lock(&self.inner.workers);
            workers.get_mut(&key).is_some_and(|worker| {
                if worker.generation == generation && worker.health.state == WorkerState::Starting {
                    worker.bootstrap_complete = true;
                    true
                } else {
                    false
                }
            })
        };
        if !bootstrap_generation_is_current {
            return self
                .cleanup_published_generation(
                    &key,
                    generation,
                    Some(lifecycle_tx.clone()),
                    "worker-unavailable",
                    deadline,
                )
                .await;
        }
        self.inner.worker_io.open(&worker_io_keys[1]);
        self.inner.worker_io.open(&worker_io_keys[2]);
        let hello_result =
            startup_wait(&mut cancel_rx, time::timeout(HELLO_DEADLINE, hello_rx)).await;
        match hello_result {
            Ok(Ok(Ok(Ok(())))) => {
                tracing::info!(
                    target: "package_worker",
                    package_id = %key.0,
                    version = %key.1,
                    generation,
                    "worker hello",
                );
                let inner = self.inner.clone();
                let heartbeat_key = key.clone();
                let heartbeat_task =
                    tokio::spawn(
                        async move { heartbeat_watch(inner, heartbeat_key, generation).await },
                    );
                let mut heartbeat_task = Some(heartbeat_task);
                let installed = {
                    let mut workers = lock(&self.inner.workers);
                    workers.get_mut(&key).is_some_and(|worker| {
                        if worker.generation == generation
                            && worker.health.state == WorkerState::Running
                            && worker.heartbeat_task.is_none()
                        {
                            worker.heartbeat_task = heartbeat_task.take();
                            true
                        } else {
                            false
                        }
                    })
                };
                if !installed {
                    let task = heartbeat_task.expect("heartbeat task not installed");
                    task.abort();
                    let _ = task.await;
                    return self
                        .cleanup_published_generation(
                            &key,
                            generation,
                            Some(lifecycle_tx.clone()),
                            "worker-unavailable",
                            deadline,
                        )
                        .await;
                }
                if restart_allowed {
                    let _ = self.activate(&key.0, &key.1);
                }
                let startup_key = TaskKey::Startup {
                    package: key.0.clone(),
                    version: key.1.clone(),
                    generation,
                };
                if !self.inner.startups.detach_process_holder(&startup_key) {
                    return self
                        .cleanup_published_generation(
                            &key,
                            generation,
                            Some(lifecycle_tx.clone()),
                            "worker-unavailable",
                            deadline,
                        )
                        .await;
                }
                Ok(())
            }
            _ => {
                self.cleanup_published_generation(
                    &key,
                    generation,
                    Some(lifecycle_tx),
                    "worker-unavailable",
                    deadline,
                )
                .await
            }
        }
    }
}
