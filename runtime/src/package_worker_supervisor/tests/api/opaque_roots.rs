use super::super::super::dispatch;
use super::super::super::*;

struct OpaqueWorkerFixture {
    _directory: tempfile::TempDir,
    supervisor: PackageWorkerSupervisor,
    authority: std::sync::Arc<crate::grant_authority::GrantAuthorityRegistry>,
    broker: BrokerConfig,
    grant_a: Grant,
    token_a: String,
    grant_b: Grant,
    token_b: String,
    persistent_a: String,
    selected_root: PathBuf,
}

fn opaque_manifest(
    id: &str,
    root: &std::path::Path,
) -> (
    crate::package_manifest::PackageManifest,
    crate::package_manifest::VersionedManifest,
) {
    use crate::package_manifest::{
        ManifestData, ManifestTarget, ManifestV2, PackageKind, PermissionRequest, TargetOs,
        TargetRuntime, VersionedManifest,
    };

    let permissions = vec![
        PermissionRequest {
            capability: "filesystem.read".into(),
            scopes: vec![root.to_string_lossy().into_owned()],
        },
        PermissionRequest {
            capability: "filesystem.write".into(),
            scopes: vec![root.to_string_lossy().into_owned()],
        },
    ];
    let common = crate::package_manifest::PackageManifest {
        schema_version: 1,
        id: id.into(),
        name: id.into(),
        version: "1.0.0".into(),
        kind: PackageKind::Source,
        engine_api: ">=1".into(),
        entrypoint: "worker.exe".into(),
        publisher: "kosmos".into(),
        permissions: permissions.clone(),
    };
    let v2 = ManifestV2 {
        schema_version: 2,
        id: id.into(),
        name: id.into(),
        description: None,
        version: "1.0.0".into(),
        kind: PackageKind::Source,
        engine_api: ">=1".into(),
        entrypoint: "worker.exe".into(),
        icon: None,
        publisher: "kosmos".into(),
        permissions,
        targets: vec![ManifestTarget {
            runtime: TargetRuntime::Worker,
            os: vec![TargetOs::Windows, TargetOs::Macos, TargetOs::Linux],
            arch: None,
            entrypoint: Some("worker.exe".into()),
        }],
        data: ManifestData {
            access: vec![],
            defines: vec![],
            mappings: vec![],
        },
        integration: None,
    };
    (common, VersionedManifest::V2(v2))
}

fn install_opaque_package(
    store: &crate::package_store::PackageStore,
    directory: &std::path::Path,
    manifest: &crate::package_manifest::VersionedManifest,
) -> crate::package_store::InstalledPackage {
    use std::io::Write;
    use zip::{write::FileOptions, ZipWriter};

    let archive_path = directory.join(format!("{}.kspkg", manifest.id()));
    let file = std::fs::File::create(&archive_path).expect("opaque archive");
    let mut archive = ZipWriter::new(file);
    archive
        .start_file("manifest.json", FileOptions::default())
        .expect("manifest entry");
    archive
        .write_all(&serde_json::to_vec(manifest).expect("manifest bytes"))
        .expect("manifest write");
    archive
        .start_file("worker.exe", FileOptions::default())
        .expect("worker entry");
    archive.write_all(b"test worker").expect("worker write");
    archive.finish().expect("archive finish");

    let bytes = std::fs::read(&archive_path).expect("archive bytes");
    let hash = {
        use sha2::Digest;
        sha2::Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let installed = store
        .install_versioned(&archive_path, bytes.len() as u64, &hash, manifest, 1)
        .expect("install opaque package");
    store
        .enable_worker(manifest.id(), manifest.version())
        .expect("enable opaque worker");
    installed
}

fn opaque_worker_fixture() -> OpaqueWorkerFixture {
    use crate::grant_authority::{GrantOwner, GrantProvenance};

    let directory = tempfile::tempdir().expect("opaque fixture directory");
    let selected_root = directory.path().join("selected");
    std::fs::create_dir(&selected_root).expect("selected root");
    let store = std::sync::Arc::new(
        crate::package_store::PackageStore::new(directory.path().join("store"))
            .expect("package store"),
    );
    let (manifest_a, versioned_a) = opaque_manifest("pkg-a", &selected_root);
    let (manifest_b, versioned_b) = opaque_manifest("pkg-b", &selected_root);
    let installed_a = install_opaque_package(&store, directory.path(), &versioned_a);
    let installed_b = install_opaque_package(&store, directory.path(), &versioned_b);
    let authority = std::sync::Arc::new(
        crate::grant_authority::GrantAuthorityRegistry::with_data_dir(
            directory.path().join("grants"),
        ),
    );
    let owner_a = GrantOwner {
        session_id: "session-a:pkg-a".into(),
        generation: 7,
        connection_id: 41,
    };
    let persistent_a = authority
        .register(
            &owner_a,
            "pkg-a",
            &selected_root,
            false,
            GrantProvenance::NativeDialog,
            None,
        )
        .expect("persist selected root")
        .2
        .expect("persistent grant id");
    let (grant_a, token_a) = Grant::derive(
        &manifest_a,
        installed_a.hash,
        41,
        7,
        "session-a".into(),
        std::slice::from_ref(&selected_root),
    )
    .expect("package A grant");
    let (grant_b, token_b) = Grant::derive(
        &manifest_b,
        installed_b.hash,
        42,
        7,
        "session-b".into(),
        std::slice::from_ref(&selected_root),
    )
    .expect("package B grant");
    let supervisor = PackageWorkerSupervisor::new(1);
    supervisor.bind_store(store);
    supervisor.bind_grant_authority(authority.clone());
    let broker = BrokerConfig::new(std::iter::empty::<&str>(), vec![]).expect("broker");
    OpaqueWorkerFixture {
        _directory: directory,
        supervisor,
        authority,
        broker,
        grant_a,
        token_a,
        grant_b,
        token_b,
        persistent_a,
        selected_root,
    }
}

fn worker_call(
    token: &str,
    generation: u64,
    operation: WorkerMethod,
    params: serde_json::Value,
) -> CallMessage {
    CallMessage {
        method: "worker.call".into(),
        id: uuid::Uuid::new_v4().to_string(),
        generation,
        token: token.into(),
        operation,
        params,
    }
}

#[path = "opaque_roots/package_binding.rs"]
mod package_binding;

#[cfg(feature = "package-worker-fixture")]
#[tokio::test]
async fn network_response_chunks_cross_the_worker_dispatch_boundary() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let size = 25 * 1024 * 1024;
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        socket.read(&mut request).await.unwrap();
        socket
            .write_all(
                format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        socket.write_all(&vec![42; size]).await.unwrap();
    });
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PackageStore::new(directory.path().join("store")).unwrap());
    let (mut common, mut versioned) = opaque_manifest("network-test", directory.path());
    common.permissions = vec![crate::package_manifest::PermissionRequest {
        capability: "network".into(),
        scopes: vec![origin.clone()],
    }];
    if let crate::package_manifest::VersionedManifest::V2(manifest) = &mut versioned {
        manifest.permissions = common.permissions.clone();
    }
    let installed = install_opaque_package(&store, directory.path(), &versioned);
    let (grant, token) =
        Grant::derive(&common, installed.hash, 41, 7, "network-test".into(), &[]).unwrap();
    let supervisor = PackageWorkerSupervisor::new(1);
    supervisor.bind_store(store);
    let key = (common.id.clone(), common.version.clone());
    supervisor.insert_failed(key.clone());
    {
        let mut workers = lock(&supervisor.inner.workers);
        let worker = workers.get_mut(&key).unwrap();
        worker.generation = 7;
        worker.health.state = WorkerState::Running;
        worker.grant = Some(grant.clone());
    }
    let broker = BrokerConfig::new([origin.clone()], vec![])
        .unwrap()
        .enable_local_test_origin();
    let reply = dispatch(
        &supervisor.inner,
        &grant,
        &broker,
        None,
        &worker_call(
            &token,
            7,
            WorkerMethod::NetworkFetch,
            serde_json::json!({"url": origin, "response_mode": "chunks"}),
        ),
    )
    .await
    .unwrap();
    server.await.unwrap();
    assert_eq!(reply["size"], size);
    let handle = reply["response_handle"].as_str().unwrap();
    let mut restored = Vec::new();
    while restored.len() < size {
        let reply = dispatch(
            &supervisor.inner,
            &grant,
            &broker,
            None,
            &worker_call(
                &token,
                7,
                WorkerMethod::NetworkFetch,
                serde_json::json!({"url": origin, "response_handle": handle,
                    "offset": restored.len(), "length": 256 * 1024}),
            ),
        )
        .await
        .unwrap();
        assert!(serde_json::to_vec(&reply).unwrap().len() < 700 * 1024);
        restored.extend(
            base64::engine::general_purpose::STANDARD
                .decode(reply["bytes"].as_str().unwrap())
                .unwrap(),
        );
    }
    assert_eq!(restored, vec![42; size]);
    assert!(dispatch(
        &supervisor.inner,
        &grant,
        &broker,
        None,
        &worker_call(
            &token,
            8,
            WorkerMethod::NetworkFetch,
            serde_json::json!({"url": origin, "response_handle": handle, "offset": 0, "length": 1})
        )
    )
    .await
    .is_err());
    dispatch(
        &supervisor.inner,
        &grant,
        &broker,
        None,
        &worker_call(
            &token,
            7,
            WorkerMethod::NetworkFetch,
            serde_json::json!({"url": origin, "response_handle": handle, "close": true}),
        ),
    )
    .await
    .unwrap();
    assert_eq!(supervisor.inner.network_responses.len(), 0);
    let owner = super::super::super::calls_dispatch::network_owner(&key.0, &key.1, 7);
    supervisor
        .inner
        .network_responses
        .reserve(&owner, &key.0, &origin, vec![1])
        .unwrap();
    supervisor.stop(&key.0, &key.1).await.unwrap();
    assert_eq!(supervisor.inner.network_responses.len(), 0);
}

#[tokio::test]
async fn opaque_worker_root_rejects_traversal_and_generation_cleanup() {
    let fixture = opaque_worker_fixture();
    let opened = dispatch(
        &fixture.supervisor.inner,
        &fixture.grant_a,
        &fixture.broker,
        None,
        &worker_call(
            &fixture.token_a,
            7,
            WorkerMethod::FilesystemRootOpen,
            serde_json::json!({"persistent_grant_id": fixture.persistent_a.as_str()}),
        ),
    )
    .await
    .expect("open opaque root");
    let root_id = opened["root_id"]
        .as_str()
        .expect("opaque root id")
        .to_owned();

    for operation in [
        WorkerMethod::FilesystemRead,
        WorkerMethod::FilesystemList,
        WorkerMethod::FilesystemWrite,
        WorkerMethod::FilesystemDelete,
        WorkerMethod::FilesystemCreateDir,
    ] {
        let mut params = serde_json::json!({
            "root_id": root_id.as_str(),
            "relative_path": "../escape"
        });
        if operation == WorkerMethod::FilesystemWrite {
            params["bytes"] =
                serde_json::json!(base64::engine::general_purpose::STANDARD.encode("escape"));
        }
        assert!(dispatch(
            &fixture.supervisor.inner,
            &fixture.grant_a,
            &fixture.broker,
            None,
            &worker_call(&fixture.token_a, 7, operation, params),
        )
        .await
        .is_err());
    }

    assert_eq!(fixture.authority.close_generation("session-a:pkg-a", 7), 2);
    assert!(dispatch(
        &fixture.supervisor.inner,
        &fixture.grant_a,
        &fixture.broker,
        None,
        &worker_call(
            &fixture.token_a,
            7,
            WorkerMethod::FilesystemRead,
            serde_json::json!({"root_id": root_id.as_str(), "relative_path": "safe.txt"}),
        ),
    )
    .await
    .is_err());
}
