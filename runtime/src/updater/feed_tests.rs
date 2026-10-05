use super::*;
use httpmock::MockServer;

const MANIFEST: &str = r#"{"schema":"mundus-release-manifest","schema_version":1,
  "version":"0.6.0","platforms":{"win":{"file":"Mundus-Setup-0.6.0.exe","size":10,
  "sha512":"MANIFEST=="},"mac":{"file":"Mundus-0.6.0.dmg","size":7,"sha512":"MAC=="}}}"#;
const LEGACY: &str = concat!(
    "version: 0.5.3\nfiles:\n  - url: Mundus-Setup-0.5.3.exe\n    sha512: AAA\n  ",
    "  size: 10\n"
);

async fn mock<'a>(
    server: &'a MockServer,
    path: &str,
    status: u16,
    body: &str,
) -> httpmock::Mock<'a> {
    let body = body.to_owned();
    let path = path.to_owned();
    server
        .mock_async(move |when, then| {
            when.method(httpmock::Method::GET).path(path);
            then.status(status).body(body);
        })
        .await
}

#[tokio::test]
async fn prefers_manifest_json_over_the_legacy_channel_file() {
    let server = MockServer::start_async().await;
    let manifest = mock(&server, "/manifest.json", 200, MANIFEST).await;
    let legacy = mock(&server, "/latest.yml", 200, LEGACY).await;
    let release = fetch_release(&build_client().unwrap(), &server.base_url(), "win")
        .await
        .unwrap();
    assert_eq!(release.version, "0.6.0");
    let file = release.file.unwrap();
    assert_eq!(file.url, "Mundus-Setup-0.6.0.exe");
    assert_eq!(file.sha512, "MANIFEST==");
    manifest.assert_async().await;
    legacy.assert_calls_async(0).await;
}

#[tokio::test]
async fn mac_reads_its_own_manifest_entry() {
    let server = MockServer::start_async().await;
    mock(&server, "/manifest.json", 200, MANIFEST).await;
    let release = fetch_release(&build_client().unwrap(), &server.base_url(), "mac")
        .await
        .unwrap();
    assert_eq!(release.file.unwrap().url, "Mundus-0.6.0.dmg");
}

#[tokio::test]
async fn falls_back_to_latest_yml_while_dual_publish_is_on() {
    let server = MockServer::start_async().await;
    mock(&server, "/manifest.json", 404, "").await;
    mock(&server, "/latest.yml", 200, LEGACY).await;
    let release = fetch_release_with(&build_client().unwrap(), &server.base_url(), "win", true)
        .await
        .unwrap();
    assert_eq!(release.version, "0.5.3");
    assert_eq!(release.file.unwrap().url, "Mundus-Setup-0.5.3.exe");
}

#[tokio::test]
async fn an_unreadable_manifest_also_falls_back_during_dual_publish() {
    let server = MockServer::start_async().await;
    mock(&server, "/manifest.json", 200, "{\"schema_version\":99}").await;
    mock(
        &server,
        "/latest-mac.yml",
        200,
        &LEGACY.replace(".exe", ".dmg"),
    )
    .await;
    let release = fetch_release_with(&build_client().unwrap(), &server.base_url(), "mac", true)
        .await
        .unwrap();
    assert_eq!(release.file.unwrap().url, "Mundus-Setup-0.5.3.dmg");
}

#[tokio::test]
async fn after_cutover_a_missing_manifest_is_an_error_even_with_latest_yml() {
    let server = MockServer::start_async().await;
    mock(&server, "/manifest.json", 404, "").await;
    let legacy = mock(&server, "/latest.yml", 200, LEGACY).await;
    let error = fetch_release_with(&build_client().unwrap(), &server.base_url(), "win", false)
        .await
        .unwrap_err();
    assert!(matches!(error, UpdaterError::Network(_)));
    legacy.assert_calls_async(0).await;
}

#[tokio::test]
async fn surfaces_non_success_status_as_network_error() {
    let server = MockServer::start_async().await;
    mock(&server, "/manifest.json", 404, "").await;
    mock(&server, "/latest.yml", 404, "").await;
    let error = fetch_release(&build_client().unwrap(), &server.base_url(), "win")
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
    assert_eq!(MANIFEST_FILE, "manifest.json");
}

#[test]
fn host_platform_is_a_manifest_key() {
    assert!(["win", "mac", "linux"].contains(&host_platform()));
}

#[test]
fn asset_url_accepts_only_safe_installer_filenames_for_the_platform() {
    assert_eq!(
        asset_url("https://example.test/dl", "Mundus-Setup-0.5.3.exe", "win").unwrap(),
        "https://example.test/dl/Mundus-Setup-0.5.3.exe"
    );
    assert_eq!(
        asset_url("https://example.test/dl", "Mundus-0.5.3.dmg", "mac").unwrap(),
        "https://example.test/dl/Mundus-0.5.3.dmg"
    );
    for filename in [
        "../evil.exe",
        "dir/evil.exe",
        "evil.exe?x=1",
        "notes.txt",
        ".exe",
    ] {
        assert!(asset_url("https://example.test/dl", filename, "win").is_err());
    }
    assert!(asset_url("https://example.test/dl", "Mundus-0.5.3.dmg", "win").is_err());
    assert!(asset_url("https://example.test/dl", "Mundus-Setup-0.5.3.exe", "mac").is_err());
    assert!(asset_url("https://example.test/dl", "Mundus-0.5.3.exe", "linux").is_err());
}
