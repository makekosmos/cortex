use super::super::scan::env_flag_enabled;
use super::*;

#[test]
fn env_flag_parser_treats_zero_false_off_no_as_disabled() {
    let _guard = ENV_FLAG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let key = "KEPLER_FILE_INDEX_TEST_FLAG";
    let prev = std::env::var(key).ok();
    for value in ["0", "false", "off", "no"] {
        unsafe {
            std::env::set_var(key, value);
        }
        assert!(!env_flag_enabled(key, true), "value {value}");
    }
    unsafe {
        std::env::set_var(key, "1");
    }
    assert!(env_flag_enabled(key, false));
    unsafe {
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}

#[tokio::test]
async fn noisy_folders_can_be_included_and_setting_persists() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::fs::write(root.path().join(".git/index-note.txt"), "v1").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    // To index .git/* we need ALL three off: noisy filter, hidden filter,
    // and gitignore semantics (the ignore crate's WalkBuilder skips
    // .git directories when git_ignore is on).
    index
        .set_settings(FileIndexSettingsPatch {
            exclude_noisy_folders: Some(false),
            include_hidden: Some(true),
            respect_gitignore: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    index.rescan().await.unwrap();
    assert_eq!(index.search("index-note", 10).unwrap().len(), 1);

    let reopened = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    assert!(!reopened.settings().unwrap().exclude_noisy_folders);
}

#[tokio::test]
async fn ignore_patterns_filter_matching_files() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("keep.md"), "v1").unwrap();
    std::fs::write(root.path().join("scratch.tmp"), "v1").unwrap();

    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    index.add_ignore_pattern("*.tmp").await.unwrap();
    index.rescan().await.unwrap();

    assert_eq!(index.search("keep", 10).unwrap().len(), 1);
    assert!(index.search("scratch", 10).unwrap().is_empty());
}
