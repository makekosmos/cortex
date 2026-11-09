//! The catalog: which plugins exist.
//!
//! A JSON file at a URL — nothing in the Engine lists plugins, so publishing a
//! plugin is publishing a manifest. The file is cached in the Engine's data
//! directory: a fresh copy answers without the network, and when the network is
//! gone the cached copy still answers (marked `stale`) instead of leaving the
//! person with an empty page.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::manifest::{PluginManifest, MANIFEST_VERSION};
use super::PluginError;

/// Where the product's catalog lives unless `MUNDUS_PLUGIN_CATALOG` says
/// otherwise (a fork, a mirror, a test server).
pub const DEFAULT_URL: &str =
    "https://raw.githubusercontent.com/makekosmos/cortex/main/plugins/catalog.json";

const CACHE_FILE: &str = "plugins-catalog.json";
/// How long a fetched catalog answers before it is fetched again.
const FRESH_FOR: Duration = Duration::from_secs(6 * 60 * 60);
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
/// A catalog is a list of manifests; anything larger is not one.
const MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub plugins: Vec<PluginManifest>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cached {
    schema_version: u32,
    fetched_at: u64,
    url: String,
    catalog: Catalog,
}

impl Catalog {
    /// Parsing without the network, for the tests that guard the published file.
    #[cfg(test)]
    pub fn parse_for_test(body: &str) -> Result<Self, PluginError> {
        Self::parse(body)
    }

    fn parse(body: &str) -> Result<Self, PluginError> {
        let catalog: Catalog = serde_json::from_str(body)
            .map_err(|error| PluginError::Catalog(format!("описание не разобрано: {error}")))?;
        if catalog.schema_version != MANIFEST_VERSION {
            return Err(PluginError::Catalog(format!(
                "формат каталога {} — движок понимает {MANIFEST_VERSION}",
                catalog.schema_version
            )));
        }
        for plugin in &catalog.plugins {
            plugin.validate().map_err(|error| {
                PluginError::Catalog(format!("плагин «{}»: {error}", plugin.id))
            })?;
        }
        Ok(catalog)
    }
}

/// The catalog's URL: the product's, or the one this machine was pointed at.
pub fn url() -> String {
    crate::brand::env("PLUGIN_CATALOG")
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
}

fn cache_path() -> Result<PathBuf, PluginError> {
    let dir = crate::data_dir::mundus_data_dir()
        .map_err(|error| PluginError::Catalog(format!("нет каталога данных: {error}")))?;
    Ok(dir.join(CACHE_FILE))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn read_cache(url: &str) -> Option<Cached> {
    let bytes = fs::read(cache_path().ok()?).ok()?;
    let cached: Cached = serde_json::from_slice(&bytes).ok()?;
    (cached.url == url && cached.schema_version == MANIFEST_VERSION).then_some(cached)
}

fn write_cache(url: &str, catalog: &Catalog) {
    let Some(path) = cache_path().ok() else {
        return;
    };
    let cached = Cached {
        schema_version: MANIFEST_VERSION,
        fetched_at: now(),
        url: url.to_owned(),
        catalog: catalog.clone(),
    };
    let Ok(bytes) = serde_json::to_vec(&cached) else {
        return;
    };
    let temp = path.with_extension(format!("json.{}", crate::native_apps::unique_temp_name("")));
    if fs::write(&temp, &bytes).is_ok() && fs::rename(&temp, &path).is_err() {
        let _ = fs::remove_file(&temp);
    }
}

/// The catalog as the pages get it: the manifests, when they were fetched, and
/// whether the network answered at all.
#[derive(Debug, Clone, Serialize)]
pub struct Answer {
    pub catalog: Catalog,
    pub fetched_at: u64,
    pub stale: bool,
    pub url: String,
}

fn answer(catalog: Catalog, fetched_at: u64, stale: bool) -> Answer {
    Answer {
        catalog,
        fetched_at,
        stale,
        url: url(),
    }
}

/// Fetch the catalog: a cached copy younger than [`FRESH_FOR`] answers at once,
/// otherwise the URL is asked and the cache is refreshed; a failed fetch falls
/// back to the cache, however old.
pub async fn fetch() -> Result<Answer, PluginError> {
    let url = url();
    let cached = read_cache(&url);
    if let Some(cached) = &cached {
        if now().saturating_sub(cached.fetched_at) < FRESH_FOR.as_secs() {
            return Ok(answer(cached.catalog.clone(), cached.fetched_at, false));
        }
    }
    match download(&url).await {
        Ok(catalog) => {
            write_cache(&url, &catalog);
            Ok(answer(catalog, now(), false))
        }
        Err(error) => match cached {
            Some(cached) => {
                tracing::warn!(
                    target: "plugins",
                    %error,
                    %url,
                    "catalog fetch failed; serving the cached copy"
                );
                Ok(answer(cached.catalog, cached.fetched_at, true))
            }
            None => Err(error),
        },
    }
}

async fn download(url: &str) -> Result<Catalog, PluginError> {
    if !url.starts_with("https://") && !url.starts_with("http://127.0.0.1") {
        return Err(PluginError::Catalog("нужен https".into()));
    }
    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .user_agent(crate::brand::ENGINE_BINARY_STEM)
        .build()
        .map_err(|error| PluginError::Catalog(error.to_string()))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| PluginError::Catalog(error.to_string()))?;
    if !response.status().is_success() {
        return Err(PluginError::Catalog(format!("ответ {}", response.status())));
    }
    let body = response
        .text()
        .await
        .map_err(|error| PluginError::Catalog(error.to_string()))?;
    if body.len() > MAX_BYTES {
        return Err(PluginError::Catalog("каталог слишком большой".into()));
    }
    Catalog::parse(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(plugins: &str) -> String {
        format!(r#"{{"schema_version": 1, "plugins": [{plugins}]}}"#)
    }

    fn dictation() -> &'static str {
        r#"{
            "schema_version": 1,
            "id": "com.kosmos.dictation",
            "name": "Диктовка",
            "summary": "Речь в текст",
            "source": { "kind": "release", "repository": "makekosmos/dictation",
                        "tag_prefix": "gpui-v", "asset": "dictation-gpui" },
            "settings_file": "dictation-config.json",
            "settings": [
                { "key": "hotkey", "label": "Клавиша", "kind": { "type": "text" } }
            ]
        }"#
    }

    #[test]
    fn a_catalog_parses_into_manifests() {
        let catalog = Catalog::parse(&body(dictation())).expect("catalog");
        assert_eq!(catalog.plugins.len(), 1);
        assert_eq!(catalog.plugins[0].id, "com.kosmos.dictation");
        assert_eq!(catalog.plugins[0].settings[0].key, "hotkey");
    }

    #[test]
    fn a_newer_catalog_format_is_refused() {
        let text = body(dictation()).replace("\"schema_version\": 1", "\"schema_version\": 2");
        assert!(Catalog::parse(&text).is_err());
    }

    #[test]
    fn a_catalog_naming_a_broken_plugin_is_refused_whole() {
        let text = body(
            &dictation()
                .replace("\"komosmos\"", "x")
                .replace("\"name\": \"Диктовка\"", "\"name\": \"\""),
        );
        let error = Catalog::parse(&text).expect_err("empty name");
        assert!(matches!(error, PluginError::Catalog(_)));
    }

    #[test]
    fn an_unknown_field_in_the_catalog_is_refused() {
        let text = body(dictation()).replace("\"plugins\"", "\"plugin\": [], \"plugins\"");
        assert!(Catalog::parse(&text).is_err());
    }

    #[test]
    fn the_url_can_be_pointed_somewhere_else() {
        // The override is the product's escape hatch; the default is HTTPS.
        assert!(DEFAULT_URL.starts_with("https://"));
        assert!(url().starts_with("https://") || url().starts_with("http://127.0.0.1"));
    }
}
