use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use super::state::{Phase, UpdaterStatus};
use super::{download, feed, install, version, UpdaterError};

const STARTUP_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
struct PendingUpdate {
    version: String,
    asset_url: String,
    sha512: String,
    size: u64,
    installer_path: PathBuf,
}

struct Inner {
    status: UpdaterStatus,
    pending: Option<PendingUpdate>,
}

pub struct UpdaterService {
    feed_base: String,
    downloads_dir: PathBuf,
    client: reqwest::Client,
    inner: Mutex<Inner>,
    downloading: AtomicBool,
}

impl UpdaterService {
    pub fn new(data_dir: PathBuf) -> Arc<Self> {
        Self::try_with_feed_base(data_dir, feed::DEFAULT_FEED_BASE.to_string())
            .expect("updater HTTP client")
    }

    fn try_with_feed_base(data_dir: PathBuf, feed_base: String) -> Result<Arc<Self>, UpdaterError> {
        Ok(Arc::new(Self {
            feed_base,
            downloads_dir: data_dir.join("updates"),
            client: feed::build_client()?,
            inner: Mutex::new(Inner {
                status: UpdaterStatus::idle(version::current_version()),
                pending: None,
            }),
            downloading: AtomicBool::new(false),
        }))
    }

    #[cfg(test)]
    fn with_feed_base(data_dir: PathBuf, feed_base: String) -> Arc<Self> {
        Self::try_with_feed_base(data_dir, feed_base).unwrap()
    }

    fn set_status(&self, status: UpdaterStatus) -> Value {
        let json = status.to_json();
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status = status;
        json
    }

    pub(crate) fn status(&self) -> Value {
        self.snapshot().to_json()
    }

    pub(crate) async fn check(self: &Arc<Self>) -> Value {
        if self.downloading.load(Ordering::SeqCst) {
            return self.status();
        }
        self.set_status(UpdaterStatus {
            phase: Phase::Checking,
            ..self.snapshot()
        });
        let current = version::current_version();
        let result = feed::fetch_manifest(&self.client, &self.feed_base)
            .await
            .and_then(|manifest| {
                let file = manifest.primary_file()?.clone();
                let asset_url = feed::asset_url(&self.feed_base, &file.url)?;
                Ok((manifest.version, file, asset_url))
            });
        let checked_at_ms = now_ms();
        match result {
            Ok((new_version, file, asset_url)) if version::is_newer(&new_version, &current) => {
                let pending = PendingUpdate {
                    version: new_version.clone(),
                    asset_url,
                    sha512: file.sha512,
                    size: file.size,
                    installer_path: self.downloads_dir.join(file.url),
                };
                self.inner
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .pending = Some(pending);
                let json = self.set_status(UpdaterStatus {
                    phase: Phase::Available,
                    current_version: current,
                    new_version: Some(new_version),
                    percent: None,
                    message: None,
                    checked_at_ms: Some(checked_at_ms),
                });
                self.start_download();
                json
            }
            Ok(_) => {
                self.clear_pending();
                self.set_status(UpdaterStatus {
                    phase: Phase::NotAvailable,
                    current_version: current,
                    new_version: None,
                    percent: None,
                    message: None,
                    checked_at_ms: Some(checked_at_ms),
                })
            }
            Err(error) => self.set_error(error, Some(checked_at_ms)),
        }
    }

    pub(crate) fn download(self: &Arc<Self>) -> Value {
        if matches!(
            self.snapshot().phase,
            Phase::Downloading | Phase::Downloaded
        ) {
            return self.status();
        }
        if self.snapshot_pending().is_none() {
            return self.set_error(
                UpdaterError::NoUpdateAvailable,
                self.snapshot().checked_at_ms,
            );
        }
        self.start_download();
        self.status()
    }

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
        let part_path = pending.installer_path.with_extension("exe.part");
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
                    current_version: version::current_version(),
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
            current_version: version::current_version(),
            new_version: Some(pending.version.clone()),
            percent: Some(percent),
            message: None,
            checked_at_ms: self.snapshot().checked_at_ms,
        });
    }

    pub(crate) fn install(&self) -> Value {
        let Some(pending) = self.snapshot_pending() else {
            return self.set_error(
                UpdaterError::NoUpdateAvailable,
                self.snapshot().checked_at_ms,
            );
        };
        if self.snapshot().phase != Phase::Downloaded {
            return self.set_error(UpdaterError::NotDownloaded, self.snapshot().checked_at_ms);
        }
        match install::launch_silent_detached(&pending.installer_path) {
            Ok(()) => self.status(),
            Err(error) => self.set_error(error, self.snapshot().checked_at_ms),
        }
    }

    fn set_error(&self, error: UpdaterError, checked_at_ms: Option<i64>) -> Value {
        self.set_status(UpdaterStatus {
            phase: Phase::Error,
            current_version: version::current_version(),
            new_version: None,
            percent: None,
            message: Some(error.to_string()),
            checked_at_ms,
        })
    }

    fn clear_pending(&self) {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pending = None;
    }

    fn snapshot(&self) -> UpdaterStatus {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status
            .clone()
    }

    fn snapshot_pending(&self) -> Option<PendingUpdate> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pending
            .clone()
    }

    pub async fn run_startup_check_after_grace(
        self: Arc<Self>,
        desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    ) {
        tokio::time::sleep(STARTUP_GRACE).await;
        if desktop_authority.len() == 0 {
            self.check().await;
        }
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
