use super::*;

// Regression L1 (2026-05-24): cargo test parallelizes tests within a
// single binary by default. Tests that mutate process env races with any
// other test reading the same vars. This static lock serializes them.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn env_override_takes_precedence() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Сохраняем и восстанавливаем env чтобы не ломать другие тесты в
    // том же процессе.
    let prev_roots = std::env::var("MUNDUS_FILE_INDEX_ROOTS").ok();
    let prev_test = std::env::var("MUNDUS_TEST_MODE").ok();
    // SAFETY: тесты в этом модуле выполняются последовательно (cargo test
    // не запускает несколько тестов одного бинаря параллельно по умолчанию
    // только если --test-threads=1; на больших проектах могут параллельно,
    // но здесь восстановление env идёт сразу).
    unsafe {
        std::env::set_var("MUNDUS_FILE_INDEX_ROOTS", r"C:\;D:\projects");
        std::env::remove_var("MUNDUS_TEST_MODE");
    }
    let roots = default_roots();
    assert_eq!(
        roots,
        vec![PathBuf::from(r"C:\"), PathBuf::from(r"D:\projects")],
    );

    unsafe {
        match prev_roots {
            Some(v) => std::env::set_var("MUNDUS_FILE_INDEX_ROOTS", v),
            None => std::env::remove_var("MUNDUS_FILE_INDEX_ROOTS"),
        }
        match prev_test {
            Some(v) => std::env::set_var("MUNDUS_TEST_MODE", v),
            None => std::env::remove_var("MUNDUS_TEST_MODE"),
        }
    }
}

#[test]
fn production_default_roots_are_empty_without_opt_in() {
    // Regression: 2026-06-08. Startup must not scan USERPROFILE unless the
    // user/process explicitly opts into roots through MUNDUS_FILE_INDEX_ROOTS.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev_roots = std::env::var("MUNDUS_FILE_INDEX_ROOTS").ok();
    let prev_test = std::env::var("MUNDUS_TEST_MODE").ok();
    let prev_profile = std::env::var("USERPROFILE").ok();
    unsafe {
        std::env::remove_var("MUNDUS_FILE_INDEX_ROOTS");
        std::env::remove_var("MUNDUS_TEST_MODE");
        std::env::set_var("USERPROFILE", r"C:\Users\real-user");
    }

    let roots = default_roots();

    assert!(roots.is_empty());

    unsafe {
        match prev_roots {
            Some(v) => std::env::set_var("MUNDUS_FILE_INDEX_ROOTS", v),
            None => std::env::remove_var("MUNDUS_FILE_INDEX_ROOTS"),
        }
        match prev_test {
            Some(v) => std::env::set_var("MUNDUS_TEST_MODE", v),
            None => std::env::remove_var("MUNDUS_TEST_MODE"),
        }
        match prev_profile {
            Some(v) => std::env::set_var("USERPROFILE", v),
            None => std::env::remove_var("USERPROFILE"),
        }
    }
}

#[test]
fn scan_walk_stops_when_cancelled() {
    // Regression: 2026-06-08. Generation checks after the full walk were too
    // late; scanner must observe cancellation inside the loop.
    let root = tempfile::tempdir().unwrap();
    for n in 0..1_000 {
        std::fs::write(root.path().join(format!("file-{n}.txt")), b"v").unwrap();
    }
    let opts = ScanOptions {
        exclude_noisy_folders: true,
        respect_gitignore: true,
        include_hidden: false,
        ntfs_accelerated: false,
        ignore_patterns: Vec::new(),
    };
    let seen = std::sync::atomic::AtomicUsize::new(0);
    let files = scan_roots_with_progress(
        &[root.path().to_path_buf()],
        &opts,
        |_| {},
        |_, _| {},
        || seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 8,
    );

    assert!(files.len() < 1_000);
}

#[test]
fn ignore_pattern_matches_file_name_anywhere() {
    let opts = ScanOptions {
        exclude_noisy_folders: true,
        respect_gitignore: true,
        include_hidden: false,
        ntfs_accelerated: false,
        ignore_patterns: vec!["*.tmp".to_string()],
    };
    assert!(!should_index_with_options(
        Path::new(r"D:\docs\scratch.tmp"),
        &opts
    ));
    assert!(should_index_with_options(
        Path::new(r"D:\docs\notes.md"),
        &opts
    ));
}

#[cfg(windows)]
#[test]
fn drive_root_detection_accepts_picker_variants() {
    // Regression H2 (2026-05-24): native folder picker may return any of
    // "D:", "D:\\", "D:\\\\", "d:/" — NTFS fast path must engage for all
    // of them, not only the canonical "D:\\".
    assert!(is_drive_root(Path::new("D:")));
    assert!(is_drive_root(Path::new(r"D:\")));
    assert!(is_drive_root(Path::new(r"D:\\")));
    assert!(is_drive_root(Path::new("d:/")));
    assert!(!is_drive_root(Path::new(r"D:\Projects")));
    assert!(!is_drive_root(Path::new("notadrive")));
}

#[test]
fn validate_ignore_pattern_rejects_unbuildable_globs() {
    // Regression M10 (2026-05-24): "[abc" parses through Glob::new but
    // fails at GlobSetBuilder.build → silent drop at scan time.
    assert!(validate_ignore_pattern("[abc").is_err());
    assert!(validate_ignore_pattern("*.tmp").is_ok());
    assert!(validate_ignore_pattern("   ").is_err());
}

#[test]
fn hidden_dot_paths_are_excluded_by_default() {
    let opts = ScanOptions {
        exclude_noisy_folders: true,
        respect_gitignore: true,
        include_hidden: false,
        ntfs_accelerated: false,
        ignore_patterns: Vec::new(),
    };
    // Synthetic path — no I/O happens, only Path component inspection.
    // Forward slashes parse identically on Windows and POSIX (backslash
    // is a literal char on Linux, so a Windows-style path would collapse
    // to a single non-dot component on CI).
    assert!(!should_index_with_options(Path::new("docs/.secret"), &opts));
}

#[cfg(windows)]
#[test]
fn ntfs_hidden_attribute_detected_by_path_helper() {
    // Regression C2 (2026-05-24): on Windows "hidden" is the NTFS attribute
    // bit, not a dot-prefix. AppData/ntuser.dat/Thumbs.db are NOT dot-prefixed
    // but ARE hidden — used to slip through the filter on the NTFS fast path
    // and via watcher events. Test path_has_hidden_attribute directly because
    // a higher-level test path would be pre-filtered by DEFAULT_IGNORE_PATTERNS
    // (tempdir lives under %TEMP% = AppData on Windows).
    let dir = tempfile::Builder::new()
        .prefix("mundus-test-")
        .tempdir()
        .unwrap();
    let visible = dir.path().join("visible.txt");
    std::fs::write(&visible, b"v").unwrap();
    let hidden = dir.path().join("ntuser-like.dat");
    std::fs::write(&hidden, b"v").unwrap();
    let status = std::process::Command::new("attrib")
        .args(["+H", hidden.to_str().unwrap()])
        .status()
        .expect("attrib must run on Windows");
    assert!(status.success(), "attrib +H failed");

    assert!(!path_has_hidden_attribute(&visible));
    assert!(path_has_hidden_attribute(&hidden));
}
