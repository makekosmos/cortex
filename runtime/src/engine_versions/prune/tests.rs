use super::*;
use std::fs;

fn root() -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let versions = root.path().join("versions");
    fs::create_dir_all(&versions).unwrap();
    (root, versions)
}

fn make_version(versions: &Path, name: &str) -> PathBuf {
    let dir = versions.join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("mundus-engine.exe"), b"exe").unwrap();
    dir
}

fn write_current(root: &Path, version: &str) {
    fs::write(
        root.join("current.json"),
        format!(r#"{{"schema_version":1,"version":"{version}"}}"#),
    )
    .unwrap();
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn eight_versions_leave_current_and_previous() {
    let (root, versions) = root();
    for v in [
        "0.1.2", "0.1.3", "0.1.4", "0.9.22", "0.9.23", "0.9.24", "0.9.34", "0.9.35",
    ] {
        make_version(&versions, v);
    }
    write_current(root.path(), "0.9.35");

    let report = prune(root.path()).unwrap().unwrap();

    assert_eq!(report.removed.len(), 6);
    assert_eq!(names(&versions), vec!["0.9.34", "0.9.35"]);
    assert_eq!(report.previous.as_deref(), Some("0.9.34"));
    assert!(report.removed_bytes > 0);
}

#[test]
fn newer_than_current_and_unknown_names_survive() {
    let (root, versions) = root();
    for v in ["1.0.0", "1.1.0", "1.2.0"] {
        make_version(&versions, v);
    }
    make_version(&versions, "not-a-version");
    make_version(&versions, "1.2");
    write_current(root.path(), "1.1.0"); // откат на предыдущую

    let report = prune(root.path()).unwrap().unwrap();

    assert!(report.removed.is_empty());
    assert_eq!(report.kept_newer, vec!["1.2.0"]);
    assert_eq!(report.previous.as_deref(), Some("1.0.0"));
    assert_eq!(names(&versions).len(), 5);
}

#[test]
fn missing_or_invalid_pointer_deletes_nothing() {
    for current in [
        None,
        Some("not json"),
        Some(r#"{"schema_version":2,"version":"1.0.0"}"#),
        Some(r#"{"schema_version":1,"version":"9.9.9"}"#), // dir отсутствует
    ] {
        let (root, versions) = root();
        make_version(&versions, "1.0.0");
        make_version(&versions, "0.9.0");
        fs::create_dir(versions.join(".tombstone-0.0.1-0")).unwrap();
        if let Some(content) = current {
            fs::write(root.path().join("current.json"), content).unwrap();
        }

        assert!(prune(root.path()).unwrap().is_none());
        assert_eq!(names(&versions).len(), 3);
    }
}

#[test]
fn stale_tombstone_is_removed_live_install_temp_is_not() {
    let (root, versions) = root();
    make_version(&versions, "1.0.0");
    make_version(&versions, "0.9.0");
    write_current(root.path(), "1.0.0");
    fs::create_dir(versions.join(".tombstone-0.8.0-0")).unwrap();
    fs::create_dir(versions.join("0.8.1.4242.tmp")).unwrap(); // молодой temp

    // temp_grace=0 делает любой temp «старым»; сначала убеждаемся, что при
    // обычном grace молодой temp выживает.
    let report = prune(root.path()).unwrap().unwrap();
    assert_eq!(report.removed_tombstones, 1);
    assert_eq!(report.kept_young_temps, 1);
    assert!(versions.join("0.8.1.4242.tmp").is_dir());

    let report = prune_with_grace(root.path(), Duration::ZERO)
        .unwrap()
        .unwrap();
    assert_eq!(report.removed_temps, 1);
    assert_eq!(names(&versions), vec!["0.9.0", "1.0.0"]);
}

#[test]
fn three_sequential_installs_leave_exactly_two() {
    let (root, versions) = root();
    for v in ["1.0.0", "1.1.0", "1.2.0"] {
        make_version(&versions, v);
        write_current(root.path(), v);
        prune(root.path()).unwrap().unwrap();
    }
    assert_eq!(names(&versions), vec!["1.1.0", "1.2.0"]);
}

/// Открытый файл внутри директории блокирует её rename на Windows —
/// жертва пропускается и остаётся до следующего прохода.
#[test]
#[cfg(windows)]
fn locked_version_dir_is_skipped() {
    let (root, versions) = root();
    make_version(&versions, "1.0.0");
    make_version(&versions, "0.9.0"); // previous — выживает
    let locked = make_version(&versions, "0.8.0"); // жертва
    write_current(root.path(), "1.0.0");
    // Дефолтный share mode std::fs включает FILE_SHARE_DELETE — с ним
    // rename родительской директории прошёл бы. Убираем DELETE, чтобы
    // честно имитировать запущенный exe.
    use std::os::windows::fs::OpenOptionsExt;
    let _open = fs::File::options()
        .read(true)
        .share_mode(0x1 | 0x2) // FILE_SHARE_READ | FILE_SHARE_WRITE
        .open(locked.join("mundus-engine.exe"))
        .unwrap();

    let report = prune(root.path()).unwrap().unwrap();

    assert_eq!(report.skipped, vec!["0.8.0"]);
    assert!(locked.is_dir());
    drop(_open);
    // Следующий проход после освобождения доводит чистку до конца.
    let report = prune(root.path()).unwrap().unwrap();
    assert_eq!(report.removed, vec!["0.8.0"]);
    assert_eq!(names(&versions), vec!["0.9.0", "1.0.0"]);
}

#[test]
fn tombstone_names_do_not_collide_with_leftovers() {
    let (root, versions) = root();
    make_version(&versions, "1.0.0");
    make_version(&versions, "0.9.0"); // previous — выживает
    make_version(&versions, "0.8.0"); // жертва; свободное tombstone-имя -0
    write_current(root.path(), "1.0.0");
    fs::create_dir(versions.join(".tombstone-0.8.0-0")).unwrap();

    prune_with_grace(root.path(), Duration::ZERO)
        .unwrap()
        .unwrap();

    assert_eq!(names(&versions), vec!["0.9.0", "1.0.0"]);
}

#[test]
fn engine_root_derived_only_from_installed_layout() {
    assert_eq!(
        engine_root_of_exe(Path::new(
            r"C:\Mundus\Engine\versions\1.2.3\mundus-engine.exe"
        )),
        Some(PathBuf::from(r"C:\Mundus\Engine"))
    );
    // Dev-сборка: имя родительской директории — не semver.
    assert_eq!(
        engine_root_of_exe(Path::new(r"C:\repo\target\debug\mundus-engine.exe")),
        None
    );
    // Нет уровня `versions` или чужое имя exe.
    assert_eq!(
        engine_root_of_exe(Path::new(r"C:\Mundus\Engine\1.2.3\mundus-engine.exe")),
        None
    );
    assert_eq!(
        engine_root_of_exe(Path::new(r"C:\Mundus\Engine\versions\1.2.3\other.exe")),
        None
    );
}

#[test]
fn exe_root_with_missing_pointer_prunes_nothing() {
    // Даже при правильном layout exe → root без валидного current.json
    // prune молчит.
    let (root, versions) = root();
    make_version(&versions, "1.0.0");
    let exe = versions.join("1.0.0").join("mundus-engine.exe");
    assert_eq!(engine_root_of_exe(&exe).as_deref(), Some(root.path()));
    assert!(prune(root.path()).unwrap().is_none());
}
