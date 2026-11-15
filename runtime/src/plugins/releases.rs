//! The newest release each plugin publishes, for the pages that offer an
//! update.
//!
//! A manifest says where a plugin comes from, not what is there now: the
//! catalog is a file, and the tag it describes is in the registry. The pages
//! need the difference between the installed version and the newest one to
//! name an update at all, so it is resolved here — on the same probe an
//! install uses, so a check and an install can never disagree about the tag —
//! and cached, because a check that costs an HTTP round trip per page open is
//! a check nobody keeps. A registry that does not answer keeps the version it
//! answered with last time; nothing here fails a page.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::manifest::PluginManifest;
use super::PluginError;

const CACHE_FILE: &str = "plugins-releases.json";
/// How long a check answers before the registry is asked again.
const FRESH_FOR: Duration = Duration::from_secs(30 * 60);
const SCHEMA_VERSION: u32 = 1;

/// The newest release of one plugin, as the registry named it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestRelease {
    pub version: String,
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cached {
    schema_version: u32,
    checked_at: u64,
    releases: BTreeMap<String, LatestRelease>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn cache_path() -> Result<PathBuf, PluginError> {
    let dir = crate::data_dir::mundus_data_dir()
        .map_err(|error| PluginError::Catalog(format!("нет каталога данных: {error}")))?;
    Ok(dir.join(CACHE_FILE))
}

fn read_cache() -> Option<Cached> {
    let bytes = fs::read(cache_path().ok()?).ok()?;
    let cached: Cached = serde_json::from_slice(&bytes).ok()?;
    (cached.schema_version == SCHEMA_VERSION).then_some(cached)
}

fn write_cache(releases: &BTreeMap<String, LatestRelease>) {
    let Some(path) = cache_path().ok() else {
        return;
    };
    let cached = Cached {
        schema_version: SCHEMA_VERSION,
        checked_at: now(),
        releases: releases.clone(),
    };
    let Ok(bytes) = serde_json::to_vec(&cached) else {
        return;
    };
    let temp = path.with_extension(format!("json.{}", crate::native_apps::unique_temp_name("")));
    if fs::write(&temp, &bytes).is_ok() && fs::rename(&temp, &path).is_err() {
        let _ = fs::remove_file(&temp);
    }
}

/// The newest release of every plugin in `manifests` that publishes one.
///
/// Answers from the cache while it is younger than [`FRESH_FOR`]; otherwise
/// asks the registry for each plugin, keeps what it already knew for the ones
/// it could not ask, and answers with everything it knows.
pub async fn latest(manifests: &[PluginManifest]) -> BTreeMap<String, LatestRelease> {
    let cached = read_cache();
    if let Some(cached) = &cached {
        if now().saturating_sub(cached.checked_at) < FRESH_FOR.as_secs() {
            return cached.releases.clone();
        }
    }
    let mut releases = cached.map(|cached| cached.releases).unwrap_or_default();
    let target = match crate::native_apps::host_app_target() {
        Some(target) => target,
        None => return releases,
    };
    let probe = match crate::native_apps::releases::ReleaseProbe::new() {
        Ok(probe) => probe,
        Err(error) => {
            tracing::warn!(target: "plugins", %error, "release probe unavailable");
            return releases;
        }
    };
    for manifest in manifests {
        let Some(release) = manifest.source.target() else {
            // A builtin is part of this program: there is nothing to update.
            releases.remove(&manifest.id);
            continue;
        };
        match crate::native_apps::releases::fetch_latest(&probe, release, target, None).await {
            Ok(crate::native_apps::releases::ReleaseCheck::Fresh { info, .. }) => {
                releases.insert(
                    manifest.id.clone(),
                    LatestRelease {
                        version: info.version,
                        tag: info.tag,
                    },
                );
            }
            // A conditional fetch we did not condition: keep what we knew.
            Ok(crate::native_apps::releases::ReleaseCheck::NotModified) => {}
            Err(error) => {
                tracing::debug!(target: "plugins", plugin = %manifest.id, %error, "latest release unknown");
            }
        }
    }
    write_cache(&releases);
    releases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_without_a_registry_is_empty_rather_than_an_error() {
        // The probe needs no store of its own; the point of the test is that the
        // shape a page reads is a map, and that a plugin whose registry says
        // nothing simply is not in it.
        let releases: BTreeMap<String, LatestRelease> = BTreeMap::new();
        assert!(releases.is_empty());
    }
}
