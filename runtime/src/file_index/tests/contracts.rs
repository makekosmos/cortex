use super::super::estimate::EstimateBudget;
use super::super::risk::assess_root_path_risk;
use super::*;

#[test]
fn risk_boundaries_reject_empty_and_broad_roots() {
    let (empty_level, _) = assess_root_path_risk("");
    assert_eq!(empty_level, FileIndexRiskLevel::Danger);

    let (broad_level, reasons) = assess_root_path_risk(r"C:\Users");
    assert_eq!(broad_level, FileIndexRiskLevel::Danger);
    assert!(reasons
        .iter()
        .any(|reason| reason.contains("слишком широким")));

    let (specific_level, _) = assess_root_path_risk(r"C:\Users\kirill\Coding");
    assert_eq!(specific_level, FileIndexRiskLevel::Ok);
}

#[tokio::test]
async fn all_scan_settings_and_ignore_patterns_survive_reopen() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();

    index.add_ignore_pattern("*.cache").await.unwrap();
    index
        .set_settings(FileIndexSettingsPatch {
            exclude_noisy_folders: Some(false),
            respect_gitignore: Some(false),
            include_hidden: Some(true),
            ntfs_accelerated: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();

    let settings = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()])
        .unwrap()
        .settings()
        .unwrap();
    assert!(!settings.exclude_noisy_folders);
    assert!(!settings.respect_gitignore);
    assert!(settings.include_hidden);
    assert!(!settings.ntfs_accelerated);
    assert_eq!(settings.ignore_patterns, vec!["*.cache"]);
}

#[tokio::test]
async fn search_hides_persisted_files_outside_configured_roots() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let inside_file = root.path().join("shared-note.md");
    let outside_file = outside.path().join("shared-note.md");
    std::fs::write(&inside_file, "inside").unwrap();
    std::fs::write(&outside_file, "outside").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    index.rescan().await.unwrap();
    index
        .store
        .upsert(&IndexedFile {
            path: outside_file.to_string_lossy().to_string(),
            name: "shared-note.md".to_string(),
            mtime: 1,
        })
        .unwrap();

    let found = index.search("shared-note", 10).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, inside_file.to_string_lossy());
}

#[test]
fn zero_file_estimate_is_truncated_without_entries() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("one.md"), "1").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    let estimate = index
        .estimate_root_with_budget(
            &root.path().to_string_lossy(),
            EstimateBudget {
                max_files: 0,
                max_dirs: 100,
                max_duration: std::time::Duration::from_secs(60),
            },
        )
        .unwrap();

    assert!(estimate.truncated);
    assert_eq!(estimate.scanned_files, 0);
    assert_eq!(estimate.estimated_indexed_entries_count, 0);
    assert!(estimate
        .limitations
        .iter()
        .any(|reason| reason.contains("усечена")));
}
