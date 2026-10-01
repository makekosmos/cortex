
    /// The KOS-49 race: a parent component is swapped for a junction after
    /// the root was pinned. Every operation must fail closed and the foreign
    /// file must stay unread, unmodified, and undeleted.
    #[test]
    fn swapped_parent_junction_fails_closed_for_every_operation() {
        let (dir, roots, id) = roots();
        let outside = tempfile::tempdir().unwrap();
        let app = app_dir(dir.path());
        let parent = app.join("attachments");
        let moved = app.join("attachments-held");
        fs::create_dir_all(&parent).unwrap();
        fs::write(parent.join("secret.bin"), b"inside").unwrap();
        fs::write(outside.path().join("secret.bin"), b"outside").unwrap();
        fs::rename(&parent, &moved).unwrap();
        crate::test_links::link_dir(outside.path(), &parent).expect("junction");
        let key = "attachments/secret.bin";
        assert_eq!(roots.read(&id, APP, key), Err(UserDataError::Io));
        assert_eq!(roots.stat(&id, APP, key), Err(UserDataError::Io));
        assert_eq!(roots.delete(&id, APP, key), Err(UserDataError::Io));
        assert_eq!(
            roots.write(&id, APP, key, b"injected"),
            Err(UserDataError::Io)
        );
        assert_eq!(
            fs::read(outside.path().join("secret.bin")).unwrap(),
            b"outside"
        );
        assert!(!outside.path().join("injected").exists());
        drop(roots);
        let _ = fs::remove_dir(&parent);
        let _ = fs::remove_dir_all(dir.path());
    }

    /// The same swap one level higher: `extension-data` itself becomes a
    /// junction between root registration and the operation.
    #[test]
    fn swapped_extension_data_junction_fails_closed() {
        let (dir, roots, id) = roots();
        let outside = tempfile::tempdir().unwrap();
        let target = dir.path().join(APP_DIRECTORY);
        let held = dir.path().join("extension-data-held");
        roots.write(&id, APP, "task.bin", b"inside").unwrap();
        fs::create_dir_all(outside.path().join(APP)).unwrap();
        fs::write(outside.path().join(APP).join("task.bin"), b"outside").unwrap();
        fs::rename(&target, &held).unwrap();
        crate::test_links::link_dir(outside.path(), &target).expect("junction");
        assert_eq!(roots.read(&id, APP, "task.bin"), Err(UserDataError::Io));
        assert_eq!(roots.stat(&id, APP, "task.bin"), Err(UserDataError::Io));
        assert_eq!(roots.delete(&id, APP, "task.bin"), Err(UserDataError::Io));
        assert_eq!(
            roots.write(&id, APP, "task.bin", b"injected"),
            Err(UserDataError::Io)
        );
        assert_eq!(
            fs::read(outside.path().join(APP).join("task.bin")).unwrap(),
            b"outside"
        );
        drop(roots);
        let _ = fs::remove_dir(&target);
    }

    /// A junction on the app directory component must also fail closed.
    #[test]
    fn swapped_app_directory_junction_fails_closed() {
        let (dir, roots, id) = roots();
        let outside = tempfile::tempdir().unwrap();
        let app = app_dir(dir.path());
        let held = dir.path().join(APP_DIRECTORY).join("app-held");
        roots.write(&id, APP, "task.bin", b"inside").unwrap();
        fs::write(outside.path().join("task.bin"), b"outside").unwrap();
        fs::rename(&app, &held).unwrap();
        crate::test_links::link_dir(outside.path(), &app).expect("junction");
        assert_eq!(roots.read(&id, APP, "task.bin"), Err(UserDataError::Io));
        assert_eq!(roots.stat(&id, APP, "task.bin"), Err(UserDataError::Io));
        assert_eq!(roots.delete(&id, APP, "task.bin"), Err(UserDataError::Io));
        assert_eq!(
            roots.write(&id, APP, "task.bin", b"injected"),
            Err(UserDataError::Io)
        );
        assert_eq!(
            fs::read(outside.path().join("task.bin")).unwrap(),
            b"outside"
        );
        drop(roots);
        let _ = fs::remove_dir(&app);
    }

    /// A linked leaf is never followed: reads and stats fail, and delete
    /// refuses rather than unlinking through the link. unix builds a file
    /// symlink; Windows cannot create one without SeCreateSymbolicLinkPrivilege,
    /// so it builds a junctioned directory leaf — the same "leaf is a reparse
    /// point" production check.
    #[test]
    fn linked_target_file_is_never_read_written_or_deleted() {
        let (dir, roots, id) = roots();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.bin");
        fs::write(&secret, b"outside").unwrap();
        let app = app_dir(dir.path());
        fs::create_dir_all(&app).unwrap();
        let link = app.join("linked.bin");
        #[cfg(unix)]
        crate::test_links::link_file(&secret, &link).expect("file symlink");
        #[cfg(windows)]
        crate::test_links::link_dir(outside.path(), &link).expect("junction leaf");
        assert_eq!(roots.read(&id, APP, "linked.bin"), Err(UserDataError::Io));
        assert_eq!(roots.stat(&id, APP, "linked.bin"), Err(UserDataError::Io));
        assert_eq!(roots.delete(&id, APP, "linked.bin"), Err(UserDataError::Io));
        assert_eq!(
            roots.write(&id, APP, "linked.bin", b"injected"),
            Err(UserDataError::Io)
        );
        assert_eq!(fs::read(&secret).unwrap(), b"outside");
    }

    #[test]
    fn replaced_root_keeps_operations_on_the_pinned_directory() {
        let dir = tempfile::tempdir().unwrap();
        let roots = UserDataRoots::new();
        let id = roots.open(dir.path()).unwrap();
        roots.write(&id, APP, "file.bin", b"old").unwrap();
        let moved = dir.path().with_extension("moved");
        fs::rename(dir.path(), &moved).unwrap();
        fs::create_dir(dir.path()).unwrap();
        // The pinned handle still refers to the original directory; a fresh
        // directory at the same path is invisible until re-registration.
        assert_eq!(roots.read(&id, APP, "file.bin"), Ok(b"old".to_vec()));
        drop(roots);
        fs::remove_dir_all(&moved).unwrap();
    }
