use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

#[cfg(unix)]
use super::macos;
use super::state::{Phase, UpdaterStatus};
use super::{cleanup, download, feed, install, version, UpdaterError};

const STARTUP_GRACE: Duration = Duration::from_secs(5);
// Engine-owned discovery (KOS-357): Manager polls `updater.status`, which is
// side-effect free, so it must never be the only trigger. Recheck the feed
// periodically so an update released while Mundus is running still surfaces.
const RECHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

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
    // `manifest.json` platforms key this Engine updates from (KOS-350).
    platform: &'static str,
    feed_base: String,
    downloads_dir: PathBuf,
    client: reqwest::Client,
    // Baked-in product version this binary runs as, fixed for the life of
    // the service; tests inject their own since a test build has no
    // MUNDUS_PRODUCT_VERSION and reports `dev`, which is not semver.
    current_version: String,
    inner: Mutex<Inner>,
    downloading: AtomicBool,
    /// Reentrancy guard for `start_mac_install` — the Windows arm spawns
    /// instantly and needs none.
    #[cfg(unix)]
    installing: AtomicBool,
}

impl UpdaterService {
    pub fn new(data_dir: PathBuf) -> Arc<Self> {
        Self::try_with_feed_base(
            data_dir,
            feed::DEFAULT_FEED_BASE.to_string(),
            version::current_version(),
            cfg!(windows) || cfg!(target_os = "macos"),
            feed::host_platform(),
        )
        .expect("updater HTTP client")
    }

    fn try_with_feed_base(
        data_dir: PathBuf,
        feed_base: String,
        current_version: String,
        install_supported: bool,
        platform: &'static str,
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
            platform,
            feed_base,
            downloads_dir,
            client: feed::build_client()?,
            current_version: current_version.clone(),
            inner: Mutex::new(Inner {
                status: UpdaterStatus::idle(current_version),
                pending: None,
            }),
            downloading: AtomicBool::new(false),
            #[cfg(unix)]
            installing: AtomicBool::new(false),
        }))
    }

    #[cfg(test)]
    fn with_feed_base(data_dir: PathBuf, feed_base: String, current_version: &str) -> Arc<Self> {
        Self::try_with_feed_base(
            data_dir,
            feed_base,
            current_version.to_string(),
            true,
            "win",
        )
        .unwrap()
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
        let platform = self.platform;
        let result = feed::fetch_release(&self.client, &self.feed_base, platform)
            .await
            .and_then(|release| {
                // A release without a build for this platform is "nothing
                // to install", not an error.
                let Some(file) = release.file else {
                    return Ok(None);
                };
                let asset_url = feed::asset_url(&self.feed_base, &file.url, platform)?;
                Ok(Some((release.version, file, asset_url)))
            });
        let checked_at_ms = now_ms();
        match result {
            Ok(Some((new_version, file, asset_url)))
                if version::is_newer(&new_version, &current) =>
            {
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

    pub(crate) fn install(self: &Arc<Self>) -> Value {
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
        match self.platform {
            "win" => match install::launch_silent_detached(&pending.installer_path) {
                Ok(()) => self.status(),
                Err(error) => self.set_error(error, self.snapshot().checked_at_ms),
            },
            #[cfg(unix)]
            "mac" => self.start_mac_install(pending.installer_path.clone()),
            _ => self.set_error(
                UpdaterError::UnsupportedPlatform,
                self.snapshot().checked_at_ms,
            ),
        }
    }

    /// macOS apply (KOS-377): mount the DMG, stage the `.app` next to the
    /// running bundle, spawn the detached swap+relaunch helper. Runs on a
    /// spawned task — staging a ~40 MB app takes seconds — so the RPC
    /// returns immediately with the current status; failures land in
    /// `Phase::Error` for the UI.
    #[cfg(unix)]
    fn start_mac_install(self: &Arc<Self>, dmg: std::path::PathBuf) -> Value {
        if self.installing.swap(true, Ordering::SeqCst) {
            return self.status();
        }
        let this = self.clone();
        tokio::spawn(async move {
            if let Err(error) = macos::apply_update(&dmg).await {
                this.set_error(error, this.snapshot().checked_at_ms);
            }
            this.installing.store(false, Ordering::SeqCst);
        });
        self.status()
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

    /// Engine-owned update discovery (KOS-357): one check shortly after
    /// startup, then a recheck every `RECHECK_INTERVAL` for the life of the
    /// process. Runs unconditionally — desktop authority previously gated the
    /// startup check, so with Manager already connected a new release was
    /// never discovered without the manual button.
    pub async fn run_check_loop(self: Arc<Self>) {
        tokio::time::sleep(STARTUP_GRACE).await;
        loop {
            self.check_tick().await;
            tokio::time::sleep(RECHECK_INTERVAL).await;
        }
    }

    /// One iteration of `run_check_loop`. Skipped once an installer is
    /// downloaded — a recheck would only re-download the same payload and
    /// flicker the status; the next Engine restart discovers anything newer.
    /// An in-flight download already makes `check()` a no-op.
    async fn check_tick(self: &Arc<Self>) {
        if self.snapshot().phase != Phase::Downloaded {
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

#[cfg(all(test, unix))]
#[path = "service_tests_macos.rs"]
mod macos_tests;
