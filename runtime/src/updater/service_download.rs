// Download half of `UpdaterService` — `include!`d into service.rs (same
// module, same scope: `native_apps/install.rs` pattern). Kept separate so
// neither file crosses the 300-line review threshold.

impl UpdaterService {
    fn start_download(self: &Arc<Self>) {
        if self.downloading.swap(true, Ordering::SeqCst) {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            this.run_download().await;
            this.downloading.store(false, Ordering::SeqCst);
        });
    }

    async fn run_download(self: &Arc<Self>) {
        let Some(pending) = self.snapshot_pending() else {
            return;
        };
        self.update_progress(&pending, 0);
        let part_path = cleanup::part_path(&pending.installer_path);
        let this = self.clone();
        let progress_pending = pending.clone();
        let transfer = download::download_resumable(
            &self.client,
            &pending.asset_url,
            &part_path,
            pending.size,
            move |done, total| {
                let percent = if total == 0 {
                    100
                } else {
                    ((done as f64 / total as f64) * 100.0).round() as u32
                };
                this.update_progress(&progress_pending, percent.min(100));
            },
        )
        .await;
        let result = match transfer {
            Ok(()) => {
                download::verify_and_commit(
                    &part_path,
                    &pending.installer_path,
                    pending.size,
                    &pending.sha512,
                )
                .await
            }
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => {
                self.set_status(UpdaterStatus {
                    phase: Phase::Downloaded,
                    current_version: self.current_version.clone(),
                    new_version: Some(pending.version),
                    percent: Some(100),
                    message: None,
                    checked_at_ms: self.snapshot().checked_at_ms,
                });
            }
            Err(error) => {
                let _ = tokio::fs::remove_file(&part_path).await;
                self.set_error(error, self.snapshot().checked_at_ms);
            }
        }
    }

    fn update_progress(&self, pending: &PendingUpdate, percent: u32) {
        self.set_status(UpdaterStatus {
            phase: Phase::Downloading,
            current_version: self.current_version.clone(),
            new_version: Some(pending.version.clone()),
            percent: Some(percent),
            message: None,
            checked_at_ms: self.snapshot().checked_at_ms,
        });
    }
}
