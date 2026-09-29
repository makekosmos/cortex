// Native app operations (`apps.*` RPCs). The app list is hardcoded in
// `native_apps::NATIVE_APPS`; update checks and downloads go straight to
// each app's GitHub Releases (`native_apps::releases`) — the package-index
// catalog plays no part in native apps.

use std::time::{Duration, Instant};

use crate::native_apps::{self, NativeAppInstall};
use crate::native_apps::releases::{ReleaseError, ReleaseInfo, ReleaseProbe};

/// Manager-facing row for one native app.
#[derive(Debug, Clone, Serialize)]
pub struct NativeAppSummary {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub installed: bool,
    pub installed_version: Option<String>,
    /// Latest version GitHub Releases offers for this host's target.
    pub latest_version: Option<String>,
    /// Set when the release offers a newer version than the install.
    pub update_version: Option<String>,
    /// installed | update-available | not-installed | offline | unsupported
    pub state: String,
}

/// How long a fetched latest-release stays fresh before the next list call
/// revalidates it (ETag/If-None-Match makes revalidation cheap).
const RELEASE_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

pub(crate) struct CachedRelease {
    pub info: ReleaseInfo,
    pub etag: Option<String>,
    pub checked_at: Instant,
}

impl PackageService {
    fn native_store(&self) -> Result<&crate::native_apps::NativeAppStore, PackageError> {
        self.native_apps.as_ref().ok_or(PackageError::Invalid)
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
        let etag = {
            let cache = Self::lock(&self.release_cache);
            match cache.get(desc.id) {
                Some(cached) if cached.checked_at.elapsed() < RELEASE_CACHE_TTL && !force => {
                    return Ok(cached.info.clone())
                }
                Some(cached) => Some(cached.etag.clone()),
                None => None,
            }
        };
        match releases::fetch_latest(probe, desc, target, etag.flatten().as_deref()).await {
            Ok(releases::ReleaseCheck::Fresh { info, etag }) => {
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
                let mut cache = Self::lock(&self.release_cache);
                let cached = cache.get_mut(desc.id).ok_or(ReleaseError::Unavailable)?;
                cached.checked_at = Instant::now();
                Ok(cached.info.clone())
            }
            Err(error) => Err(error),
        }
    }

    /// Stale cache entry for offline fallback, regardless of age.
    fn cached_release(&self, id: &str) -> Option<ReleaseInfo> {
        Self::lock(&self.release_cache)
            .get(id)
            .map(|cached| cached.info.clone())
    }

    fn native_summary(
        desc: &'static native_apps::NativeAppDescriptor,
        record: Option<&NativeAppInstall>,
        latest: Option<&ReleaseInfo>,
        offline: bool,
        supported: bool,
    ) -> NativeAppSummary {
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
        let state = if !supported {
            "unsupported"
        } else if update_version.is_some() {
            "update-available"
        } else if record.is_some() {
            "installed"
        } else if offline && latest_version.is_none() {
            "offline"
        } else {
            "not-installed"
        };
        NativeAppSummary {
            id: desc.id.to_owned(),
            name: desc.name.to_owned(),
            icon: desc.icon.map(str::to_owned),
            installed: record.is_some(),
            installed_version,
            latest_version,
            update_version,
            state: state.to_owned(),
        }
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
        let probe = match ReleaseProbe::new() {
            Ok(probe) => probe,
            Err(_) => return Err(PackageError::Invalid),
        };
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
        Ok(native_apps::NATIVE_APPS
            .iter()
            .zip(checks)
            .map(|(desc, check)| {
                let record = store.current(desc.id);
                let (latest, offline) = match check {
                    Some(Ok(info)) => (Some(info), false),
                    // Offline/failed check: fall back to the stale cached
                    // version when one exists, else report no latest.
                    _ => (self.cached_release(desc.id), true),
                };
                Self::native_summary(
                    desc,
                    record.as_ref(),
                    latest.as_ref(),
                    offline,
                    target.is_some(),
                )
            })
            .collect())
    }
}

fn native_store_error(error: crate::native_apps::NativeAppError) -> PackageError {
    match error {
        crate::native_apps::NativeAppError::Running => PackageError::Worker("app-running"),
        crate::native_apps::NativeAppError::HashMismatch
        | crate::native_apps::NativeAppError::SizeMismatch => PackageError::Invalid,
        crate::native_apps::NativeAppError::Invalid(_) => PackageError::Invalid,
        _ => PackageError::Persistence,
    }
}
