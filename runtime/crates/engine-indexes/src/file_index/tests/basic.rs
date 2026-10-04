use super::*;

#[tokio::test]
async fn scan_searches_regular_files_and_skips_noisy_folders_by_default() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("roadmap.txt"), "v1").unwrap();
    std::fs::create_dir_all(root.path().join("node_modules").join("pkg")).unwrap();
    std::fs::write(root.path().join("node_modules/pkg/roadmap-noise.txt"), "v1").unwrap();
    std::fs::create_dir_all(root.path().join(".venv").join("Lib")).unwrap();
    std::fs::write(root.path().join(".venv/Lib/roadmap-site-package.py"), "v1").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    let stats = index.rescan().await.unwrap();
    let found = index.search("roadmap", 10).unwrap();

    assert_eq!(stats.total, 1);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "roadmap.txt");
}

#[tokio::test]
async fn disabled_index_exposes_empty_safe_surface() {
    // Regression: 2026-06-08. MUNDUS_FILE_INDEX=0 must be a real kill switch,
    // not just "skip startup scan while old persisted roots keep working".
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    let enabled = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    enabled
        .store
        .upsert(&IndexedFile {
            path: root
                .path()
                .join("persisted-note.txt")
                .to_string_lossy()
                .to_string(),
            name: "persisted-note.txt".to_string(),
            mtime: 1,
        })
        .unwrap();

    let disabled = FileIndex::new_disabled(data.path()).unwrap();

    assert!(disabled.settings().unwrap().roots.is_empty());
    assert!(disabled.search("persisted", 10).unwrap().is_empty());
    assert_eq!(disabled.request_rescan().unwrap().total, 0);
    assert_eq!(disabled.rescan().await.unwrap().roots, 0);
}

#[tokio::test]
async fn settings_enabled_false_disables_search_without_losing_roots() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("toggle-note.txt"), "v1").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    index.rescan().await.unwrap();
    assert_eq!(index.search("toggle-note", 10).unwrap().len(), 1);

    index
        .set_settings(FileIndexSettingsPatch {
            enabled: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!index.settings().unwrap().enabled);
    assert_eq!(index.settings().unwrap().roots.len(), 1);
    assert!(index.search("toggle-note", 10).unwrap().is_empty());
    assert_eq!(index.request_rescan().unwrap().total, 1);

    index
        .set_settings(FileIndexSettingsPatch {
            enabled: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    index.rescan().await.unwrap();
    assert_eq!(index.search("toggle-note", 10).unwrap().len(), 1);
}

#[tokio::test]
async fn enabling_existing_index_does_not_force_rescan() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("old-note.txt"), "v1").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    index.rescan().await.unwrap();
    std::fs::write(root.path().join("new-note.txt"), "v1").unwrap();

    index
        .set_settings(FileIndexSettingsPatch {
            enabled: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    index
        .set_settings(FileIndexSettingsPatch {
            enabled: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(index.search("old-note", 10).unwrap().len(), 1);
    assert!(index.search("new-note", 10).unwrap().is_empty());
}
