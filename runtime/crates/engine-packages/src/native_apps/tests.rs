use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;
use tempfile::TempDir;
use zip::{write::FileOptions, ZipWriter};

fn spec(version: &str, archive: &Path) -> NativeInstallSpec {
    spec_for(version, archive, "agenda-gpui.exe")
}

fn spec_for(version: &str, archive: &Path, executable: &str) -> NativeInstallSpec {
    let bytes = fs::read(archive).unwrap();
    NativeInstallSpec {
        id: "com.kosmos.agenda".into(),
        version: version.into(),
        executable: executable.into(),
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
#[test]
fn install_from_tarball_extracts_bundle_and_keeps_exec_bit() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let archive = dir.path().join("app.tar.gz");
    let executable = "Agenda.app/Contents/MacOS/agenda-gpui";
    // The bundle binary may arrive 0644 in the archive — the extractor must
    // still land the declared executable runnable.
    write_tar_gz(
        &archive,
        &[
            ("Agenda.app/Contents/Info.plist", 0o644, b"plist"),
            (executable, 0o644, b"binary"),
        ],
    );
    store
        .install_archive(&spec_for("0.2.0", &archive, executable), &archive)
        .unwrap();
    let installed = store.executable_path("com.kosmos.agenda").unwrap();
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
    let executable = "Agenda.app/Contents/MacOS/agenda-gpui";

    // `..` escapes staging — rejected like the zip traversal case.
    let traversal = dir.path().join("evil.tar.gz");
    write_raw_tar_gz(
        &traversal,
        &[(executable, b"binary"), ("../escape", b"bad")],
    );
    assert!(store
        .install_archive(&spec_for("0.1.0", &traversal, executable), &traversal)
        .is_err());
    assert!(store.current("com.kosmos.agenda").unwrap().is_none());
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
        append_tar_link(&mut builder, "Agenda.app/link");
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
    let agenda = app_descriptor("com.kosmos.agenda").unwrap();
    assert_eq!(
        agenda.executable("x86_64-pc-windows-msvc"),
        "agenda-gpui.exe"
    );
    assert_eq!(
        agenda.executable("aarch64-apple-darwin"),
        "Agenda.app/Contents/MacOS/agenda-gpui"
    );
    assert_eq!(
        app_descriptor("com.kosmos.dictation")
            .unwrap()
            .executable("aarch64-apple-darwin"),
        "Dictation.app/Contents/MacOS/dictation-gpui"
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
        &[("agenda-gpui.exe", b"exe"), ("LICENSE", b"license")],
    );
    let first = store
        .install_archive(&spec("0.1.0", &archive), &archive)
        .unwrap();
    assert_eq!(first.version, "0.1.0");
    assert_eq!(
        store.current("com.kosmos.agenda").unwrap().unwrap().version,
        "0.1.0"
    );
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
    assert_eq!(current.expect("record").version, "0.1.0");
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
    assert_eq!(
        store.current("com.kosmos.agenda").unwrap().unwrap().version,
        "0.1.0"
    );
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
    assert_eq!(
        store.current("com.kosmos.agenda").unwrap().unwrap().version,
        "0.1.0"
    );
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
    assert!(store.current("com.kosmos.agenda").unwrap().is_none());
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
    assert!(store.current("com.kosmos.agenda").unwrap().is_none());
    store.uninstall("com.kosmos.agenda").unwrap();
    assert!(!app_dir.exists());
}

// --- Release metadata (KOS-265): sums parsing, tag/version agreement, ---
// --- target line selection, redirect Location → tag extraction.        ---

include!("releases_tests.rs");
