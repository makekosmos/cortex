#[test]
fn fresh_service_opens_catalog_free_and_ready() {
    let dir = tempdir().expect("tempdir");
    let service = PackageService::open_for_test(dir.path()).expect("service");
    assert!(service.catalog_summary().is_none());
    assert!(service.catalog_fault().is_none());
    assert!(matches!(
        service.catalog_packages(None),
        Err(PackageError::CatalogUnavailable)
    ));
}

#[test]
fn catalog_applies_and_replay_or_tamper_fails() {
    let dir = tempdir().expect("tempdir");
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let doc = catalog(1, "a".repeat(64), 1, "2030-01-01T00:00:00Z");
    let bytes = document_bytes(&doc);
    assert_eq!(service.apply_catalog(&bytes).expect("catalog").sequence, 1);
    let app_catalog = service
        .catalog_packages(Some(&PackageKind::App))
        .expect("app catalog");
    assert_eq!(app_catalog.len(), 1);
    assert_eq!(app_catalog[0].archive_size, 1);
    assert!(service
        .catalog_packages(Some(&PackageKind::Source))
        .expect("source catalog")
        .is_empty());
    // Re-fetching the same document is an idempotent no-op, not a replay.
    assert_eq!(service.apply_catalog(&bytes).expect("re-apply").sequence, 1);
    // An older document is rejected as a rollback.
    let older = catalog(0, "b".repeat(64), 1, "2030-01-01T00:00:00Z");
    assert!(matches!(
        service.apply_catalog(document_bytes(&older)),
        Err(PackageError::Catalog(CatalogError::Replay))
            | Err(PackageError::Catalog(CatalogError::Invalid(_)))
    ));
    let mut tampered = bytes;
    tampered[0] ^= 1;
    assert!(service.apply_catalog(tampered).is_err());
}

#[test]
fn expired_catalog_is_rejected() {
    let dir = tempdir().expect("tempdir");
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let mut expired = catalog(1, "a".repeat(64), 1, "2025-01-01T00:00:00Z");
    expired.issued_at = "2024-01-01T00:00:00Z".into();
    assert!(matches!(
        service.apply_catalog(document_bytes(&expired)),
        Err(PackageError::Catalog(CatalogError::Expired))
    ));
    assert!(service.catalog_summary().is_none());
}

#[test]
fn persisted_catalog_reloads_on_restart() {
    let dir = tempdir().expect("tempdir");
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let doc = catalog(1, "a".repeat(64), 1, "2030-01-01T00:00:00Z");
    service
        .apply_catalog(document_bytes(&doc))
        .expect("catalog");
    drop(service);
    let restarted = PackageService::open_for_test(dir.path()).expect("restart");
    assert_eq!(restarted.catalog_summary().expect("summary").sequence, 1);
    assert_eq!(restarted.catalog_packages(None).expect("packages").len(), 1);
}

#[test]
fn catalog_bound_archive_installs_and_enable_refuses_hash_mismatch() {
    let dir = tempdir().expect("tempdir");
    let (archive, hash, size) = archive(dir.path());
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
    service
        .apply_catalog(document_bytes(&doc))
        .expect("catalog");
    assert_eq!(
        service
            .install_from_path("com.kosmos.demo", "1.0.0", &archive)
            .expect("install")
            .id,
        "com.kosmos.demo"
    );
    service.enable("com.kosmos.demo", "1.0.0").expect("enable");

    let replacement = catalog(2, "c".repeat(64), size, "2030-01-01T00:00:00Z");
    service
        .apply_catalog(document_bytes(&replacement))
        .expect("replacement catalog");
    assert!(service.enable("com.kosmos.demo", "1.0.0").is_err());
}

#[tokio::test]
async fn install_from_catalog_verifies_size_and_sha256() {
    let dir = tempdir().expect("tempdir");
    let (archive_path, hash, size) = archive(dir.path());
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let mut doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
    doc.packages[0].archive_url = reqwest::Url::from_file_path(&archive_path)
        .expect("archive url")
        .to_string();
    service
        .apply_catalog(document_bytes(&doc))
        .expect("catalog");
    assert_eq!(
        service
            .install_from_catalog("com.kosmos.demo", "1.0.0")
            .await
            .expect("install")
            .id,
        "com.kosmos.demo"
    );

    // A catalog whose size does not match the bytes must refuse the install.
    let dir2 = tempdir().expect("tempdir2");
    let (archive2, hash2, _) = archive(dir2.path());
    let service2 = PackageService::open_for_test(dir2.path()).expect("service2");
    let mut doc2 = catalog(1, hash2, size + 1, "2030-01-01T00:00:00Z");
    doc2.packages[0].archive_url = reqwest::Url::from_file_path(&archive2)
        .expect("archive url")
        .to_string();
    service2
        .apply_catalog(document_bytes(&doc2))
        .expect("catalog2");
    assert!(service2
        .install_from_catalog("com.kosmos.demo", "1.0.0")
        .await
        .is_err());

    // And a sha256 mismatch too.
    let dir3 = tempdir().expect("tempdir3");
    let (archive3, _, size3) = archive(dir3.path());
    let service3 = PackageService::open_for_test(dir3.path()).expect("service3");
    let mut doc3 = catalog(1, "0".repeat(64), size3, "2030-01-01T00:00:00Z");
    doc3.packages[0].archive_url = reqwest::Url::from_file_path(&archive3)
        .expect("archive url")
        .to_string();
    service3
        .apply_catalog(document_bytes(&doc3))
        .expect("catalog3");
    assert!(service3
        .install_from_catalog("com.kosmos.demo", "1.0.0")
        .await
        .is_err());
}

#[test]
fn catalog_revocation_marks_installed_package_across_restart() {
    let dir = tempdir().expect("tempdir");
    let (archive, hash, size) = archive(dir.path());
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
    service
        .apply_catalog(document_bytes(&doc))
        .expect("catalog");
    service
        .install_from_path("com.kosmos.demo", "1.0.0", archive)
        .expect("install");
    let mut revoked = catalog(2, hash.clone(), size, "2030-01-01T00:00:00Z");
    revoked.revoked = vec![PackageRevocation {
        id: "com.kosmos.demo".into(),
        version: "1.0.0".into(),
        sha256: hash,
        reason: Some("test".into()),
    }];
    service
        .apply_catalog(document_bytes(&revoked))
        .expect("revoking catalog");
    assert!(service.list().expect("list").packages[0].revoked);
    drop(service);
    let restarted = PackageService::open_for_test(dir.path()).expect("restart");
    assert!(restarted.list().expect("restarted list").packages[0].revoked);
}

/// KOS-320 migration: a data dir written by the signed-catalog engine keeps
/// every installed package while the trust files are dropped on sight.
#[test]
fn legacy_trust_state_is_dropped_but_installed_packages_survive() {
    let dir = tempdir().expect("tempdir");
    // Build an installed state with the current engine, then plant the files
    // the signed pipeline persisted.
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let (archive, hash, size) = archive(dir.path());
    let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
    service
        .apply_catalog(document_bytes(&doc))
        .expect("catalog");
    service
        .install_from_path("com.kosmos.demo", "1.0.0", archive)
        .expect("install");
    service.enable("com.kosmos.demo", "1.0.0").expect("enable");
    drop(service);

    let root = dir.path().join("packages");
    // Old signed envelope persisted as catalog.json, plus per-document
    // transition/revocation envelopes and the store dir's signed catalog.
    let envelope = serde_json::json!({
        "bytes": "dGVzdA==",
        "signatures": {"schema_version": 1, "signatures": [{
            "key_id": "old-release-key",
            "algorithm": "ed25519",
            "signature": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        }]}
    });
    fs::write(
        root.join("catalog.json"),
        serde_json::to_vec(&envelope).unwrap(),
    )
    .expect("legacy catalog");
    fs::write(
        root.join("transition-00000000000000000001.json"),
        serde_json::to_vec(&envelope).unwrap(),
    )
    .expect("legacy transition");
    fs::write(
        root.join("revocation-00000000000000000001.json"),
        serde_json::to_vec(&envelope).unwrap(),
    )
    .expect("legacy revocation");
    fs::create_dir_all(dir.path().join("store")).expect("store dir");
    fs::write(
        dir.path().join("store").join("catalog.json"),
        serde_json::to_vec(&envelope).unwrap(),
    )
    .expect("legacy store catalog");

    let restarted = PackageService::open_for_test(dir.path()).expect("restart");
    // Installed package is untouched: still enabled, still listed.
    let installed = restarted
        .store
        .installed("com.kosmos.demo", "1.0.0")
        .expect("installed");
    assert!(installed.enabled);
    assert!(!installed.revoked);
    // Every trust-era file is gone; no catalog is loaded.
    for name in [
        "catalog.json",
        "transition-00000000000000000001.json",
        "revocation-00000000000000000001.json",
    ] {
        assert!(!root.join(name).exists(), "{name} must be dropped");
    }
    assert!(!dir.path().join("store").join("catalog.json").exists());
    assert!(restarted.catalog_summary().is_none());
    assert!(matches!(
        restarted.enable("com.kosmos.demo", "1.0.0"),
        Err(PackageError::CatalogUnavailable)
    ));
    // A fresh catalog applies cleanly on top of the migrated state.
    let doc = catalog(1, hash, size, "2030-01-01T00:00:00Z");
    restarted
        .apply_catalog(document_bytes(&doc))
        .expect("new catalog");
    assert_eq!(restarted.catalog_summary().expect("summary").sequence, 1);
}

#[test]
fn store_catalog_lists_packages_and_external_apps() {
    let dir = tempdir().expect("tempdir");
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let mut doc = catalog(1, "a".repeat(64), 1, "2030-01-01T00:00:00Z");
    doc.packages[0].manifest = {
        let mut manifest = manifest();
        manifest.icon = Some("icon.png".into());
        manifest.store = Some(crate::package_manifest::ManifestStore {
            description: Some("Демо".into()),
            categories: vec!["integrations".into()],
            connects_to: Some("external.demo".into()),
            data_compatibility: vec![],
        });
        VersionedManifest::V2(manifest)
    };
    doc.external_apps = vec![crate::catalog::ExternalAppListing {
        id: "external.demo".into(),
        name: "Demo".into(),
        publisher: "Demo Inc".into(),
        publisher_tier: crate::catalog::PublisherTier::Verified,
        description: "d".into(),
        categories: vec!["education".into()],
        platforms: vec![crate::catalog::Platform::Windows],
        official_url: "https://demo.example".into(),
        icon_url: None,
        data_compatibility: vec![],
    }];
    service
        .apply_catalog(document_bytes(&doc))
        .expect("catalog");
    let dto = service.store_catalog(Utc::now(), vec![]);
    assert_eq!(dto.state, "fresh");
    assert_eq!(dto.listings.len(), 2);
    let listing = dto
        .listings
        .iter()
        .find(|l| l.id == "com.kosmos.demo")
        .expect("package listing");
    assert_eq!(listing.description, "Демо");
    assert_eq!(
        listing.icon_url.as_deref(),
        Some(concat!(
            "https://github.com/makekosmos/integrations",
            "/releases/latest/download/icon-com.kosmos.demo.png"
        ))
    );
    assert_eq!(
        service
            .store_external_url("external.demo")
            .expect("external url"),
        "https://demo.example"
    );
}
