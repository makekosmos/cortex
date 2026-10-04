use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;
use tempfile::TempDir;
use zip::{write::FileOptions, ZipWriter};

fn install_fixture(store: &NativeAppStore, dir: &TempDir, version: &str) -> NativeAppInstall {
    let archive = dir.path().join(format!("app-{version}.zip"));
    let file = fs::File::create(&archive).unwrap();
    let mut zip = ZipWriter::new(file);
    zip.start_file("agenda-gpui.exe", FileOptions::default())
        .unwrap();
    zip.write_all(b"MZ fixture").unwrap();
    zip.finish().unwrap();
    let bytes = fs::read(&archive).unwrap();
    store
        .install_archive(
            &NativeInstallSpec {
                id: "com.kosmos.agenda".into(),
                version: version.into(),
                executable: "agenda-gpui.exe".into(),
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                size: bytes.len() as u64,
                repository: "makekosmos/agenda-gpui".into(),
                release_tag: format!("v{version}"),
            },
            &archive,
        )
        .unwrap()
}

fn agenda() -> &'static NativeAppDescriptor {
    app_descriptor("com.kosmos.agenda").unwrap()
}

fn link(dir: &TempDir) -> PathBuf {
    link_path(dir.path(), agenda())
}

#[test]
fn link_lives_under_the_product_subfolder() {
    // The path must stay inside `<Programs>\Mundus\` — the installer's flat
    // legacy deletes (`$SMPROGRAMS\Agenda.lnk`) can never reach it.
    let link = link_path(Path::new("Programs"), agenda());
    assert_eq!(
        link,
        Path::new("Programs").join("Mundus").join("Agenda.lnk")
    );
}

#[test]
fn reconcile_removes_links_without_an_install_record() {
    let dir = TempDir::new().unwrap();
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    let link = link(&dir);
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    fs::write(&link, b"stale").unwrap();
    reconcile_shortcuts(&store, dir.path());
    assert!(!link.exists());
}

#[test]
fn link_points_at_matches_its_target_only() {
    assert!(same_path(
        Path::new("C:\\Apps\\a.EXE"),
        Path::new("c:\\apps\\a.exe")
    ));
    assert!(!same_path(
        Path::new("C:\\Apps\\a.exe"),
        Path::new("C:\\Apps\\b.exe")
    ));
    // A file that is not a link reports no match rather than an error.
    let dir = TempDir::new().unwrap();
    let garbage = dir.path().join("Agenda.lnk");
    fs::write(&garbage, b"not a link").unwrap();
    assert!(!link_points_at(&garbage, Path::new("C:\\anywhere.exe")));
}

#[cfg(windows)]
#[test]
fn same_path_matches_a_file_to_its_canonical_spelling() {
    // `\\?\` from canonicalize and the 8.3 TEMP path are the same file.
    // A case-only compare reports that the shortcut points somewhere else.
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Agenda.exe");
    fs::write(&file, b"MZ").unwrap();
    let canonical = fs::canonicalize(&file).unwrap();
    assert!(same_path(&file, &canonical), "{file:?} vs {canonical:?}");
}

#[cfg(windows)]
#[test]
fn sync_writes_and_repoints_the_real_lnk() {
    let dir = TempDir::new().unwrap();
    let programs = dir.path().join("Programs");
    let store = NativeAppStore::new(dir.path().join("Apps")).unwrap();
    install_fixture(&store, &dir, "0.1.0");

    sync_shortcut(&store, &programs, agenda()).unwrap();
    let link = link_path(&programs, agenda());
    assert!(link.is_file());
    let target = store.executable_for(agenda()).unwrap();
    assert!(link_points_at(&link, &target));

    // Idempotent: a correct link is not rewritten.
    let written = fs::metadata(&link).unwrap().modified().unwrap();
    sync_shortcut(&store, &programs, agenda()).unwrap();
    assert_eq!(fs::metadata(&link).unwrap().modified().unwrap(), written);

    // Update: the link repoints at the new version's executable.
    install_fixture(&store, &dir, "0.2.0");
    sync_shortcut(&store, &programs, agenda()).unwrap();
    let target = store.executable_for(agenda()).unwrap();
    assert!(target.to_string_lossy().contains("0.2.0"));
    assert!(link_points_at(&link, &target));

    // Uninstall: the link goes with the app.
    store.uninstall("com.kosmos.agenda").unwrap();
    sync_shortcut(&store, &programs, agenda()).unwrap();
    assert!(!link.exists());
}
