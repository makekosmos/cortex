use super::super::*;
use super::owner;

#[test]
fn persisted_reopen_same_identity_reads_bytes_and_owner_is_fenced() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    fs::write(&file, b"bytes").unwrap();
    let reg = GrantAuthorityRegistry::with_data_dir(dir.path().join("engine"));
    let old = owner(1);
    let (_, _, persistent) = reg
        .register(
            &old,
            "ext",
            &file,
            true,
            GrantProvenance::NativeDialog,
            None,
        )
        .unwrap();
    let persistent = persistent.unwrap();
    assert_eq!(reg.close_owner(old), 1);
    let new = owner(2);
    let (id, _) = reg.reopen(&new, &persistent, "ext").unwrap();
    assert_eq!(
        reg.read(&id, &new, "ext", &["a.txt"], 64).unwrap(),
        b"bytes"
    );
    assert_eq!(
        reg.read(&id, &owner(1), "ext", &["a.txt"], 64),
        Err(GrantError::OwnerMismatch)
    );
}
#[test]
fn replacement_root_and_exact_file_are_denied() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let file = root.join("a.txt");
    fs::write(&file, b"a").unwrap();
    let reg = GrantAuthorityRegistry::with_data_dir(dir.path().join("engine"));
    let o = owner(1);
    let (_, _, p) = reg
        .register(&o, "ext", &root, false, GrantProvenance::NativeDialog, None)
        .unwrap();
    let p = p.unwrap();
    fs::rename(&root, dir.path().join("A")).unwrap();
    fs::create_dir(&root).unwrap();
    assert_eq!(reg.reopen(&o, &p, "ext"), Err(GrantError::IdentityChanged));
    let root2 = dir.path().join("root2");
    fs::create_dir(&root2).unwrap();
    let f = root2.join("x");
    fs::write(&f, b"a").unwrap();
    let (_, _, p2) = reg
        .register(&o, "ext", &f, true, GrantProvenance::NativeDialog, None)
        .unwrap();
    let p2 = p2.unwrap();
    // Move-in replacement: the new file is created while the original
    // still exists, so its identity differs on every filesystem.
    let swapped = root2.join("x.new");
    fs::write(&swapped, b"b").unwrap();
    fs::remove_file(&f).unwrap();
    fs::rename(&swapped, &f).unwrap();
    assert_eq!(reg.reopen(&o, &p2, "ext"), Err(GrantError::IdentityChanged));
}
#[test]
fn registered_exact_file_denies_replacement_before_read() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    fs::write(&file, b"old").unwrap();
    let reg = GrantAuthorityRegistry::new();
    let o = owner(1);
    let (id, _, _) = reg
        .register(&o, "ext", &file, true, GrantProvenance::NativeDialog, None)
        .unwrap();
    // Move-in replacement: the new file is created while the original
    // still exists, so its identity differs on every filesystem.
    let swapped = dir.path().join("a.txt.new");
    fs::write(&swapped, b"replacement").unwrap();
    fs::remove_file(&file).unwrap();
    fs::rename(&swapped, &file).unwrap();

    assert_eq!(
        reg.read(&id, &o, "ext", &["a.txt"], 64),
        Err(GrantError::IdentityChanged)
    );
}
#[test]
fn directory_grant_supports_bounded_write_list_delete_and_mkdir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let reg = GrantAuthorityRegistry::new();
    let o = owner(1);
    let (id, _, _) = reg
        .register(&o, "ext", &root, false, GrantProvenance::NativeDialog, None)
        .unwrap();

    reg.mkdir(&id, &o, "ext", &["nested"]).unwrap();
    reg.write(&id, &o, "ext", &["nested", "note.txt"], b"hello")
        .unwrap();
    let entries = reg.list(&id, &o, "ext", &["nested"]).unwrap();
    assert_eq!(
        entries,
        vec![handle_relative_fs::RelativeEntry {
            name: "note.txt".into(),
            directory: false,
        }]
    );
    assert_eq!(
        reg.read(&id, &o, "ext", &["nested", "note.txt"], 64)
            .unwrap(),
        b"hello"
    );
    reg.delete(&id, &o, "ext", &["nested", "note.txt"]).unwrap();
    assert!(reg.list(&id, &o, "ext", &["nested"]).unwrap().is_empty());
}
#[test]
fn directory_grant_rejects_traversal_and_root_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let reg = GrantAuthorityRegistry::new();
    let o = owner(1);
    let (id, _, _) = reg
        .register(&o, "ext", &root, false, GrantProvenance::NativeDialog, None)
        .unwrap();
    assert!(reg.write(&id, &o, "ext", &["..", "escape"], b"x").is_err());
    assert!(reg.list(&id, &o, "ext", &["a/b"]).is_err());

    let old_root = dir.path().join("root.old");
    fs::rename(&root, &old_root).unwrap();
    fs::create_dir(&root).unwrap();
    reg.write(&id, &o, "ext", &["safe"], b"pinned").unwrap();
    assert_eq!(fs::read(old_root.join("safe")).unwrap(), b"pinned");
    assert!(!root.join("safe").exists());
}
#[test]
fn exact_file_grant_keeps_directory_operations_out_of_scope() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("only.txt");
    fs::write(&file, b"old").unwrap();
    let reg = GrantAuthorityRegistry::new();
    let o = owner(1);
    let (id, _, _) = reg
        .register(&o, "ext", &file, true, GrantProvenance::NativeDialog, None)
        .unwrap();
    assert!(reg.list(&id, &o, "ext", &[]).is_err());
    assert!(reg.mkdir(&id, &o, "ext", &["nested"]).is_err());
    assert!(reg.write(&id, &o, "ext", &["other.txt"], b"no").is_err());
    assert!(reg.write(&id, &o, "ext", &["only.txt"], b"new").is_err());
    assert!(reg.delete(&id, &o, "ext", &["only.txt"]).is_err());
    assert_eq!(fs::read(file).unwrap(), b"old");
}
#[cfg(unix)]
#[test]
fn root_symlink_and_child_symlink_are_denied() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    fs::create_dir(&real).unwrap();
    fs::write(real.join("a"), b"a").unwrap();
    symlink(&real, dir.path().join("link")).unwrap();
    let reg = GrantAuthorityRegistry::new();
    let o = owner(1);
    assert_eq!(
        reg.register(
            &o,
            "ext",
            &dir.path().join("link"),
            false,
            GrantProvenance::NativeDialog,
            None
        ),
        Err(GrantError::Invalid)
    );
    symlink(real.join("a"), real.join("b")).unwrap();
    let (id, _, _) = reg
        .register(&o, "ext", &real, false, GrantProvenance::NativeDialog, None)
        .unwrap();
    assert!(reg.read(&id, &o, "ext", &["b"], 64).is_err());
}
#[test]
fn ten_thousand_reads_keep_cardinality_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a");
    fs::write(&f, b"a").unwrap();
    let reg = GrantAuthorityRegistry::new();
    let o = owner(1);
    let (id, _, _) = reg
        .register(&o, "ext", &f, true, GrantProvenance::NativeDialog, None)
        .unwrap();
    for _ in 0..10_000 {
        assert_eq!(reg.read(&id, &o, "ext", &["a"], 8).unwrap(), b"a");
    }
    assert_eq!(reg.len(), 1);
    assert_eq!(reg.close_owner(o), 1);
    assert_eq!(reg.len(), 0);
}

#[test]
fn generation_close_releases_only_matching_handles() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a");
    fs::write(&file, b"a").unwrap();
    let reg = GrantAuthorityRegistry::new();
    for generation in [1, 2] {
        reg.register(
            &owner(generation),
            "ext",
            &file,
            true,
            GrantProvenance::NativeDialog,
            None,
        )
        .unwrap();
    }

    assert_eq!(reg.close_generation("s", 1), 1);
    assert_eq!(reg.len(), 1);
}

#[test]
fn persistence_failure_after_fsync_preserves_previous_record_and_redacts_errors() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a");
    fs::write(&file, b"a").unwrap();
    let data = dir.path().join("engine");
    let reg = GrantAuthorityRegistry::with_data_dir(data.clone());
    let o = owner(1);
    let (_, _, persistent) = reg
        .register(&o, "ext", &file, true, GrantProvenance::NativeDialog, None)
        .unwrap();
    let path = data.join("grant-authority.json");
    let before = fs::read(&path).unwrap();
    FAIL_PERSIST_AFTER_FSYNC.with(|fail| fail.set(true));
    let result = reg.register(&o, "ext", &file, true, GrantProvenance::NativeDialog, None);
    FAIL_PERSIST_AFTER_FSYNC.with(|fail| fail.set(false));
    assert_eq!(result, Err(GrantError::Persistence));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!path.with_extension("json.tmp").exists());
    let reopened = GrantAuthorityRegistry::with_data_dir(data);
    assert!(reopened
        .reopen(&owner(2), persistent.as_deref().unwrap(), "ext")
        .is_ok());
    fs::write(
        &path,
        br#"[{"version":99,"selected_path":"/secret","extension_id":"ext"}]"#,
    )
    .unwrap();
    let error = reopened.reopen(&owner(2), "secret", "ext").unwrap_err();
    let text = format!("{error:?}");
    assert!(!text.contains("/secret"));
    assert!(!text.contains("ext"));
}
