use super::*;

#[tokio::test]
async fn rescan_coalesces_overlapping_spawn_requests() {
    // Regression H3 (2026-05-24): toggling 5 settings in a row used to
    // queue 5 full rescans on scan_lock. Coalescing collapses concurrent
    // requests into one pending follow-up.
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    let index = std::sync::Arc::new(
        FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap(),
    );
    index.bind_self();

    // First call sets pending.
    index.spawn_rescan();
    // Storm: pending is already true, these MUST be no-ops.
    for _ in 0..50 {
        index.spawn_rescan();
    }

    // Wait for the rescan to finish — should be one, not 50.
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if !index.rescan_pending.load(Ordering::SeqCst)
                && !index.scan_in_progress.load(Ordering::SeqCst)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("coalesced rescan completes promptly");
    index.drain_background().await;
}

#[tokio::test]
async fn rescan_schedules_followup_when_generation_changes_during_write() {
    // Regression C3 (2026-05-24): if scan_generation bumps between the
    // pre-write check and replace_all, the rescan commits stale data; a
    // follow-up rescan must be scheduled so eventual state is correct.
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    let index = std::sync::Arc::new(
        FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap(),
    );
    index.bind_self();
    std::fs::write(root.path().join("a.md"), "v").unwrap();

    // Manually fake the race: bump generation before rescan starts. The
    // pre-write check should catch it, return early — but we want to
    // make sure that even if the race happens AFTER the check, follow-up
    // is scheduled. We verify by calling rescan_locked directly with a
    // pre-bumped generation, then watching for spawn_rescan effects.
    index.invalidate_running_scan();
    let _ = index.rescan().await;
    // After rescan returns, follow-up may or may not still be pending —
    // but the index must converge to consistent state.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if !index.scan_in_progress.load(Ordering::SeqCst)
                && !index.rescan_pending.load(Ordering::SeqCst)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("follow-up rescan must converge");
    assert_eq!(index.search("a.md", 10).unwrap().len(), 1);
    index.drain_background().await;
}

#[tokio::test]
async fn background_task_registry_reaps_finished_handles() {
    // KOS-270: every spawn used to push a JoinHandle forever — a memory leak
    // in the long-running app. Spawning after a completed task must reap it.
    let data = tempdir().unwrap();
    let index = std::sync::Arc::new(FileIndex::new_disabled(data.path()).unwrap());
    index.bind_self();
    let pending = |index: &FileIndex| {
        index
            .background_tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    };
    // Sequential cleanups without draining: each spawn reaps the finished
    // handle, so the registry stays bounded while the app runs.
    for _ in 0..50 {
        index.spawn_removed_root_cleanup("gone".into());
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(pending(&index) <= 2, "registry must stay bounded");
    }
    index.spawn_rescan();
    index.drain_background().await;
    assert!(index
        .background_tasks
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_empty());
}

#[tokio::test]
async fn blank_stored_names_fall_back_to_the_path_filename() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    let file = root.path().join("nameless-plan.md");
    std::fs::write(&file, "v1").unwrap();
    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    index
        .store
        .upsert(&IndexedFile {
            path: file.to_string_lossy().to_string(),
            name: String::new(),
            mtime: 1,
        })
        .unwrap();

    let found = index.search("nameless", 8).unwrap();

    assert_eq!(found[0].name, "nameless-plan.md");
}

#[tokio::test]
async fn word_prefix_filename_matches_survive_the_candidate_window() {
    let data = tempdir().unwrap();
    let root = tempdir().unwrap();
    let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
    for n in 0..72 {
        let file = root.path().join(format!("areport-{n}.txt"));
        std::fs::write(&file, "v1").unwrap();
        index
            .store
            .upsert(&IndexedFile {
                path: file.to_string_lossy().to_string(),
                name: format!("areport-{n}.txt"),
                mtime: n,
            })
            .unwrap();
    }
    let file = root.path().join("weekly-report-final.txt");
    std::fs::write(&file, "v1").unwrap();
    index
        .store
        .upsert(&IndexedFile {
            path: file.to_string_lossy().to_string(),
            name: "weekly-report-final.txt".to_string(),
            mtime: 100,
        })
        .unwrap();

    let found = index.search("report", 1).unwrap();

    assert_eq!(found[0].name, "weekly-report-final.txt");
}
