use super::walk::walk_files_with_hook;
use super::*;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::TempDir;
fn limits() -> Limits {
    Limits {
        max_bytes_per_file: 1024,
        max_total_bytes: 4096,
        max_files: 16,
        max_depth: 8,
    }
}
fn fixture() -> (TempDir, RootHandle) {
    let td = tempfile::tempdir().unwrap();
    fs::create_dir_all(td.path().join("nested/deep")).unwrap();
    fs::write(td.path().join("root.txt"), b"root").unwrap();
    fs::write(td.path().join("nested/deep/file.txt"), b"nested").unwrap();
    let root = open_root(td.path()).unwrap();
    (td, root)
}
#[test]
fn walks_nested_files_without_exposing_paths() {
    let (_td, root) = fixture();
    let mut files = walk_files(&root, limits()).unwrap();
    files.sort_by(|a, b| a.components.cmp(&b.components));
    assert_eq!(files[0].components, vec!["nested", "deep", "file.txt"]);
    assert_eq!(files[1].components, vec!["root.txt"]);
    assert_eq!(files[0].bytes, b"nested");
}
#[test]
fn read_relative_rejects_traversal_and_reads_by_handle() {
    let (_td, root) = fixture();
    assert_eq!(
        read_relative(&root, &["nested", "deep", "file.txt"], 100).unwrap(),
        b"nested"
    );
    for bad in [vec![".."], vec!["a/b"], vec!["a\\b"], vec!["a\0b"]] {
        assert!(read_relative(&root, &bad, 100).is_err());
    }
}
#[cfg(unix)]
#[test]
fn denies_symlinks_and_unsupported_entries() {
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("good"), b"ok").unwrap();
    symlink(td.path().join("good"), td.path().join("link-file")).unwrap();
    symlink(td.path(), td.path().join("link-dir")).unwrap();
    std::os::unix::net::UnixListener::bind(td.path().join("socket")).unwrap();
    let root = open_root(td.path()).unwrap();
    assert!(walk_files(&root, limits()).is_err());
}
#[cfg(unix)]
#[test]
fn walk_entries_reports_metadata_and_skips_links() {
    let td = tempfile::tempdir().unwrap();
    fs::create_dir_all(td.path().join("nested")).unwrap();
    fs::create_dir_all(td.path().join("skip-me")).unwrap();
    fs::write(td.path().join("nested/a.md"), b"12345").unwrap();
    fs::write(td.path().join("root.md"), b"hi").unwrap();
    fs::write(td.path().join("skip-me/hidden.md"), b"x").unwrap();
    symlink(td.path().join("root.md"), td.path().join("link.md")).unwrap();
    symlink(td.path(), td.path().join("link-dir")).unwrap();
    std::os::unix::net::UnixListener::bind(td.path().join("socket")).unwrap();
    let root = open_root(td.path()).unwrap();
    let entries = walk_entries(&root, 8, 64, &|name| name == "skip-me").unwrap();
    assert_eq!(
        entries,
        vec![
            TreeEntry {
                components: vec!["nested".to_string(), "a.md".to_string()],
                size: 5,
            },
            TreeEntry {
                components: vec!["root.md".to_string()],
                size: 2,
            },
        ]
    );
    // Hidden-dir filter by prefix callback.
    let entries = walk_entries(&root, 8, 64, &|name| name.starts_with('.')).unwrap();
    assert_eq!(entries.len(), 3);
}
#[test]
fn walk_entries_enforces_count_cap() {
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("a"), b"1").unwrap();
    fs::write(td.path().join("b"), b"2").unwrap();
    let root = open_root(td.path()).unwrap();
    assert!(walk_entries(&root, 8, 1, &|_| false).is_err());
}
#[test]
fn enforces_size_count_and_depth_caps() {
    let td = tempfile::tempdir().unwrap();
    fs::create_dir_all(td.path().join("a/b/c")).unwrap();
    fs::write(td.path().join("a/b/c/file"), vec![0u8; 20]).unwrap();
    let root = open_root(td.path()).unwrap();
    let mut capped = limits();
    capped.max_bytes_per_file = 10;
    assert!(walk_files(&root, capped).is_err(), "size");
    let mut capped = limits();
    capped.max_depth = 0;
    assert!(walk_files(&root, capped).is_err(), "depth");
    fs::write(td.path().join("one"), b"1").unwrap();
    let mut capped = limits();
    capped.max_files = 0;
    assert!(walk_files(&root, capped).is_err(), "count");
}
#[test]
fn root_replacement_stays_on_open_handle() {
    let td = tempfile::tempdir().unwrap();
    fs::create_dir_all(td.path().join("nested")).unwrap();
    fs::write(td.path().join("nested/file"), b"old").unwrap();
    let root = open_root(td.path()).unwrap();
    fs::rename(td.path(), td.path().with_extension("old")).unwrap();
    fs::create_dir_all(td.path().join("nested")).unwrap();
    fs::write(td.path().join("nested/file"), b"new").unwrap();
    assert_eq!(
        read_relative(&root, &["nested", "file"], 100).unwrap(),
        b"old"
    );
    // The rename moved the tempdir's contents to a sibling path — drop of
    // `td` only covers the live path, so remove the ".old" tree or it
    // leaks into %TEMP% (KOS-270).
    fs::remove_dir_all(td.path().with_extension("old")).unwrap();
}
#[cfg(unix)]
#[test]
fn fault_hook_and_read_failure_are_observed() {
    let (_td, root) = fixture();
    let hook = |point: FaultPoint, _path: &[String]| {
        if point == FaultPoint::BeforeRead {
            Err(io::Error::other("injected"))
        } else {
            Ok(())
        }
    };
    assert!(walk_files_with_hook(&root, limits(), Some(&hook)).is_err());
    let mut capped = limits();
    capped.max_total_bytes = 0;
    assert!(walk_files_with_hook(&root, capped, Some(&|_, _| Ok(()))).is_err());
}
#[test]
fn child_swap_hook_never_returns_attacker_bytes() {
    use std::cell::Cell;
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("victim"), b"original").unwrap();
    let root = open_root(td.path()).unwrap();
    let moved = td.path().join("victim.held");
    let attacker = td.path().join("victim");
    let swapped = Cell::new(false);
    let hook = |point: FaultPoint, path: &[String]| {
        if path == ["victim"] && point == FaultPoint::AfterChildOpen && !swapped.get() {
            fs::rename(&attacker, &moved)?;
            fs::write(&attacker, b"attacker")?;
            swapped.set(true);
        } else if path == ["victim"] && point == FaultPoint::BeforeRead && swapped.get() {
            fs::remove_file(&attacker)?;
            fs::rename(&moved, &attacker)?;
        }
        Ok(())
    };
    let files = walk_files_with_hook(&root, limits(), Some(&hook)).unwrap();
    assert_eq!(
        files
            .iter()
            .find(|f| f.components == ["victim"])
            .unwrap()
            .bytes,
        b"original"
    );
}

#[cfg(unix)]
#[test]
fn repeated_root_listings_and_walks_do_not_consume_the_directory_stream() {
    let (_td, root) = fixture();
    let first = list_relative(&root, &[], 16).unwrap();
    assert!(!first.is_empty());
    // dup() shares the open file description's directory offset; a scan
    // that reuses it would start at EOF and report an empty root.
    let second = list_relative(&root, &[], 16).unwrap();
    assert_eq!(second, first);
    let first_walk = walk_files(&root, limits()).unwrap();
    assert!(!first_walk.is_empty());
    let second_walk = walk_files(&root, limits()).unwrap();
    assert_eq!(second_walk, first_walk);
    // Listing the root after a full walk must still see the entries.
    assert_eq!(list_relative(&root, &[], 16).unwrap(), first);
}

#[cfg(unix)]
#[test]
fn repeated_malformed_walks_do_not_leak_fds() {
    let (_td, root) = fixture();
    let before = fs::read_dir("/proc/self/fd").unwrap().count();
    for _ in 0..100 {
        let mut capped = limits();
        capped.max_files = 0;
        assert!(walk_files(&root, capped).is_err());
    }
    let after = fs::read_dir("/proc/self/fd").unwrap().count();
    assert!(
        after <= before + 2,
        "fd leak: before={before} after={after}"
    );
}
