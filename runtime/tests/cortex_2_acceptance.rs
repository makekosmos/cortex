#![allow(clippy::panic, clippy::unwrap_used)]

use engine::{
    package_manifest::{PackageKind, PackageManifest, VersionedManifest},
    package_store::PackageStore,
    package_worker_protocol::{parse_json_line, WorkerMessage},
    package_worker_protocol::{Grant, GrantError},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, io::Write};
use tempfile::TempDir;
use zip::{write::FileOptions, ZipWriter};

const ARCADIA_APP_MANIFEST: &str = include_str!("fixtures/cortex-2-arcadia-app.json");
const WORKER_TRANSCRIPT: &str = include_str!("fixtures/cortex-2-worker-transcript.jsonl");

#[test]
fn cortex_has_no_embedded_arcadia_domain_module() {
    assert!(!std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/arrancador/mod.rs")
        .exists());
}

#[test]
fn worker_invoke_and_result_are_correlated_by_request_id() {
    let mut requests = HashMap::new();
    let mut results = HashMap::new();

    for line in WORKER_TRANSCRIPT.lines().filter(|line| !line.is_empty()) {
        match parse_json_line(line.as_bytes()).expect("fixture protocol line") {
            WorkerMessage::Invoke(message) => {
                assert_eq!(message.method, "worker.invoke");
                requests.insert(message.id.clone(), message);
            }
            WorkerMessage::Result(message) => {
                assert_eq!(message.method, "worker.result");
                results.insert(message.id.clone(), message);
            }
            message => panic!("unexpected fixture message: {message:?}"),
        }
    }

    assert_eq!(requests.len(), 2);
    assert_eq!(results.len(), requests.len());
    assert_eq!(results["invoke-a"].error.as_deref(), Some("denied"));
    assert_eq!(
        results["invoke-b"].result,
        Some(json!({"source": "rawg", "matched": true}))
    );
    assert!(requests["invoke-a"].operation.starts_with("arcadia."));
    assert_eq!(requests["invoke-b"].operation, "rawg.lookup");
}

#[test]
fn denied_grants_fail_closed_before_any_host_operation() {
    let manifest = PackageManifest {
        schema_version: 1,
        id: "com.kosmos.arcadia".into(),
        name: "Arcadia".into(),
        version: "1.0.0".into(),
        kind: PackageKind::Source,
        engine_api: ">=1.0.0".into(),
        entrypoint: "arcadia-worker.exe".into(),
        publisher: "kosmos".into(),
        permissions: vec![engine::package_manifest::PermissionRequest {
            capability: "process.spawn".into(),
            scopes: vec![std::env::temp_dir()
                .join("outside-arcadia-root")
                .to_string_lossy()
                .into_owned()],
        }],
    };

    assert_eq!(
        Grant::derive(
            &manifest,
            "hash".into(),
            1,
            1,
            "test".into(),
            &[std::env::temp_dir().join("allowed")],
        ),
        Err(GrantError::InvalidScope)
    );
}

#[test]
fn app_manifest_keeps_html_entrypoint_and_optional_worker_entrypoint() {
    let VersionedManifest::V2(manifest) =
        PackageManifest::parse(ARCADIA_APP_MANIFEST).expect("Arcadia manifest")
    else {
        panic!("expected v2 app manifest");
    };

    assert_eq!(manifest.kind, PackageKind::App);
    assert_eq!(manifest.entrypoint, "index.html");
    assert_eq!(
        VersionedManifest::V2(manifest.clone()).worker_entrypoint(),
        Some("arcadia-worker.exe")
    );
    assert_eq!(
        VersionedManifest::V2(manifest.clone())
            .common_manifest()
            .entrypoint,
        "index.html"
    );
}

#[test]
fn uninstall_removes_package_record_without_touching_migrated_user_data() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let user_data = directory.path().join("user-data").join("games.json");
    std::fs::create_dir_all(user_data.parent().unwrap()).unwrap();
    std::fs::write(&user_data, br#"{"game":"Portal 2","version":1}"#).unwrap();

    let archive = write_arcadia_archive(&directory);
    let bytes = std::fs::read(&archive).unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let VersionedManifest::V2(manifest) = PackageManifest::parse(ARCADIA_APP_MANIFEST).unwrap()
    else {
        panic!("expected v2 app manifest");
    };
    let store = PackageStore::new(directory.path().join("store")).unwrap();
    let installed = store
        .install_versioned(
            &archive,
            bytes.len() as u64,
            &hash,
            &VersionedManifest::V2(manifest.clone()),
            1,
        )
        .unwrap();
    store
        .enable_worker(&manifest.id, &manifest.version)
        .unwrap();
    assert!(store.immutable_entrypoint(&installed).is_ok());

    store.uninstall(&manifest.id, &manifest.version).unwrap();

    assert!(store.installed(&manifest.id, &manifest.version).is_err());
    assert!(store.list().unwrap().is_empty());
    assert_eq!(
        std::fs::read(&user_data).unwrap(),
        br#"{"game":"Portal 2","version":1}"#
    );
}

fn write_arcadia_archive(directory: &TempDir) -> std::path::PathBuf {
    let archive_path = directory.path().join("arcadia.kspkg");
    let file = std::fs::File::create(&archive_path).unwrap();
    let mut archive = ZipWriter::new(file);
    let options = FileOptions::default();
    archive.start_file("manifest.json", options).unwrap();
    archive.write_all(ARCADIA_APP_MANIFEST.as_bytes()).unwrap();
    archive.start_file("index.html", options).unwrap();
    archive.write_all(b"<html>Arcadia</html>").unwrap();
    archive.start_file("arcadia-worker.exe", options).unwrap();
    archive.write_all(b"fixture worker").unwrap();
    archive.finish().unwrap();
    archive_path
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
#[tokio::test]
async fn worker_invocation_round_trips_through_the_process_boundary() {
    use engine::{
        package_worker_process::test_support, package_worker_supervisor::PackageWorkerSupervisor,
    };

    let _lock = test_support::serialized();
    let supervisor = PackageWorkerSupervisor::new(1);
    let state = tempfile::tempdir().expect("worker state directory");
    let manifest = engine::package_manifest::PackageManifest {
        schema_version: 1,
        id: "fixture.invoke".into(),
        name: "Fixture".into(),
        version: "1.0.0".into(),
        kind: PackageKind::Source,
        engine_api: ">=1.0.0".into(),
        entrypoint: "package-worker-fixture.exe".into(),
        publisher: "kosmos".into(),
        permissions: vec![],
    };
    let fixture = std::path::PathBuf::from(env!("CARGO_BIN_EXE_package-worker-fixture"));

    supervisor
        .start(
            &manifest,
            fixture,
            state.path().to_path_buf(),
            "hash".into(),
            &[],
            "cortex-2-invoke".into(),
            None,
            None,
        )
        .await
        .expect("fixture hello");
    let result = supervisor
        .invoke(
            &manifest.id,
            &manifest.version,
            "games.list",
            json!({"limit": 10}),
        )
        .await
        .expect("worker result");
    assert_eq!(
        result,
        json!({"operation":"games.list","params":{"limit":10}})
    );
    supervisor
        .stop(&manifest.id, &manifest.version)
        .await
        .expect("worker stop");
}

#[cfg(all(windows, feature = "package-worker-fixture"))]
#[tokio::test]
async fn worker_crash_resolves_pending_invocation() {
    use engine::{
        package_worker_process::test_support, package_worker_supervisor::PackageWorkerSupervisor,
    };

    let _lock = test_support::serialized();
    let supervisor = PackageWorkerSupervisor::new(1);
    let state = tempfile::tempdir().expect("worker state directory");
    let manifest = engine::package_manifest::PackageManifest {
        schema_version: 1,
        id: "fixture.crash".into(),
        name: "Fixture".into(),
        version: "1.0.0".into(),
        kind: PackageKind::Source,
        engine_api: ">=1.0.0".into(),
        entrypoint: "package-worker-fixture.exe".into(),
        publisher: "kosmos".into(),
        permissions: vec![],
    };
    let fixture = std::path::PathBuf::from(env!("CARGO_BIN_EXE_package-worker-fixture"));

    supervisor
        .start(
            &manifest,
            fixture,
            state.path().to_path_buf(),
            "hash".into(),
            &[],
            "cortex-2-crash".into(),
            None,
            None,
        )
        .await
        .expect("fixture hello");

    let result = supervisor
        .invoke(
            "fixture.crash",
            "1.0.0",
            "arcadia.scan",
            json!({"root":"games"}),
        )
        .await;
    assert_eq!(result, Err("unavailable"));
}
