use super::*;

#[tokio::test]
async fn fixture_workers_validate_protocol_and_fail_closed() {
    let directory = tempfile::tempdir().expect("fixture marker directory");
    let markers = fixture_env(&directory);
    // Short injected backoff: the production schedule (1s/5s/30s, see
    // restart_policy_uses_bounded_backoff) is exercised shape-for-shape here,
    // including the assertion that retries are spaced, without spending 36 s
    // of wall time on sleeps.
    let restart_delays = vec![
        Duration::from_millis(250),
        Duration::from_millis(500),
        Duration::from_millis(750),
    ];
    let min_retry_wait: Duration = restart_delays.iter().sum();
    let supervisor = PackageWorkerSupervisor::with_restart_delays(1, restart_delays);
    let state = tempfile::tempdir().expect("worker state directory");
    let roots = [std::env::temp_dir()];
    let normal = manifest("fixture.normal");
    test_support::reset_resume_count();
    let mut barrier = test_support::pause_next_before_resume();
    let state_root = state.path().to_path_buf();
    let start_supervisor = supervisor.clone();
    let start_manifest = normal.clone();
    let launch = tokio::spawn(async move {
        start_supervisor
            .start(
                &start_manifest,
                fixture(),
                state_root,
                "hash".into(),
                &[std::env::temp_dir()],
                "corr".into(),
                None,
                None,
            )
            .await
    });
    // The barrier fires inside the worker's pre-resume hook — the event is the
    // synchronization, no wall-clock cap needed.
    barrier.suspended_ready().await;
    assert!(!launch.is_finished());
    assert!(!markers.entry.exists());
    assert!(!markers.bootstrap.exists());
    assert_eq!(test_support::resume_count(), 0);
    barrier.release();
    launch
        .await
        .expect("start task")
        .expect("normal worker start");
    assert_eq!(test_support::resume_count(), 1);
    assert_eq!(
        supervisor.health(&normal.id, &normal.version).state,
        WorkerState::Running
    );
    assert!(
        markers.entry.exists(),
        "entry marker must precede bootstrap"
    );
    assert_eq!(
        std::fs::read_to_string(&markers.bootstrap).unwrap(),
        "bootstrap:1\n"
    );
    assert_eq!(
        supervisor
            .start(
                &normal,
                fixture(),
                state.path().to_path_buf(),
                "hash".into(),
                &roots,
                "corr".into(),
                None,
                None
            )
            .await,
        Err("already-running")
    );
    supervisor
        .stop(&normal.id, &normal.version)
        .await
        .expect("worker stop");

    let m = manifest("fixture.wrong-token");
    let started = Instant::now();
    let result = tokio::time::timeout(
        HANG_GUARD,
        supervisor.start(
            &m,
            fixture(),
            state.path().to_path_buf(),
            "hash".into(),
            &roots,
            "corr".into(),
            None,
            None,
        ),
    )
    .await
    .expect("fixture timeout");
    assert_eq!(result, Err("worker-unavailable"));
    assert!(
        started.elapsed() >= min_retry_wait,
        "retries must wait out the injected backoff schedule, elapsed {:?}",
        started.elapsed()
    );
    assert_eq!(
        supervisor.health(&m.id, &m.version).state,
        WorkerState::Failed
    );
    let diagnostic = supervisor
        .diagnostics()
        .into_iter()
        .find(|worker| worker.id == m.id)
        .expect("diagnostic");
    assert_eq!(diagnostic.generation, 4);
    assert_eq!(diagnostic.restart_count, 3);
    let health = format!("{:?}", supervisor.health(&m.id, &m.version));
    assert!(!health.contains("hash") && !health.contains("corr"));
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup.
    let _ = supervisor.stop_all().await;
}

#[tokio::test]
async fn stop_suppresses_initial_failure_retries() {
    let _lock = test_support::serialized();
    // Short first backoff lets one retry run fast; the long second delay is
    // what `stop` must cancel before the third generation ever launches.
    let supervisor = PackageWorkerSupervisor::with_restart_delays(
        1,
        vec![Duration::from_millis(400), Duration::from_secs(3)],
    );
    let m = manifest("fixture.initial-fail");
    let state = tempfile::tempdir().expect("worker state directory");
    let start_manifest = m.clone();
    let running = supervisor.clone();
    let start = tokio::spawn(async move {
        running
            .start(
                &start_manifest,
                fixture(),
                state.path().to_path_buf(),
                "hash".into(),
                &[std::env::temp_dir()],
                "corr".into(),
                None,
                None,
            )
            .await
    });
    // Wait on the real event — generation 2 means the first attempt failed and
    // the 400 ms retry was consumed — instead of a fixed sleep that CPU
    // starvation can delay past the point it was meant to observe.
    // The 3 s second delay is then the window `stop` must cancel.
    tokio::time::timeout(HANG_GUARD, async {
        loop {
            if supervisor
                .diagnostics()
                .into_iter()
                .find(|worker| worker.id == m.id)
                .is_some_and(|worker| worker.generation >= 2)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("first retry never started");
    supervisor
        .stop(&m.id, &m.version)
        .await
        .expect("worker stop");
    assert_eq!(start.await.expect("start task"), Err("worker-unavailable"));
    let diagnostic = supervisor
        .diagnostics()
        .into_iter()
        .find(|worker| worker.id == m.id)
        .expect("diagnostic");
    assert!(diagnostic.generation <= 2);
    assert_eq!(
        supervisor.health(&m.id, &m.version).state,
        WorkerState::Stopped
    );
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup.
    let _ = supervisor.stop_all().await;
}
