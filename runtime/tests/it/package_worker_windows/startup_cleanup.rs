use super::*;

#[tokio::test]
async fn prepublication_wait_failures_quarantine_and_reap_exact_startup() {
    let _lock = test_support::serialized();
    for (suffix, cleanup_failure) in [
        ("wait-timeout", FailureStage::WaitTimeout),
        ("wait-failed", FailureStage::WaitFailed),
    ] {
        let state = tempfile::tempdir().expect("worker state directory");
        test_support::reset();
        let supervisor = PackageWorkerSupervisor::new(1);
        let worker = manifest(&format!("fixture.{suffix}"));
        test_support::fail_next(FailureStage::PreResume);
        test_support::fail_cleanup_next(cleanup_failure);
        assert_eq!(
            supervisor
                .test_start_once(
                    &worker,
                    fixture(),
                    state.path().to_path_buf(),
                    "hash".into(),
                    &[],
                    suffix.into(),
                )
                .await,
            Err("process-cleanup-failed")
        );
        assert_eq!(supervisor.test_registry_snapshot(), (0, 1, 0, 0));
        assert_eq!(
            supervisor.test_startup_reservation_snapshot(&worker.id, &worker.version, 1),
            Some((true, true))
        );
        assert_eq!(
            supervisor
                .test_start_once(
                    &worker,
                    fixture(),
                    state.path().to_path_buf(),
                    "hash".into(),
                    &[],
                    "replacement-before-reap".into(),
                )
                .await,
            Err("already-running")
        );
        test_support::reset();
        supervisor
            .test_reap_worker_startup(&worker.id, &worker.version)
            .await;
        assert_eq!(supervisor.test_registry_snapshot(), (0, 0, 0, 0));
        test_support::capture_next_process();
        assert_eq!(
            supervisor
                .test_start_once(
                    &worker,
                    fixture(),
                    state.path().to_path_buf(),
                    "hash".into(),
                    &[],
                    "replacement-after-reap".into(),
                )
                .await,
            Ok(())
        );
        let running = supervisor
            .test_worker_snapshot(&worker.id, &worker.version)
            .await;
        assert!(running.3, "replacement did not publish a real process");
        assert!(supervisor.stop(&worker.id, &worker.version).await.is_ok());
        assert!(test_support::take_captured_process()
            .expect("replacement process")
            .wait_object_0());
        assert_eq!(supervisor.test_registry_snapshot(), (0, 0, 0, 0));
        // Drain every supervisor registry so no task keeps a worker child or
        // an ArkHost handle alive past the test's tempdir cleanup.
        let _ = supervisor.stop_all().await;
    }
    test_support::reset();
}

#[tokio::test]
async fn supervisor_pid_unavailable_rolls_back_all_worker_reservations() {
    let _lock = test_support::serialized();
    let supervisor = PackageWorkerSupervisor::new(1);
    let worker = manifest("fixture.pid-unavailable");
    let state = tempfile::tempdir().expect("worker state directory");
    test_support::fail_next(engine::package_worker_process::FailureStage::PidUnavailable);

    assert_eq!(
        supervisor
            .test_start_once(
                &worker,
                fixture(),
                state.path().to_path_buf(),
                "hash".into(),
                &[],
                "pid-unavailable".into(),
            )
            .await,
        Err("pid-unavailable")
    );
    assert_eq!(supervisor.test_registry_snapshot(), (0, 0, 0, 0));
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup.
    let _ = supervisor.stop_all().await;
}

#[tokio::test]
async fn supervisor_cleanup_failure_retains_holder_and_disables_replacement() {
    let _lock = test_support::serialized();
    let supervisor = PackageWorkerSupervisor::new(1);
    let worker = manifest("fixture.cleanup-retained");
    let state = tempfile::tempdir().expect("worker state directory");
    supervisor
        .start(
            &worker,
            fixture(),
            state.path().to_path_buf(),
            "hash".into(),
            &[],
            "cleanup-retained".into(),
            None,
            None,
        )
        .await
        .expect("worker start");

    test_support::fail_cleanup_next(engine::package_worker_process::FailureStage::Terminate);
    assert_eq!(
        supervisor.stop(&worker.id, &worker.version).await,
        Err("cleanup-failed")
    );
    let snapshot = supervisor
        .test_worker_snapshot(&worker.id, &worker.version)
        .await;
    assert_eq!(snapshot.0, WorkerState::Failed);
    assert_eq!(snapshot.1.as_deref(), Some("cleanup-failed"));
    assert!(!snapshot.2);
    assert!(snapshot.3);
    assert_eq!(
        supervisor
            .start(
                &worker,
                fixture(),
                state.path().to_path_buf(),
                "hash".into(),
                &[],
                "replacement".into(),
                None,
                None,
            )
            .await,
        Err("already-running")
    );
    assert!(supervisor.stop(&worker.id, &worker.version).await.is_ok());
    assert!(
        !supervisor
            .test_worker_snapshot(&worker.id, &worker.version)
            .await
            .3
    );
    test_support::capture_next_process();
    assert!(supervisor
        .start(
            &worker,
            fixture(),
            state.path().to_path_buf(),
            "hash".into(),
            &[],
            "replacement-after-stop".into(),
            None,
            None,
        )
        .await
        .is_ok());
    let replacement = supervisor
        .test_worker_snapshot(&worker.id, &worker.version)
        .await;
    assert!(replacement.3, "replacement did not retain a real process");
    let mut holder_gate = supervisor
        .test_hold_process_holder(&worker.id, &worker.version)
        .expect("holder");
    holder_gate.ready().await;
    let terminations = test_support::termination_count();
    assert_eq!(
        supervisor
            .test_stop_all_until(std::time::Instant::now() + Duration::from_millis(1))
            .await,
        Err("cleanup-failed")
    );
    assert_eq!(test_support::termination_count(), terminations);
    holder_gate.release();
    assert!(supervisor.stop(&worker.id, &worker.version).await.is_ok());
    assert!(test_support::take_captured_process()
        .expect("replacement process")
        .wait_object_0());
    assert!(
        !supervisor
            .test_worker_snapshot(&worker.id, &worker.version)
            .await
            .3
    );
}

#[tokio::test]
async fn cancellation_after_process_publication_reaps_without_losing_holder() {
    let _lock = test_support::serialized();
    let supervisor = PackageWorkerSupervisor::new(1);
    let worker = manifest("fixture.cancel-after-launch");
    let warmup = manifest("fixture.handle-warmup");
    let state = tempfile::tempdir().expect("worker state directory");
    supervisor
        .test_start_once(
            &warmup,
            fixture(),
            state.path().to_path_buf(),
            "hash".into(),
            &[],
            "warmup".into(),
        )
        .await
        .expect("worker handle accounting warmup");
    supervisor
        .stop(&warmup.id, &warmup.version)
        .await
        .expect("warmup stop");
    let baseline_handles = test_support::process_handle_count();
    test_support::capture_next_process();
    let mut gate = supervisor.test_pause_after_launch();
    let task_supervisor = supervisor.clone();
    let task_worker = worker.clone();
    let task_state = state.path().to_path_buf();
    let launch = tokio::spawn(async move {
        task_supervisor
            .test_start_once(
                &task_worker,
                fixture(),
                task_state,
                "hash".into(),
                &[],
                "cancel-after-launch".into(),
            )
            .await
    });
    tokio::time::timeout(HANG_GUARD, gate.ready())
        .await
        .expect("after-launch gate");
    assert!(!launch.is_finished());
    assert_eq!(
        supervisor.test_startup_reservation_snapshot(&worker.id, &worker.version, 1),
        Some((true, true))
    );
    assert_eq!(supervisor.test_registry_snapshot().1, 1);

    assert_eq!(
        supervisor
            .test_start_once(
                &worker,
                fixture(),
                state.path().to_path_buf(),
                "replacement".into(),
                &[],
                "replacement-while-starting".into(),
            )
            .await,
        Err("worker-unavailable")
    );
    let cancel_supervisor = supervisor.clone();
    let cancel_worker = worker.clone();
    let cancel = tokio::spawn(async move {
        cancel_supervisor
            .test_reap_worker_startup(&cancel_worker.id, &cancel_worker.version)
            .await;
    });
    tokio::task::yield_now().await;
    assert_eq!(
        supervisor.test_startup_reservation_snapshot(&worker.id, &worker.version, 1),
        Some((true, true))
    );
    assert_eq!(supervisor.test_registry_snapshot().1, 1);
    assert!(!launch.is_finished());

    gate.release();
    assert!(matches!(
        tokio::time::timeout(HANG_GUARD, launch)
            .await
            .expect("launch cleanup")
            .expect("launch task"),
        Err("worker-unavailable")
    ));
    tokio::time::timeout(HANG_GUARD, cancel)
        .await
        .expect("startup cancellation")
        .expect("cancellation task");
    assert!(test_support::take_captured_process()
        .expect("published process")
        .wait_object_0());
    assert_eq!(test_support::process_handle_count(), baseline_handles);
    assert_eq!(
        supervisor.test_startup_reservation_snapshot(&worker.id, &worker.version, 1),
        None
    );
    assert_eq!(supervisor.test_registry_snapshot(), (0, 0, 0, 0));
}
