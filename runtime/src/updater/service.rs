use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use super::state::{Phase, UpdaterStatus};
use super::{cleanup, download, feed, install, version, UpdaterError};

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
    // The current installer is Windows-only. UI consumes this capability,
    // never guesses it from its own OS, and other hosts don't download .exe.
    install_supported: bool,
    feed_base: String,
    downloads_dir: PathBuf,
    client: reqwest::Client,
    // Baked-in product version this binary runs as, fixed for the life of
    // the service; tests inject their own since a test build has no
    // MUNDUS_PRODUCT_VERSION and reports `dev`, which is not semver.
    current_version: String,
    inner: Mutex<Inner>,
    downloading: AtomicBool,
}

impl UpdaterService {
    pub fn new(data_dir: PathBuf) -> Arc<Self> {
        Self::try_with_feed_base(
            data_dir,
            feed::DEFAULT_FEED_BASE.to_string(),
            version::current_version(),
            cfg!(windows),
        )
        .expect("updater HTTP client")
    }

    fn try_with_feed_base(
        data_dir: PathBuf,
        feed_base: String,
        current_version: String,
        install_supported: bool,
    ) -> Result<Arc<Self>, UpdaterError> {
        let downloads_dir = data_dir.join("updates");
        // KOS-301: a fresh service has no pending update and no live download,
        // so every payload in `updates/` is a leftover from a previous run
        // (applied installer, superseded version, crashed .part).
        let report = cleanup::sweep_downloads(&downloads_dir, &cleanup::keep_set(None));
        if report.removed > 0 || report.failed > 0 {
            tracing::info!(
                removed = report.removed,
                failed = report.failed,
                "updater sweep"
            );
        }
        Ok(Arc::new(Self {
            install_supported,
            feed_base,
            downloads_dir,
            client: feed::build_client()?,
            current_version: current_version.clone(),
            inner: Mutex::new(Inner {
                status: UpdaterStatus::idle(current_version),
                pending: None,
            }),
            downloading: AtomicBool::new(false),
        }))
    }

    #[cfg(test)]
    fn with_feed_base(data_dir: PathBuf, feed_base: String, current_version: &str) -> Arc<Self> {
        Self::try_with_feed_base(data_dir, feed_base, current_version.to_string(), true).unwrap()
    }

    fn status_json(&self, status: &UpdaterStatus) -> Value {
        let mut value = status.to_json();
        value["canInstall"] = self.install_supported.into();
        if !self.install_supported {
            value["installUnavailableReason"] =
                "Автоматическая установка обновлений для этого пакета пока не поддерживается."
                    .into();
        }
        value
    }

    fn set_status(&self, status: UpdaterStatus) -> Value {
        let json = self.status_json(&status);
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status = status;
        json
    }

    pub(crate) fn status(&self) -> Value {
        self.status_json(&self.snapshot())
    }

    pub(crate) async fn check(self: &Arc<Self>) -> Value {
        if self.downloading.load(Ordering::SeqCst) {
            return self.status();
        }
        self.set_status(UpdaterStatus {
            phase: Phase::Checking,
            ..self.snapshot()
        });
        let current = self.current_version.clone();
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
                let keep = cleanup::keep_set(Some(&pending.installer_path));
                self.inner
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .pending = Some(pending);
                // Superseded payloads go away once the new pending is known.
                cleanup::sweep_downloads(&self.downloads_dir, &keep);
                let json = self.set_status(UpdaterStatus {
                    phase: Phase::Available,
                    current_version: current,
                    new_version: Some(new_version),
                    percent: None,
                    message: None,
                    checked_at_ms: Some(checked_at_ms),
                });
                if self.install_supported {
                    self.start_download();
                }
                json
            }
            Ok(_) => {
                self.clear_pending();
                // No pending update — nothing in updates/ is still needed.
                cleanup::sweep_downloads(&self.downloads_dir, &cleanup::keep_set(None));
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
        if !self.install_supported {
            return self.set_error(
                UpdaterError::UnsupportedPlatform,
                self.snapshot().checked_at_ms,
            );
        }
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

    pub(crate) fn install(&self) -> Value {
        if !self.install_supported {
            return self.set_error(
                UpdaterError::UnsupportedPlatform,
                self.snapshot().checked_at_ms,
            );
        }
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
            current_version: self.current_version.clone(),
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

include!("service_download.rs");

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
