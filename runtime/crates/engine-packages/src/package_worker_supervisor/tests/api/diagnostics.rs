use super::*;

#[test]
fn diagnostics_are_sorted_bounded_and_redacted() {
    let supervisor = PackageWorkerSupervisor::new(1);
    supervisor.insert_failed(("b".into(), "1".into()));
    supervisor.insert_failed(("a".into(), "1".into()));
    let key = ("a".to_string(), "1".to_string());
    let workers = lock(&supervisor.inner.workers);
    let worker = workers.get(&key).expect("worker");
    lock(&worker.stdout_tail).push(concat!(
        "token=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ",
        "alice@example.com C:\\Users\\alice\\note.txt"
    ));
    lock(&worker.stderr_tail).push("scope=filesystem.read /home/alice/private");
    drop(workers);
    let diagnostics = supervisor.diagnostics();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].id, "a");
    let json = serde_json::to_string(&diagnostics).expect("json");
    for forbidden in [
        "secret",
        "alice@example.com",
        "note.txt",
        "filesystem.read",
        "/home/alice",
    ] {
        assert!(!json.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn bridge_status_is_bounded_and_path_free() {
    let status = sanitize_bridge_status(BridgeStatus {
        last_sync: Some("C:\\vault\\body".into()),
        conflict_count: u32::MAX,
        last_conflict_at: Some("2026-07-29T12:00:00Z".into()),
    });
    assert_eq!(status.last_sync, None);
    assert_eq!(status.conflict_count, 1_000_000);
    assert_eq!(
        status.last_conflict_at.as_deref(),
        Some("2026-07-29T12:00:00Z")
    );
}

#[cfg(windows)]
#[tokio::test]
async fn launch_rollback_reports_cleanup_failure_over_setup_error() {
    let supervisor = PackageWorkerSupervisor::new(1);
    let process_holder = Arc::new(AsyncMutex::new(None));
    let _guard = process_holder.lock().await;
    let transaction = LaunchTransaction {
        inner: supervisor.inner.clone(),
        lifecycle_key: TaskKey::Lifecycle {
            package: "rollback".into(),
            version: "1".into(),
            generation: 1,
        },
        worker_io_keys: Vec::new(),
        process_holder: process_holder.clone(),
    };

    assert_eq!(
        transaction
            .rollback_error(
                "worker-unavailable",
                Instant::now() + Duration::from_millis(5)
            )
            .await,
        Err("cleanup-failed")
    );
}
