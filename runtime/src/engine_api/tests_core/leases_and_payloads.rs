use super::*;

#[test]
pub(super) fn launch_scoped_token_is_independent_and_revoked_with_lease() {
    let mut leases = LaunchLeaseRegistry::default();
    let lease = leases
        .create_with_typed_grant(
            AssetGrant {
                id: "com.kosmos.app".into(),
                version: "1.0.0".into(),
                hash: "a".repeat(64),
            },
            app_test_grant(),
        )
        .expect("typed lease");
    let token = lease.launch_token.clone().expect("launch token");
    assert_ne!(token, lease.asset_token);
    assert!(leases.typed_grant(&lease.launch_id, &token).is_some());
    assert!(leases.typed_grant(&lease.launch_id, "wrong").is_none());
    assert!(leases.revoke(&lease.launch_id));
    assert!(leases.typed_grant(&lease.launch_id, &token).is_none());
}

#[test]
pub(super) fn asset_paths_decode_traversal_and_headers_are_deterministic() {
    assert_eq!(
        percent_decode("dist%2Findex.html").as_deref(),
        Some("dist/index.html")
    );
    assert_eq!(
        percent_decode("%2e%2e%2Fsecret").as_deref(),
        Some("../secret")
    );
    assert!(percent_decode("bad%ZZ").is_none());

    let html = asset_response("index.html", b"<html />".to_vec());
    assert_eq!(
        html.headers().get(CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );
    assert_eq!(html.headers().get("cache-control").unwrap(), "no-store");
    assert_eq!(
        html.headers().get("referrer-policy").unwrap(),
        "no-referrer"
    );
    assert!(html.headers().contains_key("content-security-policy"));

    let css = asset_response("styles.css", b"body{}".to_vec());
    assert_eq!(
        css.headers().get(CONTENT_TYPE).unwrap(),
        "text/css; charset=utf-8"
    );
    assert!(!css.headers().contains_key("content-security-policy"));
}

#[test]
pub(super) fn launch_payload_redacts_store_fields() {
    let package = crate::package_store::InstalledPackage {
        id: "com.kosmos.demo".into(),
        version: "1.0.0".into(),
        hash: "a".repeat(64),
        manifest: crate::package_manifest::VersionedManifest::V1(
            crate::package_manifest::PackageManifest {
                schema_version: 1,
                id: "com.kosmos.demo".into(),
                name: "Demo".into(),
                version: "1.0.0".into(),
                kind: crate::package_manifest::PackageKind::App,
                engine_api: ">=0.1.0".into(),
                entrypoint: "index.html".into(),
                publisher: "kosmos".into(),
                permissions: vec![],
            },
        ),
        enabled: true,
        revoked: false,
        installed_at: 1,
        catalog_sequence: 1,
    };
    let lease = LaunchLeaseRegistry::default()
        .create(AssetGrant {
            id: package.id.clone(),
            version: package.version.clone(),
            hash: package.hash.clone(),
        })
        .expect("lease");
    let body = launch_payload(1234, &lease, &package, LAUNCH_LEASE_TTL).to_string();
    assert!(body.contains(&lease.asset_token));
    assert!(!body.contains(&package.hash));
    for forbidden in ["bearer", "signature", "public_key", "\\\\blobs\\\\"] {
        assert!(!body.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
pub(super) fn launch_payload_exposes_only_granted_agents_events() {
    let package = crate::package_store::InstalledPackage {
        id: "com.kosmos.daedalus".into(),
        version: "0.1.0".into(),
        hash: "a".repeat(64),
        manifest: crate::package_manifest::VersionedManifest::V1(
            crate::package_manifest::PackageManifest {
                schema_version: 1,
                id: "com.kosmos.daedalus".into(),
                name: "Daedalus".into(),
                version: "0.1.0".into(),
                kind: crate::package_manifest::PackageKind::App,
                engine_api: ">=0.1.0".into(),
                entrypoint: "index.html".into(),
                publisher: "kosmos".into(),
                permissions: vec![],
            },
        ),
        enabled: true,
        revoked: false,
        installed_at: 1,
        catalog_sequence: 1,
    };
    let lease = LaunchLeaseRegistry::default()
        .create_with_typed_grant(
            AssetGrant {
                id: package.id.clone(),
                version: package.version.clone(),
                hash: package.hash.clone(),
            },
            agents_test_grant(),
        )
        .expect("lease");
    let body = launch_payload(1234, &lease, &package, LAUNCH_LEASE_TTL);
    assert_eq!(body["data"]["effective_events"], json!(["agents_event"]));
    assert_eq!(body["data"]["effective_read_types"], json!([]));

    let mut write_only_grant = agents_test_grant();
    write_only_grant.capabilities = vec![crate::runtime_grants::ScopedCapability::Agents {
        operations: vec!["agents.sessions.send".into()],
    }];
    let lease = LaunchLeaseRegistry::default()
        .create_with_typed_grant(
            AssetGrant {
                id: package.id.clone(),
                version: package.version.clone(),
                hash: package.hash.clone(),
            },
            write_only_grant,
        )
        .expect("write-only lease");
    let body = launch_payload(1234, &lease, &package, LAUNCH_LEASE_TTL);
    assert_eq!(body["data"]["effective_events"], json!([]));
}

#[test]
pub(super) fn launch_leases_are_unique_and_independently_revocable() {
    let mut leases = LaunchLeaseRegistry::default();
    let first = leases
        .create(AssetGrant {
            id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            hash: "a".repeat(64),
        })
        .expect("first lease");
    let second = leases
        .create(AssetGrant {
            id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            hash: "a".repeat(64),
        })
        .expect("second lease");
    assert_ne!(first.launch_id, second.launch_id);
    assert_ne!(first.asset_token, second.asset_token);
    assert!(leases.asset(&first.asset_token).is_some());
    assert!(leases.revoke(&first.launch_id));
    assert!(leases.asset(&first.asset_token).is_none());
    assert!(leases.asset(&second.asset_token).is_some());
    assert!(!leases.revoke("not-a-launch-id"));
}

#[test]
pub(super) fn lease_registry_purges_expired_before_capacity_and_never_evicts_live_leases() {
    let mut leases = LaunchLeaseRegistry::with_limits(Duration::from_secs(1), 2);
    let first = leases
        .try_create_at(
            AssetGrant {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                hash: "a".repeat(64),
            },
            Instant::now(),
        )
        .expect("first live lease");
    let second = leases
        .try_create_at(
            AssetGrant {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                hash: "b".repeat(64),
            },
            Instant::now(),
        )
        .expect("second live lease");
    assert!(matches!(
        leases.try_create_at(
            AssetGrant {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                hash: "c".repeat(64),
            },
            Instant::now(),
        ),
        Err(LeaseCapacityError)
    ));
    assert!(leases.asset(&first.asset_token).is_some());
    assert!(leases.asset(&second.asset_token).is_some());
    leases.purge_expired_at(Instant::now() + Duration::from_secs(2));
    assert_eq!(leases.len(), 0);
}

#[test]
pub(super) fn resolve_payload_is_manifest_only_and_never_contains_launch_fields() {
    let package = crate::package_store::InstalledPackage {
        id: "com.kosmos.demo".into(),
        version: "1.0.0".into(),
        hash: "a".repeat(64),
        manifest: crate::package_manifest::VersionedManifest::V1(
            crate::package_manifest::PackageManifest {
                schema_version: 1,
                id: "com.kosmos.demo".into(),
                name: "Demo".into(),
                version: "1.0.0".into(),
                kind: crate::package_manifest::PackageKind::App,
                engine_api: ">=0.1.0".into(),
                entrypoint: "index.html".into(),
                publisher: "kosmos".into(),
                permissions: vec![],
            },
        ),
        enabled: true,
        revoked: false,
        installed_at: 1,
        catalog_sequence: 1,
    };
    let body = resolve_payload(&package).to_string();
    assert!(body.contains("\"id\":\"com.kosmos.demo\""));
    for launch_only in [
        "launch_url",
        "launch_id",
        "asset_token",
        "expires_at",
        "ttl_seconds",
    ] {
        assert!(!body.contains(launch_only), "resolve leaked {launch_only}");
    }
}

#[test]
pub(super) fn repeated_resolve_payloads_never_evict_an_active_lease() {
    let mut leases = LaunchLeaseRegistry::default();
    let lease = leases
        .create(AssetGrant {
            id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            hash: "a".repeat(64),
        })
        .expect("lease");
    let package = crate::package_store::InstalledPackage {
        id: "com.kosmos.demo".into(),
        version: "1.0.0".into(),
        hash: "a".repeat(64),
        manifest: crate::package_manifest::VersionedManifest::V1(
            crate::package_manifest::PackageManifest {
                schema_version: 1,
                id: "com.kosmos.demo".into(),
                name: "Demo".into(),
                version: "1.0.0".into(),
                kind: crate::package_manifest::PackageKind::App,
                engine_api: ">=0.1.0".into(),
                entrypoint: "index.html".into(),
                publisher: "kosmos".into(),
                permissions: vec![],
            },
        ),
        enabled: true,
        revoked: false,
        installed_at: 1,
        catalog_sequence: 1,
    };
    for _ in 0..10_000 {
        assert!(resolve_payload(&package).get("data").is_some());
    }
    assert!(leases.asset(&lease.asset_token).is_some());
}
