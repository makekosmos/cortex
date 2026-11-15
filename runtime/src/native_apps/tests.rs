use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;
use tempfile::TempDir;
use zip::{write::FileOptions, ZipWriter};

fn spec(version: &str, archive: &Path) -> NativeInstallSpec {
    spec_for(version, archive, "test-app.exe")
}

fn spec_for(version: &str, archive: &Path, executable: &str) -> NativeInstallSpec {
    let bytes = fs::read(archive).unwrap();
    NativeInstallSpec {
        id: "computer.mundus.test-app".into(),
        version: version.into(),
        executable: executable.into(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        size: bytes.len() as u64,
        repository: "makekosmos/test-app".into(),
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

fn write_tar_gz(path: &Path, entries: &[(&str, u32, &[u8])]) {
    let encoder =
        flate2::write::GzEncoder::new(fs::File::create(path).unwrap(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);
    for (name, mode, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(*mode);
        header.set_size(data.len() as u64);
        header.set_cksum();
        builder.append_data(&mut header, name, *data).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap();
}

/// Raw ustar entry writer — `tar::Builder` refuses `..` names, so the
/// traversal fixture is emitted by hand (header + data padded to 512).
fn raw_tar_entry(out: &mut Vec<u8>, name: &str, data: &[u8]) {
    let mut header = [0u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    header[100..107].copy_from_slice(b"0000644");
    header[124..135].copy_from_slice(format!("{:011o}", data.len()).as_bytes());
    header[136..147].copy_from_slice(b"00000000000");
    header[148..156].copy_from_slice(b"        ");
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    let sum: u32 = header.iter().map(|byte| *byte as u32).sum();
    header[148..156].copy_from_slice(format!("{:06o}\0 ", sum).as_bytes());
    out.extend_from_slice(&header);
    out.extend_from_slice(data);
    out.resize(out.len() + (512 - data.len() % 512) % 512, 0);
}

fn write_raw_tar_gz(path: &Path, entries: &[(&str, &[u8])]) {
    let mut tar_bytes = Vec::new();
    for (name, data) in entries {
        raw_tar_entry(&mut tar_bytes, name, data);
    }
    tar_bytes.extend_from_slice(&[0u8; 1024]);
    let mut encoder =
        flate2::write::GzEncoder::new(fs::File::create(path).unwrap(), flate2::Compression::fast());
    encoder.write_all(&tar_bytes).unwrap();
    encoder.finish().unwrap();
}

fn append_tar_link(builder: &mut tar::Builder<flate2::write::GzEncoder<fs::File>>, name: &str) {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_mode(0o777);
    header.set_size(0);
    header.set_cksum();
    builder.append_link(&mut header, name, "target").unwrap();
}

/// The darwin release layout: `<Name>.app` bundle at the root with the
/// binary under `Contents/MacOS`. `spec_for` mirrors what
/// `run_native_install_inner` builds from `desc.executable(target)`.
/// Unix-only: Windows record hygiene requires a `*.exe` executable, so the
/// `.app` path is rejected by validation before extraction there.
#[cfg(unix)]
#[test]
fn install_from_tarball_extracts_bundle_and_keeps_exec_bit() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let archive = dir.path().join("app.tar.gz");
    let executable = "Test App.app/Contents/MacOS/test-app";
    // The bundle binary may arrive 0644 in the archive — the extractor must
    // still land the declared executable runnable.
    write_tar_gz(
        &archive,
        &[
            ("Test App.app/Contents/Info.plist", 0o644, b"plist"),
            (executable, 0o644, b"binary"),
        ],
    );
    store
        .install_archive(&spec_for("0.2.0", &archive, executable), &archive)
        .unwrap();
    let installed = store.executable_path("computer.mundus.test-app").unwrap();
    assert!(installed.is_file());
    assert_eq!(fs::read(&installed).unwrap(), b"binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&installed).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
}

#[test]
fn tarball_safety_gate_rejects_traversal_links_and_missing_exe() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    // Windows record hygiene only accepts `*.exe`; darwin tarballs carry the
    // `.app` bundle path. Either way the safety gate must run.
    let executable = if cfg!(windows) {
        "test-app.exe"
    } else {
        "Test App.app/Contents/MacOS/test-app"
    };

    // `..` escapes staging — rejected like the zip traversal case.
    let traversal = dir.path().join("evil.tar.gz");
    write_raw_tar_gz(
        &traversal,
        &[(executable, b"binary"), ("../escape", b"bad")],
    );
    assert!(store
        .install_archive(&spec_for("0.1.0", &traversal, executable), &traversal)
        .is_err());
    assert!(store.current("computer.mundus.test-app").unwrap().is_none());
    assert!(!dir.path().join("Apps").join("escape").exists());

    // Symlinks are rejected outright — same rule as zip unix-mode markers.
    let linked = dir.path().join("linked.tar.gz");
    {
        let encoder = flate2::write::GzEncoder::new(
            fs::File::create(&linked).unwrap(),
            flate2::Compression::fast(),
        );
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o755);
        header.set_size(6);
        header.set_cksum();
        builder
            .append_data(&mut header, executable, b"binary".as_slice())
            .unwrap();
        append_tar_link(&mut builder, "Test App.app/link");
        builder.into_inner().unwrap().finish().unwrap();
    }
    assert!(store
        .install_archive(&spec_for("0.1.0", &linked, executable), &linked)
        .is_err());

    // A valid tarball without the declared executable fails too.
    let missing = dir.path().join("missing.tar.gz");
    write_tar_gz(&missing, &[("other.bin", 0o644, b"x")]);
    assert!(store
        .install_archive(&spec_for("0.1.0", &missing, executable), &missing)
        .is_err());
}

#[test]
fn executable_path_matches_each_target_layout() {
    let app = &TEST_APP;
    assert_eq!(app.executable("x86_64-pc-windows-msvc"), "test-app.exe");
    assert_eq!(
        app.executable("aarch64-apple-darwin"),
        "Test App.app/Contents/MacOS/test-app"
    );
    assert_eq!(
        app_descriptor("computer.mundus.test-app")
            .map(|app| app.executable("x86_64-pc-windows-msvc")),
        None,
        "the product ships no hardcoded apps any more"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn host_app_target_is_the_published_darwin_triple() {
    assert_eq!(host_app_target(), Some("aarch64-apple-darwin"));
}

#[test]
fn install_flips_pointer_and_cleans_old_versions() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let archive = dir.path().join("app.zip");
    write_zip(
        &archive,
        &[("test-app.exe", b"exe"), ("LICENSE", b"license")],
    );
    let first = store
        .install_archive(&spec("0.1.0", &archive), &archive)
        .unwrap();
    assert_eq!(first.version, "0.1.0");
    assert_eq!(
        store
            .current("computer.mundus.test-app")
            .unwrap()
            .unwrap()
            .version,
        "0.1.0"
    );
    assert!(store
        .executable_path("computer.mundus.test-app")
        .unwrap()
        .is_file());

    // Version bump in-place: same archive, new version spec.
    let second = store
        .install_archive(&spec("0.2.0", &archive), &archive)
        .unwrap();
    assert_eq!(second.version, "0.2.0");
    let app_dir = dir.path().join("Apps").join("computer.mundus.test-app");
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
    write_zip(&archive, &[("test-app.exe", b"exe")]);
    store
        .install_archive(&spec("0.1.0", &archive), &archive)
        .unwrap();

    let mut bad = spec("0.2.0", &archive);
    bad.sha256 = "0".repeat(64);
    assert!(matches!(
        store.install_archive(&bad, &archive),
        Err(NativeAppError::HashMismatch)
    ));
    let current = store.current("computer.mundus.test-app").unwrap();
    assert_eq!(current.expect("record").version, "0.1.0");
    assert!(store
        .executable_path("computer.mundus.test-app")
        .unwrap()
        .is_file());
}

#[test]
fn bad_archive_leaves_previous_version_current() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let good = dir.path().join("good.zip");
    write_zip(&good, &[("test-app.exe", b"exe")]);
    store.install_archive(&spec("0.1.0", &good), &good).unwrap();

    // Traversal entry fails the archive-safety gate mid-extraction.
    let evil = dir.path().join("evil.zip");
    {
        let file = fs::File::create(&evil).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("test-app.exe", FileOptions::default())
            .unwrap();
        zip.write_all(b"exe").unwrap();
        zip.add_directory("../escape", FileOptions::default())
            .unwrap();
        zip.finish().unwrap();
    }
    let bad = spec("0.2.0", &evil);
    let result = store.install_archive(&bad, &evil);
    assert!(result.is_err());
    assert_eq!(
        store
            .current("computer.mundus.test-app")
            .unwrap()
            .unwrap()
            .version,
        "0.1.0"
    );
    assert!(store
        .root()
        .join("computer.mundus.test-app")
        .join("0.1.0")
        .join("test-app.exe")
        .is_file());
    assert!(!dir.path().join("Apps").join("escape").exists());

    // A valid zip without the declared executable fails too.
    let missing = dir.path().join("missing.zip");
    write_zip(&missing, &[("other.exe", b"exe")]);
    let spec_missing = spec("0.2.0", &missing);
    assert!(store.install_archive(&spec_missing, &missing).is_err());
    assert_eq!(
        store
            .current("computer.mundus.test-app")
            .unwrap()
            .unwrap()
            .version,
        "0.1.0"
    );
}

#[test]
fn uninstall_removes_everything() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let archive = dir.path().join("app.zip");
    write_zip(&archive, &[("test-app.exe", b"exe")]);
    store
        .install_archive(&spec("0.1.0", &archive), &archive)
        .unwrap();
    assert_eq!(store.list().len(), 1);
    store.uninstall("computer.mundus.test-app").unwrap();
    assert!(store.list().is_empty());
    assert!(store.current("computer.mundus.test-app").unwrap().is_none());
    assert!(!dir
        .path()
        .join("Apps")
        .join("computer.mundus.test-app")
        .exists());
    // Uninstall is idempotent.
    store.uninstall("computer.mundus.test-app").unwrap();
}

#[test]
fn corrupt_record_reads_as_absent() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let app_dir = dir.path().join("Apps").join("computer.mundus.test-app");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join(INSTALL_FILE), b"not json").unwrap();
    assert!(store.current("computer.mundus.test-app").unwrap().is_none());
    store.uninstall("computer.mundus.test-app").unwrap();
    assert!(!app_dir.exists());
}

#[test]
fn an_install_under_the_old_id_moves_to_the_renamed_one() {
    let dir = TempDir::new().unwrap();
    let apps = dir.path().join("Apps");
    let old = apps.join("com.kosmos.dictation");
    fs::create_dir_all(old.join("0.4.7")).unwrap();
    fs::write(
        old.join(INSTALL_FILE),
        br#"{"id":"com.kosmos.dictation","version":"0.4.7"}"#,
    )
    .unwrap();
    fs::write(old.join(PLUGIN_FILE), br#"{"id":"com.kosmos.dictation"}"#).unwrap();

    // Opening the store is what migrates: every start re-runs it.
    let _store = NativeAppStore::new(apps.clone()).unwrap();

    assert!(!old.exists(), "the old directory is gone");
    let new = apps.join("computer.mundus.dictation");
    assert!(new.join("0.4.7").is_dir(), "the payload moved with it");
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(new.join(INSTALL_FILE)).unwrap()).unwrap();
    assert_eq!(record["id"], "computer.mundus.dictation");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(new.join(PLUGIN_FILE)).unwrap()).unwrap();
    assert_eq!(manifest["id"], "computer.mundus.dictation");
}

#[test]
fn a_second_install_under_the_new_id_is_left_alone() {
    let dir = TempDir::new().unwrap();
    let apps = dir.path().join("Apps");
    let old = apps.join("com.kosmos.dictation");
    let new = apps.join("computer.mundus.dictation");
    fs::create_dir_all(&old).unwrap();
    fs::create_dir_all(&new).unwrap();
    fs::write(old.join(INSTALL_FILE), br#"{"id":"com.kosmos.dictation"}"#).unwrap();

    let _store = NativeAppStore::new(apps.clone()).unwrap();

    // Two installs of one app: guessing which one is wanted would be worse
    // than leaving both where they are.
    assert!(old.join(INSTALL_FILE).is_file());
    assert!(new.is_dir());
}

#[test]
fn a_retired_component_install_is_deleted() {
    let dir = TempDir::new().unwrap();
    let apps = dir.path().join("Apps");
    let new_id = apps.join("computer.mundus.memoria");
    let old_id = apps.join("com.kosmos.memoria");
    fs::create_dir_all(new_id.join("0.7.3")).unwrap();
    fs::create_dir_all(&old_id).unwrap();
    fs::write(
        new_id.join(INSTALL_FILE),
        br#"{"id":"computer.mundus.memoria"}"#,
    )
    .unwrap();

    let _store = NativeAppStore::new(apps.clone()).unwrap();

    // Nothing can list, launch or uninstall an install nothing describes.
    assert!(!new_id.exists());
    assert!(!old_id.exists());
}

// --- Release metadata: sums parsing, tag/version agreement, ---
// --- target line selection, redirect Location → tag extraction.        ---

#[path = "releases_tests.rs"]
mod releases_tests;
