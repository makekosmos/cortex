use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;
use tempfile::TempDir;
use zip::{write::FileOptions, ZipWriter};

fn spec(version: &str, archive: &Path) -> NativeInstallSpec {
    let bytes = fs::read(archive).unwrap();
    NativeInstallSpec {
        id: "com.kosmos.agenda".into(),
        version: version.into(),
        executable: "agenda-gpui.exe".into(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        size: bytes.len() as u64,
        repository: "makekosmos/agenda-gpui".into(),
        release_tag: format!("v{version}"),
    }
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = ZipWriter::new(file);
    for (name, data) in entries {
        zip.start_file(*name, FileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn install_flips_pointer_and_cleans_old_versions() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let archive = dir.path().join("app.zip");
    write_zip(
        &archive,
        &[("agenda-gpui.exe", b"exe"), ("LICENSE", b"license")],
    );
    let first = store
        .install_archive(&spec("0.1.0", &archive), &archive)
        .unwrap();
    assert_eq!(first.version, "0.1.0");
    assert_eq!(store.current("com.kosmos.agenda").unwrap().version, "0.1.0");
    assert!(store
        .executable_path("com.kosmos.agenda")
        .unwrap()
        .is_file());

    // Version bump in-place: same archive, new version spec.
    let second = store
        .install_archive(&spec("0.2.0", &archive), &archive)
        .unwrap();
    assert_eq!(second.version, "0.2.0");
    let app_dir = dir.path().join("Apps").join("com.kosmos.agenda");
    let dirs: Vec<_> = fs::read_dir(&app_dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0].file_name().to_str().unwrap(), "0.2.0");
    assert!(!app_dir.join("0.1.0").exists());
}

#[test]
fn bad_sha_leaves_previous_version_current() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let archive = dir.path().join("app.zip");
    write_zip(&archive, &[("agenda-gpui.exe", b"exe")]);
    store
        .install_archive(&spec("0.1.0", &archive), &archive)
        .unwrap();

    let mut bad = spec("0.2.0", &archive);
    bad.sha256 = "0".repeat(64);
    assert!(matches!(
        store.install_archive(&bad, &archive),
        Err(NativeAppError::HashMismatch)
    ));
    let current = store.current("com.kosmos.agenda").unwrap();
    assert_eq!(current.version, "0.1.0");
    assert!(store
        .executable_path("com.kosmos.agenda")
        .unwrap()
        .is_file());
}

#[test]
fn bad_archive_leaves_previous_version_current() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let good = dir.path().join("good.zip");
    write_zip(&good, &[("agenda-gpui.exe", b"exe")]);
    store.install_archive(&spec("0.1.0", &good), &good).unwrap();

    // Traversal entry fails the archive-safety gate mid-extraction.
    let evil = dir.path().join("evil.zip");
    {
        let file = fs::File::create(&evil).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("agenda-gpui.exe", FileOptions::default())
            .unwrap();
        zip.write_all(b"exe").unwrap();
        zip.add_directory("../escape", FileOptions::default())
            .unwrap();
        zip.finish().unwrap();
    }
    let bad = spec("0.2.0", &evil);
    let result = store.install_archive(&bad, &evil);
    assert!(result.is_err());
    assert_eq!(store.current("com.kosmos.agenda").unwrap().version, "0.1.0");
    assert!(store
        .root()
        .join("com.kosmos.agenda")
        .join("0.1.0")
        .join("agenda-gpui.exe")
        .is_file());
    assert!(!dir.path().join("Apps").join("escape").exists());

    // A valid zip without the declared executable fails too.
    let missing = dir.path().join("missing.zip");
    write_zip(&missing, &[("other.exe", b"exe")]);
    let spec_missing = spec("0.2.0", &missing);
    assert!(store.install_archive(&spec_missing, &missing).is_err());
    assert_eq!(store.current("com.kosmos.agenda").unwrap().version, "0.1.0");
}

#[test]
fn uninstall_removes_everything() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let archive = dir.path().join("app.zip");
    write_zip(&archive, &[("agenda-gpui.exe", b"exe")]);
    store
        .install_archive(&spec("0.1.0", &archive), &archive)
        .unwrap();
    assert_eq!(store.list().len(), 1);
    store.uninstall("com.kosmos.agenda").unwrap();
    assert!(store.list().is_empty());
    assert!(store.current("com.kosmos.agenda").is_none());
    assert!(!dir.path().join("Apps").join("com.kosmos.agenda").exists());
    // Uninstall is idempotent.
    store.uninstall("com.kosmos.agenda").unwrap();
}

#[test]
fn corrupt_record_reads_as_absent() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let app_dir = dir.path().join("Apps").join("com.kosmos.agenda");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join(INSTALL_FILE), b"not json").unwrap();
    assert!(store.current("com.kosmos.agenda").is_none());
    store.uninstall("com.kosmos.agenda").unwrap();
    assert!(!app_dir.exists());
}

// --- Release metadata (KOS-265): sums parsing, tag/version agreement, ---
// --- target line selection, redirect Location → tag extraction.        ---

include!("releases_tests.rs");
