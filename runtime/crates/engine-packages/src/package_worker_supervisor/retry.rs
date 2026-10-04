use super::*;

pub(super) async fn schedule_retry(
    inner: &Arc<SupervisorInner>,
    key: &(String, String),
    spec: LaunchSpec,
    generation: u64,
    failures: u8,
) {
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        // Package workers are intentionally unsupported on Unix; leave the
        // failed state recorded rather than attempting a Windows launch.
        let _ = (inner, key, spec, generation, failures);
        return;
    }
    #[cfg(any(windows, target_os = "macos"))]
    {
        if restart_delay(&inner.restart_delays, failures).is_some() {
            let retry_inner = inner.clone();
            let retry_key = key.clone();
            let old_lifecycle_key = TaskKey::Lifecycle {
                package: key.0.clone(),
                version: key.1.clone(),
                generation,
            };
            inner.retry_tasks.reap_completed().await;
            let retry_registry_key = TaskKey::Retry {
                package: key.0.clone(),
                version: key.1.clone(),
                generation,
            };
            let Some((retry_start, mut retry_cancel)) =
                inner.retry_tasks.reserve(retry_registry_key.clone())
            else {
                return;
            };
            let retry_task = tokio::spawn(async move {
                let retry_started = tokio::select! {
                    _ = &mut retry_cancel => return,
                    started = retry_start => started.unwrap_or(false),
                };
                if !retry_started {
                    return;
                }
                let joined = tokio::select! {
                    _ = &mut retry_cancel => return,
                    joined = retry_inner.lifecycles.join_key(&old_lifecycle_key) => joined,
                };
                if !joined {
                    return;
                }
                let inner = retry_inner;
                let key = retry_key;
                let delay = restart_delay(&inner.restart_delays, failures).expect("retry delay");
                let next_generation = generation.saturating_add(1);
                tokio::select! {
                    _ = &mut retry_cancel => return,
                    _ = time::sleep(delay) => {}
                }
                let supervisor = PackageWorkerSupervisor {
                    inner: inner.clone(),
                };
                let valid = {
                    let workers = lock(&inner.workers);
                    workers.get(&key).is_some_and(|worker| {
                        worker.health.state == WorkerState::Failed
                            && worker.restart_allowed
                            && worker.generation == generation
                    })
                };
                if !valid {
                    tracing::info!(
                        target: "package_worker",
                        package_id = %key.0,
                        version = %key.1,
                        generation,
                        "worker retry canceled",
                    );
                    return;
                }
                if let Some(store) = lock(&inner.store).clone() {
                    let Ok(installed) = store.installed(&key.0, &key.1) else {
                        tracing::warn!(
                            target: "package_worker",
                            package_id = %key.0,
                            version = %key.1,
                            "worker retry package missing",
                        );
                        return;
                    };
                    let Ok(entrypoint) = store.immutable_entrypoint(&installed) else {
                        tracing::warn!(
                            target: "package_worker",
                            package_id = %key.0,
                            version = %key.1,
                            "worker retry immutable entrypoint invalid",
                        );
                        return;
                    };
                    if !installed.enabled
                        || installed.revoked
                        || !installed.hash.eq_ignore_ascii_case(&spec.hash)
                        || entrypoint != spec.executable
                    {
                        tracing::warn!(
                            target: "package_worker",
                            package_id = %key.0,
                            version = %key.1,
                            "worker retry package state invalid",
                        );
                        return;
                    }
                }
                if lock(&inner.typed_launches)
                    .get(&key)
                    .is_some_and(|launch| launch.generation != generation)
                {
                    return;
                }
                {
                    let mut workers = lock(&inner.workers);
                    let Some(worker) = workers.get_mut(&key) else {
                        return;
                    };
                    if worker.health.state != WorkerState::Failed
                        || !worker.restart_allowed
                        || worker.generation != generation
                    {
                        return;
                    }
                    worker.health.state = WorkerState::Starting;
                    worker.health.restart_count = failures as u32;
                    worker.generation = next_generation;
                    worker.lifecycle_reason = Some("restarting".into());
                }
                if let Some(launch) = lock(&inner.typed_launches).get_mut(&key) {
                    launch.generation = next_generation;
                }
                tracing::info!(
                    target: "package_worker",
                    package_id = %key.0,
                    version = %key.1,
                    generation = next_generation,
                    "worker retry",
                );
                inner.startups.reap_completed().await;
                let start_result = tokio::select! {
                    _ = &mut retry_cancel => return,
                    result = supervisor.start_windows_boxed(
                        key.clone(),
                        spec,
                        next_generation,
                        failures as u32,
                        failures,
                        true,
                    ) => result,
                };
                if start_result.is_err() {
                    let should_finish = {
                        let workers = lock(&inner.workers);
                        workers.get(&key).is_some_and(|worker| {
                            worker.generation == next_generation
                                && worker.health.state == WorkerState::Starting
                        })
                    };
                    if should_finish {
                        if let Some(tx) = lock(&inner.workers)
                            .get(&key)
                            .and_then(|worker| worker.lifecycle_tx.clone())
                        {
                            let _ = tx.send(WorkerLifecycleEvent::Finish {
                                generation: next_generation,
                                state: WorkerState::Failed,
                            });
                        }
                    }
                }
            });
            if let Err(task) = inner.retry_tasks.install(&retry_registry_key, retry_task) {
                let _ = task.await;
            }
        }
    }
}
