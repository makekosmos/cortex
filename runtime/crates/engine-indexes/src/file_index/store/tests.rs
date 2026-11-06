use super::*;
use tempfile::tempdir;

#[test]
fn remove_tree_keeps_paths_that_only_share_a_prefix() {
    let data = tempdir().unwrap();
    let store = FileStore::open(&data.path().join("files.db")).unwrap();
    for path in [r"C:\foo\note.txt", r"C:\foobar\note.txt"] {
        store
            .upsert(&IndexedFile {
                path: path.to_string(),
                name: "note.txt".to_string(),
                mtime: 1,
            })
            .unwrap();
    }

    store.remove_tree(r"C:\foo").unwrap();

    assert_eq!(store.search("note", 10).unwrap().len(), 1);
    assert_eq!(
        store.search("note", 10).unwrap()[0].path,
        r"C:\foobar\note.txt"
    );
}

#[test]
fn replace_all_dedupes_duplicate_paths() {
    // Regression: 2026-05-23. NTFS fast scan возвращал один и тот же path
    // дважды (junction / symlink / mount point), `INSERT` валил всю
    // транзакцию по `files.path PRIMARY KEY` → backend в зависе.
    let data = tempdir().unwrap();
    let store = FileStore::open(&data.path().join("files.db")).unwrap();
    let files = vec![
        IndexedFile {
            path: r"C:\junction\note.txt".to_string(),
            name: "note.txt".to_string(),
            mtime: 1,
        },
        IndexedFile {
            path: r"C:\junction\note.txt".to_string(),
            name: "note.txt".to_string(),
            mtime: 2,
        },
        IndexedFile {
            path: r"C:\real\note.txt".to_string(),
            name: "note.txt".to_string(),
            mtime: 3,
        },
    ];

    store
        .replace_all(&files)
        .expect("replace_all не должен падать на дубликатах");

    let results = store.search("note", 10).unwrap();
    assert_eq!(results.len(), 2, "должно остаться 2 уникальных path");
    let paths: std::collections::HashSet<_> = results.iter().map(|r| r.path.as_str()).collect();
    assert!(paths.contains(r"C:\junction\note.txt"));
    assert!(paths.contains(r"C:\real\note.txt"));
}

#[test]
fn short_queries_do_not_scan_files_table() {
    // Regression: 2026-06-08. Launcher polling with "a"/"do" hit the
    // substring LIKE path forever after the window was hidden.
    let data = tempdir().unwrap();
    let store = FileStore::open(&data.path().join("files.db")).unwrap();
    store
        .upsert(&IndexedFile {
            path: r"C:\docs\alpha.txt".to_string(),
            name: "alpha.txt".to_string(),
            mtime: 1,
        })
        .unwrap();

    assert!(store.search("a", 10).unwrap().is_empty());
    assert!(store.search("al", 10).unwrap().is_empty());
    assert_eq!(store.search("alp", 10).unwrap().len(), 1);
}

#[test]
fn roots_and_ignore_patterns_are_persisted_and_deduped() {
    let data = tempdir().unwrap();
    let db = data.path().join("files.db");
    let store = FileStore::open(&db).unwrap();

    store.add_root(r"D:\Personal").unwrap();
    store.add_root(r"D:\Personal").unwrap();
    store.add_ignore_pattern("*.tmp").unwrap();
    // Second insert is a duplicate (case-insensitive) and must error, not
    // silently swallow — UI shows the message.
    assert!(store.add_ignore_pattern("*.tmp").is_err());

    let reopened = FileStore::open(&db).unwrap();
    assert_eq!(reopened.roots().unwrap(), vec![r"D:\Personal".to_string()]);
    assert_eq!(
        reopened.ignore_patterns().unwrap(),
        vec!["*.tmp".to_string()]
    );
}

#[test]
fn roots_dedup_is_case_and_trailing_slash_insensitive() {
    // Regression M4 (2026-05-24): "D:\\Personal", "d:\\personal\\" and
    // "D:\\Personal\\" used to insert as three distinct rows because
    // SQLite PRIMARY KEY is case-sensitive — watcher then double-watched.
    let data = tempdir().unwrap();
    let store = FileStore::open(&data.path().join("files.db")).unwrap();
    store.add_root(r"D:\Personal").unwrap();
    store.add_root(r"d:\personal\").unwrap();
    store.add_root(r"D:\Personal\").unwrap();
    assert_eq!(store.roots().unwrap().len(), 1);
    // remove_root must match regardless of casing/trailing-slash.
    store.remove_root_record(r"d:\PERSONAL").unwrap();
    assert!(store.roots().unwrap().is_empty());
}

#[test]
fn ignore_pattern_dedup_is_case_insensitive() {
    // Regression M3 (2026-05-24): "*.TMP" and "*.tmp" used to coexist —
    // double-matching slows scans and bloats settings.
    let data = tempdir().unwrap();
    let store = FileStore::open(&data.path().join("files.db")).unwrap();
    store.add_ignore_pattern("*.tmp").unwrap();
    let err = store.add_ignore_pattern("*.TMP").unwrap_err();
    assert!(matches!(err, FileIndexError::InvalidSetting(_)));
    assert_eq!(store.ignore_patterns().unwrap().len(), 1);
}

#[test]
fn drive_root_normalization_keeps_trailing_backslash() {
    let data = tempdir().unwrap();
    let store = FileStore::open(&data.path().join("files.db")).unwrap();
    // Picker can return "D:" — must be stored as "D:\\" so watcher gets a
    // real directory path.
    store.add_root("D:").unwrap();
    assert_eq!(store.roots().unwrap(), vec![r"D:\".to_string()]);
}

#[test]
fn new_file_search_settings_have_safe_defaults() {
    let data = tempdir().unwrap();
    let store = FileStore::open(&data.path().join("files.db")).unwrap();

    assert!(store.respect_gitignore().unwrap());
    assert!(!store.include_hidden().unwrap());
    assert!(!store.ntfs_accelerated().unwrap());
}

#[test]
fn fts_search_matches_filename_and_path_substrings() {
    let data = tempdir().unwrap();
    let store = FileStore::open(&data.path().join("files.db")).unwrap();
    for (path, name) in [
        (r"C:\notes\roadmap-final.md", "roadmap-final.md"),
        (r"D:\reports\q4\invoice.txt", "invoice.txt"),
    ] {
        store
            .upsert(&IndexedFile {
                path: path.to_string(),
                name: name.to_string(),
                mtime: 1,
            })
            .unwrap();
    }

    assert_eq!(
        store.search("map-f", 10).unwrap()[0].name,
        "roadmap-final.md"
    );
    assert_eq!(store.search("reports", 10).unwrap()[0].name, "invoice.txt");
}

#[test]
fn checkpoint_truncate_wal_clears_wal_bytes() {
    let data = tempdir().unwrap();
    let db = data.path().join("files.db");
    let store = FileStore::open(&db).unwrap();
    // One bulk transaction fills the WAL; per-row upserts commit through
    // 4000 fsyncs and dominated the test's runtime without adding signal.
    store
        .replace_all(
            &(0..4_000)
                .map(|n| IndexedFile {
                    path: format!(r"C:\docs\checkpoint-{n}.txt"),
                    name: format!("checkpoint-{n}.txt"),
                    mtime: n,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
    assert!(store.database_size_snapshot().unwrap().wal_size_bytes > 0);

    store.checkpoint_truncate_wal().unwrap();

    let sizes = store.database_size_snapshot().unwrap();
    assert_eq!(sizes.wal_size_bytes, 0);
    assert!(sizes.db_size_bytes > 0);
    assert_eq!(sizes.total_size_bytes, sizes.db_size_bytes);
}
