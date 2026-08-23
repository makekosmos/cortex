use super::types::{WsConnectionSlot, WsRequestSlot};
use super::*;

#[derive(Clone)]
pub struct WsShutdownHandle {
    pub(super) lifecycle: Arc<WsLifecycle>,
}

impl WsShutdownHandle {
    pub async fn begin_shutdown(&self) {
        let _admission = self
            .lifecycle
            .admission
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if !self.lifecycle.closed.swap(true, Ordering::AcqRel) {
            self.lifecycle.request_closed.store(true, Ordering::Release);
            self.lifecycle.shutdown.notify_waiters();
        }
    }

    pub(super) async fn cancelled(&self) {
        let notified = self.lifecycle.shutdown.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.lifecycle.closed.load(Ordering::Acquire) {
            return;
        }
        notified.await;
    }

    pub(super) async fn reap(&self) {
        let finished = {
            let mut tasks = self
                .lifecycle
                .tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let ids = tasks
                .iter()
                .filter_map(|(id, slot)| match slot {
                    WsConnectionSlot::Installed(task) if task.is_finished() => Some(*id),
                    WsConnectionSlot::Reserved { .. } | WsConnectionSlot::Installed(_) => None,
                })
                .collect::<Vec<_>>();
            ids.into_iter()
                .filter_map(|id| tasks.remove(&id))
                .collect::<Vec<_>>()
        };
        for slot in finished {
            if let WsConnectionSlot::Installed(task) = slot {
                let _ = task.await;
            }
        }
        let finished_requests = {
            let mut tasks = self
                .lifecycle
                .request_tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let ids = tasks
                .iter()
                .filter_map(|(id, slot)| match slot {
                    WsRequestSlot::Installed { task, .. } if task.is_finished() => Some(*id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            ids.into_iter()
                .filter_map(|id| tasks.remove(&id))
                .collect::<Vec<_>>()
        };
        for slot in finished_requests {
            if let WsRequestSlot::Installed { task, .. } = slot {
                let _ = task.await;
            }
        }
    }

    pub(super) async fn finish_request(&self, request_id: u64, cancel: bool) {
        let slot = self
            .lifecycle
            .request_tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&request_id);
        let Some(slot) = slot else {
            return;
        };
        match slot {
            WsRequestSlot::Reserved {
                permit,
                cancel: sender,
            } => {
                let _ = permit.lock().unwrap_or_else(|p| p.into_inner()).take();
                if let Some(sender) = sender.lock().unwrap_or_else(|p| p.into_inner()).take() {
                    let _ = sender.send(());
                }
            }
            WsRequestSlot::Installed {
                task,
                cancel: sender,
            } => {
                if cancel {
                    if let Some(sender) = sender.lock().unwrap_or_else(|p| p.into_inner()).take() {
                        let _ = sender.send(());
                    }
                    let mut task = task;
                    if tokio::time::timeout(WS_SEND_DEADLINE, &mut task)
                        .await
                        .is_err()
                    {
                        task.abort();
                        let _ = task.await;
                    }
                } else {
                    let _ = task.await;
                }
            }
        }
    }

    pub(super) fn install_request<F>(
        &self,
        request_id: u64,
        permit: Arc<Mutex<Option<tokio::sync::OwnedSemaphorePermit>>>,
        cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
        spawn: F,
    ) -> bool
    where
        F: FnOnce(tokio::sync::oneshot::Receiver<()>) -> tokio::task::JoinHandle<()>,
    {
        let _admission = self
            .lifecycle
            .admission
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if self.lifecycle.request_closed.load(Ordering::Acquire) {
            let _ = permit.lock().unwrap_or_else(|p| p.into_inner()).take();
            return false;
        }
        self.lifecycle
            .request_tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                request_id,
                WsRequestSlot::Reserved {
                    permit: permit.clone(),
                    cancel: cancel.clone(),
                },
            );
        let (start_sender, start_receiver) = tokio::sync::oneshot::channel();
        let task = spawn(start_receiver);
        let old = self
            .lifecycle
            .request_tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(request_id, WsRequestSlot::Installed { task, cancel });
        debug_assert!(matches!(old, Some(WsRequestSlot::Reserved { .. })));
        let _ = start_sender.send(());
        true
    }

    pub fn task_count(&self) -> usize {
        self.lifecycle
            .tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }

    pub fn request_task_count(&self) -> usize {
        self.lifecycle
            .request_tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }

    pub fn available_capacity(&self) -> usize {
        self.lifecycle.capacity.available_permits()
    }

    pub(super) fn response_deadline(&self) -> Duration {
        *self
            .lifecycle
            .response_deadline
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    #[cfg(test)]
    pub(super) fn set_response_deadline(&self, deadline: Duration) {
        *self
            .lifecycle
            .response_deadline
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = deadline;
    }

    pub(super) async fn drain(&self, deadline: Duration) -> Result<(), &'static str> {
        self.begin_shutdown().await;
        let started = Instant::now();
        let mut tasks = {
            let mut registry = self
                .lifecycle
                .tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            registry.drain().map(|(_, slot)| slot).collect::<Vec<_>>()
        };
        let mut deadline_breached = false;
        for slot in &mut tasks {
            match slot {
                WsConnectionSlot::Reserved { resources, .. } => {
                    let _ = resources.lock().unwrap_or_else(|p| p.into_inner()).take();
                }
                WsConnectionSlot::Installed(task) => {
                    let remaining = deadline.saturating_sub(started.elapsed());
                    let grace = remaining.min(Duration::from_secs(2));
                    match tokio::time::timeout(grace, &mut *task).await {
                        Ok(Ok(())) | Ok(Err(_)) => {}
                        Err(_) => {
                            deadline_breached = true;

                            // Cancellation is cooperative, but the server still owns the
                            // task. Force-reap a handler that ignores the signal before
                            // the truthful overall deadline expires; its OwnerLease guard
                            // then runs synchronously during task destruction.
                            task.abort();
                            let _ = task.await;
                        }
                    }
                }
            }
        }
        let mut request_slots = {
            let mut registry = self
                .lifecycle
                .request_tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            registry.drain().map(|(_, slot)| slot).collect::<Vec<_>>()
        };
        for slot in &mut request_slots {
            match slot {
                WsRequestSlot::Reserved { permit, cancel } => {
                    let _ = permit.lock().unwrap_or_else(|p| p.into_inner()).take();
                    if let Some(cancel) = cancel.lock().unwrap_or_else(|p| p.into_inner()).take() {
                        let _ = cancel.send(());
                    }
                }
                WsRequestSlot::Installed { task, cancel } => {
                    if let Some(cancel) = cancel.lock().unwrap_or_else(|p| p.into_inner()).take() {
                        let _ = cancel.send(());
                    }
                    let remaining = deadline
                        .saturating_sub(started.elapsed())
                        .min(Duration::from_secs(2));
                    if tokio::time::timeout(remaining, &mut *task).await.is_err() {
                        deadline_breached = true;
                        task.abort();
                        let _ = task.await;
                    }
                }
            }
        }
        self.reap().await;
        let connection_tasks = self.task_count();
        let request_tasks = self.request_task_count();
        if deadline_breached || connection_tasks != 0 || request_tasks != 0 {
            tracing::error!(?deadline, elapsed = ?started.elapsed(), connection_tasks, request_tasks, "WS shutdown exceeded its bounded cleanup lifecycle");
            Err("WS shutdown exceeded its deadline")
        } else {
            Ok(())
        }
    }

    pub async fn shutdown(&self) -> Result<(), &'static str> {
        self.drain(WS_SHUTDOWN_DEADLINE).await
    }
}
