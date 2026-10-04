#[test]
fn development_app_install_uses_the_validated_archive_manifest() {
    let dir = tempdir().expect("temp dir");
    let manifest = VersionedManifest::V2(manifest_v2_with_canonical_access());
    let (archive, _, _) = archive_with_versioned_manifest(dir.path(), &manifest);
    let service = PackageService::open_for_test(dir.path()).expect("service");

    let installed = service
        .install_development_app_from_path("com.kosmos.demo", "2.0.0", &archive)
        .expect("development install");

    assert!(installed.enabled);
    assert_eq!(installed.id, "com.kosmos.demo");
    assert_eq!(installed.version, "2.0.0");
}

#[tokio::test]
async fn development_app_can_be_disabled_and_re_enabled_without_catalog() {
    let dir = tempdir().expect("temp dir");
    let manifest = VersionedManifest::V2(manifest_v2_with_canonical_access());
    let (archive, _, _) = archive_with_versioned_manifest(dir.path(), &manifest);
    let service = PackageService::open_for_test(dir.path()).expect("service");

    service
        .install_development_app_from_path("com.kosmos.demo", "2.0.0", &archive)
        .expect("development install");

    let disabled = service
        .set_enabled("com.kosmos.demo", "2.0.0", false)
        .await
        .expect("disable");
    assert!(!disabled.enabled);

    let enabled = service
        .set_enabled("com.kosmos.demo", "2.0.0", true)
        .await
        .expect("re-enable");
    assert!(enabled.enabled);
}

#[test]
fn replacement_verification_is_read_only_and_checks_exact_archive_identity() {
    let dir = tempdir().expect("temp dir");
    let (service, _, hash) = enabled_app_service(dir.path());
    let verified = service
        .verify_installed_app("com.kosmos.demo", "1.0.0", &hash, 1)
        .expect("verified replacement");
    assert_eq!(verified.hash, hash);
    assert!(service
        .verify_installed_app("com.kosmos.demo", "1.0.0", "00", 1)
        .is_err());
    assert!(service
        .verify_installed_app("com.kosmos.demo", "1.0.0", &hash, 2)
        .is_err());
    let blob = dir
        .path()
        .join("packages/blobs")
        .join(format!("{hash}.kspkg"));
    fs::write(blob, b"tampered").expect("tamper immutable blob");
    assert!(service
        .verify_installed_app("com.kosmos.demo", "1.0.0", &hash, 1)
        .is_err());
}

#[tokio::test]
async fn uninstall_preserves_package_state_for_reinstall() {
    let dir = tempdir().expect("temp dir");
    let (service, _, _) = enabled_app_service(dir.path());
    let state = service
        .storage_root()
        .join("package-state/com.kosmos.demo/state.json");
    fs::create_dir_all(state.parent().expect("state parent")).expect("state directory");
    fs::write(&state, b"persist across reinstall").expect("state write");

    service
        .uninstall_with_worker_stop("com.kosmos.demo", "1.0.0")
        .await
        .expect("uninstall");

    assert_eq!(
        fs::read(state).expect("retained package state"),
        b"persist across reinstall"
    );
}

#[test]
fn failed_update_restores_revoked_record_without_reenabling_it() {
    let dir = tempdir().unwrap();
    let prior = manifest_v2_with_canonical_access();
    let prior_versioned = VersionedManifest::V2(prior.clone());
    let (prior_archive, prior_hash, prior_size) =
        archive_with_versioned_manifest(dir.path(), &prior_versioned);
    let service = PackageService::open_for_test(dir.path()).unwrap();
    let mut initial = catalog(1, prior_hash.clone(), prior_size, "2030-01-01T00:00:00Z");
    initial.packages[0].manifest = prior_versioned;
    service.apply_catalog(document_bytes(&initial)).unwrap();
    service
        .install_from_path(&prior.id, &prior.version, &prior_archive)
        .unwrap();
    let mut revoking = initial;
    revoking.sequence = 2;
    revoking.revoked = vec![PackageRevocation {
        id: prior.id.clone(),
        version: prior.version.clone(),
        sha256: prior_hash,
        reason: Some("test".into()),
    }];
    service.apply_catalog(document_bytes(&revoking)).unwrap();
    let before = service.store.installed(&prior.id, &prior.version).unwrap();
    assert!(before.revoked);
    assert!(!before.enabled);

    let mut replacement = prior.clone();
    replacement.data.access[0].type_id = "com.kosmos.unknown".into();
    let replacement_versioned = VersionedManifest::V2(replacement.clone());
    let (replacement_archive, replacement_hash, replacement_size) =
        archive_with_versioned_manifest(dir.path(), &replacement_versioned);
    let mut update = catalog(
        3,
        replacement_hash,
        replacement_size,
        "2030-01-01T00:00:00Z",
    );
    update.packages[0].manifest = replacement_versioned;
    service.apply_catalog(document_bytes(&update)).unwrap();

    assert!(service
        .install_from_path(&replacement.id, &replacement.version, replacement_archive)
        .is_err());
    assert_eq!(
        service.store.installed(&prior.id, &prior.version).unwrap(),
        before,
        "rollback must preserve revoked/enabled/timestamp/catalog metadata"
    );
}

#[test]
fn restart_gate_requires_the_exact_prior_package_record() {
    let dir = tempdir().unwrap();
    let (archive, hash, size) = archive(dir.path());
    let store = PackageStore::new(dir.path().join("store")).unwrap();
    store
        .install_versioned(&archive, size, &hash, &VersionedManifest::V2(manifest()), 1)
        .unwrap();
    store.enable("com.kosmos.demo", "1.0.0").unwrap();
    let previous = store.installed("com.kosmos.demo", "1.0.0").unwrap();

    assert!(restored_package_record_matches(
        &store,
        &previous,
        &previous.id,
        &previous.version
    ));
    store.disable(&previous.id, &previous.version).unwrap();
    assert!(!restored_package_record_matches(
        &store,
        &previous,
        &previous.id,
        &previous.version
    ));
}

#[test]
fn invalid_installed_v2_contract_does_not_brick_restart() {
    let dir = tempdir().expect("tempdir");
    let mut manifest = manifest_v2_with_canonical_access();
    manifest.data.access[0].type_id = "com.kosmos.unknown".into();
    let versioned = VersionedManifest::V2(manifest.clone());
    let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &versioned);
    let service = PackageService::open_for_test(dir.path()).expect("service");
    service
        .store
        .install_versioned(&archive, size, &hash, &versioned, 1)
        .expect("install fixture");
    service
        .store
        .enable(&manifest.id, &manifest.version)
        .expect("enable fixture");
    drop(service);

    let restarted = PackageService::open_for_test(dir.path()).expect("restart");
    assert!(!restarted.list().expect("list").packages[0].enabled);
    assert!(restarted.store_installed_listings().expect("listing")[0]
        .effective_grants
        .is_empty());
    assert!(restarted.enable(&manifest.id, &manifest.version).is_err());
}

#[test]
fn legacy_v1_package_cannot_launch_or_survive_restart_enabled() {
    let dir = tempdir().expect("tempdir");
    let legacy = legacy_manifest();
    let (archive, hash, size) = archive_with_manifest(dir.path(), &legacy);
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let mut legacy_catalog = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
    legacy_catalog.packages[0].manifest = VersionedManifest::V1(legacy.clone());
    service
        .apply_catalog(document_bytes(&legacy_catalog))
        .expect("legacy catalog");
    assert!(service
        .install_from_path("com.kosmos.demo", "1.0.0", &archive)
        .is_err());
    assert!(service
        .store
        .install(&archive, size, &hash, &legacy, 1)
        .is_err());
    assert!(service.list().expect("list").packages.is_empty());
}

#[test]
fn source_package_never_resolves_as_app() {
    let dir = tempdir().expect("tempdir");
    let mut source = manifest();
    source.kind = PackageKind::Source;
    let versioned = VersionedManifest::V2(source.clone());
    let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &versioned);
    let service = PackageService::open(dir.path()).expect("service");
    service
        .store
        .install_versioned(&archive, size, &hash, &versioned, 1)
        .expect("install source");
    assert!(service.launch_app("com.kosmos.demo", None).is_err());
}
