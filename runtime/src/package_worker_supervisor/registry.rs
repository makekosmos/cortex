use super::*;

/// Server-owned task admission. A reservation is published before spawn; the
/// start gate prevents work from beginning until its JoinHandle is installed.
pub(super) struct TaskRegistry {
    admissions_open: AtomicBool,
    capacity: usize,
    pub(super) slots: Mutex<HashMap<TaskKey, TaskSlot>>,
    pub(super) quarantine: Mutex<HashMap<TaskKey, TaskSlot>>,
}

impl TaskRegistry {
    pub(super) fn owned(capacity: usize) -> Arc<Self> {
        Arc::new(Self::new(capacity))
    }

    pub(super) fn new(capacity: usize) -> Self {
        Self {
            admissions_open: AtomicBool::new(true),
            capacity,
            slots: Mutex::new(HashMap::new()),
            quarantine: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn reserve(
        &self,
        key: TaskKey,
    ) -> Option<(oneshot::Receiver<bool>, oneshot::Receiver<()>)> {
        let mut slots = lock(&self.slots);
        let quarantine = lock(&self.quarantine);
        if !self.admissions_open.load(Ordering::Acquire)
            || slots.len() + quarantine.len() >= self.capacity
            || slots.contains_key(&key)
            || quarantine.contains_key(&key)
        {
            return None;
        }
        let (start_tx, start_rx) = oneshot::channel();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        slots.insert(
            key,
            TaskSlot {
                start: Some(start_tx),
                cancel: Some(cancel_tx),
                task: None,
                #[cfg(windows)]
                owner: None,
                #[cfg(windows)]
                process_holder: None,
            },
        );
        Some((start_rx, cancel_rx))
    }

    #[cfg(windows)]
    pub(super) fn reserve_startup(
        &self,
        key: TaskKey,
    ) -> Option<(
        oneshot::Receiver<bool>,
        oneshot::Receiver<()>,
        Arc<LaunchCleanupOwner>,
        WorkerProcessHolder,
    )> {
        let owner = LaunchCleanupOwner::new();
        let process_holder = Arc::new(AsyncMutex::new(None));
        let mut slots = lock(&self.slots);
        let quarantine = lock(&self.quarantine);
        if !self.admissions_open.load(Ordering::Acquire)
            || slots.len() + quarantine.len() >= self.capacity
            || slots.contains_key(&key)
            || quarantine.contains_key(&key)
        {
            return None;
        }
        let (start_tx, start_rx) = oneshot::channel();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        slots.insert(
            key,
            TaskSlot {
                start: Some(start_tx),
                cancel: Some(cancel_tx),
                task: None,
                owner: Some(owner.clone()),
                process_holder: Some(process_holder.clone()),
            },
        );
        Some((start_rx, cancel_rx, owner, process_holder))
    }

    #[cfg(windows)]
    pub(super) fn owner(&self, key: &TaskKey) -> Option<Arc<LaunchCleanupOwner>> {
        lock(&self.slots)
            .get(key)
            .and_then(|slot| slot.owner.clone())
            .or_else(|| {
                lock(&self.quarantine)
                    .get(key)
                    .and_then(|slot| slot.owner.clone())
            })
    }

    pub(super) fn install_pending(
        &self,
        key: &TaskKey,
        task: tokio::task::JoinHandle<()>,
    ) -> Result<(), tokio::task::JoinHandle<()>> {
        let mut slots = lock(&self.slots);
        let Some(slot) = slots.get_mut(key) else {
            return Err(task);
        };
        slot.task = Some(task);
        Ok(())
    }

    #[cfg(windows)]
    pub(super) fn detach_process_holder(&self, key: &TaskKey) -> bool {
        let mut slots = lock(&self.slots);
        let Some(slot) = slots.get_mut(key) else {
            return false;
        };
        slot.process_holder = None;
        true
    }

    #[cfg(windows)]
    pub(super) async fn release_clean_startup(&self, key: &TaskKey) -> bool {
        let (owner, holder) = {
            let slots = lock(&self.slots);
            let Some(slot) = slots.get(key) else {
                return false;
            };
            (slot.owner.clone(), slot.process_holder.clone())
        };
        if owner.is_some_and(|owner| owner.is_armed()) {
            return false;
        }
        if let Some(holder) = holder {
            if !holder_empty_until(holder, Instant::now() + STOP_DEADLINE).await {
                return false;
            }
        }
        let Some(mut slot) = self.remove(key) else {
            return false;
        };
        if let Some(task) = slot.task.take() {
            let _ = task.await;
        }
        true
    }

    pub(super) fn insert_quarantine(
        quarantine: &mut HashMap<TaskKey, TaskSlot>,
        key: TaskKey,
        slot: TaskSlot,
    ) -> Result<(), TaskSlot> {
        if quarantine.contains_key(&key) {
            Err(slot)
        } else {
            quarantine.insert(key, slot);
            Ok(())
        }
    }

    pub(super) fn open(&self, key: &TaskKey) -> bool {
        lock(&self.slots)
            .get_mut(key)
            .and_then(|slot| slot.start.take())
            .is_some_and(|start| start.send(true).is_ok())
    }

    pub(super) fn install(
        &self,
        key: &TaskKey,
        task: tokio::task::JoinHandle<()>,
    ) -> Result<(), tokio::task::JoinHandle<()>> {
        if let Err(task) = self.install_pending(key, task) {
            return Err(task);
        }
        self.open(key);
        Ok(())
    }

    pub(super) fn remove(&self, key: &TaskKey) -> Option<TaskSlot> {
        lock(&self.slots).remove(key)
    }

    pub(super) fn contains(&self, key: &TaskKey) -> bool {
        lock(&self.slots).contains_key(key)
    }

    #[cfg(all(windows, feature = "package-worker-fixture"))]
    pub(super) fn is_finished(&self, key: &TaskKey) -> bool {
        lock(&self.slots)
            .get(key)
            .and_then(|slot| slot.task.as_ref())
            .is_some_and(tokio::task::JoinHandle::is_finished)
    }

    pub(super) fn has_generation(&self, package: &str, version: &str, generation: u64) -> bool {
        let matches = |key: &TaskKey| match key {
            TaskKey::WorkerStdin {
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
            _ => false,
        };
        lock(&self.slots).keys().any(matches) || lock(&self.quarantine).keys().any(matches)
    }

    pub(super) fn close(&self) {
        self.admissions_open.store(false, Ordering::Release);
    }

    pub(super) async fn reap_completed(&self) {
        self.reap_completed_until(Instant::now() + STOP_DEADLINE)
            .await;
    }
}
