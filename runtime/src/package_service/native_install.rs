// Native app install/update/uninstall — download from the app's own GitHub
// Releases, verify the archive against its `SHA256SUMS.txt` line, then the
// atomic extract/pointer-flip path in `native_apps`.
//
// Installs run as background jobs (`start_native_install`): the RPC returns
// a row with `state: "installing"` immediately and `apps.list` reports
// progress while the job downloads, verifies and extracts. One job per app
// — a second request (or the startup migration) gets `Busy`.

use std::sync::Arc;

use crate::native_apps::releases::{self, TempDownload};
use crate::native_apps::{NativeAppDescriptor, NativeInstallSpec};
use crate::package_store::eq_hash;

impl PackageService {
    /// Queue a background install/update for `desc` and return the row the
    /// Store should show right away. `Busy` while a job for this id runs;
    /// the job records its outcome into `native_jobs` for `apps.list`.
    pub fn start_native_install(
        self: &Arc<Self>,
        desc: &'static NativeAppDescriptor,
    ) -> Result<NativeAppSummary, PackageError> {
        let probe = ReleaseProbe::new().map_err(|_| PackageError::Offline)?;
        self.start_native_install_with(desc, probe)
    }

    pub(crate) fn start_native_install_with(
        self: &Arc<Self>,
        desc: &'static NativeAppDescriptor,
        probe: ReleaseProbe,
    ) -> Result<NativeAppSummary, PackageError> {
        let store = self.native_store()?;
        crate::native_apps::host_app_target().ok_or(PackageError::Unsupported)?;
        let Some(job) = self.claim_native_job(desc.id) else {
            return Err(PackageError::Busy);
        };
        let service = Arc::clone(self);
        // `job` moves into the task: a panic unwinds through its Drop and the
        // row still settles — `installing` can never outlive the install.
        tokio::spawn(async move {
            let result = service.run_native_install_with(&probe, desc, None).await;
            job.finish(&result);
        });
        // The claim is already held — the row reports Installing directly.
        Ok(self.native_summary(NativeRowInput {
            desc,
            record: store.current(desc.id).map_err(native_store_error)?.as_ref(),
            latest: self.cached_release(desc.id).as_ref(),
            availability: NativeAvailability::Ready,
            job: Some(NativeJob::Installing {
                downloaded: 0,
                total: None,
            }),
            icon_path: self.ensure_native_icon(&store, desc),
        }))
    }

    /// The job body — also the inline path for tests and the startup
    /// migration (`version` pins a tag; `None` installs the latest release).
    /// Callers must hold the per-id `NativeJobGuard` first.
    /// Dictation is Engine-managed, so its install goes through the
    /// stop → install → relaunch wrapper in `dictation_app.rs`.
    pub(crate) async fn run_native_install_with(
        &self,
        probe: &ReleaseProbe,
        desc: &'static NativeAppDescriptor,
        version: Option<&str>,
    ) -> Result<NativeAppSummary, PackageError> {
        if desc.id == DICTATION_APP_ID {
            return self.run_dictation_native_install(probe, desc, version).await;
        }
        self.run_native_install_inner(probe, desc, version).await
    }

    async fn run_native_install_inner(
        &self,
        probe: &ReleaseProbe,
        desc: &'static NativeAppDescriptor,
        version: Option<&str>,
    ) -> Result<NativeAppSummary, PackageError> {
        let store = self.native_store()?;
        let target = crate::native_apps::host_app_target().ok_or(PackageError::Unsupported)?;

        // Refuse early when the app is running — before any download —
        // because the pointer flip deletes the live tree.
        if store.app_is_running(desc.id).map_err(native_store_error)? {
            return Err(PackageError::AppRunning);
        }

        let current_version = || -> Result<Option<String>, PackageError> {
            Ok(store
                .current(desc.id)
                .map_err(native_store_error)?
                .map(|record| record.version))
        };

        let info = match version {
            Some(version) => {
                if semver::Version::parse(version).is_err() {
                    return Err(PackageError::Invalid);
                }
                // A pin matching the live version needs no network at all.
                if current_version()?.as_deref() == Some(version) {
                    return self.native_ok_summary(desc, None);
                }
                releases::fetch_tagged(probe, desc, &desc.release_tag(version), target)
                    .await
                    .map_err(map_release_error)?
            }
            // Same cached revalidation path `apps.list` uses, forced fresh —
            // an install must install what GitHub actually offers now.
            None => self
                .check_app_release(probe, desc, target, true)
                .await
                .map_err(map_release_error)?,
        };
        // Same-version reinstall is a no-op — the pointer already agrees.
        if current_version()?.as_deref() == Some(info.version.as_str()) {
            return self.native_ok_summary(desc, Some(&info));
        }

        let app_dir = store.root().join(desc.id);
        retry_io(|| fs::create_dir_all(&app_dir)).map_err(|_| PackageError::Persistence)?;
        // Unique per attempt — a concurrent or previous attempt can never
        // collide with (or truncate) this download. The guard deletes the
        // file on drop, including on cancellation or any failure below.
        let download = TempDownload::at(
            app_dir.join(crate::native_apps::unique_temp_name(".download-")),
        );
        let url = releases::asset_url(&probe.base, desc.repository, &info.tag, &info.asset);
        let id = desc.id;
        let (sha256, size) = releases::download(probe, &url, download.path(), &mut |done, total| {
            self.note_native_progress(id, done, total);
        })
        .await
        .map_err(map_release_error)?;
        if !eq_hash(&sha256, &info.sha256) {
            return Err(PackageError::Integrity);
        }
        let spec = NativeInstallSpec {
            id: desc.id.to_owned(),
            version: info.version.clone(),
            executable: desc.executable(target),
            sha256: info.sha256.clone(),
            size,
            repository: desc.repository.to_owned(),
            release_tag: info.tag.clone(),
        };
        // Zip parse + extract + rename retries are synchronous fs work —
        // off the async worker, inside the per-app job claim. The download
        // guard drops (and deletes the file) when the blocking task ends.
        tokio::task::spawn_blocking(move || {
            let result = store.install_archive(&spec, download.path());
            drop(download);
            result
        })
        .await
        .map_err(|_| PackageError::Persistence)?
        .map_err(native_store_error)?;
        self.native_ok_summary(desc, Some(&info))
    }

    /// The ready row after a successful install — a fresh read, not a
    /// hand-assembled copy of the spec. Every path that lands here proves
    /// the app is installed, so the Start-menu link is synced too.
    fn native_ok_summary(
        &self,
        desc: &'static NativeAppDescriptor,
        latest: Option<&releases::ReleaseInfo>,
    ) -> Result<NativeAppSummary, PackageError> {
        self.sync_native_shortcut(desc);
        let record = self
            .native_store()?
            .current(desc.id)
            .map_err(native_store_error)?;
        let store = self.native_store()?;
        let icon_path = self.ensure_native_icon(&store, desc);
        Ok(self.native_summary(NativeRowInput {
            desc,
            record: record.as_ref(),
            latest,
            availability: NativeAvailability::Ready,
            job: None,
            icon_path,
        }))
    }

    /// Uninstall removes the whole `<id>` dir — record and every version —
    /// refusing while the app is running. User data is never touched. The
    /// Start-menu link goes with it. Dictation is Engine-managed: the Engine
    /// stops the background process itself instead of making the Store
    /// refuse the uninstall.
    pub fn uninstall_native_app(&self, desc: &NativeAppDescriptor) -> Result<(), PackageError> {
        if desc.id == DICTATION_APP_ID && !self.stop_dictation_app() {
            return Err(PackageError::AppRunning);
        }
        self.native_store()?
            .uninstall(desc.id)
            .map_err(native_store_error)?;
        self.sync_native_shortcut(desc);
        Ok(())
    }

    /// The Engine owns store-app Start-menu links: `install`/`uninstall`
    /// keep one app's link in step, and `reconcile` — run on every Engine
    /// start — converges the whole table (pre-existing installs, manual
    /// deletes, update repoints). Best-effort: failures are logged, never
    /// surfaced to the caller's operation.
    fn sync_native_shortcut(&self, desc: &NativeAppDescriptor) {
        let (Some(store), Some(programs)) = (
            self.native_apps.as_deref(),
            crate::native_apps::start_menu_dir(),
        ) else {
            return;
        };
        if let Err(error) = crate::native_apps::sync_shortcut(store, &programs, desc) {
            tracing::warn!(
                target: "native_apps",
                id = desc.id,
                %error,
                "start-menu shortcut sync failed"
            );
        }
    }

    pub fn reconcile_native_shortcuts(&self) {
        let (Some(store), Some(programs)) = (
            self.native_apps.as_deref(),
            crate::native_apps::start_menu_dir(),
        ) else {
            return;
        };
        crate::native_apps::reconcile_shortcuts(store, &programs);
    }

    /// Absolute executable path for the installed app (env override first —
    /// development only), or `NotFound` when not installed.
    pub fn native_app_executable(
        &self,
        desc: &NativeAppDescriptor,
    ) -> Result<PathBuf, PackageError> {
        self.native_store()?
            .executable_for(desc)
            .ok_or(PackageError::NotFound)
    }

    /// Launch the installed app.
    pub fn open_native_app(&self, desc: &NativeAppDescriptor) -> Result<(), PackageError> {
        let executable = self.native_app_executable(desc)?;
        #[cfg(windows)]
        let mut command = {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let mut command = std::process::Command::new(&executable);
            command.creation_flags(CREATE_NO_WINDOW);
            command
        };
        #[cfg(not(windows))]
        let mut command = std::process::Command::new(&executable);
        command.spawn().map_err(|error| {
            tracing::warn!(
                target: "native_apps",
                executable = %executable.display(),
                %error,
                "native app launch failed"
            );
            PackageError::Persistence
        })?;
        Ok(())
    }
}

/// `ReleaseError` → the `apps.*`-mapped service error: unreachable endpoints
/// read as `offline`, protocol violations (bad redirect, oversized or
/// unverifiable bodies) read as `integrity`, io as `io`.
fn map_release_error(error: releases::ReleaseError) -> PackageError {
    match error {
        releases::ReleaseError::Unavailable => PackageError::Offline,
        releases::ReleaseError::Invalid(_) => PackageError::Integrity,
        releases::ReleaseError::Io(_) => PackageError::Persistence,
    }
}
