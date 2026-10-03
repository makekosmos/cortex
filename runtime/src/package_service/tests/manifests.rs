#[test]
fn six_provider_manifests_install_with_typed_grants() {
    let dir = tempdir().expect("tempdir");
    let mut packages = Vec::new();
    let mut archives = Vec::new();
    // Manifests pinned from makekosmos/integrations 66f9040 (the repo the
    // provider sources moved to); runtime only needs their parsed shape.
    for package in [
        "bigfrontend",
        "greatfrontend",
        "leetcode",
        "codewars",
        "hevy",
        "toggl",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(format!("{package}.package.manifest.json"));
        let raw = fs::read_to_string(path).unwrap_or_else(|_| panic!("{package} manifest"));
        let VersionedManifest::V2(manifest) =
            PackageManifest::parse(&raw).unwrap_or_else(|_| panic!("valid {package} manifest"))
        else {
            panic!("{package} must use a v2 manifest");
        };
        let package_dir = dir.path().join(package);
        fs::create_dir_all(&package_dir).expect("package tempdir");
        let (archive, hash, size) =
            archive_with_versioned_manifest(&package_dir, &VersionedManifest::V2(manifest.clone()));
        packages.push(CatalogEntry {
            manifest: VersionedManifest::V2(manifest.clone()),
            archives: vec![catalog_archive(
                format!("https://packages.kosmos.dev/{package}.kspkg"),
                hash,
                size,
            )],
        });
        archives.push((manifest.id, manifest.version, archive));
    }

    let service = PackageService::open_for_test(dir.path()).expect("service");
    let catalog = CatalogDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        packages,
        external_apps: vec![],
        revoked: vec![],
    };
    let bytes = document_bytes(&catalog);
    service.apply_catalog(&bytes).expect("catalog");
    for (id, version, archive) in archives {
        service
            .install_from_path(&id, &version, archive)
            .unwrap_or_else(|_| panic!("install {id}@{version}"));
    }
    let listings = service.store_installed_listings().expect("listings");
    assert_eq!(listings.len(), 6);
    assert!(listings
        .iter()
        .all(|listing| !listing.effective_grants.is_empty()));
}

#[test]
fn provider_package_manifests_use_the_generic_integration_contract() {
    // Manifests pinned from makekosmos/integrations 66f9040 (the repo the
    // provider sources moved to); runtime only needs their parsed shape.
    for package in [
        "bigfrontend",
        "greatfrontend",
        "leetcode",
        "codewars",
        "hevy",
        "toggl",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(format!("{package}.package.manifest.json"));
        let raw = fs::read_to_string(path).unwrap_or_else(|_| panic!("{package} manifest"));
        let VersionedManifest::V2(manifest) =
            PackageManifest::parse(&raw).unwrap_or_else(|_| panic!("valid {package} manifest"))
        else {
            panic!("{package} must use a v2 manifest");
        };
        assert_eq!(manifest.kind, PackageKind::Source);
        assert!(manifest.icon.is_some(), "{package} icon");
        assert!(manifest.targets.iter().any(|target| {
            target.runtime == TargetRuntime::Worker && target.os.contains(&TargetOs::Windows)
        }));
        assert!(manifest.integration.is_some(), "{package} integration");
    }
}

#[test]
fn dictation_package_manifest_has_only_the_required_engine_grants() {
    // Pinned from Dictation source 3225cea and artifact SHA256
    // a7eaf9c84ee63fc01799df531ba0469390a20c4a4df6e37f1c9901af8a4c5fd5.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dictation-0.2.4.package.manifest.json");
    let raw = fs::read_to_string(path).expect("Dictation package manifest");
    let VersionedManifest::V2(manifest) =
        PackageManifest::parse(&raw).expect("valid Dictation manifest")
    else {
        panic!("Dictation must be a v2 package");
    };
    let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "test-digest")
        .expect("compiled Dictation grant");
    for operation in [
        "dictation.get_state",
        "dictation.get_config",
        "dictation.list_local_models",
        "dictation.update_config",
        "dictation.start_recording",
        "dictation.cancel",
    ] {
        assert!(grant.allows_dictation_operation(operation), "{operation}");
    }
    for operation in ["dictation.submit_audio", "dictation.delete_config"] {
        assert!(!grant.allows_dictation_operation(operation), "{operation}");
    }
}
