use super::super::*;

#[tokio::test]
async fn task_registry_rejects_duplicate_and_capacity_overflow() {
    let registry = TaskRegistry::new(1);
    let first = TaskKey::Lifecycle {
        package: "pkg".into(),
        version: "1".into(),
        generation: 1,
    };
    let second = TaskKey::Lifecycle {
        package: "pkg".into(),
        version: "1".into(),
        generation: 2,
    };
    assert!(registry.reserve(first.clone()).is_some());
    assert!(registry.reserve(first).is_none());
    assert!(registry.reserve(second).is_none());
    assert!(registry.shutdown().await);
}

#[tokio::test]
async fn startup_key_is_generation_specific_and_registry_owned() {
    let registry = TaskRegistry::new(2);
    let key = TaskKey::Startup {
        package: "pkg".into(),
        version: "1".into(),
        generation: 9,
    };
    let (start_rx, cancel_rx) = registry.reserve(key.clone()).expect("startup reservation");
    let task = tokio::spawn(async move {
        assert!(start_rx.await.expect("startup gate"));
        let _ = cancel_rx.await;
    });
    registry.install(&key, task).expect("startup install");
    assert!(registry.contains(&key));
    assert!(registry.cancel_generation("pkg", "1", 9).await);
    assert!(!registry.contains(&key));
    assert!(registry.shutdown().await);
}

#[tokio::test]
async fn new_generation_has_an_exact_distinct_lifecycle_key() {
    let registry = TaskRegistry::new(2);
    let old = TaskKey::Lifecycle {
        package: "pkg".into(),
        version: "1".into(),
        generation: 1,
    };
    let new = TaskKey::Lifecycle {
        package: "pkg".into(),
        version: "1".into(),
        generation: 2,
    };
    let (_old_start, _old_cancel) = registry.reserve(old).expect("old lifecycle");
    let (_new_start, _new_cancel) = registry.reserve(new).expect("new lifecycle");
    assert_eq!(registry.len(), 2);
    assert!(registry.shutdown().await);
}

#[tokio::test]
async fn generation_cleanup_is_exact_and_preserves_replacement_slots() {
    let registry = TaskRegistry::new(6);
    let old_keys = [
        TaskKey::WorkerStdin {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        },
        TaskKey::WorkerStdout {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        },
        TaskKey::WorkerStderr {
            package: "pkg".into(),
            version: "1".into(),
            generation: 1,
        },
    ];
    let replacement = TaskKey::WorkerStdin {
        package: "pkg".into(),
        version: "1".into(),
        generation: 2,
    };
    for key in old_keys.iter().chain(std::iter::once(&replacement)) {
        let (start, _cancel) = registry.reserve(key.clone()).expect("reservation");
        drop(start);
    }
    assert!(registry.has_generation("pkg", "1", 1));
    assert!(registry.has_generation("pkg", "1", 2));
    assert!(registry.cancel_generation("pkg", "1", 1).await);
    assert!(!registry.has_generation("pkg", "1", 1));
    assert!(registry.has_generation("pkg", "1", 2));
    assert!(registry.shutdown().await);
}

#[tokio::test]
async fn stale_finish_cannot_claim_replacement_generation() {
    let supervisor = PackageWorkerSupervisor::new(1);
    let key = ("pkg".into(), "1".into());
    supervisor.insert_failed(key.clone());
    {
        let mut workers = lock(&supervisor.inner.workers);
        let worker = workers.get_mut(&key).expect("worker");
        worker.generation = 2;
        worker.lifecycle_reason = Some("replacement".into());
    }
    finish_inner_until(
        &supervisor.inner,
        &key,
        1,
        WorkerState::Failed,
        Instant::now() + STOP_DEADLINE,
    )
    .await;
    let workers = lock(&supervisor.inner.workers);
    let worker = workers.get(&key).expect("replacement");
    assert_eq!(worker.generation, 2);
    assert_eq!(worker.lifecycle_reason.as_deref(), Some("replacement"));
}
#[test]
fn cleanup_failure_is_a_distinct_diagnostic_reason() {
    let supervisor = PackageWorkerSupervisor::new(1);
    let key = ("cleanup".into(), "1".into());
    supervisor.insert_failed(key.clone());
    if let Some(worker) = lock(&supervisor.inner.workers).get_mut(&key) {
        worker.lifecycle_reason = Some("cleanup-failed".into());
    }
    assert_eq!(
        supervisor.diagnostics()[0].lifecycle_reason.as_deref(),
        Some("cleanup-failed")
    );
}
#[test]
fn app_and_non_exe_fail_closed() {
    let mut m = crate::package_manifest::PackageManifest {
        schema_version: 1,
        id: "x".into(),
        name: "x".into(),
        version: "1.0.0".into(),
        kind: PackageKind::App,
        engine_api: ">=1".into(),
        entrypoint: "x.exe".into(),
        publisher: "kosmos".into(),
        permissions: vec![],
    };
    assert!(PackageWorkerSupervisor::validate_manifest(&m).is_err());
    m.kind = PackageKind::Source;
    m.entrypoint = "x.js".into();
    assert!(PackageWorkerSupervisor::validate_manifest(&m).is_err());
}
#[test]
fn token_hash_does_not_accept_wrong_hello_token() {
    assert!(!token_matches(&hash_token("right"), "wrong"));
}

#[test]
fn restart_policy_uses_bounded_backoff() {
    assert_eq!(restart_delay(1), Some(Duration::from_secs(1)));
    assert_eq!(restart_delay(2), Some(Duration::from_secs(5)));
    assert_eq!(restart_delay(3), Some(Duration::from_secs(30)));
    assert_eq!(restart_delay(4), None);
}

#[cfg(not(windows))]
#[tokio::test]
async fn start_is_unsupported_on_non_windows_and_records_failure() {
    let supervisor = PackageWorkerSupervisor::new(1);
    let manifest = PackageManifest {
        schema_version: 1,
        id: "linux-test".into(),
        name: "linux-test".into(),
        version: "1.0.0".into(),
        kind: PackageKind::Source,
        engine_api: ">=1".into(),
        entrypoint: "worker.exe".into(),
        publisher: "kosmos".into(),
        permissions: vec![],
    };
    let result = supervisor
        .start(
            &manifest,
            PathBuf::from("/tmp/worker.exe"),
            "hash".into(),
            &[],
            "correlation".into(),
            None,
        )
        .await;
    assert_eq!(result, Err("unsupported-platform"));
    assert_eq!(
        supervisor.health("linux-test", "1.0.0").state,
        WorkerState::Failed
    );
}

#[test]
fn diagnostics_are_sorted_bounded_and_redacted() {
    let supervisor = PackageWorkerSupervisor::new(1);
    supervisor.insert_failed(("b".into(), "1".into()));
    supervisor.insert_failed(("a".into(), "1".into()));
    let key = ("a".to_string(), "1".to_string());
    let workers = lock(&supervisor.inner.workers);
    let worker = workers.get(&key).expect("worker");
    lock(&worker.stdout_tail).push("token=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa alice@example.com C:\\Users\\alice\\note.txt");
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
