use super::super::risk::assess_root_path_risk;
use super::*;

#[tokio::test]
async fn removing_root_hides_scope_immediately_and_cleans_index_in_background() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("scope-note.md"), "v1").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    index.rescan().await.unwrap();
    assert_eq!(index.search("scope-note", 10).unwrap().len(), 1);

    index
        .remove_root(&root.path().to_string_lossy())
        .await
        .unwrap();

    assert!(index.settings().unwrap().roots.is_empty());
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            if index.search("scope-note", 10).unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("removed scope cleanup should finish in the background");
    index.drain_background().await;
}

#[tokio::test]
async fn clear_cache_removes_index_rows_but_keeps_settings_and_roots() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("cache-note.md"), "v1").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    index.set_exclude_noisy_folders(false).await.unwrap();
    index.rescan().await.unwrap();
    assert_eq!(index.search("cache-note", 10).unwrap().len(), 1);

    let stats = index.clear_cache().unwrap();
    assert_eq!(stats.total, 0);
    assert_eq!(index.search("cache-note", 10).unwrap().len(), 0);

    let settings = index.settings().unwrap();
    assert_eq!(settings.roots.len(), 1);
    assert!(!settings.exclude_noisy_folders);

    index.rescan().await.unwrap();
    assert_eq!(index.search("cache-note", 10).unwrap().len(), 1);
}

#[tokio::test]
async fn diagnostics_include_sizes_roots_and_last_scan() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("diag-note.md"), "v1").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    index.rescan().await.unwrap();

    let diag = index.diagnostics().unwrap();

    assert_eq!(diag.roots_count, 1);
    assert_eq!(diag.files_count, 1);
    assert!(diag.db_size_bytes > 0);
    assert_eq!(diag.wal_size_bytes, 0);
    assert!(diag.last_scan.is_some());
}

#[tokio::test]
async fn estimate_root_counts_text_media_and_skipped_files() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("keep.md"), "hello").unwrap();
    std::fs::write(root.path().join("photo.png"), "png").unwrap();
    std::fs::write(root.path().join("scratch.tmp"), "tmp").unwrap();
    std::fs::create_dir_all(root.path().join("node_modules/pkg")).unwrap();
    std::fs::write(root.path().join("node_modules/pkg/noise.js"), "ignored").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    index.add_ignore_pattern("*.tmp").await.unwrap();

    let estimate = index.estimate_root(&root.path().to_string_lossy()).unwrap();

    assert_eq!(estimate.indexable_text_files_count, 1);
    assert_eq!(estimate.metadata_only_media_files_count, 1);
    assert!(estimate.ignored_or_skipped_files >= 1);
    assert_eq!(estimate.estimated_indexed_entries_count, 2);
}

#[test]
fn estimate_root_can_be_truncated_by_budget() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("one.md"), "1").unwrap();
    std::fs::write(root.path().join("two.md"), "2").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    let estimate = index
        .estimate_root_with_budget(
            &root.path().to_string_lossy(),
            EstimateBudget {
                max_files: 1,
                max_dirs: 100,
                max_duration: std::time::Duration::from_secs(60),
            },
        )
        .unwrap();

    assert!(estimate.truncated);
    assert_eq!(estimate.scanned_files, 1);
    assert!(estimate
        .limitations
        .iter()
        .any(|reason| reason.contains("усечена")));
}

#[test]
fn broad_drive_root_is_marked_as_danger() {
    let (level, reasons) = assess_root_path_risk(r"C:\");
    assert_eq!(level, FileIndexRiskLevel::Danger);
    assert!(reasons.iter().any(|reason| reason.contains("корень диска")));
}

#[tokio::test]
async fn search_hides_deleted_files_before_cleanup_or_rescan() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    let file = root.path().join("deleted-note.md");
    std::fs::write(&file, "v1").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    index.rescan().await.unwrap();
    assert_eq!(index.search("deleted-note", 10).unwrap().len(), 1);

    std::fs::remove_file(file).unwrap();
    assert!(index.search("deleted-note", 10).unwrap().is_empty());
}

#[tokio::test]
async fn scope_remove_does_not_cleanup_large_index_synchronously() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    let root_path = root.path().to_string_lossy();
    for n in 0..1_000 {
        index
            .store
            .upsert(&IndexedFile {
                path: root
                    .path()
                    .join(format!("bulk-{n}.txt"))
                    .to_string_lossy()
                    .to_string(),
                name: format!("bulk-{n}.txt"),
                mtime: n,
            })
            .unwrap();
    }

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        index.remove_root(&root_path),
    )
    .await
    .expect("scope_remove must not synchronously delete the indexed subtree")
    .unwrap();
    index.drain_background().await;
}

#[tokio::test]
async fn scope_remove_does_not_wait_for_running_rescan_lock() {
    // Regression: 2026-05-24. Settings actions used to wait for scan_lock,
    // so removing D:\ while a long NTFS/full rescan was running timed out
    // through IPC after 30s. Mutations must invalidate the running scan
    // and return quickly.
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    let _scan_guard = index.scan_lock.lock().await;

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        index.remove_root(&root.path().to_string_lossy()),
    )
    .await
    .expect("scope_remove must not wait for scan_lock")
    .unwrap();
    drop(_scan_guard);
    index.drain_background().await;
}
