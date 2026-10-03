use super::*;
use base64::Engine as _;
use httpmock::MockServer;
use sha2::{Digest, Sha512};

fn hash(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes))
}

async fn service_with_manifest(
    manifest: String,
    current_version: &str,
) -> (tempfile::TempDir, Arc<UpdaterService>) {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/latest.yml");
            then.status(200).body(manifest);
        })
        .await;
    let dir = tempfile::tempdir().unwrap();
    let service = UpdaterService::with_feed_base(
        dir.path().to_path_buf(),
        server.base_url(),
        current_version,
    );
    (dir, service)
}

#[tokio::test]
async fn equal_and_older_versions_are_not_available() {
    for candidate in ["0.5.0", "0.0.1"] {
        let manifest =
            format!("version: {candidate}\nfiles:\n  - url: a.exe\n    sha512: AAA\n    size: 1\n");
        let (_dir, service) = service_with_manifest(manifest, "0.5.0").await;
        assert_eq!(service.check().await["state"], "not-available");
    }
}

#[tokio::test]
async fn unsupported_package_reports_release_without_downloading_foreign_installer() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/latest.yml");
            then.status(200)
                .body("version: 99.0.0\nfiles:\n  - url: a.exe\n    sha512: AAA\n    size: 1\n");
        })
        .await;
    let payload = server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/a.exe");
            then.status(200).body("x");
        })
        .await;
    let dir = tempfile::tempdir().unwrap();
    let service =
        UpdaterService::try_with_feed_base(
            dir.path().to_owned(),
            server.base_url(),
            crate::build_info::display_version().to_string(),
            false,
        )
            .unwrap();
    let status = service.check().await;
    assert_eq!(status["state"], "available");
    assert_eq!(status["newVersion"], "99.0.0");
    assert_eq!(
        status["currentVersion"],
        crate::build_info::display_version()
    );
    assert_eq!(status["channel"], crate::build_info::channel());
    assert_eq!(status["canInstall"], false);
    assert!(!status["installUnavailableReason"]
        .as_str()
        .unwrap()
        .is_empty());
    assert!(!service.downloading.load(Ordering::SeqCst));
    assert_eq!(service.download()["state"], "error");
    assert_eq!(service.install()["state"], "error");
    payload.assert_hits_async(0).await;
    assert!(!dir.path().join("updates/a.exe").exists());
}

#[tokio::test]
async fn feed_failure_is_an_error_state() {
    let dir = tempfile::tempdir().unwrap();
    let service = UpdaterService::with_feed_base(
        dir.path().to_path_buf(),
        "http://127.0.0.1:1".into(),
        "0.5.0",
    );
    assert_eq!(service.check().await["state"], "error");
}

#[tokio::test]
async fn check_downloads_and_verifies_update_in_background() {
    let body = b"installer payload".repeat(50);
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/latest.yml");
            then.status(200).body(format!(
                "version: 99.0.0\nfiles:\n  - url: Mundus-Setup-99.0.0.exe\n    sha512: {}\n \
   size: {}\n",
                hash(&body),
                body.len()
            ));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/Mundus-Setup-99.0.0.exe");
            then.status(200).body(body.clone());
        })
        .await;
    let dir = tempfile::tempdir().unwrap();
    let service =
        UpdaterService::with_feed_base(dir.path().to_path_buf(), server.base_url(), "0.5.0");

    assert_eq!(service.check().await["state"], "available");
    for _ in 0..200 {
        if service.status()["state"] == "downloaded" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(service.status()["state"], "downloaded");
    assert_eq!(
        tokio::fs::read(dir.path().join("updates/Mundus-Setup-99.0.0.exe"))
            .await
            .unwrap(),
        body
    );
    assert!(!dir
        .path()
        .join("updates/Mundus-Setup-99.0.0.exe.part")
        .exists());
}

#[tokio::test]
async fn download_without_check_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let service = UpdaterService::new(dir.path().to_path_buf());
    assert_eq!(service.download()["state"], "error");
}

#[tokio::test]
async fn install_requires_finished_download() {
    let dir = tempfile::tempdir().unwrap();
    let service = UpdaterService::new(dir.path().to_path_buf());
    service.inner.lock().unwrap().pending = Some(PendingUpdate {
        version: "9.9.9".into(),
        asset_url: "http://example.invalid/a.exe".into(),
        sha512: "AAA".into(),
        size: 1,
        installer_path: dir.path().join("a.exe"),
    });
    assert_eq!(service.install()["state"], "error");
}

#[test]
fn construction_sweeps_payloads_left_by_a_previous_run() {
    // KOS-301: applied installers and crashed `.part` downloads used to
    // accumulate in updates/ forever.
    let dir = tempfile::tempdir().unwrap();
    let updates = dir.path().join("updates");
    std::fs::create_dir(&updates).unwrap();
    std::fs::write(updates.join("Mundus-Setup-1.0.0.exe"), b"x").unwrap();
    std::fs::write(updates.join("Mundus-Setup-1.1.0.exe.part"), b"x").unwrap();

    let _service = UpdaterService::new(dir.path().to_path_buf());

    assert!(std::fs::read_dir(&updates).unwrap().next().is_none());
}

#[tokio::test]
async fn check_replacing_pending_drops_the_superseded_installer() {
    let body = b"installer payload".repeat(50);
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/latest.yml");
            then.status(200).body(format!(
                "version: 99.0.0\nfiles:\n  - url: Mundus-Setup-99.0.0.exe\n    sha512: {}\n \
   size: {}\n",
                hash(&body),
                body.len()
            ));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/Mundus-Setup-99.0.0.exe");
            then.status(200).body(body.clone());
        })
        .await;
    let dir = tempfile::tempdir().unwrap();
    let updates = dir.path().join("updates");
    std::fs::create_dir(&updates).unwrap();

    let service =
        UpdaterService::with_feed_base(dir.path().to_path_buf(), server.base_url(), "0.5.0");
    // A payload staged by a previous pending (construction already swept
    // anything older); check() must drop it once pending moves to 99.0.0.
    std::fs::write(updates.join("Mundus-Setup-98.0.0.exe"), b"old").unwrap();
    service.check().await;

    assert_eq!(service.status()["newVersion"], "99.0.0");
    assert!(!updates.join("Mundus-Setup-98.0.0.exe").exists());
}

#[test]
fn desktop_lease_keeps_startup_state_idle() {
    let dir = tempfile::tempdir().unwrap();
    let service = UpdaterService::new(dir.path().to_path_buf());
    let authority = crate::desktop_authority::DesktopAuthorityRegistry::new();
    authority.register("session".into(), 1, 123, "credential");
    assert!(authority.len() > 0);
    assert_eq!(service.status()["state"], "idle");
}
