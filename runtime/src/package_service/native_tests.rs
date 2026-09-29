// Native apps (KOS-265): GitHub Releases install/update/uninstall through
// the real job path. A local httpmock stub stands in for github.com — no
// network in tests. Compiled only where `host_app_target` resolves — no
// vacuous passes on unsupported hosts.

use crate::native_apps::releases::ReleaseProbe;
use crate::native_apps::{NativeAppDescriptor, NativeAppStore, NativeInstallSpec};
use crate::package_service::{native_error_code, NativeAppState, NativeJob};
use httpmock::Method::GET;
use std::sync::Arc;

const TARGET: &str = "x86_64-pc-windows-msvc";

fn native_zip(root: &Path, name: &str, exe: &str) -> PathBuf {
    let path = root.join(name);
    native_zip_at(&path, exe);
    path
}

fn native_zip_at(path: &Path, exe: &str) {
    let file = File::create(path).expect("zip file");
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file(exe, FileOptions::default())
        .expect("exe entry");
    zip.write_all(b"MZ test fixture").expect("exe write");
    zip.finish().expect("zip finish");
}

/// Mock a tagged release: the sums file lists the real asset for the host
/// target, and the asset serves `archive`'s bytes. `sha_override` publishes
/// a corrupted sums line. No `latest` redirect — see `stub_latest`.
async fn stub_tagged(
    server: &httpmock::MockServer,
    desc: &'static NativeAppDescriptor,
    version: &str,
    archive: &Path,
    sha_override: Option<String>,
) {
    let bytes = fs::read(archive).expect("archive bytes");
    let sha = sha_override.unwrap_or_else(|| format!("{:x}", Sha256::digest(&bytes)));
    let tag = desc.release_tag(version);
    let asset = desc.asset_name(version, TARGET);
    let sums = format!("{sha}  {asset}\n");
    server
        .mock_async(|when, then| {
            when.method(GET).path(format!(
                "/{}/releases/download/{tag}/SHA256SUMS.txt",
                desc.repository
            ));
            then.status(200)
                .header("ETag", format!("\"sums-{tag}\""))
                .body(sums);
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path(format!(
                "/{}/releases/download/{tag}/{asset}",
                desc.repository
            ));
            then.status(200).body(bytes);
        })
        .await;
}

/// `releases/latest/download/SHA256SUMS.txt` 302s to `tag`'s sums URL — the
/// hop that carries the latest tag, like github.com does.
async fn stub_latest(
    server: &httpmock::MockServer,
    desc: &'static NativeAppDescriptor,
    version: &str,
) {
    let tag = desc.release_tag(version);
    server
        .mock_async(|when, then| {
            when.method(GET).path(format!(
                "/{}/releases/latest/download/SHA256SUMS.txt",
                desc.repository
            ));
            then.status(302).header(
                "Location",
                format!(
                    "{}/{}/releases/download/{tag}/SHA256SUMS.txt",
                    server.base_url(),
                    desc.repository
                ),
            );
        })
        .await;
}

/// Full release mock: latest redirect + tagged sums + asset.
async fn stub_release(
    server: &httpmock::MockServer,
    desc: &'static NativeAppDescriptor,
    version: &str,
    archive: &Path,
    sha_override: Option<String>,
) {
    stub_latest(server, desc, version).await;
    stub_tagged(server, desc, version, archive, sha_override).await;
}

fn native_service(dir: &tempfile::TempDir) -> Arc<PackageService> {
    Arc::new(
        PackageService::from_parts(
            dir.path().join("packages"),
            None,
            Some(dir.path().join("apps")),
        )
        .expect("service"),
    )
}

fn probe(server: &httpmock::MockServer) -> ReleaseProbe {
    ReleaseProbe::with_base(server.base_url()).expect("probe")
}

fn dead_probe() -> ReleaseProbe {
    // Port 9 (discard) — nothing answers; every check fails fast.
    ReleaseProbe::with_base("http://127.0.0.1:9".into()).expect("probe")
}

fn agenda() -> &'static NativeAppDescriptor {
    crate::native_apps::app_descriptor("com.kosmos.agenda").unwrap()
}

/// Inline claim+run+finish — the same body the spawned job runs, without
/// the timing lottery.
async fn install_now(
    service: &Arc<PackageService>,
    probe: &ReleaseProbe,
    desc: &'static NativeAppDescriptor,
    version: Option<&str>,
) -> Result<NativeAppSummary, PackageError> {
    let job = service.claim_native_job(desc.id).expect("claim");
    let result = service.run_native_install_with(probe, desc, version).await;
    job.finish(&result);
    result
}

/// Poll the public list until the app's job leaves `installing` (bounded —
/// a stuck job fails the test instead of hanging the suite).
async fn await_job(service: &Arc<PackageService>, probe: &ReleaseProbe, id: &str) {
    for _ in 0..400 {
        let rows = service.native_apps_with(probe, false).await.expect("list");
        let row = rows.iter().find(|row| row.id == id).expect("row");
        if row.state != NativeAppState::Installing {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("native install job for {id} did not settle");
}

#[tokio::test]
async fn native_install_resolves_launch_and_uninstalls() {
    let dir = tempdir().expect("temp dir");
    let zip = native_zip(dir.path(), "agenda.zip", &agenda().executable(TARGET));
    let server = httpmock::MockServer::start_async().await;
    stub_release(&server, agenda(), "0.1.1", &zip, None).await;
    let service = native_service(&dir);

    let summary = install_now(&service, &probe(&server), agenda(), None)
        .await
        .expect("native install");
    assert!(summary.installed);
    assert_eq!(summary.installed_version.as_deref(), Some("0.1.1"));
    assert_eq!(summary.name, "Agenda");
    assert_eq!(summary.state, NativeAppState::Installed);

    let exe_path = service
        .native_app_executable(agenda())
        .expect("executable resolves");
    assert!(exe_path.is_file());
    // Install layout: <root>/apps/<id>/<version>/<exe>.
    let expected = dir
        .path()
        .join("apps")
        .join("com.kosmos.agenda")
        .join("0.1.1")
        .join(agenda().executable(TARGET));
    assert_eq!(exe_path, expected);

    // Same-version reinstall is a no-op row, not a re-download.
    let again = install_now(&service, &probe(&server), agenda(), None)
        .await
        .expect("idempotent reinstall");
    assert_eq!(again.installed_version.as_deref(), Some("0.1.1"));

    service.uninstall_native_app(agenda()).expect("uninstall");
    assert!(service.native_app_executable(agenda()).is_err());
    assert!(!dir.path().join("apps").join("com.kosmos.agenda").exists());
}

#[tokio::test]
async fn concurrent_install_of_same_app_reports_busy() {
    // B77/A2: the real spawned job — a second request while it runs is
    // `busy`, and the row reports `installing` until the job settles.
    let dir = tempdir().expect("temp dir");
    let zip = native_zip(dir.path(), "agenda.zip", &agenda().executable(TARGET));
    let server = httpmock::MockServer::start_async().await;
    stub_release(&server, agenda(), "0.1.1", &zip, None).await;
    let service = native_service(&dir);

    let row = service
        .start_native_install_with(agenda(), probe(&server))
        .expect("first install starts");
    assert_eq!(row.state, NativeAppState::Installing);
    assert!(matches!(
        service.start_native_install_with(agenda(), probe(&server)),
        Err(PackageError::Busy)
    ));
    await_job(&service, &probe(&server), agenda().id).await;
    assert!(service.native_app_executable(agenda()).is_ok());
}

#[tokio::test]
async fn native_list_reports_hardcoded_rows_and_update() {
    let dir = tempdir().expect("temp dir");
    let agenda_zip = native_zip(dir.path(), "agenda.zip", &agenda().executable(TARGET));
    let old_zip = native_zip(dir.path(), "agenda-old.zip", &agenda().executable(TARGET));
    let memoria = crate::native_apps::app_descriptor("com.kosmos.memoria").unwrap();
    let dictation = crate::native_apps::app_descriptor("com.kosmos.dictation").unwrap();
    let memoria_zip = native_zip(dir.path(), "memoria.zip", &memoria.executable(TARGET));
    let dictation_zip = native_zip(dir.path(), "dictation.zip", &dictation.executable(TARGET));
    // One stub serves the pinned install (tagged sums + asset only); a
    // second serves `latest` → 0.2.0 plus the other apps' rows.
    let old_server = httpmock::MockServer::start_async().await;
    stub_tagged(&old_server, agenda(), "0.1.0", &old_zip, None).await;
    let server = httpmock::MockServer::start_async().await;
    stub_release(&server, agenda(), "0.2.0", &agenda_zip, None).await;
    stub_release(&server, memoria, "0.7.0", &memoria_zip, None).await;
    stub_release(&server, dictation, "0.3.0", &dictation_zip, None).await;
    let service = native_service(&dir);
    install_now(&service, &probe(&old_server), agenda(), Some("0.1.0"))
        .await
        .expect("install pinned");

    let apps = service
        .native_apps_with(&probe(&server), false)
        .await
        .expect("list");
    assert_eq!(apps.len(), crate::native_apps::NATIVE_APPS.len());
    let agenda_row = apps
        .iter()
        .find(|row| row.id == "com.kosmos.agenda")
        .expect("agenda row");
    assert_eq!(agenda_row.installed_version.as_deref(), Some("0.1.0"));
    assert_eq!(agenda_row.state, NativeAppState::UpdateAvailable);
    assert_eq!(agenda_row.update_version.as_deref(), Some("0.2.0"));
    let memoria_row = apps
        .iter()
        .find(|row| row.id == "com.kosmos.memoria")
        .expect("memoria row");
    assert!(!memoria_row.installed);
    assert_eq!(memoria_row.latest_version.as_deref(), Some("0.7.0"));
    assert_eq!(memoria_row.state, NativeAppState::NotInstalled);
}

#[tokio::test]
async fn native_update_keeps_previous_on_bad_sha() {
    let dir = tempdir().expect("temp dir");
    let exe = agenda().executable(TARGET);
    let v1 = native_zip(dir.path(), "agenda-1.zip", &exe);
    let server = httpmock::MockServer::start_async().await;
    stub_release(&server, agenda(), "0.1.0", &v1, None).await;
    let service = native_service(&dir);
    install_now(&service, &probe(&server), agenda(), None)
        .await
        .expect("install v1");

    // A fresh stub offers 0.2.0 but with a corrupted sums line.
    let v2 = native_zip(dir.path(), "agenda-2.zip", &exe);
    let server2 = httpmock::MockServer::start_async().await;
    stub_release(&server2, agenda(), "0.2.0", &v2, Some("0".repeat(64))).await;
    let err = install_now(&service, &probe(&server2), agenda(), None)
        .await
        .expect_err("bad sha must fail");
    assert!(matches!(err, PackageError::Integrity));
    assert_eq!(native_error_code(&err), "integrity");
    let apps = service.native_apps_with(&dead_probe(), false).await.expect("list");
    let row = apps
        .iter()
        .find(|row| row.id == "com.kosmos.agenda")
        .unwrap();
    assert_eq!(row.installed_version.as_deref(), Some("0.1.0"));
    assert!(dir
        .path()
        .join("apps")
        .join("com.kosmos.agenda")
        .join("0.1.0")
        .join(&exe)
        .is_file());
    // The failed job left a typed failure state for the Store row.
    assert_eq!(row.state, NativeAppState::Failed);
    assert_eq!(row.failure, Some("integrity"));
}

#[tokio::test]
async fn native_update_rolls_back_on_bad_archive() {
    let dir = tempdir().expect("temp dir");
    let exe = agenda().executable(TARGET);
    let v1 = native_zip(dir.path(), "agenda-1.zip", &exe);
    let server = httpmock::MockServer::start_async().await;
    stub_release(&server, agenda(), "0.1.0", &v1, None).await;
    let service = native_service(&dir);
    install_now(&service, &probe(&server), agenda(), None)
        .await
        .expect("install v1");

    // Sums sha256 matches the offered zip, but the zip lacks the
    // descriptor's executable — extraction fails after the hash passes.
    let v2 = native_zip(dir.path(), "agenda-2.zip", "other.exe");
    let server2 = httpmock::MockServer::start_async().await;
    stub_release(&server2, agenda(), "0.2.0", &v2, None).await;
    assert!(install_now(&service, &probe(&server2), agenda(), None)
        .await
        .is_err());
    let apps = service.native_apps_with(&dead_probe(), false).await.expect("list");
    let row = apps
        .iter()
        .find(|row| row.id == "com.kosmos.agenda")
        .unwrap();
    assert_eq!(row.installed_version.as_deref(), Some("0.1.0"));
    assert!(service.native_app_executable(agenda()).is_ok());
}

#[tokio::test]
async fn native_update_replaces_old_version() {
    let dir = tempdir().expect("temp dir");
    let exe = agenda().executable(TARGET);
    let v1 = native_zip(dir.path(), "agenda-1.zip", &exe);
    let v2 = native_zip(dir.path(), "agenda-2.zip", &exe);
    let server = httpmock::MockServer::start_async().await;
    stub_release(&server, agenda(), "0.2.0", &v2, None).await;
    let service = native_service(&dir);
    // Pin the older version through a second stub so `latest` stays 0.2.0.
    let server1 = httpmock::MockServer::start_async().await;
    stub_release(&server1, agenda(), "0.1.0", &v1, None).await;
    install_now(&service, &probe(&server1), agenda(), Some("0.1.0"))
        .await
        .expect("install 0.1.0");
    install_now(&service, &probe(&server), agenda(), None)
        .await
        .expect("install latest");
    let apps = service.native_apps_with(&probe(&server), false).await.expect("list");
    let row = apps
        .iter()
        .find(|row| row.id == "com.kosmos.agenda")
        .unwrap();
    assert_eq!(row.installed_version.as_deref(), Some("0.2.0"));
    assert!(row.update_version.is_none());
    // The superseded version dir is swept after the pointer flip.
    assert!(!dir
        .path()
        .join("apps")
        .join("com.kosmos.agenda")
        .join("0.1.0")
        .exists());
}

#[tokio::test]
async fn native_list_offline_keeps_installed_state() {
    let dir = tempdir().expect("temp dir");
    let service = native_service(&dir);
    // Nothing installed, probe dead → every row reports offline.
    let apps = service.native_apps_with(&dead_probe(), false).await.expect("list");
    assert_eq!(apps.len(), crate::native_apps::NATIVE_APPS.len());
    assert!(apps.iter().all(|row| !row.installed));
    assert!(apps
        .iter()
        .all(|row| row.latest_version.is_none() && row.state == NativeAppState::Offline));

    // An installed app keeps its installed row offline.
    let store = NativeAppStore::new(dir.path().join("apps")).unwrap();
    let archive = native_zip(dir.path(), "agenda.zip", "agenda-gpui.exe");
    let bytes = fs::read(&archive).unwrap();
    store
        .install_archive(
            &NativeInstallSpec {
                id: "com.kosmos.agenda".into(),
                version: "0.1.0".into(),
                executable: "agenda-gpui.exe".into(),
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                size: bytes.len() as u64,
                repository: "makekosmos/agenda-gpui".into(),
                release_tag: "v0.1.0".into(),
            },
            &archive,
        )
        .unwrap();
    let apps = service.native_apps_with(&dead_probe(), false).await.expect("list");
    let row = apps
        .iter()
        .find(|row| row.id == "com.kosmos.agenda")
        .unwrap();
    assert!(row.installed);
    assert_eq!(row.installed_version.as_deref(), Some("0.1.0"));
    assert_eq!(row.state, NativeAppState::Installed);
}

#[tokio::test]
async fn failed_install_leaves_typed_row_state() {
    // B75: a failed background install surfaces as `failed` + the typed
    // code, not a silent revert.
    let dir = tempdir().expect("temp dir");
    let service = native_service(&dir);
    let err = install_now(&service, &dead_probe(), agenda(), None)
        .await
        .expect_err("offline install must fail");
    assert!(matches!(err, PackageError::Offline));
    let apps = service.native_apps_with(&dead_probe(), false).await.expect("list");
    let row = apps
        .iter()
        .find(|row| row.id == "com.kosmos.agenda")
        .unwrap();
    assert_eq!(row.state, NativeAppState::Failed);
    assert_eq!(row.failure, Some("offline"));
}

#[test]
fn dropped_job_claim_records_failure_and_frees_the_slot() {
    // The claim is an RAII guard: an install task that dies without
    // finishing (panic, abort) must still settle the row — a bare claim
    // used to leave `installing` forever.
    let dir = tempdir().expect("temp dir");
    let service = native_service(&dir);
    {
        let _job = service
            .claim_native_job("com.kosmos.agenda")
            .expect("claim");
        // A second claim while the guard is held reports busy.
        assert!(service.claim_native_job("com.kosmos.agenda").is_none());
    }
    assert!(matches!(
        service.current_job("com.kosmos.agenda"),
        Some(NativeJob::Failed("unavailable"))
    ));
    // The slot is free again for the next attempt.
    let job = service.claim_native_job("com.kosmos.agenda").expect("re-claim");
    job.finish(&Err(PackageError::Offline));
    assert!(matches!(
        service.current_job("com.kosmos.agenda"),
        Some(NativeJob::Failed("offline"))
    ));
}

