use super::*;

impl FileIndex {
    pub(super) fn spawn_rescan(&self) {
        // Regression H3 (2026-05-24): coalesce redundant rescan requests. If a
        // rescan is already scheduled, skip — the in-flight task will pick up
        // the latest settings/roots when it acquires scan_lock.
        if self.rescan_pending.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = self
            .self_ref
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        // If self_ref was never wired (tests skip bind_self), reset pending now
        // so subsequent calls don't get stuck in coalesce skip.
        if weak.upgrade().is_none() {
            self.rescan_pending.store(false, Ordering::SeqCst);
            return;
        }
        let task = tokio::spawn(async move {
            let Some(index) = weak.upgrade() else {
                return;
            };
            // Reset BEFORE waiting on scan_lock so the next mutation can
            // schedule a follow-up rescan that will run after us.
            index.rescan_pending.store(false, Ordering::SeqCst);
            if let Err(e) = index.rescan().await {
                tracing::warn!(target: "file_index", error = %e, "background rescan failed");
            }
        });
        self.track_background_task(task);
    }

    fn track_background_task(&self, task: tokio::task::JoinHandle<()>) {
        let mut tasks = self
            .background_tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // Reap finished handles first — the registry must stay bounded while
        // the app runs, not just at teardown (KOS-270).
        tasks.retain(|t| !t.is_finished());
        tasks.push(task);
    }

    /// Wait until every spawned rescan/cleanup task has finished — the tasks
    /// hold `Arc<FileStore>` (an open SQLite connection), so callers tearing
    /// down the index's data dir must drain first (KOS-270).
    pub async fn drain_background(&self) {
        loop {
            let task = self
                .background_tasks
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop();
            let Some(task) = task else { return };
            let _ = task.await;
        }
    }

    pub(super) fn spawn_removed_root_cleanup(&self, path: String) {
        let store = self.store.clone();
        let task = tokio::task::spawn_blocking(move || {
            if let Err(e) = store.remove_tree(&path) {
                tracing::warn!(
                    target: "file_index",
                    path = %path,
                    error = %e,
                    "removed scope cleanup failed"
                );
            }
        });
        self.track_background_task(task);
    }

    pub(super) fn invalidate_running_scan(&self) {
        self.scan_generation.fetch_add(1, Ordering::SeqCst);
    }
}
