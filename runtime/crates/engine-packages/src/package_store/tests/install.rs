use super::*;
#[test]
fn installs_and_preserves_previous_on_bad_update() {
    let d = tempdir().unwrap();
    let p = d.path().join("a.kspkg");
    let m = manifest_v2();
    archive(&p, &m, "index.html");
    let bytes = fs::read(&p).unwrap();
    let s = PackageStore::new(d.path().join("store")).unwrap();
    assert!(s
        .install_versioned(&p, bytes.len() as u64, &hex_hash(&bytes), &m, 1)
        .is_ok());
    assert!(s
        .install_versioned(&p, bytes.len() as u64, "00", &m, 2)
        .is_err());
    assert_eq!(s.list().unwrap().len(), 1);
}

#[test]
fn installs_and_reloads_v2_manifest_without_downgrade() {
    let d = tempdir().unwrap();
    let p = d.path().join("v2.kspkg");
    let expected = manifest_v2();
    archive_versioned(&p, &expected, "index.html");
    let bytes = fs::read(&p).unwrap();
    let hash = hex_hash(&bytes);
    let store_root = d.path().join("store");
    let store = PackageStore::new(&store_root).unwrap();
    store
        .install_versioned(&p, bytes.len() as u64, &hash, &expected, 1)
        .unwrap();
    assert_eq!(
        store
            .installed("com.kosmos.v2-demo", "2.0.0")
            .unwrap()
            .manifest,
        expected
    );
    drop(store);
    let reopened = PackageStore::new(&store_root).unwrap();
    assert_eq!(
        reopened
            .installed("com.kosmos.v2-demo", "2.0.0")
            .unwrap()
            .manifest,
        expected
    );
}

#[test]
fn rejects_v1_at_both_install_boundaries() {
    let d = tempdir().unwrap();
    let p = d.path().join("v1.kspkg");
    let legacy = manifest();
    archive(&p, &legacy, "index.html");
    let bytes = fs::read(&p).unwrap();
    let store = PackageStore::new(d.path().join("store")).unwrap();
    assert!(store
        .install(&p, bytes.len() as u64, &hex_hash(&bytes), &legacy, 1)
        .is_err());
    assert!(store
        .install_versioned(
            &p,
            bytes.len() as u64,
            &hex_hash(&bytes),
            &VersionedManifest::V1(legacy),
            1,
        )
        .is_err());
    assert!(!store.state_path().exists());
}
#[test]
fn rejects_traversal() {
    let d = tempdir().unwrap();
    let p = d.path().join("a.kspkg");
    let f = fs::File::create(&p).unwrap();
    let mut z = ZipWriter::new(f);
    z.start_file("../evil", FileOptions::default()).unwrap();
    z.write_all(b"x").unwrap();
    z.finish().unwrap();
    let s = PackageStore::new(d.path().join("store")).unwrap();
    assert!(s
        .install_versioned(
            &p,
            fs::metadata(&p).unwrap().len(),
            &hex_hash(&fs::read(&p).unwrap()),
            &manifest_v2(),
            1
        )
        .is_err());
}

#[test]
fn rejects_windows_unsafe_and_case_collision_paths_without_state() {
    for (label, names) in [
        ("case", vec!["Foo", "foo"]),
        ("file-directory-case", vec!["index.html", "INDEX.HTML/"]),
        ("ads", vec!["name:stream"]),
        ("device", vec!["CON.txt"]),
    ] {
        let d = tempdir().unwrap();
        let p = d.path().join(format!("{label}.kspkg"));
        let f = fs::File::create(&p).unwrap();
        let mut z = ZipWriter::new(f);
        for name in names {
            z.start_file(name, FileOptions::default()).unwrap();
            z.write_all(b"x").unwrap();
        }
        z.finish().unwrap();
        let bytes = fs::read(&p).unwrap();
        let store = PackageStore::new(d.path().join("store")).unwrap();
        assert!(store
            .install_versioned(&p, bytes.len() as u64, &hex_hash(&bytes), &manifest_v2(), 1)
            .is_err());
        assert!(!store.state_path().exists());
        assert!(fs::read_dir(d.path().join("store")).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".staging-")
        }));
    }
}

#[test]
fn atomic_state_write_failure_preserves_previous_active_package() {
    let dir = tempdir().unwrap();
    let first = dir.path().join("first.kspkg");
    let second = dir.path().join("second.kspkg");
    let manifest = manifest_v2();
    archive(&first, &manifest, "index.html");
    archive(&second, &manifest, "index.html");
    fs::OpenOptions::new()
        .append(true)
        .open(&second)
        .unwrap()
        .write_all(b"different immutable bytes")
        .unwrap();

    let store = PackageStore::new(dir.path().join("store")).unwrap();
    let first_bytes = fs::read(&first).unwrap();
    let original = store
        .install_versioned(
            &first,
            first_bytes.len() as u64,
            &hex_hash(&first_bytes),
            &manifest_v2(),
            1,
        )
        .unwrap();
    store.fail_next_state_write.store(true, Ordering::Relaxed);
    let second_bytes = fs::read(&second).unwrap();
    assert!(store
        .install_versioned(
            &second,
            second_bytes.len() as u64,
            &hex_hash(&second_bytes),
            &manifest,
            2,
        )
        .is_err());
    assert_eq!(store.list().unwrap()[0].hash, original.hash);
}

#[test]
fn rejects_declared_expanded_size_bomb() {
    let dir = tempdir().unwrap();
    let archive = dir.path().join("bomb.kspkg");
    let file = fs::File::create(&archive).unwrap();
    let mut zip = ZipWriter::new(file);
    zip.start_file("manifest.json", FileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&manifest_v2()).unwrap())
        .unwrap();
    zip.start_file("index.html", FileOptions::default())
        .unwrap();
    zip.write_all(b"ok").unwrap();
    zip.start_file("bomb.bin", FileOptions::default()).unwrap();
    zip.write_all(b"x").unwrap();
    zip.finish().unwrap();

    let mut bytes = fs::read(&archive).unwrap();
    let central = central_entry(&bytes, b"bomb.bin");
    let local = u32::from_le_bytes(bytes[central + 42..central + 46].try_into().unwrap()) as usize;
    let declared = (MAX_EXPANDED + 1) as u32;
    bytes[central + 24..central + 28].copy_from_slice(&declared.to_le_bytes());
    bytes[local + 22..local + 26].copy_from_slice(&declared.to_le_bytes());
    fs::write(&archive, &bytes).unwrap();

    let store = PackageStore::new(dir.path().join("store")).unwrap();
    assert!(matches!(
        store.install_versioned(
            &archive,
            bytes.len() as u64,
            &hex_hash(&bytes),
            &manifest_v2(),
            1
        ),
        Err(StoreError::Archive(message)) if message == "expanded archive too large"
    ));
    assert!(!store.state_path().exists());
}

#[test]
fn rejects_actual_symlink_metadata_without_state() {
    let dir = tempdir().unwrap();
    let archive = dir.path().join("symlink.kspkg");
    let file = fs::File::create(&archive).unwrap();
    let mut zip = ZipWriter::new(file);
    zip.start_file("manifest.json", FileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&manifest_v2()).unwrap())
        .unwrap();
    zip.start_file("index.html", FileOptions::default())
        .unwrap();
    zip.write_all(b"ok").unwrap();
    zip.start_file("link", FileOptions::default()).unwrap();
    zip.write_all(b"index.html").unwrap();
    zip.finish().unwrap();

    let mut bytes = fs::read(&archive).unwrap();
    let central = central_entry(&bytes, b"link");
    bytes[central + 5] = 3;
    bytes[central + 38..central + 42].copy_from_slice(&((0o120777u32) << 16).to_le_bytes());
    fs::write(&archive, &bytes).unwrap();
    let store = PackageStore::new(dir.path().join("store")).unwrap();
    assert!(matches!(
        store.install_versioned(
            &archive,
            bytes.len() as u64,
            &hex_hash(&bytes),
            &manifest_v2(),
            1
        ),
        Err(StoreError::Archive(message)) if message == "symlink rejected"
    ));
    assert!(!store.state_path().exists());
}

#[test]
fn rejects_non_regular_file_metadata_without_state() {
    let dir = tempdir().unwrap();
    let archive_path = dir.path().join("device.kspkg");
    let manifest = manifest();
    archive(&archive_path, &manifest, "index.html");

    let mut bytes = fs::read(&archive_path).unwrap();
    let central = central_entry(&bytes, b"index.html");
    bytes[central + 5] = 3;
    bytes[central + 38..central + 42].copy_from_slice(&((0o020644u32) << 16).to_le_bytes());
    fs::write(&archive_path, &bytes).unwrap();

    let store = PackageStore::new(dir.path().join("store")).unwrap();
    assert!(matches!(
        store.install_versioned(
            &archive_path,
            bytes.len() as u64,
            &hex_hash(&bytes),
            &manifest_v2(),
            1
        ),
        Err(StoreError::Archive(message)) if message == "unsupported file type"
    ));
    assert!(!store.state_path().exists());
}

#[test]
fn rejects_entry_limit_and_manifest_mismatch_without_state() {
    for (label, build) in [
        (
            "entry-limit",
            Box::new(|zip: &mut ZipWriter<fs::File>| {
                for index in 0..=MAX_ENTRIES {
                    zip.start_file(format!("entry-{index}"), FileOptions::default())
                        .unwrap();
                }
            }) as Box<dyn Fn(&mut ZipWriter<fs::File>)>,
        ),
        (
            "manifest-mismatch",
            Box::new(|zip: &mut ZipWriter<fs::File>| {
                let mut wrong = manifest();
                wrong.version = "2.0.0".into();
                zip.start_file("manifest.json", FileOptions::default())
                    .unwrap();
                zip.write_all(&serde_json::to_vec(&wrong).unwrap()).unwrap();
                zip.start_file("index.html", FileOptions::default())
                    .unwrap();
            }),
        ),
    ] {
        let dir = tempdir().unwrap();
        let archive = dir.path().join(format!("{label}.kspkg"));
        let mut zip = ZipWriter::new(fs::File::create(&archive).unwrap());
        build(&mut zip);
        zip.finish().unwrap();
        let bytes = fs::read(&archive).unwrap();
        let store = PackageStore::new(dir.path().join("store")).unwrap();
        assert!(store
            .install_versioned(
                &archive,
                bytes.len() as u64,
                &hex_hash(&bytes),
                &manifest_v2(),
                1
            )
            .is_err());
        assert!(!store.state_path().exists());
    }
}
