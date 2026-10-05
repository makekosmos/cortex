use super::manifest::parse_latest_yml;
use super::release_manifest::{parse_release_manifest, FeedRelease};
use super::UpdaterError;

// KOS-304: releases live in makekosmos/cortex.
pub(crate) const DEFAULT_FEED_BASE: &str =
    "https://github.com/makekosmos/cortex/releases/latest/download";
/// KOS-350: the one release document; read first.
pub(crate) const MANIFEST_FILE: &str = "manifest.json";
/// Dual-publish window (KOS-350): releases still also carry the legacy
/// `latest.yml` / `latest-mac.yml`, so a missing or unreadable
/// `manifest.json` falls back to them. Flip to `false` (and later drop the
/// yml parser) together with `DUAL_PUBLISH_LEGACY_FEEDS` in
/// `desktop/scripts/release-manifest.mjs` at cutover.
pub(crate) const LEGACY_FEED_FALLBACK: bool = true;

/// `platforms` key in `manifest.json` for the OS this Engine runs on.
pub(crate) fn host_platform() -> &'static str {
    if cfg!(windows) {
        "win"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn legacy_channel_file(platform: &str) -> Option<&'static str> {
    match platform {
        "win" => Some("latest.yml"),
        "mac" => Some("latest-mac.yml"),
        _ => None,
    }
}

pub(crate) fn build_client() -> Result<reqwest::Client, UpdaterError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| UpdaterError::Network(error.to_string()))
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String, UpdaterError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| UpdaterError::Network(error.to_string()))?;
    if !response.status().is_success() {
        return Err(UpdaterError::Network(format!(
            "GET {url}: HTTP {}",
            response.status()
        )));
    }
    response
        .text()
        .await
        .map_err(|error| UpdaterError::Network(error.to_string()))
}

/// Newest release for `platform`: `manifest.json` first, then (dual-publish
/// only) the legacy channel yml.
pub(crate) async fn fetch_release(
    client: &reqwest::Client,
    feed_base: &str,
    platform: &str,
) -> Result<FeedRelease, UpdaterError> {
    fetch_release_with(client, feed_base, platform, LEGACY_FEED_FALLBACK).await
}

async fn fetch_release_with(
    client: &reqwest::Client,
    feed_base: &str,
    platform: &str,
    legacy_fallback: bool,
) -> Result<FeedRelease, UpdaterError> {
    let primary = fetch_text(client, &format!("{feed_base}/{MANIFEST_FILE}"))
        .await
        .and_then(|text| parse_release_manifest(&text, platform));
    let error = match primary {
        Ok(release) => return Ok(release),
        Err(error) => error,
    };
    let Some(channel) = legacy_channel_file(platform).filter(|_| legacy_fallback) else {
        return Err(error);
    };
    tracing::info!(%error, channel, "manifest.json unavailable; using legacy update feed");
    let legacy = parse_latest_yml(&fetch_text(client, &format!("{feed_base}/{channel}")).await?)?;
    Ok(FeedRelease {
        file: Some(legacy.primary_file()?.clone()),
        version: legacy.version,
    })
}

/// Joins a manifest filename onto the feed base. Only a plain installer name
/// with the platform's installer extension is accepted.
pub(crate) fn asset_url(
    feed_base: &str,
    filename: &str,
    platform: &str,
) -> Result<String, UpdaterError> {
    let extension = match platform {
        "win" => ".exe",
        "mac" => ".dmg",
        _ => return Err(UpdaterError::MalformedManifest),
    };
    let valid = filename.len() > extension.len()
        && filename.ends_with(extension)
        && !filename.starts_with('.')
        && filename
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if !valid {
        return Err(UpdaterError::MalformedManifest);
    }
    Ok(format!("{feed_base}/{filename}"))
}

#[cfg(test)]
#[path = "feed_tests.rs"]
mod tests;
