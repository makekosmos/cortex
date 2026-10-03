#[test]
fn app_launch_asset_lifecycle_and_immutable_tamper_boundary() {
    let dir = tempdir().expect("tempdir");
    let (service, _archive, hash) = enabled_app_service(dir.path());
    let launch = service.launch_app("com.kosmos.demo", None).expect("launch");
    assert_eq!(launch.package.hash, hash);
    assert_eq!(
        service
            .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "index.html")
            .expect("asset"),
        b"ok"
    );
    assert!(service
        .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "../index.html")
        .is_err());
    service
        .disable("com.kosmos.demo", "1.0.0")
        .expect("disable");
    assert!(service.launch_app("com.kosmos.demo", None).is_err());
    assert!(service
        .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "index.html")
        .is_err());

    let tamper_dir = tempdir().expect("tamper tempdir");
    let (service, _archive, hash) = enabled_app_service(tamper_dir.path());
    let blob = tamper_dir
        .path()
        .join("packages/blobs")
        .join(format!("{hash}.kspkg"));
    let mut bytes = fs::read(&blob).expect("blob");
    bytes[0] ^= 1;
    fs::write(blob, bytes).expect("tamper");
    assert!(service
        .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "index.html")
        .is_err());
}

#[test]
fn v2_grants_compile_from_canonical_registry_and_survive_restart() {
    let dir = tempdir().expect("tempdir");
    let manifest = manifest_v2_with_canonical_access();
    let versioned = VersionedManifest::V2(manifest.clone());
    let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &versioned);
    let expected_hash = hash.clone();
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let catalog = CatalogDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        packages: vec![CatalogEntry {
            manifest: versioned,
            archive_url: "https://packages.kosmos.dev/demo-v2.kspkg".into(),
            sha256: hash,
            size,
        }],
        external_apps: vec![],
        revoked: vec![],
    };
    let bytes = document_bytes(&catalog);
    service.apply_catalog(&bytes).expect("catalog");
    service
        .install_from_path(&manifest.id, &manifest.version, &archive)
        .expect("install");
    let listing = service
        .store_installed_listings()
        .expect("installed listing");
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].effective_grants.len(), 1);
    assert_eq!(listing[0].effective_grants[0].type_id, "com.kosmos.note");
    assert_eq!(
        listing[0].effective_grants[0].fields_read,
        ["props.description"]
    );
    assert_eq!(listing[0].effective_grants[0].relations_read, ["related"]);
    service
        .enable(&manifest.id, &manifest.version)
        .expect("enable");
    assert_eq!(
        service
            .launch_app(&manifest.id, Some(&manifest.version))
            .expect("launch")
            .package
            .hash,
        expected_hash
    );

    drop(service);
    let restarted = PackageService::open_for_test(dir.path()).expect("restart");
    let listing = restarted
        .store_installed_listings()
        .expect("restarted listing");
    assert_eq!(listing[0].effective_grants[0].type_id, "com.kosmos.note");
    assert!(restarted
        .launch_app(&manifest.id, Some(&manifest.version))
        .is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn package_definitions_are_archive_bound_registered_in_ark_and_survive_uninstall() {
    let dir = tempdir().expect("tempdir");
    let mut manifest = manifest_v2_with_canonical_access();
    manifest.id = "com.kosmos.demo.definitions".into();
    manifest.name = "Defined v2".into();
    manifest.version = "1.0.0".into();
    manifest.data.access = vec![crate::package_manifest::DataAccessRule {
        type_id: "com.kosmos.demo.definitions.journal".into(),
        versions: "^1.0.0".into(),
        actions: vec![crate::package_manifest::DataAction::Read],
        fields: crate::package_manifest::FieldAccess {
            read: vec!["props.body".into()],
            write: vec![],
        },
        relations: Some(crate::package_manifest::RelationAccess {
            read: vec!["related".into()],
            write: vec![],
        }),
    }];
    manifest.data.defines = vec![crate::package_manifest::DefinitionReference {
        type_id: "com.kosmos.demo.definitions.journal".into(),
        version: "1.0.0".into(),
        schema: "schemas/journal.schema.json".into(),
        content_contract: Some("schemas/journal.content.json".into()),
        relations: Some("schemas/journal.relations.json".into()),
    }];
    manifest.validate().expect("definition manifest");
    let versioned = VersionedManifest::V2(manifest.clone());
    let documents = [
        (
            "schemas/journal.schema.json",
            br#"{"type":"object","properties":{"body":{"type":"string"}}}"# as &[u8],
        ),
        ("schemas/journal.content.json", br#"{"kind":"text"}"#),
        ("schemas/journal.relations.json", br#"[{"type":"related"}]"#),
    ];
    let (archive, hash, size) =
        archive_with_versioned_manifest_and_documents(dir.path(), &versioned, &documents);
    let docs = definition_documents_from_archive(&archive, &manifest).expect("documents");
    let probe = crate::package_registration::PackageRegistrationRegistry::open(
        dir.path().join("probe-definitions"),
    )
    .expect("probe registry");
    probe
        .validate_manifest(&manifest, &docs)
        .expect("definition contract");
    let service = std::sync::Arc::new(PackageService::open_for_test(dir.path()).expect("service"));
    let ark = std::sync::Arc::new(
        crate::ark_host::ArkHost::open(dir.path().join("ark.db").to_str().expect("db path"))
            .await
            .expect("ark host"),
    );
    let dispatcher = package_definition_dispatcher(ark.clone());
    service.configure_package_definition_dispatcher(dispatcher.clone());
    let catalog = CatalogDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        packages: vec![CatalogEntry {
            manifest: versioned,
            archive_url: "https://packages.kosmos.dev/defined.kspkg".into(),
            sha256: hash,
            size,
        }],
        external_apps: vec![],
        revoked: vec![],
    };
    let bytes = document_bytes(&catalog);
    service.apply_catalog(&bytes).expect("catalog");
    {
        let service = service.clone();
        let id = manifest.id.clone();
        let version = manifest.version.clone();
        tokio::task::spawn_blocking(move || service.install_from_path(&id, &version, &archive))
            .await
            .expect("install task")
            .expect("install");
    }
    assert_eq!(
        ark.request(
            "types.get",
            serde_json::json!({ "typeId": manifest.data.defines[0].type_id }),
        )
        .await
        .expect("ARK response")
        .data
        .pointer("/summary/ownerKind")
        .and_then(serde_json::Value::as_str),
        Some("package")
    );
    let listing = service.store_installed_listings().expect("listing");
    assert_eq!(
        listing[0].effective_grants[0].type_id,
        manifest.data.defines[0].type_id
    );
    assert_eq!(listing[0].effective_grants[0].fields_read, ["props.body"]);
    let persisted = dir.path().join("packages/definitions/definitions.json");
    assert!(persisted.is_file());
    service
        .uninstall(&manifest.id, &manifest.version)
        .expect("uninstall");
    assert!(
        persisted.is_file(),
        "definitions are retained after uninstall"
    );
    drop(service);
    drop(dispatcher);
    drop(ark);
    let restarted = PackageService::open_for_test(dir.path()).expect("restart");
    let ark = std::sync::Arc::new(
        crate::ark_host::ArkHost::open(dir.path().join("ark.db").to_str().expect("db path"))
            .await
            .expect("ARK restart"),
    );
    let dispatcher = package_definition_dispatcher(ark.clone());
    let definitions = fs::read_to_string(&persisted).expect("definitions");
    assert!(definitions.contains(&manifest.data.defines[0].type_id));
    restarted.configure_package_definition_dispatcher(dispatcher.clone());
    crate::engine_api::register_package_definitions(&restarted, &dispatcher)
        .await
        .expect("idempotent replay after restart");
    assert_eq!(
        ark.request(
            "types.listVersions",
            serde_json::json!({ "typeId": manifest.data.defines[0].type_id }),
        )
        .await
        .expect("ARK response")
        .data
        .as_array()
        .expect("version list")
        .len(),
        1
    );
    assert!(restarted
        .store_installed_listings()
        .expect("listing")
        .is_empty());
}

#[tokio::test]
async fn ark_conflict_rolls_back_to_the_enabled_package() {
    let dir = tempdir().unwrap();
    let prior = manifest_v2_with_canonical_access();
    let prior_versioned = VersionedManifest::V2(prior.clone());
    let (prior_archive, prior_hash, prior_size) =
        archive_with_versioned_manifest(dir.path(), &prior_versioned);
    let service = std::sync::Arc::new(PackageService::open_for_test(dir.path()).unwrap());
    let initial = CatalogDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        packages: vec![CatalogEntry {
            manifest: prior_versioned,
            archive_url: "https://packages.kosmos.dev/prior.kspkg".into(),
            sha256: prior_hash.clone(),
            size: prior_size,
        }],
        external_apps: vec![],
        revoked: vec![],
    };
    let bytes = document_bytes(&initial);
    service.apply_catalog(&bytes).unwrap();
    service
        .install_from_path(&prior.id, &prior.version, &prior_archive)
        .unwrap();
    service.enable(&prior.id, &prior.version).unwrap();
    let ark = std::sync::Arc::new(
        crate::ark_host::ArkHost::open(dir.path().join("ark.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    service.configure_package_definition_dispatcher(package_definition_dispatcher(ark.clone()));
    let type_id = "com.kosmos.demo.journal";
    assert!(
        ark.request(
            "types.registerPackageDefinitions",
            serde_json::json!({
                "registrations": [{
                    "type_id": type_id,
                    "name": "Foreign",
                    "schema_json": "{}",
                    "ui_schema_json": "{}",
                    "content_contract_json": "{}",
                    "relations_json": "[]",
                    "sync_policy_json": "{}",
                    "version": "1.0.0",
                    "schema_hash": "",
                    "owner_kind": "package",
                    "owner_id": "com.example.foreign",
                    "status": "active",
                    "base_type_id": null,
                    "aliases": [],
                    "created_at": "now"
                }]
            })
        )
        .await
        .unwrap()
        .ok
    );
    let mut replacement = manifest_v2_with_canonical_access();
    replacement.id = prior.id.clone();
    replacement.version = prior.version.clone();
    replacement.data.access[0].fields.read = vec!["props.title".into()];
    replacement.data.defines = vec![crate::package_manifest::DefinitionReference {
        type_id: type_id.into(),
        version: "1.0.0".into(),
        schema: "schema.json".into(),
        content_contract: None,
        relations: None,
    }];
    replacement.validate().unwrap();
    let (archive, hash, size) = archive_with_versioned_manifest_and_documents(
        dir.path(),
        &VersionedManifest::V2(replacement.clone()),
        &[("schema.json", br#"{}"#)],
    );
    let update = CatalogDocument {
        schema_version: 1,
        sequence: 2,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        packages: vec![CatalogEntry {
            manifest: VersionedManifest::V2(replacement),
            archive_url: "https://packages.kosmos.dev/update.kspkg".into(),
            sha256: hash,
            size,
        }],
        external_apps: vec![],
        revoked: vec![],
    };
    let bytes = document_bytes(&update);
    service.apply_catalog(&bytes).unwrap();
    let install = {
        let service = service.clone();
        let id = prior.id.clone();
        let version = prior.version.clone();
        tokio::task::spawn_blocking(move || service.install_from_path(&id, &version, archive))
            .await
            .unwrap()
    };
    assert!(install.is_err());
    assert_eq!(
        service
            .resolve_app(&prior.id, Some(&prior.version))
            .unwrap()
            .package
            .hash,
        prior_hash
    );
    assert!(service.list().unwrap().packages[0].enabled);
    let listing = service.store_installed_listings().unwrap();
    assert_eq!(listing[0].effective_grants.len(), 1);
    assert_eq!(
        listing[0].effective_grants[0].fields_read,
        ["props.description"]
    );
    assert!(
        service
            .package_type_registrations()
            .unwrap()
            .iter()
            .all(|registration| registration.type_id != type_id),
        "a failed install must not persist the rejected definition"
    );
}
