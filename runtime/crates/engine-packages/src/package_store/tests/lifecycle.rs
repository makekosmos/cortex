use super::*;
#[test]
fn enabling_an_app_replaces_its_active_version() {
    let dir = tempdir().unwrap();
    let store = PackageStore::new(dir.path().join("store")).unwrap();
    for version in ["2.0.0", "2.1.0"] {
        let VersionedManifest::V2(mut manifest) = manifest_v2() else {
            panic!("expected v2 manifest");
        };
        manifest.version = version.into();
        let expected = VersionedManifest::V2(manifest);
        let archive_path = dir.path().join(format!("{version}.kspkg"));
        archive_versioned(&archive_path, &expected, "index.html");
        let bytes = fs::read(&archive_path).unwrap();
        store
            .install_versioned(
                &archive_path,
                bytes.len() as u64,
                &hex_hash(&bytes),
                &expected,
                1,
            )
            .unwrap();
        store.enable(expected.id(), version).unwrap();
    }
    let enabled: Vec<_> = store
        .list()
        .unwrap()
        .into_iter()
        .filter(|package| package.enabled)
        .map(|package| package.version)
        .collect();
    assert_eq!(enabled, ["2.1.0"]);
}

#[test]
fn source_stays_disabled_and_revocation_is_state_only() {
    let d = tempdir().unwrap();
    let p = d.path().join("source.kspkg");
    let VersionedManifest::V2(mut m) = manifest_v2() else {
        panic!("expected v2 manifest")
    };
    m.kind = PackageKind::Source;
    m.entrypoint = "worker.exe".into();
    m.targets[0].runtime = crate::package_manifest::TargetRuntime::Worker;
    m.targets[0].os = vec![
        crate::package_manifest::TargetOs::Windows,
        crate::package_manifest::TargetOs::Macos,
        crate::package_manifest::TargetOs::Linux,
    ];
    m.targets[0].entrypoint = Some("worker.exe".into());
    archive(&p, &m, "worker.exe");
    let bytes = fs::read(&p).unwrap();
    let hash = hex_hash(&bytes);
    let s = PackageStore::new(d.path().join("store")).unwrap();
    let expected = VersionedManifest::V2(m.clone());
    s.install_versioned(&p, bytes.len() as u64, &hash, &expected, 1)
        .unwrap();
    assert!(matches!(
        s.enable(&m.id, &m.version),
        Err(StoreError::WorkerRequired)
    ));
    s.enable_worker(&m.id, &m.version).unwrap();
    assert!(s.installed(&m.id, &m.version).unwrap().enabled);
    s.reconcile_revocations(vec![(m.id.clone(), m.version.clone(), hash.clone())])
        .unwrap();
    let item = &s.list().unwrap()[0];
    assert!(item.revoked && !item.enabled);
    assert!(d
        .path()
        .join("store/blobs")
        .join(format!("{hash}.kspkg"))
        .exists());
}

#[test]
fn enabling_a_worker_replaces_its_active_version_atomically() {
    let d = tempdir().unwrap();
    let store = PackageStore::new(d.path().join("store")).unwrap();
    for version in ["1.0.0", "2.0.0"] {
        let VersionedManifest::V2(mut manifest) = manifest_v2() else {
            panic!("expected v2 manifest");
        };
        manifest.kind = PackageKind::Source;
        manifest.version = version.into();
        manifest.entrypoint = "worker.exe".into();
        manifest.targets[0].runtime = crate::package_manifest::TargetRuntime::Worker;
        manifest.targets[0].os = vec![
            crate::package_manifest::TargetOs::Windows,
            crate::package_manifest::TargetOs::Macos,
            crate::package_manifest::TargetOs::Linux,
        ];
        manifest.targets[0].entrypoint = Some("worker.exe".into());
        let expected = VersionedManifest::V2(manifest);
        let archive_path = d.path().join(format!("{version}.kspkg"));
        archive_versioned(&archive_path, &expected, "worker.exe");
        let bytes = fs::read(&archive_path).unwrap();
        store
            .install_versioned(
                &archive_path,
                bytes.len() as u64,
                &hex_hash(&bytes),
                &expected,
                1,
            )
            .unwrap();
        if version == "2.0.0" {
            store.fail_next_state_write.store(true, Ordering::Relaxed);
            assert!(store.enable_worker(expected.id(), version).is_err());
            assert!(store.installed(expected.id(), "1.0.0").unwrap().enabled);
            assert!(!store.installed(expected.id(), version).unwrap().enabled);
        } else {
            store.enable_worker(expected.id(), version).unwrap();
        }
    }
    let store_root = d.path().join("store");
    drop(store);
    let store = PackageStore::new(store_root).unwrap();
    store.enable_worker("com.kosmos.v2-demo", "2.0.0").unwrap();
    let enabled = store
        .list()
        .unwrap()
        .into_iter()
        .filter(|package| package.id == "com.kosmos.v2-demo" && package.enabled)
        .map(|package| package.version)
        .collect::<Vec<_>>();
    assert_eq!(enabled, ["2.0.0"]);
}
