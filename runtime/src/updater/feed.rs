use super::release_manifest::{parse_release_manifest, FeedRelease};
use super::UpdaterError;

// KOS-304: releases live in makekosmos/cortex.
pub(crate) const DEFAULT_FEED_BASE: &str =
    "https://github.com/makekosmos/cortex/releases/latest/download";
/// KOS-350: the one release document.
pub(crate) const MANIFEST_FILE: &str = "manifest.json";

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

pub(crate) fn build_client() -> Result<reqwest::Client, UpdaterError> {
    engine_base::http::client_builder()
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

/// Newest release for `platform`. A manifest without an entry for it means
/// no installable update yet (`file: None`); the next check sees it once the
/// platform's build is merged in and the CDN refreshes (KOS-377).
pub(crate) async fn fetch_release(
    client: &reqwest::Client,
    feed_base: &str,
    platform: &str,
) -> Result<FeedRelease, UpdaterError> {
    let text = fetch_text(client, &format!("{feed_base}/{MANIFEST_FILE}")).await?;
    parse_release_manifest(&text, platform)
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
