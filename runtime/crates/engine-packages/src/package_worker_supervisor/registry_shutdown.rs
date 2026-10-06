use super::*;

impl TaskRegistry {
    pub(super) async fn shutdown_until(&self, deadline: Instant) -> bool {
        if deadline <= Instant::now() {
            return false;
        }
        self.close();
        let ok = self.cancel_matching_until(|_| true, deadline).await;
        loop {
            self.reap_completed_until(deadline).await;
            if self.len() == 0 {
                break;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::task::yield_now().await;
        }
        ok
    }

    #[cfg(test)]
    pub(super) async fn shutdown(&self) -> bool {
        self.shutdown_until(Instant::now() + STOP_DEADLINE).await
    }

    #[cfg(test)]
    pub(super) async fn cancel_matching<F>(&self, predicate: F) -> bool
    where
        F: Fn(&TaskKey) -> bool,
    {
        self.cancel_matching_until(predicate, Instant::now() + STOP_DEADLINE)
            .await
    }

    pub(super) async fn cancel_matching_until<F>(&self, predicate: F, deadline: Instant) -> bool
    where
        F: Fn(&TaskKey) -> bool,
    {
        if deadline <= Instant::now() {
            return false;
        }
        self.reap_completed_until(deadline).await;
        let already_quarantined = {
            let _slots = lock(&self.slots);
            let quarantine = lock(&self.quarantine);
            quarantine.keys().any(|key| predicate(key))
        };
        let (entries, startup_keys, collision) = {
            let mut slots = lock(&self.slots);
            let keys: Vec<_> = slots.keys().filter(|key| predicate(key)).cloned().collect();
            let mut entries = Vec::new();
            let mut startup_keys = Vec::new();
            let mut collision = false;
            let mut quarantine = lock(&self.quarantine);
            for key in keys {
                let Some(slot) = slots.remove(&key) else {
                    continue;
                };
                if matches!(key, TaskKey::Startup { .. }) {
                    match Self::insert_quarantine(&mut quarantine, key.clone(), slot) {
                        Ok(()) => startup_keys.push(key),
                        Err(slot) => {
                            // A Startup slot is never routed through generic
                            // abort handling. Preserve it in active ownership.
                            slots.insert(key, slot);
                            collision = true;
                        }
                    }
                } else {
                    entries.push((key, slot));
                }
            }
            (entries, startup_keys, collision)
        };
        let mut ok = !already_quarantined && !collision;
        for key in &startup_keys {
            let mut quarantine = lock(&self.quarantine);
            if let Some(slot) = quarantine.get_mut(key) {
                if let Some(start) = slot.start.take() {
                    let _ = start.send(false);
                }
                if let Some(cancel) = slot.cancel.take() {
                    let _ = cancel.send(());
                }
            }
        }
        while !startup_keys.is_empty() {
            self.reap_completed_until(deadline).await;
            if startup_keys
                .iter()
                .all(|key| !self.contains_quarantined(key))
            {
                break;
            }
            if Instant::now() >= deadline {
                ok = false;
                break;
            }
            tokio::task::yield_now().await;
        }
        for (_key, mut slot) in entries {
            if let Some(start) = slot.start.take() {
                let _ = start.send(false);
            }
            if let Some(cancel) = slot.cancel.take() {
                let _ = cancel.send(());
            }
            if let Some(mut task) = slot.task.take() {
                if time::timeout(
                    deadline.saturating_duration_since(Instant::now()),
                    &mut task,
                )
                .await
                .is_err()
                {
                    ok = false;
                    task.abort();
                    let _ = task.await;
                }
            }
        }
        ok
    }

    pub(super) async fn cancel_keys_until(&self, keys: &[TaskKey], deadline: Instant) -> bool {
        let wanted = keys
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        self.cancel_matching_until(|key| wanted.contains(key), deadline)
            .await
    }

    pub(super) async fn join_key_until(&self, key: &TaskKey, deadline: Instant) -> bool {
        self.reap_completed_until(deadline).await;
        if lock(&self.quarantine).contains_key(key) {
            return false;
        }
        let Some(mut slot) = self.remove(key) else {
            return true;
        };
        if let Some(start) = slot.start.take() {
            let _ = start.send(true);
        }
        let Some(mut task) = slot.task.take() else {
            return true;
        };
        if time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            &mut task,
        )
        .await
        .is_err()
        {
            task.abort();
            let _ = task.await;
            return false;
        }
        true
    }

    pub(super) async fn join_key(&self, key: &TaskKey) -> bool {
        self.join_key_until(key, Instant::now() + STOP_DEADLINE)
            .await
    }

    pub(super) fn len(&self) -> usize {
        lock(&self.slots).len() + lock(&self.quarantine).len()
    }

    pub(super) fn contains_quarantined(&self, key: &TaskKey) -> bool {
        lock(&self.quarantine).contains_key(key)
    }

    pub(super) fn contains_quarantined_startup(&self, package: &str, version: &str) -> bool {
        lock(&self.quarantine).keys().any(|key| {
            matches!(
                key,
                TaskKey::Startup {
                    package: p,
                    version: v,
                    ..
                } if p == package && v == version
            )
        })
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub(super) fn startup_reservation_snapshot(&self, key: &TaskKey) -> Option<(bool, bool)> {
        let slots = lock(&self.slots);
        let quarantine = lock(&self.quarantine);
        slots
            .get(key)
            .or_else(|| quarantine.get(key))
            .map(|slot| (slot.owner.is_some(), slot.process_holder.is_some()))
    }
}
