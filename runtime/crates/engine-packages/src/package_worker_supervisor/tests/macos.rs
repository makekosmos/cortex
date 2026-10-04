use super::super::*;
use crate::package_manifest::{
    ManifestData, ManifestTarget, ManifestV2, TargetOs, TargetRuntime, VersionedManifest,
};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::sync::Mutex;
use zip::{write::FileOptions, ZipWriter};

fn fixture() -> Option<PathBuf> {
    std::env::var_os("MUNDUS_WORKER_FIXTURE").map(PathBuf::from)
}

fn manifest(id: &str) -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: id.into(),
        name: "fixture".into(),
        version: "1.0.0".into(),
        kind: PackageKind::Source,
        engine_api: ">=1".into(),
        entrypoint: "package-worker-fixture.exe".into(),
        publisher: "kosmos".into(),
        permissions: if id.ends_with(".ark-write") {
            vec![crate::package_manifest::PermissionRequest {
                capability: "ark.write".into(),
                scopes: vec!["upsert_object_type".into()],
            }]
        } else {
            vec![]
        },
    }
}

fn install_fixture(
    root: &Path,
    manifest: &PackageManifest,
    binary: &Path,
) -> (Arc<PackageStore>, PathBuf, String) {
    let archive_path = root.join("worker.kspkg");
    let versioned = VersionedManifest::V2(ManifestV2 {
        schema_version: 2,
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        description: None,
        version: manifest.version.clone(),
        kind: manifest.kind.clone(),
        engine_api: manifest.engine_api.clone(),
        entrypoint: manifest.entrypoint.clone(),
        icon: None,
        publisher: manifest.publisher.clone(),
        permissions: manifest.permissions.clone(),
        targets: vec![ManifestTarget {
            runtime: TargetRuntime::Worker,
            os: vec![TargetOs::Macos],
            arch: None,
            entrypoint: Some(manifest.entrypoint.clone()),
        }],
        data: ManifestData {
            access: vec![],
            defines: vec![],
            mappings: vec![],
        },
        integration: None,
        store: None,
    });
    let file = std::fs::File::create(&archive_path).unwrap();
    let mut zip = ZipWriter::new(file);
    zip.start_file("manifest.json", FileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&versioned).unwrap())
        .unwrap();
    zip.start_file(
        &manifest.entrypoint,
        FileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .unwrap();
    zip.write_all(&std::fs::read(binary).unwrap()).unwrap();
    zip.finish().unwrap();
    let bytes = std::fs::read(&archive_path).unwrap();
    let hash = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let store = Arc::new(PackageStore::new(root.join("store")).unwrap());
    let installed = store
        .install_versioned(&archive_path, bytes.len() as u64, &hash, &versioned, 1)
        .unwrap();
    store
        .enable_worker(&manifest.id, &manifest.version)
        .unwrap();
    let executable = store.immutable_entrypoint(&installed).unwrap();
    (store, executable, hash)
}

struct RecordingArk(Mutex<Vec<String>>);

#[async_trait]
impl ArkRequestExecutor for RecordingArk {
    async fn request(
        &self,
        operation: &str,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, &'static str> {
        self.0.lock().unwrap().push(operation.into());
        Ok(serde_json::json!({}))
    }
}

#[tokio::test]
async fn installed_worker_call_uses_grant_and_broker_dispatch() {
    let Some(binary) = fixture() else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let manifest = manifest("fixture.macos.ark-write");
    let (store, executable, hash) = install_fixture(root.path(), &manifest, &binary);
    let executor = Arc::new(RecordingArk(Mutex::new(Vec::new())));
    let supervisor = PackageWorkerSupervisor::with_ark_executor(1, executor.clone());
    supervisor.bind_store(store);
    supervisor
        .start(
            &manifest,
            executable,
            root.path().join("state"),
            hash,
            &[],
            "mac-broker".into(),
            None,
            None,
        )
        .await
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while executor.0.lock().unwrap().is_empty() && Instant::now() < deadline {
        time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(*executor.0.lock().unwrap(), ["upsert_object_type"]);
    supervisor.stop_all().await.unwrap();
}

#[tokio::test]
async fn installed_worker_bootstrap_invoke_stop_and_failed_start() {
    let Some(binary) = fixture() else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let normal = manifest("fixture.macos");
    let (store, executable, hash) = install_fixture(root.path(), &normal, &binary);
    let supervisor =
        PackageWorkerSupervisor::with_restart_delays(1, vec![Duration::from_millis(10); 3]);
    supervisor.bind_store(store);
    let state_root = root.path().join("state");
    supervisor
        .start(
            &normal,
            executable.clone(),
            state_root.clone(),
            hash.clone(),
            &[],
            "mac-test".into(),
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        supervisor.health(&normal.id, &normal.version).state,
        WorkerState::Running
    );
    assert_eq!(
        supervisor
            .invoke(
                &normal.id,
                &normal.version,
                "ping",
                serde_json::json!({"n": 1})
            )
            .await
            .unwrap(),
        serde_json::json!({"operation":"ping","params":{"n":1}})
    );
    supervisor.stop(&normal.id, &normal.version).await.unwrap();
    assert_eq!(
        supervisor.health(&normal.id, &normal.version).state,
        WorkerState::Stopped
    );
    assert!(supervisor.stop_all().await.is_ok());

    let invalid = manifest("fixture.macos.wrong-token");
    let (store, executable, hash) = install_fixture(root.path(), &invalid, &binary);
    let supervisor =
        PackageWorkerSupervisor::with_restart_delays(1, vec![Duration::from_millis(10); 3]);
    supervisor.bind_store(store);
    assert!(supervisor
        .start(
            &invalid,
            executable,
            state_root,
            hash,
            &[],
            "mac-fail".into(),
            None,
            None
        )
        .await
        .is_err());
    assert_ne!(
        supervisor.health(&invalid.id, &invalid.version).state,
        WorkerState::Running
    );
    let key = (invalid.id.clone(), invalid.version.clone());
    let generation = super::super::io::lock(&supervisor.inner.workers)
        .get(&key)
        .unwrap()
        .generation;
    let deadline = Instant::now() + Duration::from_secs(2);
    let holder = io::lock(&supervisor.inner.workers)
        .get(&key)
        .unwrap()
        .process_holder
        .clone();
    let holder_empty = holder_empty_until(holder, deadline).await;
    let live = io::lock(&supervisor.inner.workers)
        .get(&key)
        .is_some_and(worker_has_live_resources);
    let io_live = supervisor
        .inner
        .worker_io
        .has_generation(&key.0, &key.1, generation);
    assert!(
        supervisor
            .exact_generation_is_clean(&key, generation, deadline)
            .await,
        "diagnostics={:?}, live={live}, holder_empty={holder_empty}, io_live={io_live}",
        supervisor.diagnostics(),
    );
    let _ = supervisor.stop_all().await;
}
