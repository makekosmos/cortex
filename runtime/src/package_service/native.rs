// Native app operations (`apps.*` RPCs). The app list is hardcoded in
// `native_apps::NATIVE_APPS`; update checks and downloads go straight to
// each app's GitHub Releases (`native_apps::releases`) — the package-index
// catalog plays no part in native apps.

use std::time::Duration;

use crate::native_apps::{self, NativeAppInstall};
use crate::native_apps::releases::{CachedReleaseMeta, ReleaseError, ReleaseInfo, ReleaseProbe};

/// Row state the Store renders for one native app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeAppState {
    Installed,
    UpdateAvailable,
    NotInstalled,
    /// A background install/update is running; `download_*` carry progress.
    Installing,
    /// The last background install failed; `failure` is the apps error code.
    Failed,
    /// The release check could not reach GitHub and nothing is cached.
    Offline,
    /// No published asset for this host target.
    Unsupported,
}

/// Manager-facing row for one native app.
#[derive(Debug, Clone, Serialize)]
pub struct NativeAppSummary {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub installed_version: Option<String>,
    /// Latest version GitHub Releases offers for this host's target.
    pub latest_version: Option<String>,
    /// Set when the release offers a newer version than the install.
    pub update_version: Option<String>,
    pub state: NativeAppState,
    /// Download progress while `state == installing` (bytes done so far and
    /// the announced content length).
    pub download_bytes: Option<u64>,
    pub download_total: Option<u64>,
    /// `apps.*` error code of the failed job while `state == failed`.
    pub failure: Option<&'static str>,
    /// Materialized product icon (`<Apps>/icons/<id>.png`) — the Manager
    /// renders it directly; `None` only when the write failed.
    pub icon_path: Option<String>,
}

/// In-flight or last-failed background install job for one app.
#[derive(Debug, Clone)]
pub(crate) enum NativeJob {
    Installing {
        downloaded: u64,
        total: Option<u64>,
    },
    /// `apps.*` error code the failed install produced.
    Failed(&'static str),
}

/// RAII handle for the per-app job claim: the holder must call [`finish`]
/// with the outcome. Dropping it unfinished — a panicked or aborted install
/// task — records `Failed` so `apps.list` can never show `installing`
/// forever.
pub(crate) struct NativeJobGuard {
    jobs: std::sync::Arc<Mutex<HashMap<&'static str, NativeJob>>>,
    id: &'static str,
    finished: bool,
}

impl NativeJobGuard {
    /// Record the outcome and release the claim: success clears the job,
    /// failure stores the wire code for the Store's failed-row badge.
    pub(crate) fn finish(mut self, result: &Result<NativeAppSummary, PackageError>) {
        self.finished = true;
        finish_native_job(&self.jobs, self.id, result);
    }
}

impl Drop for NativeJobGuard {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        tracing::warn!(
            target: "native_apps",
            id = self.id,
            "native install job dropped unfinished — recording failure"
        );
        PackageService::lock(&self.jobs).insert(self.id, NativeJob::Failed("unavailable"));
    }
}

fn finish_native_job(
    jobs: &Mutex<HashMap<&'static str, NativeJob>>,
    id: &'static str,
    result: &Result<NativeAppSummary, PackageError>,
) {
    let mut jobs = PackageService::lock(jobs);
    match result {
        Ok(_) => {
            jobs.remove(id);
        }
        Err(error) => {
            tracing::warn!(
                target: "native_apps",
                %id,
                %error,
                "native app install failed"
            );
            jobs.insert(id, NativeJob::Failed(native_error_code(error)));
        }
    }
}

/// The `apps.*` wire code for a service-layer failure — the mapping the
/// dispatcher and the Store's failed-row badge share.
pub(crate) fn native_error_code(error: &PackageError) -> &'static str {
    match error {
        PackageError::NotFound => "not-found",
        PackageError::Busy => "busy",
        PackageError::AppRunning => "app-running",
        PackageError::Offline => "offline",
        PackageError::Integrity => "integrity",
        PackageError::Unsupported => "unsupported",
        PackageError::Invalid => "invalid-request",
        PackageError::Persistence | PackageError::Store(_) => "io",
        _ => "unavailable",
    }
}

/// How long a fetched latest-release stays fresh before the next list call
/// revalidates it (ETag/If-None-Match makes revalidation cheap).
const RELEASE_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
/// A failed probe is cached briefly too — an offline Store open must not
/// wait out the per-request timeout for every app on every refresh.
const RELEASE_FAILURE_TTL: Duration = Duration::from_secs(60);

pub(crate) struct CachedRelease {
    pub info: ReleaseInfo,
    pub etag: Option<String>,
    pub checked_at: Instant,
}

/// What the release check concluded for this app on this pass — the single
/// availability axis callers declare; it replaced the old positional
/// `offline`/`supported` bool pair which could describe impossible states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeAvailability {
    /// This host has a target and the check answered (fresh or stale cache).
    Ready,
    /// This host has a target but the check failed; the row may still show
    /// a stale cached latest version.
    Offline,
    /// No published asset exists for this host target.
    Unsupported,
}

/// Everything `native_summary` needs to render one Store row — named
/// fields so call sites cannot pass the flags in the wrong order.
pub(crate) struct NativeRowInput<'a> {
    pub desc: &'static native_apps::NativeAppDescriptor,
    pub record: Option<&'a NativeAppInstall>,
    pub latest: Option<&'a ReleaseInfo>,
    pub availability: NativeAvailability,
    /// Live job snapshot — overrides the derived state.
    pub job: Option<NativeJob>,
    /// Materialized icon path (`ensure_native_icon`), if it landed.
    pub icon_path: Option<String>,
}

impl PackageService {
    fn native_store(&self) -> Result<std::sync::Arc<crate::native_apps::NativeAppStore>, PackageError> {
        self.native_apps.clone().ok_or(PackageError::Unsupported)
    }

    /// The current snapshot of an app's background job, if any.
    fn current_job(&self, id: &'static str) -> Option<NativeJob> {
        Self::lock(&self.native_jobs).get(id).cloned()
    }

    /// Claim the per-app in-flight slot, returning the guard that owns it.
    /// `None` while a job runs — install, update and the startup migration
    /// all funnel through this, so a user click can never race the
    /// migration's download.
    pub(crate) fn claim_native_job(&self, id: &'static str) -> Option<NativeJobGuard> {
        let mut jobs = Self::lock(&self.native_jobs);
        if matches!(jobs.get(id), Some(NativeJob::Installing { .. })) {
            return None;
        }
        jobs.insert(
            id,
            NativeJob::Installing {
                downloaded: 0,
                total: None,
            },
        );
        Some(NativeJobGuard {
            jobs: std::sync::Arc::clone(&self.native_jobs),
            id,
            finished: false,
        })
    }

    /// Publish download progress for the running job — cheap enough to do
    /// per chunk.
    fn note_native_progress(&self, id: &'static str, downloaded: u64, total: Option<u64>) {
        if let Some(job @ NativeJob::Installing { .. }) =
            Self::lock(&self.native_jobs).get_mut(id)
        {
            *job = NativeJob::Installing { downloaded, total };
        }
    }

    /// Latest release for one app, cached per id: fresh within
    /// `RELEASE_CACHE_TTL`, ETag-revalidated after. `Err` means "offline /
    /// unavailable" — the caller degrades to stale-or-empty, never fails.
    async fn check_app_release(
        &self,
        probe: &ReleaseProbe,
        desc: &'static native_apps::NativeAppDescriptor,
        target: &str,
        force: bool,
    ) -> Result<ReleaseInfo, ReleaseError> {
        // A recent failure short-circuits the retry — otherwise every Store
        // open while offline waits out the full probe timeout per app.
        if !force
            && Self::lock(&self.release_failures)
                .get(desc.id)
                .is_some_and(|failed_at| failed_at.elapsed() < RELEASE_FAILURE_TTL)
        {
            return Err(ReleaseError::Unavailable);
        }
        let cached_meta = {
            let cache = Self::lock(&self.release_cache);
            match cache.get(desc.id) {
                Some(cached) if cached.checked_at.elapsed() < RELEASE_CACHE_TTL && !force => {
                    return Ok(cached.info.clone())
                }
                Some(cached) => cached
                    .etag
                    .as_deref()
                    .map(|etag| CachedReleaseMeta {
                        tag: cached.info.tag.clone(),
                        etag: etag.to_owned(),
                    }),
                None => None,
            }
        };
        match releases::fetch_latest(probe, desc, target, cached_meta.as_ref()).await {
            Ok(releases::ReleaseCheck::Fresh { info, etag }) => {
                Self::lock(&self.release_failures).remove(desc.id);
                Self::lock(&self.release_cache).insert(
                    desc.id.to_owned(),
                    CachedRelease {
                        info: info.clone(),
                        etag,
                        checked_at: Instant::now(),
                    },
                );
                Ok(info)
            }
            Ok(releases::ReleaseCheck::NotModified) => {
                Self::lock(&self.release_failures).remove(desc.id);
                let mut cache = Self::lock(&self.release_cache);
                let cached = cache.get_mut(desc.id).ok_or(ReleaseError::Unavailable)?;
                cached.checked_at = Instant::now();
                Ok(cached.info.clone())
            }
            Err(error) => {
                Self::lock(&self.release_failures)
                    .insert(desc.id.to_owned(), Instant::now());
                Err(error)
            }
        }
    }

    /// Stale cache entry for offline fallback, regardless of age.
    fn cached_release(&self, id: &str) -> Option<ReleaseInfo> {
        Self::lock(&self.release_cache)
            .get(id)
            .map(|cached| cached.info.clone())
    }

    /// `job` overrides the derived state — `apps.list` passes the live job
    /// snapshot, install paths pass the outcome they just produced.
    fn native_summary(&self, input: NativeRowInput<'_>) -> NativeAppSummary {
        let NativeRowInput {
            desc,
            record,
            latest,
            availability,
            job,
            icon_path,
        } = input;
        let installed_version = record.map(|record| record.version.clone());
        let latest_version = latest.map(|info| info.version.clone());
        let update_version = match (&installed_version, &latest_version) {
            (Some(installed), Some(latest)) => {
                match (
                    semver::Version::parse(installed),
                    semver::Version::parse(latest),
                ) {
                    (Ok(installed), Ok(latest)) if latest > installed => Some(latest.to_string()),
                    _ => None,
                }
            }
            _ => None,
        };
        let (job_state, progress, failure) = match job {
            Some(NativeJob::Installing { downloaded, total }) => {
                (Some(NativeAppState::Installing), Some((downloaded, total)), None)
            }
            Some(NativeJob::Failed(code)) => (Some(NativeAppState::Failed), None, Some(code)),
            None => (None, None, None),
        };
        let state = match job_state {
            Some(state) => state,
            None => match availability {
                NativeAvailability::Unsupported => NativeAppState::Unsupported,
                _ if update_version.is_some() => NativeAppState::UpdateAvailable,
                _ if record.is_some() => NativeAppState::Installed,
                NativeAvailability::Offline if latest_version.is_none() => {
                    NativeAppState::Offline
                }
                _ => NativeAppState::NotInstalled,
            },
        };
        NativeAppSummary {
            id: desc.id.to_owned(),
            name: desc.name.to_owned(),
            installed: record.is_some(),
            installed_version,
            latest_version,
            update_version,
            state,
            download_bytes: progress.map(|(bytes, _)| bytes),
            download_total: progress.and_then(|(_, total)| total),
            failure,
            icon_path,
        }
    }

    /// Materialize the descriptor's embedded product icon under
    /// `<Apps>/icons/<id>.png` and return its path for the Manager's `img`.
    /// The PNG lives next to the app records — `list()` skips it (no
    /// `install.json`) and `uninstall` only removes `<Apps>/<id>`. Rewrite
    /// when bytes drift so an icon refresh in a new build takes effect.
    /// `None` on io failure — the Store falls back to a letter placeholder.
    fn ensure_native_icon(
        &self,
        store: &crate::native_apps::NativeAppStore,
        desc: &native_apps::NativeAppDescriptor,
    ) -> Option<String> {
        let path = store.root().join("icons").join(format!("{}.png", desc.id));
        if fs::read(&path).ok().as_deref() != Some(desc.icon_png) {
            let write = fs::create_dir_all(path.parent()?)
                .and_then(|()| fs::write(&path, desc.icon_png));
            if let Err(error) = write {
                tracing::warn!(
                    target: "native_apps",
                    id = desc.id,
                    %error,
                    "app icon materialization failed"
                );
                return None;
            }
        }
        let path = path.to_string_lossy();
        Some(
            path.strip_prefix(r"\\?\")
                .unwrap_or(path.as_ref())
                .to_owned(),
        )
    }

    /// One row per hardcoded app. The release probe runs inline (bounded by
    /// per-request timeouts) so the reply reflects the current latest
    /// versions; offline or unsupported hosts get rows without them.
    /// `force` skips the TTL and revalidates each app over the wire.
    pub async fn native_apps(&self) -> Result<Vec<NativeAppSummary>, PackageError> {
        self.native_apps_refresh(false).await
    }

    pub async fn native_apps_refresh(
        &self,
        force: bool,
    ) -> Result<Vec<NativeAppSummary>, PackageError> {
        let probe = ReleaseProbe::new().map_err(|_| PackageError::Offline)?;
        self.native_apps_with(&probe, force).await
    }

    pub(crate) async fn native_apps_with(
        &self,
        probe: &ReleaseProbe,
        force: bool,
    ) -> Result<Vec<NativeAppSummary>, PackageError> {
        let store = self.native_store()?;
        let target = native_apps::host_app_target();
        let checks: Vec<Option<Result<ReleaseInfo, ReleaseError>>> = match target {
            Some(target) => {
                futures_util::future::join_all(native_apps::NATIVE_APPS.iter().map(|desc| {
                    self.check_app_release(probe, desc, target, force)
                }))
                .await
                .into_iter()
                .map(Some)
                .collect()
            }
            None => native_apps::NATIVE_APPS.iter().map(|_| None).collect(),
        };
        let mut rows = Vec::with_capacity(native_apps::NATIVE_APPS.len());
        for (desc, check) in native_apps::NATIVE_APPS.iter().zip(checks) {
            let record = store.current(desc.id).map_err(native_store_error)?;
            let (latest, offline) = match check {
                Some(Ok(info)) => (Some(info), false),
                // Offline/failed check: fall back to the stale cached
                // version when one exists, else report no latest.
                _ => (self.cached_release(desc.id), true),
            };
            rows.push(self.native_summary(NativeRowInput {
                desc,
                record: record.as_ref(),
                latest: latest.as_ref(),
                availability: if target.is_none() {
                    NativeAvailability::Unsupported
                } else if offline {
                    NativeAvailability::Offline
                } else {
                    NativeAvailability::Ready
                },
                job: self.current_job(desc.id),
                icon_path: self.ensure_native_icon(&store, desc),
            }));
        }
        Ok(rows)
    }
}

pub(crate) fn native_store_error(error: crate::native_apps::NativeAppError) -> PackageError {
    match error {
        crate::native_apps::NativeAppError::Running => PackageError::AppRunning,
        crate::native_apps::NativeAppError::HashMismatch
        | crate::native_apps::NativeAppError::SizeMismatch => PackageError::Integrity,
        crate::native_apps::NativeAppError::Invalid(_) => PackageError::Invalid,
        crate::native_apps::NativeAppError::Io(_) => PackageError::Persistence,
        _ => PackageError::Persistence,
    }
}
