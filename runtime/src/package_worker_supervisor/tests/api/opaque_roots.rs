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

#[path = "opaque_roots/network_chunks.rs"]
mod network_chunks;
#[path = "opaque_roots/package_binding.rs"]
mod package_binding;
#[path = "opaque_roots/rejections.rs"]
mod rejections;
