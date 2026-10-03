#![cfg(all(windows, feature = "package-worker-fixture"))]
#![allow(clippy::unwrap_used)]

use async_trait::async_trait;
use engine::{
    package_manifest::{
        ManifestData, ManifestTarget, ManifestV2, PackageKind, PackageManifest, PermissionRequest,
        TargetArch, TargetOs, TargetRuntime, VersionedManifest,
    },
    package_store::PackageStore,
    package_worker_supervisor::{ArkRequestExecutor, PackageWorkerSupervisor, WorkerState},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tempfile::TempDir;
use zip::{write::FileOptions, ZipWriter};

const PACKAGE_ID: &str = "com.kosmos.dictation";
const VERSION: &str = "0.2.5";
const WORKER_ENTRYPOINT: &str = "worker/dictation-worker.exe";

#[derive(Clone, Default)]
struct SyntheticCapabilities {
    calls: Arc<Mutex<Vec<(String, Value)>>>,
    fail: Arc<AtomicBool>,
}

#[async_trait]
impl ArkRequestExecutor for SyntheticCapabilities {
    async fn request(&self, operation: &str, params: Value) -> Result<Value, &'static str> {
        self.calls
            .lock()
            .unwrap()
            .push((operation.to_owned(), params.clone()));
        if self.fail.load(Ordering::Acquire) {
            return Err("synthetic-failure");
        }
        Ok(match operation {
            "dictation.window.foreground" => json!({"windowId":"window-1"}),
            "dictation.capture.start" => json!({"captureId":"capture-1"}),
            "dictation.capture.stop" => {
                json!({"captureId":"capture-1","audioB64":"UklGRg==","durationMs":1250})
            }
            "dictation.speech.transcribe" => json!({"text":"Привет"}),
            "dictation.input.insert_text" => json!({"inserted":true,"method":"test"}),
            _ => return Err("unsupported-test-capability"),
        })
    }
}

fn app_manifest() -> VersionedManifest {
    VersionedManifest::V2(ManifestV2 {
        schema_version: 2,
        id: PACKAGE_ID.into(),
        name: "Dictation".into(),
        description: None,
        version: VERSION.into(),
        kind: PackageKind::App,
        engine_api: ">=1.0.0".into(),
        entrypoint: "dist/index.html".into(),
        icon: None,
        publisher: "kosmos".into(),
        permissions: vec![
            PermissionRequest {
                capability: "dictation.control".into(),
                scopes: [
                    "dictation.capture.start",
                    "dictation.capture.stop",
                    "dictation.speech.transcribe",
                    "dictation.input.insert_text",
                    "dictation.window.foreground",
                    "dictation.lifecycle.set_autostart",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            },
            PermissionRequest {
                capability: "worker.invoke".into(),
                scopes: vec!["dictation.trigger".into()],
            },
        ],
        targets: vec![
            ManifestTarget {
                runtime: TargetRuntime::Standalone,
                os: vec![TargetOs::Windows],
                arch: Some(vec![TargetArch::X86_64]),
                entrypoint: None,
            },
            ManifestTarget {
                runtime: TargetRuntime::Worker,
                os: vec![TargetOs::Windows],
                arch: Some(vec![TargetArch::X86_64]),
                entrypoint: Some(WORKER_ENTRYPOINT.into()),
            },
        ],
        data: ManifestData {
            access: vec![],
            defines: vec![],
            mappings: vec![],
        },
        integration: None,
        store: None,
    })
}

fn projected_worker_manifest() -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: PACKAGE_ID.into(),
        name: "Dictation".into(),
        version: VERSION.into(),
        kind: PackageKind::Source,
        engine_api: ">=1.0.0".into(),
        entrypoint: WORKER_ENTRYPOINT.into(),
        publisher: "kosmos".into(),
        permissions: match app_manifest() {
            VersionedManifest::V2(manifest) => Some(manifest.permissions),
            VersionedManifest::V1(_) => None,
        }
        .expect("expected v2 manifest"),
    }
}

fn install_candidate(
    directory: &TempDir,
    worker: &std::path::Path,
) -> (Arc<PackageStore>, engine::package_store::InstalledPackage) {
    let archive_path = directory.path().join("dictation.kspkg");
    let manifest = app_manifest();
    let file = std::fs::File::create(&archive_path).unwrap();
    let mut archive = ZipWriter::new(file);
    archive
        .start_file("manifest.json", FileOptions::default())
        .unwrap();
    archive
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    archive
        .start_file("dist/index.html", FileOptions::default())
        .unwrap();
    archive.write_all(b"<html>Dictation</html>").unwrap();
    archive
        .start_file(WORKER_ENTRYPOINT, FileOptions::default())
        .unwrap();
    archive.write_all(&std::fs::read(worker).unwrap()).unwrap();
    archive.finish().unwrap();

    let bytes = std::fs::read(&archive_path).unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let store = Arc::new(PackageStore::new(directory.path().join("store")).unwrap());
    let installed = store
        .install_versioned(&archive_path, bytes.len() as u64, &hash, &manifest, 1)
        .unwrap();
    store.enable_worker(PACKAGE_ID, VERSION).unwrap();
    (store, installed)
}

// The gate builds the generic worker fixture, not a dictation worker — this
// contract test needs MUNDUS_DICTATION_WORKER_EXE pointing at a reviewed
// dictation worker build, so it runs only when explicitly invoked.
#[tokio::test]
#[ignore = "requires MUNDUS_DICTATION_WORKER_EXE pointing at a reviewed dictation worker build"]
async fn compiled_dictation_worker_round_trips_engine_capabilities() {
    let worker = PathBuf::from(
        std::env::var_os("MUNDUS_DICTATION_WORKER_EXE")
            .expect("MUNDUS_DICTATION_WORKER_EXE must name a reviewed worker candidate"),
    );
    assert!(worker.is_file(), "worker candidate missing: {worker:?}");
    // Worker launches share process-wide failure hooks with other test modules.
    let _serialized = engine::package_worker_process::test_support::serialized();

    let directory = tempfile::tempdir().unwrap();
    let (store, installed) = install_candidate(&directory, &worker);
    let executable = store.immutable_entrypoint(&installed).unwrap();
    let capabilities = SyntheticCapabilities::default();
    let supervisor = PackageWorkerSupervisor::with_ark_executor(1, Arc::new(capabilities.clone()));
    supervisor.bind_store(store);
    let state = tempfile::tempdir().unwrap();
    let manifest = projected_worker_manifest();

    supervisor
        .start(
            &manifest,
            executable,
            state.path().to_path_buf(),
            installed.hash,
            &[],
            "dictation-worker-e2e".into(),
            None,
            None,
        )
        .await
        .expect("compiled worker hello");
    assert_eq!(
        supervisor.health(PACKAGE_ID, VERSION).state,
        WorkerState::Running
    );

    let first = supervisor
        .invoke(PACKAGE_ID, VERSION, "dictation.trigger", json!({}))
        .await
        .expect("capture start result");
    assert_eq!(first["state"], "capturing");
    let second = supervisor
        .invoke(PACKAGE_ID, VERSION, "dictation.trigger", json!({}))
        .await
        .expect("capture/transcribe/insert result");
    assert_eq!(second["state"], "idle");

    let calls = capabilities.calls.lock().unwrap().clone();
    let operations: Vec<_> = calls
        .iter()
        .map(|(operation, _)| operation.as_str())
        .collect();
    assert_eq!(
        operations,
        [
            "dictation.window.foreground",
            "dictation.capture.start",
            "dictation.capture.stop",
            "dictation.speech.transcribe",
            "dictation.input.insert_text",
        ]
    );
    assert_eq!(calls[3].1["delivery"], "text_only");
    assert_eq!(calls[4].1["targetWindow"], "window-1");

    capabilities.fail.store(true, Ordering::Release);
    assert_eq!(
        supervisor
            .invoke(
                PACKAGE_ID,
                VERSION,
                "dictation.trigger",
                json!({ "kind": "ptt", "phase": "down" })
            )
            .await,
        Err("unavailable")
    );
    assert_eq!(
        supervisor.health(PACKAGE_ID, VERSION).state,
        WorkerState::Running
    );
    capabilities.fail.store(false, Ordering::Release);
    assert_eq!(
        supervisor
            .invoke(
                PACKAGE_ID,
                VERSION,
                "dictation.trigger",
                json!({ "kind": "ptt", "phase": "down" })
            )
            .await
            .expect("worker recovery result")["state"],
        "capturing"
    );

    supervisor
        .stop(PACKAGE_ID, VERSION)
        .await
        .expect("worker stop");
    assert_eq!(
        supervisor.health(PACKAGE_ID, VERSION).state,
        WorkerState::Stopped
    );
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup (KOS-270).
    let _ = supervisor.stop_all().await;
}
