use super::*;
use httpmock::MockServer;

const MANIFEST: &str = r#"{"schema":"mundus-release-manifest","schema_version":1,
  "version":"0.6.0","platforms":{"win":{"file":"Mundus-Setup-0.6.0.exe","size":10,
  "sha512":"MANIFEST=="},"mac":{"file":"Mundus-0.6.0.dmg","size":7,"sha512":"MAC=="}}}"#;

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
async fn win_reads_its_own_manifest_entry() {
    let server = MockServer::start_async().await;
    let manifest = mock(&server, "/manifest.json", 200, MANIFEST).await;
    let release = fetch_release(&build_client().unwrap(), &server.base_url(), "win")
        .await
        .unwrap();
    assert_eq!(release.version, "0.6.0");
    let file = release.file.unwrap();
    assert_eq!(file.url, "Mundus-Setup-0.6.0.exe");
    assert_eq!(file.sha512, "MANIFEST==");
    manifest.assert_async().await;
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
async fn surfaces_non_success_status_as_network_error() {
    let server = MockServer::start_async().await;
    mock(&server, "/manifest.json", 404, "").await;
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

#[tokio::test]
async fn a_manifest_missing_the_platform_entry_reports_no_update() {
    let server = MockServer::start_async().await;
    let win_only = r#"{"schema":"mundus-release-manifest","schema_version":1,
      "version":"0.10.11","platforms":{"win":{"file":"Mundus-Setup-0.10.11.exe",
      "size":10,"sha512":"WIN=="}}}"#;
    mock(&server, "/manifest.json", 200, win_only).await;
    let release = fetch_release(&build_client().unwrap(), &server.base_url(), "mac")
        .await
        .unwrap();
    assert_eq!(release.version, "0.10.11");
    assert!(release.file.is_none());
}
