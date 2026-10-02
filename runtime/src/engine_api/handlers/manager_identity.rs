/// Is the `/v1/rpc` caller the Manager *process*? The bearer token alone is
/// not Manager identity — `engine.lock.json` is readable by any same-user
/// process, and app clients share the credential class (KOS-269 round 3).
/// What proves it is the caller's process image (queried by its already
/// same-user-validated PID, never self-declared) being the *same file* as
/// the Manager exe the Engine itself resolves — file identity, not path
/// spelling, because the two paths may differ in `\\?\` prefix, `..`
/// segments, case or 8.3 short names (round 4). Deny-closed: a missing or
/// unresolvable Manager install means `false`.
#[cfg(windows)]
fn is_manager_process(pid: u32) -> bool {
    let Ok(image) = crate::auth::process_image_path(pid) else {
        return false;
    };
    let Some(manager) = crate::backend_tray::manager_executable_for_auth() else {
        return false;
    };
    crate::auth::same_file(&image, &manager)
}

/// The gpui Manager ships only on Windows.
#[cfg(not(windows))]
fn is_manager_process(_pid: u32) -> bool {
    false
}

// `mod tests` already exists in the including module — use a distinct name.
#[cfg(all(test, windows))]
mod manager_identity_tests {
    use std::path::{Path, PathBuf};

    /// TempDirs under the workspace `target/` dir — the test-isolation rule
    /// for anything this suite writes.
    fn work_target_dir() -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target");
        std::fs::create_dir_all(&dir).expect("create target dir");
        dir
    }

    #[test]
    fn same_file_through_different_spellings() {
        let dir = tempfile::tempdir_in(work_target_dir()).expect("tempdir");
        let file = dir.path().join("Manager.EXE");
        std::fs::write(&file, b"exe").expect("write");

        // Different case: NTFS lookups are case-insensitive.
        let lowercased = dir.path().join("manager.exe");
        assert!(crate::auth::same_file(&file, &lowercased));

        // `\\?\` verbatim prefix on the canonicalised path.
        let canonical = file.canonicalize().expect("canonicalize");
        assert!(canonical.to_string_lossy().starts_with(r"\\?\"));
        assert!(crate::auth::same_file(&file, &canonical));

        // `..` segment spelling.
        let dotted = dir.path().join("subdir").join("..").join("Manager.EXE");
        assert!(crate::auth::same_file(&file, &dotted));
    }

    #[test]
    fn same_file_rejects_identical_content_copy() {
        let dir = tempfile::tempdir_in(work_target_dir()).expect("tempdir");
        let original = dir.path().join("a.exe");
        let copy = dir.path().join("b.exe");
        std::fs::write(&original, b"identical bytes").expect("write original");
        std::fs::copy(&original, &copy).expect("copy");
        assert!(!crate::auth::same_file(&original, &copy));
    }

    #[test]
    fn same_file_rejects_missing_path() {
        let dir = tempfile::tempdir_in(work_target_dir()).expect("tempdir");
        let file = dir.path().join("exists.exe");
        std::fs::write(&file, b"exe").expect("write");
        let missing = dir.path().join("missing.exe");
        assert!(!crate::auth::same_file(&file, &missing));
        assert!(!crate::auth::same_file(&missing, &file));
        assert!(!crate::auth::same_file(&missing, &missing));
    }
}
