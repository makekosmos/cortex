fn manifest() -> ManifestV2 {
    ManifestV2 {
        schema_version: 2,
        id: "com.kosmos.demo".into(),
        name: "Demo".into(),
        description: None,
        version: "1.0.0".into(),
        kind: PackageKind::App,
        engine_api: ">=1.0.0".into(),
        entrypoint: "index.html".into(),
        icon: None,
        publisher: "kosmos".into(),
        permissions: vec![],
        targets: vec![crate::package_manifest::ManifestTarget {
            runtime: crate::package_manifest::TargetRuntime::Standalone,
            os: vec![
                crate::package_manifest::TargetOs::Windows,
                crate::package_manifest::TargetOs::Macos,
                crate::package_manifest::TargetOs::Linux,
            ],
            arch: None,
            entrypoint: None,
        }],
        data: crate::package_manifest::ManifestData {
            access: vec![],
            defines: vec![],
            mappings: vec![],
        },
        integration: None,
        store: None,
    }
}

fn legacy_manifest() -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: "com.kosmos.demo".into(),
        name: "Demo".into(),
        version: "1.0.0".into(),
        kind: PackageKind::App,
        engine_api: ">=1.0.0".into(),
        entrypoint: "index.html".into(),
        publisher: "kosmos".into(),
        permissions: vec![],
    }
}

fn manifest_v2_with_canonical_access() -> ManifestV2 {
    ManifestV2 {
        schema_version: 2,
        id: "com.kosmos.demo".into(),
        name: "Demo v2".into(),
        description: None,
        version: "2.0.0".into(),
        kind: PackageKind::App,
        engine_api: ">=1.0.0".into(),
        entrypoint: "index.html".into(),
        icon: None,
        publisher: "kosmos".into(),
        permissions: vec![],
        targets: vec![crate::package_manifest::ManifestTarget {
            runtime: crate::package_manifest::TargetRuntime::Standalone,
            os: vec![
                crate::package_manifest::TargetOs::Windows,
                crate::package_manifest::TargetOs::Macos,
                crate::package_manifest::TargetOs::Linux,
            ],
            arch: None,
            entrypoint: None,
        }],
        data: crate::package_manifest::ManifestData {
            access: vec![crate::package_manifest::DataAccessRule {
                type_id: "com.kosmos.note".into(),
                versions: "^1.0.0".into(),
                actions: vec![crate::package_manifest::DataAction::Read],
                fields: crate::package_manifest::FieldAccess {
                    read: vec!["props.description".into()],
                    write: vec![],
                },
                relations: Some(crate::package_manifest::RelationAccess {
                    read: vec!["related".into()],
                    write: vec![],
                }),
            }],
            defines: vec![],
            mappings: vec![],
        },
        integration: None,
        store: None,
    }
}

/// A catalog archive row for whatever platform the test runs on — fixture
/// manifests declare every OS so the catalog validates on macOS CI too.
fn catalog_archive(
    url: impl Into<String>,
    sha256: String,
    size: u64,
) -> crate::catalog::CatalogArchive {
    crate::catalog::CatalogArchive {
        os: crate::package_manifest::TargetOs::current(),
        arch: crate::package_manifest::TargetArch::current(),
        url: url.into(),
        sha256,
        size,
    }
}

fn catalog(sequence: u64, hash: String, size: u64, expires_at: &str) -> CatalogDocument {
    CatalogDocument {
        schema_version: 1,
        sequence,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: expires_at.into(),
        packages: vec![CatalogEntry {
            manifest: VersionedManifest::V2(manifest()),
            archives: vec![catalog_archive(
                "https://packages.kosmos.dev/demo.kspkg",
                hash,
                size,
            )],
        }],
        external_apps: vec![],
        revoked: vec![],
    }
}

/// Serialize the document the way `apply_catalog` receives it off the wire.
fn document_bytes(document: &CatalogDocument) -> Vec<u8> {
    serde_json::to_vec(document).expect("test json")
}

fn archive(root: &Path) -> (PathBuf, String, u64) {
    archive_with_versioned_manifest(root, &VersionedManifest::V2(manifest()))
}

fn archive_with_manifest(
    root: &Path,
    package_manifest: &PackageManifest,
) -> (PathBuf, String, u64) {
    let path = root.join("demo.kspkg");
    let file = File::create(&path).expect("archive file");
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("manifest.json", FileOptions::default())
        .expect("manifest entry");
    zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
        .expect("manifest write");
    zip.start_file("index.html", FileOptions::default())
        .expect("entrypoint entry");
    zip.write_all(b"ok").expect("entrypoint write");
    zip.finish().expect("archive finish");
    let bytes = fs::read(&path).expect("archive bytes");
    let hash = format!("{:x}", Sha256::digest(&bytes));
    (path, hash, bytes.len() as u64)
}

fn archive_with_versioned_manifest(
    root: &Path,
    package_manifest: &VersionedManifest,
) -> (PathBuf, String, u64) {
    let path = root.join("demo-v2.kspkg");
    let file = File::create(&path).expect("archive file");
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("manifest.json", FileOptions::default())
        .expect("manifest entry");
    zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
        .expect("manifest write");
    zip.start_file(package_manifest.entrypoint(), FileOptions::default())
        .expect("entrypoint entry");
    zip.write_all(b"ok").expect("entrypoint write");
    if let Some(icon) = package_manifest.icon() {
        zip.start_file(icon, FileOptions::default())
            .expect("icon entry");
        zip.write_all(b"icon").expect("icon write");
    }
    zip.finish().expect("archive finish");
    let bytes = fs::read(&path).expect("archive bytes");
    let hash = format!("{:x}", Sha256::digest(&bytes));
    (path, hash, bytes.len() as u64)
}

fn archive_with_versioned_manifest_and_documents(
    root: &Path,
    package_manifest: &VersionedManifest,
    documents: &[(&str, &[u8])],
) -> (PathBuf, String, u64) {
    let path = root.join("demo-defined.kspkg");
    let file = File::create(&path).expect("archive file");
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("manifest.json", FileOptions::default())
        .expect("manifest entry");
    zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
        .expect("manifest write");
    for (name, bytes) in documents {
        zip.start_file(*name, FileOptions::default())
            .expect("definition entry");
        zip.write_all(bytes).expect("definition write");
    }
    zip.start_file("index.html", FileOptions::default())
        .expect("entrypoint entry");
    zip.write_all(b"ok").expect("entrypoint write");
    zip.finish().expect("archive finish");
    let bytes = fs::read(&path).expect("archive bytes");
    let hash = format!("{:x}", Sha256::digest(&bytes));
    (path, hash, bytes.len() as u64)
}

#[cfg(windows)]
fn archive_bridge_binary(
    root: &Path,
    package_manifest: &VersionedManifest,
) -> (PathBuf, String, u64) {
    let path = root.join("bridge.kspkg");
    // Set when the fixture feature built the bin in this test run;
    // `cargo test --lib` skips bins, so fall back to the artifact
    // test-lib.mjs builds beforehand (honouring CARGO_TARGET_DIR).
    let binary = option_env!("CARGO_BIN_EXE_ark-markdown-bridge")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target"))
                .join("debug/ark-markdown-bridge.exe")
        });
    assert!(
        binary.is_file(),
        "build ark-markdown-bridge before this test: {binary:?}"
    );
    let file = File::create(&path).expect("archive file");
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("manifest.json", FileOptions::default())
        .expect("manifest entry");
    zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
        .expect("manifest write");
    // The worker binary is megabytes; store it uncompressed — archive
    // compression is not what these tests exercise, and inflating it on
    // every install/verify made the worker suite spend seconds per spawn.
    zip.start_file(
        package_manifest.entrypoint(),
        FileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .expect("entrypoint entry");
    zip.write_all(&fs::read(binary).expect("bridge binary"))
        .expect("entrypoint write");
    zip.finish().expect("archive finish");
    let bytes = fs::read(&path).expect("archive bytes");
    let hash = format!("{:x}", Sha256::digest(&bytes));
    (path, hash, bytes.len() as u64)
}

pub fn enabled_app_service(dir: &Path) -> (PackageService, PathBuf, String) {
    let (archive, hash, size) = archive(dir);
    let service = PackageService::open_for_test(dir).expect("service");
    let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
    service
        .apply_catalog(document_bytes(&doc))
        .expect("catalog");
    service
        .install_from_path("com.kosmos.demo", "1.0.0", &archive)
        .expect("install");
    service.enable("com.kosmos.demo", "1.0.0").expect("enable");
    (service, archive, hash)
}

pub fn enabled_filesystem_app_service(dir: &Path) -> PackageService {
    let mut package_manifest = manifest();
    package_manifest.permissions.push(PermissionRequest {
        capability: "filesystem.write".into(),
        scopes: vec![],
    });
    let versioned = VersionedManifest::V2(package_manifest);
    let (archive, hash, size) = archive_with_versioned_manifest(dir, &versioned);
    let service = PackageService::open_for_test(dir).expect("service");
    let mut doc = catalog(1, hash, size, "2030-01-01T00:00:00Z");
    doc.packages[0].manifest = versioned;
    service
        .apply_catalog(document_bytes(&doc))
        .expect("catalog");
    service
        .install_from_path("com.kosmos.demo", "1.0.0", &archive)
        .expect("install");
    service.enable("com.kosmos.demo", "1.0.0").expect("enable");
    service
}

/// Enabled app whose manifest grants read+create on `com.kosmos.note`
/// — lets app-RPC tests drive a real write past authorize into dispatch.
pub fn enabled_note_write_app_service(dir: &Path) -> PackageService {
    let mut package_manifest = manifest_v2_with_canonical_access();
    package_manifest.data.access[0].actions = vec![
        crate::package_manifest::DataAction::Read,
        crate::package_manifest::DataAction::Create,
        crate::package_manifest::DataAction::Subscribe,
    ];
    package_manifest.data.access[0].fields.write =
        ["title", "props.description", "props.extensions"]
            .into_iter()
            .map(str::to_owned)
            .collect();
    let versioned = VersionedManifest::V2(package_manifest);
    let (archive, _, _) = archive_with_versioned_manifest(dir, &versioned);
    let service = PackageService::open_for_test(dir).expect("service");
    service
        .install_development_app_from_path("com.kosmos.demo", "2.0.0", &archive)
        .expect("development install");
    service
}

fn package_definition_dispatcher(
    ark: std::sync::Arc<crate::ark_host::ArkHost>,
) -> std::sync::Arc<crate::engine_dispatch::EngineDispatcher> {
    std::sync::Arc::new(crate::engine_dispatch::EngineDispatcher::new(
        std::sync::Arc::new(move |request| {
            let ark = ark.clone();
            Box::pin(async move {
                let response = ark
                    .request(request.operation.as_str(), request.params)
                    .await
                    .map_err(|error| {
                        crate::engine_dispatch::DispatchError::Failed(error.to_string())
                    })?;
                Ok(serde_json::json!({
                    "ok": response.ok,
                    "data": response.data,
                    "error": response.error,
                }))
            })
        }),
    ))
}
