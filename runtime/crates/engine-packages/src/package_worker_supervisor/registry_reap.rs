use super::*;

impl TaskRegistry {
    pub(super) async fn reap_completed_until(&self, deadline: Instant) {
        if deadline <= Instant::now() {
            return;
        }
        // Move completed Startup slots into quarantine while holding both locks.
        // The exact key is therefore continuously reserved throughout cleanup.
        let (ordinary, startup_keys) = {
            let mut slots = lock(&self.slots);
            let mut quarantine = lock(&self.quarantine);
            let keys: Vec<_> = slots
                .iter()
                .filter_map(|(key, slot)| {
                    slot.task
                        .as_ref()
                        .filter(|task| task.is_finished())
                        .map(|_| key.clone())
                })
                .collect();
            let mut ordinary = Vec::new();
            let mut startup_keys = Vec::new();
            for key in keys {
                let Some(slot) = slots.remove(&key) else {
                    continue;
                };
                if matches!(key, TaskKey::Startup { .. }) {
                    match Self::insert_quarantine(&mut quarantine, key.clone(), slot) {
                        Ok(()) => startup_keys.push(key),
                        Err(slot) => {
                            // Never overwrite or drop an armed incoming slot.
                            slots.insert(key, slot);
                        }
                    }
                } else {
                    ordinary.push((key, slot));
                }
            }
            (ordinary, startup_keys)
        };

        for (_key, mut slot) in ordinary {
            if let Some(start) = slot.start.take() {
                let _ = start.send(false);
            }
            if let Some(task) = slot.task.take() {
                let _ = task.await;
            }
        }

        for key in &startup_keys {
            #[cfg(any(windows, target_os = "macos"))]
            let owner = {
                let quarantine = lock(&self.quarantine);
                quarantine.get(&key).and_then(|slot| slot.owner.clone())
            };
            #[cfg(any(windows, target_os = "macos"))]
            let owner_ok = match owner {
                Some(owner) if !owner.is_armed() => true,
                Some(owner) => cleanup_owner_async(owner, deadline).await.is_ok(),
                None => true,
            };
            #[cfg(any(windows, target_os = "macos"))]
            let holder = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.process_holder.clone());
            #[cfg(any(windows, target_os = "macos"))]
            let holder_ok = match holder {
                Some(holder) => cleanup_process_holder_until(holder, deadline).await,
                None => true,
            };
            #[cfg(any(windows, target_os = "macos"))]
            let cleanup_ok = owner_ok && holder_ok;
            #[cfg(not(any(windows, target_os = "macos")))]
            let cleanup_ok = true;
            #[cfg(any(windows, target_os = "macos"))]
            let holder = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.process_holder.clone());
            #[cfg(any(windows, target_os = "macos"))]
            let holder_empty = match holder {
                Some(holder) => holder_empty_until(holder, deadline).await,
                None => true,
            };
            #[cfg(not(any(windows, target_os = "macos")))]
            let holder_empty = true;
            let task_done = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.task.as_ref())
                .is_none_or(|task| task.is_finished());
            if cleanup_ok && holder_empty && task_done {
                let task = lock(&self.quarantine).remove(&key).and_then(|mut slot| {
                    if let Some(start) = slot.start.take() {
                        let _ = start.send(false);
                    }
                    slot.task.take()
                });
                if let Some(task) = task {
                    let _ = task.await;
                }
            }
        }

        // A previously quarantined Startup is inspected in place. Its key is
        // never absent while cleanup or joining is in progress.
        let newly_quarantined = startup_keys
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let keys = lock(&self.quarantine)
            .keys()
            .filter(|key| !newly_quarantined.contains(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            #[cfg(any(windows, target_os = "macos"))]
            let owner = {
                let quarantine = lock(&self.quarantine);
                quarantine.get(&key).and_then(|slot| slot.owner.clone())
            };
            #[cfg(any(windows, target_os = "macos"))]
            let owner_ok = match owner {
                Some(owner) if !owner.is_armed() => true,
                Some(owner) => cleanup_owner_async(owner, deadline).await.is_ok(),
                None => true,
            };
            #[cfg(any(windows, target_os = "macos"))]
            let holder = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.process_holder.clone());
            #[cfg(any(windows, target_os = "macos"))]
            let holder_ok = match holder {
                Some(holder) => cleanup_process_holder_until(holder, deadline).await,
                None => true,
            };
            #[cfg(any(windows, target_os = "macos"))]
            let cleanup_ok = owner_ok && holder_ok;
            #[cfg(not(any(windows, target_os = "macos")))]
            let cleanup_ok = true;
            #[cfg(any(windows, target_os = "macos"))]
            let holder = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.process_holder.clone());
            #[cfg(any(windows, target_os = "macos"))]
            let holder_empty = match holder {
                Some(holder) => holder_empty_until(holder, deadline).await,
                None => true,
            };
            #[cfg(not(any(windows, target_os = "macos")))]
            let holder_empty = true;
            let task_done = lock(&self.quarantine)
                .get(&key)
                .and_then(|slot| slot.task.as_ref())
                .is_none_or(|task| task.is_finished());
            if cleanup_ok && holder_empty && task_done {
                let task = lock(&self.quarantine)
                    .remove(&key)
                    .and_then(|mut slot| slot.task.take());
                if let Some(task) = task {
                    let _ = task.await;
                }
            }
        }
    }

    pub(super) async fn cancel_generation_until(
        &self,
        package: &str,
        version: &str,
        generation: u64,
        deadline: Instant,
    ) -> bool {
        self.cancel_matching_until(
            |key| match key {
                TaskKey::Call {
                    package: p,
                    version: v,
                    generation: g,
                    ..
                }
                | TaskKey::Startup {
                    package: p,
                    version: v,
                    generation: g,
                }
                | TaskKey::Lifecycle {
                    package: p,
                    version: v,
                    generation: g,
                }
                | TaskKey::Retry {
                    package: p,
                    version: v,
                    generation: g,
                }
                | TaskKey::WorkerStdin {
                    package: p,
                    version: v,
                    generation: g,
                }
                | TaskKey::WorkerStdout {
                    package: p,
                    version: v,
                    generation: g,
                }
                | TaskKey::WorkerStderr {
                    package: p,
                    version: v,
                    generation: g,
                } => p == package && v == version && *g == generation,
            },
            deadline,
        )
        .await
    }

    pub(super) async fn cancel_generation(
        &self,
        package: &str,
        version: &str,
        generation: u64,
    ) -> bool {
        self.cancel_generation_until(package, version, generation, Instant::now() + STOP_DEADLINE)
            .await
    }
}
