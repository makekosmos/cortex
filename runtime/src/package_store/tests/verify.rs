use super::*;

use crate::package_store::verify::full_blob_hash_count;
#[test]
fn rejects_state_manifest_tampering_against_immutable_archive() {
    let d = tempdir().unwrap();
    let p = d.path().join("v2.kspkg");
    let expected = manifest_v2();
    archive_versioned(&p, &expected, "index.html");
    let bytes = fs::read(&p).unwrap();
    let store = PackageStore::new(d.path().join("store")).unwrap();
    store
        .install_versioned(&p, bytes.len() as u64, &hex_hash(&bytes), &expected, 1)
        .unwrap();
    let state_path = store.state_path();
    let tampered = fs::read_to_string(&state_path)
        .unwrap()
        .replace("V2 Demo", "Tampered");
    fs::write(state_path, tampered).unwrap();
    assert!(matches!(store.list(), Err(StoreError::ManifestMismatch)));
}

#[test]
fn verifies_declared_icon_path_against_the_immutable_archive() {
    let dir = tempdir().unwrap();
    let archive_path = dir.path().join("icon.kspkg");
    let VersionedManifest::V2(mut manifest) = manifest_v2() else {
        panic!("expected v2 manifest");
    };
    manifest.icon = Some("icon.ico".into());
    let expected = VersionedManifest::V2(manifest);
    let file = fs::File::create(&archive_path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default();
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(&serde_json::to_vec(&expected).unwrap())
        .unwrap();
    zip.start_file("index.html", options).unwrap();
    zip.write_all(b"ok").unwrap();
    zip.start_file("icon.ico", options).unwrap();
    zip.write_all(b"icon").unwrap();
    zip.finish().unwrap();
    let bytes = fs::read(&archive_path).unwrap();
    let store = PackageStore::new(dir.path().join("store")).unwrap();
    let package = store
        .install_versioned(
            &archive_path,
            bytes.len() as u64,
            &hex_hash(&bytes),
            &expected,
            1,
        )
        .unwrap();

    let icon = store.immutable_asset_path(&package, "icon.ico").unwrap();
    assert!(icon.is_file());
    assert!(store.immutable_asset_path(&package, "../icon.ico").is_err());
    fs::write(&icon, b"tampered").unwrap();
    assert!(matches!(
        store.immutable_asset_path(&package, "icon.ico"),
        Err(StoreError::HashMismatch)
    ));
}

#[test]
fn immutable_worker_entrypoint_rejects_store_tampering() {
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
    let store = PackageStore::new(d.path().join("store")).unwrap();
    let expected = VersionedManifest::V2(m.clone());
    let installed = store
        .install_versioned(&p, bytes.len() as u64, &hash, &expected, 1)
        .unwrap();
    assert!(store.immutable_entrypoint(&installed).is_ok());
    fs::write(
        d.path()
            .join("store/unpacked")
            .join(&m.id)
            .join(&m.version)
            .join(&hash)
            .join("worker.exe"),
        b"tampered",
    )
    .unwrap();
    assert!(matches!(
        store.immutable_entrypoint(&installed),
        Err(StoreError::HashMismatch)
    ));
}

#[test]
fn entrypoint_verify_binds_identity_at_install_and_skips_rehash() {
    let d = tempdir().unwrap();
    let store = PackageStore::new(d.path().join("store")).unwrap();
    let (installed, entrypoint) = install_worker(d.path(), &store);
    let blob = blob_path(d.path(), &installed);
    assert!(identity::record_path(&blob).is_file());
    PackageStore::verify_immutable_entrypoint_path(&entrypoint).unwrap();
    PackageStore::verify_immutable_entrypoint_path(&entrypoint).unwrap();
    assert_eq!(full_blob_hash_count(&blob), 0);
}

#[test]
fn entrypoint_verify_full_hashes_once_when_identity_record_missing() {
    let d = tempdir().unwrap();
    let store = PackageStore::new(d.path().join("store")).unwrap();
    let (installed, entrypoint) = install_worker(d.path(), &store);
    let blob = blob_path(d.path(), &installed);
    fs::remove_file(identity::record_path(&blob)).unwrap();
    PackageStore::verify_immutable_entrypoint_path(&entrypoint).unwrap();
    assert_eq!(full_blob_hash_count(&blob), 1);
    assert!(identity::load(&blob).is_some());
    PackageStore::verify_immutable_entrypoint_path(&entrypoint).unwrap();
    assert_eq!(full_blob_hash_count(&blob), 1);
}

#[test]
fn entrypoint_verify_heals_corrupt_identity_record() {
    let d = tempdir().unwrap();
    let store = PackageStore::new(d.path().join("store")).unwrap();
    let (installed, entrypoint) = install_worker(d.path(), &store);
    let blob = blob_path(d.path(), &installed);
    fs::write(identity::record_path(&blob), b"not json").unwrap();
    PackageStore::verify_immutable_entrypoint_path(&entrypoint).unwrap();
    assert_eq!(full_blob_hash_count(&blob), 1);
    assert!(identity::load(&blob).is_some());
}

#[test]
fn entrypoint_verify_refuses_blob_tampered_after_install() {
    let d = tempdir().unwrap();
    let store = PackageStore::new(d.path().join("store")).unwrap();
    let (installed, entrypoint) = install_worker(d.path(), &store);
    fs::write(blob_path(d.path(), &installed), b"tampered").unwrap();
    assert!(matches!(
        PackageStore::verify_immutable_entrypoint_path(&entrypoint),
        Err(StoreError::HashMismatch)
    ));
}

#[test]
fn entrypoint_verify_survives_failed_identity_record_write() {
    let d = tempdir().unwrap();
    let store = PackageStore::new(d.path().join("store")).unwrap();
    let (installed, entrypoint) = install_worker(d.path(), &store);
    let blob = blob_path(d.path(), &installed);
    fs::remove_file(identity::record_path(&blob)).unwrap();
    identity::FAIL_NEXT_STORE.store(true, Ordering::Relaxed);
    // The verified handle is returned even when the record cannot be
    // written; the launch just keeps paying the full hash.
    PackageStore::verify_immutable_entrypoint_path(&entrypoint).unwrap();
    assert_eq!(full_blob_hash_count(&blob), 1);
    assert!(!identity::record_path(&blob).exists());
    PackageStore::verify_immutable_entrypoint_path(&entrypoint).unwrap();
    assert_eq!(full_blob_hash_count(&blob), 2);
    assert!(identity::load(&blob).is_some());
    PackageStore::verify_immutable_entrypoint_path(&entrypoint).unwrap();
    assert_eq!(full_blob_hash_count(&blob), 2);
}

#[test]
fn entrypoint_verify_reverifies_and_rebinds_swapped_blob() {
    let d = tempdir().unwrap();
    let store = PackageStore::new(d.path().join("store")).unwrap();
    let (installed, entrypoint) = install_worker(d.path(), &store);
    let blob = blob_path(d.path(), &installed);
    // Same verified bytes behind a *different* file (new file ID): the
    // recorded identity misses, the full hash re-runs and re-binds.
    let copy = d.path().join("copy.kspkg");
    fs::copy(&blob, &copy).unwrap();
    fs::rename(&copy, &blob).unwrap();
    PackageStore::verify_immutable_entrypoint_path(&entrypoint).unwrap();
    assert_eq!(full_blob_hash_count(&blob), 1);
    PackageStore::verify_immutable_entrypoint_path(&entrypoint).unwrap();
    assert_eq!(full_blob_hash_count(&blob), 1);
}
