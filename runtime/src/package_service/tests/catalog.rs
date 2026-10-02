#[test]
fn missing_compile_time_trust_fails_closed_without_blocking_engine() {
    let dir = tempdir().expect("tempdir");
    let service = PackageService::from_parts(
        dir.path().join("packages"),
        None,
        Some(dir.path().join("apps")),
    )
    .expect("service");
    assert!(!service.trust_summary().configured);
    assert_eq!(
        service.trust_summary().fault_code.as_deref(),
        Some("package_trust_unavailable")
    );
}

#[test]
fn production_open_uses_pinned_trust() {
    let dir = tempdir().expect("tempdir");
    let summary = PackageService::open(dir.path())
        .expect("service")
        .trust_summary();
    assert!(summary.configured);
    assert_eq!(summary.trusted_release_keys, 1);
    assert_eq!(summary.revoked_release_keys, 0);
}

#[test]
fn signed_catalog_applies_and_replay_or_tamper_fails() {
    let dir = tempdir().expect("tempdir");
    let (trust_store, _, release) = trust();
    let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
    let doc = catalog(1, "a".repeat(64), 1, "2030-01-01T00:00:00Z");
    let (bytes, signatures) = signed(&doc, "release-1", &release);
    assert_eq!(
        service
            .apply_catalog(&bytes, signatures.clone())
            .expect("catalog")
            .sequence,
        1
    );
    let app_catalog = service
        .catalog_packages(Some(&PackageKind::App))
        .expect("app catalog");
    assert_eq!(app_catalog.len(), 1);
    assert_eq!(app_catalog[0].archive_size, 1);
    assert!(service
        .catalog_packages(Some(&PackageKind::Source))
        .expect("source catalog")
        .is_empty());
    assert!(matches!(
        service.apply_catalog(&bytes, signatures),
        Err(PackageError::Trust(TrustError::Replay))
    ));
    let mut tampered = bytes;
    tampered[0] ^= 1;
    let (_, signatures) = signed(&doc, "release-1", &release);
    assert!(service.apply_catalog(tampered, signatures).is_err());
}

#[test]
fn expired_or_revoked_catalog_clears_cache_but_accepts_fresh_catalog() {
    let dir = tempdir().expect("tempdir");
    let (trust_store, root, release) = trust();
    let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
    let mut expired = catalog(1, "a".repeat(64), 1, "2025-01-01T00:00:00Z");
    expired.issued_at = "2024-01-01T00:00:00Z".into();
    let (expired_bytes, expired_signatures) = signed(&expired, "release-1", &release);
    assert!(service
        .apply_catalog(&expired_bytes, expired_signatures)
        .is_err());
    let (release_two, release_two_key) = key(3, "release-2");
    let transition = KeyTransitionDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        old_key_id: "release-1".into(),
        new_key: release_two_key,
    };
    let (transition_bytes, mut transition_signatures) = signed(&transition, "release-1", &release);
    transition_signatures.signatures.push(DetachedSignature {
        key_id: "release-2".into(),
        algorithm: "ed25519".into(),
        signature: STANDARD.encode(release_two.sign(&transition_bytes).to_bytes()),
    });
    service
        .apply_transition(&transition_bytes, transition_signatures)
        .expect("transition");
    let valid = catalog(2, "b".repeat(64), 1, "2030-01-01T00:00:00Z");
    let (valid_bytes, valid_signatures) = signed(&valid, "release-1", &release);
    service
        .apply_catalog(&valid_bytes, valid_signatures)
        .expect("valid catalog");
    let revoke = crate::package_trust::RevocationDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2029-01-01T00:00:00Z".into(),
        revoked_release_keys: vec!["release-1".into()],
        revoked_packages: vec![],
    };
    let (revoke_bytes, revoke_signatures) = signed(&revoke, "root", &root);
    service
        .apply_revocations(&revoke_bytes, revoke_signatures)
        .expect("revocation");
    assert!(service.catalog_summary().is_none());
    assert_eq!(
        service.trust_summary().fault_code.as_deref(),
        Some("catalog_unavailable")
    );
    let fresh = catalog(3, "c".repeat(64), 1, "2030-01-01T00:00:00Z");
    let (fresh_bytes, fresh_signatures) = signed(&fresh, "release-2", &release_two);
    assert_eq!(
        service
            .apply_catalog(fresh_bytes, fresh_signatures)
            .expect("fresh catalog")
            .sequence,
        3
    );
}

#[test]
fn transition_and_revocation_replay_are_rejected() {
    let dir = tempdir().expect("tempdir");
    let (trust_store, root, release) = trust();
    let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
    let (release_two, release_two_key) = key(3, "release-2");
    let transition = KeyTransitionDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2029-01-01T00:00:00Z".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        old_key_id: "release-1".into(),
        new_key: release_two_key,
    };
    let (bytes, mut signatures) = signed(&transition, "release-1", &release);
    signatures.signatures.push(DetachedSignature {
        key_id: "release-2".into(),
        algorithm: "ed25519".into(),
        signature: STANDARD.encode(release_two.sign(&bytes).to_bytes()),
    });
    service
        .apply_transition(&bytes, signatures.clone())
        .expect("transition");
    assert!(service.apply_transition(&bytes, signatures).is_err());
    let revocation = crate::package_trust::RevocationDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2029-01-01T00:00:00Z".into(),
        revoked_release_keys: vec!["release-1".into()],
        revoked_packages: vec![],
    };
    let (bytes, signatures) = signed(&revocation, "root", &root);
    service
        .apply_revocations(&bytes, signatures.clone())
        .expect("revocation");
    assert!(service.apply_revocations(&bytes, signatures).is_err());
}

#[test]
fn catalog_bound_archive_installs_and_enable_refuses_hash_mismatch() {
    let dir = tempdir().expect("tempdir");
    let (archive, hash, size) = archive(dir.path());
    let (trust_store, _, release) = trust();
    let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
    let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
    let (bytes, signatures) = signed(&doc, "release-1", &release);
    service.apply_catalog(bytes, signatures).expect("catalog");
    assert_eq!(
        service
            .install_from_path("com.kosmos.demo", "1.0.0", &archive)
            .expect("install")
            .id,
        "com.kosmos.demo"
    );
    service.enable("com.kosmos.demo", "1.0.0").expect("enable");

    let replacement = catalog(2, "c".repeat(64), size, "2030-01-01T00:00:00Z");
    let (bytes, signatures) = signed(&replacement, "release-1", &release);
    service
        .apply_catalog(bytes, signatures)
        .expect("replacement catalog");
    assert!(service.enable("com.kosmos.demo", "1.0.0").is_err());
}

#[test]
fn root_signed_exact_revocation_marks_installed_package() {
    let dir = tempdir().expect("tempdir");
    let (archive, hash, size) = archive(dir.path());
    let (trust_store, root, release) = trust();
    let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
    let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
    let (bytes, signatures) = signed(&doc, "release-1", &release);
    service.apply_catalog(bytes, signatures).expect("catalog");
    service
        .install_from_path("com.kosmos.demo", "1.0.0", archive)
        .expect("install");
    let revoke = crate::package_trust::RevocationDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2029-01-01T00:00:00Z".into(),
        revoked_release_keys: vec![],
        revoked_packages: vec![PackageRevocation {
            id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            sha256: hash,
        }],
    };
    let (bytes, signatures) = signed(&revoke, "root", &root);
    service
        .apply_revocations(&bytes, signatures)
        .expect("revoke");
    assert!(service.list().expect("list").packages[0].revoked);
    let (fresh_trust, _, _) = trust();
    let restarted = PackageService::open_with_trust(dir.path(), fresh_trust).expect("restart");
    assert!(restarted.list().expect("restarted list").packages[0].revoked);
}
