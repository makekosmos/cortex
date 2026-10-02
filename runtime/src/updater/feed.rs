use super::manifest::{parse_latest_yml, LatestManifest};
use super::UpdaterError;

// KOS-304: releases moved to makekosmos/cortex; makekosmos/desktop only
// carries the one-time 0.10.1 bridge release for pre-0.10.1 clients.
pub(crate) const DEFAULT_FEED_BASE: &str =
    "https://github.com/makekosmos/cortex/releases/latest/download";
const CHANNEL_FILE: &str = "latest.yml";

pub(crate) fn build_client() -> Result<reqwest::Client, UpdaterError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| UpdaterError::Network(error.to_string()))
}

pub(crate) async fn fetch_manifest(
    client: &reqwest::Client,
    feed_base: &str,
) -> Result<LatestManifest, UpdaterError> {
    let url = format!("{feed_base}/{CHANNEL_FILE}");
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| UpdaterError::Network(error.to_string()))?;
    if !response.status().is_success() {
        return Err(UpdaterError::Network(format!(
            "GET {url}: HTTP {}",
            response.status()
        )));
    }
    let text = response
        .text()
        .await
        .map_err(|error| UpdaterError::Network(error.to_string()))?;
    parse_latest_yml(&text)
}

pub(crate) fn asset_url(feed_base: &str, filename: &str) -> Result<String, UpdaterError> {
    let valid = !filename.is_empty()
        && filename.ends_with(".exe")
        && filename
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if !valid {
        return Err(UpdaterError::MalformedManifest);
    }
    Ok(format!("{feed_base}/{filename}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::MockServer;

    #[tokio::test]
    async fn fetches_and_parses_the_channel_file() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET).path("/latest.yml");
                then.status(200).body(concat!(
                    "version: 0.5.3\nfiles:\n  - url: Mundus-Setup-0.5.3.exe\n    sha512: AAA\n  ",
                    "  size: 10\n"
                ));
            })
            .await;
        let manifest = fetch_manifest(&build_client().unwrap(), &server.base_url())
            .await
            .unwrap();
        assert_eq!(manifest.version, "0.5.3");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn surfaces_non_success_status_as_network_error() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET).path("/latest.yml");
                then.status(404);
            })
            .await;
        let error = fetch_manifest(&build_client().unwrap(), &server.base_url())
            .await
            .unwrap_err();
        assert!(matches!(error, UpdaterError::Network(_)));
    }

    #[test]
    fn default_feed_reads_cortex_releases() {
        assert_eq!(
            DEFAULT_FEED_BASE,
            "https://github.com/makekosmos/cortex/releases/latest/download"
        );
    }

    #[test]
    fn asset_url_accepts_only_safe_executable_filenames() {
        assert_eq!(
            asset_url("https://example.test/dl", "Mundus-Setup-0.5.3.exe").unwrap(),
            "https://example.test/dl/Mundus-Setup-0.5.3.exe"
        );
        for filename in ["../evil.exe", "dir/evil.exe", "evil.exe?x=1", "notes.txt"] {
            assert!(asset_url("https://example.test/dl", filename).is_err());
        }
    }
}
