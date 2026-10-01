    const APP: &str = "com.kosmos.agenda";

    fn roots() -> (TempDir, UserDataRoots, String) {
        let dir = tempfile::tempdir().unwrap();
        let roots = UserDataRoots::new();
        let id = roots.open(dir.path()).unwrap();
        (dir, roots, id)
    }

    fn app_dir(base: &Path) -> PathBuf {
        base.join(APP_DIRECTORY).join(APP)
    }

    #[test]
    fn write_read_stat_delete_round_trip_inside_app_directory() {
        let (dir, roots, id) = roots();
        assert_eq!(
            roots
                .write(&id, APP, "attachments/task-1.bin", &[0, 1, 255])
                .unwrap(),
            3
        );
        assert_eq!(
            fs::read(app_dir(dir.path()).join("attachments/task-1.bin")).unwrap(),
            [0, 1, 255]
        );
        assert_eq!(
            roots.read(&id, APP, "attachments/task-1.bin").unwrap(),
            vec![0, 1, 255]
        );
        assert_eq!(roots.stat(&id, APP, "attachments/task-1.bin").unwrap(), 3);
        assert_eq!(
            roots.read(&id, APP, "attachments\\task-1.bin").unwrap(),
            vec![0, 1, 255]
        );
        assert!(roots.delete(&id, APP, "attachments/task-1.bin").is_ok());
        assert_eq!(
            roots.read(&id, APP, "attachments/task-1.bin"),
            Err(UserDataError::NotFound)
        );
        assert_eq!(
            roots.stat(&id, APP, "attachments/task-1.bin"),
            Err(UserDataError::NotFound)
        );
        assert_eq!(
            roots.delete(&id, APP, "attachments/task-1.bin"),
            Err(UserDataError::NotFound)
        );
    }

    #[test]
    fn missing_files_report_not_found_before_and_after_app_dir_exists() {
        let (_dir, roots, id) = roots();
        assert_eq!(
            roots.read(&id, APP, "missing.bin"),
            Err(UserDataError::NotFound)
        );
        assert_eq!(
            roots.stat(&id, APP, "missing.bin"),
            Err(UserDataError::NotFound)
        );
        assert_eq!(
            roots.delete(&id, APP, "missing.bin"),
            Err(UserDataError::NotFound)
        );
        roots.write(&id, APP, "present.bin", b"x").unwrap();
        assert_eq!(
            roots.read(&id, APP, "missing.bin"),
            Err(UserDataError::NotFound)
        );
    }

    #[test]
    fn rejects_traversal_and_opaque_key_abuse() {
        let (_dir, roots, id) = roots();
        for key in [
            "",
            "../escape",
            "attachments/../../escape",
            "..\\escape",
            "/absolute",
            "C:\\escape",
            "C:/escape",
            "a//b",
            "nul\0byte",
            ".",
            "..",
            ".hidden",
            "trail/",
            "with space",
        ] {
            assert_eq!(
                roots.read(&id, APP, key),
                Err(UserDataError::InvalidKey),
                "key must be rejected: {key:?}"
            );
        }
        assert_eq!(
            roots.read(&id, "bad app", "file.bin"),
            Err(UserDataError::InvalidRequest)
        );
        assert_eq!(
            roots.read(&id, "..", "file.bin"),
            Err(UserDataError::InvalidRequest)
        );
    }

    #[test]
    fn unknown_root_is_rejected_for_every_operation() {
        let (_dir, roots, _id) = roots();
        for result in [
            roots.read("missing-root", APP, "a.bin").map(|_| ()),
            roots.stat("missing-root", APP, "a.bin").map(|_| ()),
            roots.delete("missing-root", APP, "a.bin"),
            roots.write("missing-root", APP, "a.bin", b"x").map(|_| ()),
        ] {
            assert_eq!(result, Err(UserDataError::UnknownRoot));
        }
    }

    #[test]
    fn open_requires_an_absolute_plain_directory() {
        let roots = UserDataRoots::new();
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            roots.open(Path::new("relative/base")),
            Err(UserDataError::InvalidRequest)
        );
        assert_eq!(
            roots.open(&dir.path().join("missing")),
            Err(UserDataError::NotFound)
        );
        let file = dir.path().join("file.bin");
        fs::write(&file, b"x").unwrap();
        assert!(roots.open(&file).is_err());
    }

    #[test]
    fn open_root_rejects_a_linked_root() {
        let base = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let linked = base.path().join("linked");
        crate::test_links::link_dir(outside.path(), &linked).expect("junction");
        let roots = UserDataRoots::new();
        assert!(roots.open(&linked).is_err());
    }

    #[test]
    fn reopening_a_path_keeps_or_refreshes_the_pinned_handle() {
        let dir = tempfile::tempdir().unwrap();
        let roots = UserDataRoots::new();
        let first = roots.open(dir.path()).unwrap();
        assert_eq!(roots.open(dir.path()).unwrap(), first);
        let moved = dir.path().with_extension("moved");
        fs::rename(dir.path(), &moved).unwrap();
        fs::create_dir(dir.path()).unwrap();
        assert_eq!(roots.open(dir.path()).unwrap(), first);
        roots.write(&first, APP, "file.bin", b"new").unwrap();
        assert_eq!(
            fs::read(app_dir(dir.path()).join("file.bin")).unwrap(),
            b"new"
        );
        assert!(!app_dir(&moved).join("file.bin").exists());
        drop(roots);
        fs::remove_dir_all(&moved).unwrap();
    }

    #[test]
    fn enforces_the_per_file_limit() {
        let dir = tempfile::tempdir().unwrap();
        let roots = UserDataRoots::with_max_file_bytes(8);
        let id = roots.open(dir.path()).unwrap();
        assert_eq!(
            roots.write(&id, APP, "large.bin", &[0u8; 9]),
            Err(UserDataError::TooLarge)
        );
        roots.write(&id, APP, "ok.bin", &[0u8; 8]).unwrap();
        assert_eq!(roots.read(&id, APP, "ok.bin"), Ok(vec![0u8; 8]));
        fs::write(app_dir(dir.path()).join("grown.bin"), vec![0u8; 9]).unwrap();
        assert_eq!(
            roots.read(&id, APP, "grown.bin"),
            Err(UserDataError::TooLarge)
        );
        // stat reports the real size; the Host applies the size contract.
        assert_eq!(roots.stat(&id, APP, "grown.bin"), Ok(9));
        assert!(!app_dir(dir.path()).join("large.bin").exists());
    }

    #[test]
    fn directories_are_not_files_for_any_operation() {
        let (dir, roots, id) = roots();
        fs::create_dir_all(app_dir(dir.path()).join("folder")).unwrap();
        assert!(roots.read(&id, APP, "folder").is_err());
        assert!(roots.stat(&id, APP, "folder").is_err());
        assert!(roots.delete(&id, APP, "folder").is_err());
        assert!(app_dir(dir.path()).join("folder").exists());
    }

    /// Data written before KOS-49 through the Host's pathname store sits at
    /// the same `extension-data/<app>/<key>` layout and must stay readable
    /// through the handle-relative boundary.
    #[test]
    fn reads_files_written_by_the_legacy_pathname_store() {
        let (dir, roots, id) = roots();
        let legacy = app_dir(dir.path()).join("attachments/legacy.bin");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, b"legacy-bytes").unwrap();
        assert_eq!(
            roots.read(&id, APP, "attachments/legacy.bin").unwrap(),
            b"legacy-bytes"
        );
        assert_eq!(roots.stat(&id, APP, "attachments/legacy.bin").unwrap(), 12);
    }

    #[test]
    fn root_capacity_is_bounded() {
        let roots = UserDataRoots {
            roots: Mutex::new(HashMap::new()),
            max_file_bytes: USER_DATA_MAX_BYTES,
        };
        let dirs: Vec<TempDir> = (0..MAX_USER_DATA_ROOTS)
            .map(|_| tempfile::tempdir().unwrap())
            .collect();
        for dir in &dirs {
            roots.open(dir.path()).unwrap();
        }
        let extra = tempfile::tempdir().unwrap();
        assert_eq!(roots.open(extra.path()), Err(UserDataError::Unavailable));
        assert_eq!(roots.open(dirs[0].path()).map(|_| ()), Ok(()));
    }
