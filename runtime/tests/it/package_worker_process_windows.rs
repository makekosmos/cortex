#![cfg(all(windows, feature = "package-worker-fixture"))]
#![allow(clippy::await_holding_lock, clippy::unwrap_used)]

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use engine::package_worker_process::{
    test_support, FailureStage, LaunchCleanupOwner, WorkerProcess, WorkerProcessError,
};

/// Marker env vars and the worker failure hooks are process-wide, and other
/// test modules in this binary use both, so the markers own the engine's
/// worker test lock for as long as they exist. Drop resets both under it.
struct Markers {
    _serialized: test_support::FailureGuard<'static>,
    entry: PathBuf,
    bootstrap: PathBuf,
}

impl Drop for Markers {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("MUNDUS_FIXTURE_ENTRY_MARKER");
            std::env::remove_var("MUNDUS_FIXTURE_BOOTSTRAP_MARKER");
        }
        test_support::reset();
    }
}

fn markers() -> (tempfile::TempDir, Markers) {
    let serialized = test_support::serialized();
    let directory = tempfile::tempdir().expect("marker directory");
    let entry = directory.path().join("entry.marker");
    let bootstrap = directory.path().join("bootstrap.marker");
    unsafe {
        std::env::set_var("MUNDUS_FIXTURE_ENTRY_MARKER", &entry);
        std::env::set_var("MUNDUS_FIXTURE_BOOTSTRAP_MARKER", &bootstrap);
    }
    (
        directory,
        Markers {
            _serialized: serialized,
            entry,
            bootstrap,
        },
    )
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_package-worker-fixture"))
}

async fn launch(executable: PathBuf) -> Result<WorkerProcess, WorkerProcessError> {
    WorkerProcess::launch_with_owner(executable, LaunchCleanupOwner::new()).await
}

fn assert_handle_count_stable(before: u32) {
    let after = test_support::process_handle_count();
    assert!(
        after <= before + 2,
        "process handle leak: before={before}, after={after}"
    );
}

async fn handle_baseline() -> u32 {
    test_support::handle_baseline(fixture()).await
}

async fn wait_for_path(path: &std::path::Path) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("worker marker timeout");
}

async fn successful_handle_baseline(markers: &Markers) -> u32 {
    let _ = std::fs::remove_file(&markers.entry);
    let _ = std::fs::remove_file(&markers.bootstrap);
    handle_baseline().await
}

async fn pre_resume_failure(stage: FailureStage) {
    let (_directory, markers) = markers();
    test_support::reset_resume_count();
    let baseline = handle_baseline().await;
    test_support::capture_next_process();
    test_support::fail_next(stage);
    let result = launch(fixture()).await;
    assert!(
        matches!(result, Err(WorkerProcessError::Setup)),
        "result={result:?}"
    );
    assert!(!markers.entry.exists(), "entry marker unexpectedly exists");
    assert!(
        !markers.bootstrap.exists(),
        "bootstrap marker unexpectedly exists"
    );
    assert_eq!(test_support::resume_count(), 0);
    if let Some(process) = test_support::take_captured_process() {
        assert!(process.wait_object_0(), "captured process was not reaped");
    }
    assert_handle_count_stable(baseline);
}

#[tokio::test]
async fn pre_resume_pipe_no_entry() {
    pre_resume_failure(FailureStage::Pipe).await;
}

#[tokio::test]
async fn attribute_no_entry() {
    pre_resume_failure(FailureStage::Attribute).await;
}

#[tokio::test]
async fn create_process_no_entry() {
    pre_resume_failure(FailureStage::CreateProcess).await;
}

#[tokio::test]
async fn job_create_no_entry() {
    pre_resume_failure(FailureStage::JobCreate).await;
}

#[tokio::test]
async fn limits_no_entry() {
    pre_resume_failure(FailureStage::Limits).await;
}

#[tokio::test]
async fn cpu_no_entry() {
    pre_resume_failure(FailureStage::Cpu).await;
}

#[tokio::test]
async fn assign_no_entry() {
    pre_resume_failure(FailureStage::Assign).await;
}

#[tokio::test]
async fn pre_resume_no_entry() {
    pre_resume_failure(FailureStage::PreResume).await;
}

#[tokio::test]
async fn resume_failure_no_entry() {
    pre_resume_failure(FailureStage::Resume).await;
}

#[tokio::test]
async fn wrapper_failure_never_reaches_bootstrap() {
    let (_directory, markers) = markers();
    let baseline = handle_baseline().await;
    test_support::reset_resume_count();
    test_support::capture_next_process();
    test_support::fail_next(FailureStage::Wrapper);
    let result = launch(fixture()).await;
    assert!(
        matches!(result, Err(WorkerProcessError::Setup)),
        "result={result:?}"
    );
    assert!(!markers.bootstrap.exists());
    assert_eq!(test_support::resume_count(), 1);
    assert!(test_support::take_captured_process()
        .expect("captured process")
        .wait_object_0());
    assert_handle_count_stable(baseline);
}

#[tokio::test(flavor = "current_thread")]
async fn configure_failure_cleanup_does_not_block_current_thread_runtime() {
    let _lock = test_support::serialized();
    let mut gate = test_support::pause_next_cleanup_wait();
    test_support::fail_next(FailureStage::PreResume);
    let launch = tokio::spawn(launch(fixture()));
    gate.ready().await;
    let progress = Arc::new(AtomicBool::new(false));
    let progress_for_task = progress.clone();
    tokio::spawn(async move {
        progress_for_task.store(true, Ordering::SeqCst);
    });
    tokio::task::yield_now().await;
    assert!(progress.load(Ordering::SeqCst));
    gate.release();
    assert!(matches!(
        launch.await.expect("launch task"),
        Err(WorkerProcessError::Setup)
    ));
}

#[tokio::test]
async fn success_orders_entry_bootstrap_hello_and_job_limits() {
    let (_directory, markers) = markers();
    let baseline = successful_handle_baseline(&markers).await;
    test_support::reset_resume_count();
    test_support::capture_next_process();
    let mut barrier = test_support::pause_next_before_resume();
    let launch = tokio::spawn(launch(fixture()));
    tokio::time::timeout(Duration::from_secs(3), barrier.suspended_ready())
        .await
        .expect("resume barrier");
    assert!(!launch.is_finished());
    assert!(!markers.entry.exists(), "entry ran before resume");
    assert!(!markers.bootstrap.exists());
    assert_eq!(test_support::resume_count(), 0);
    barrier.release();
    let mut process = launch.await.expect("launch task").expect("launch");
    let allowlist = test_support::handle_allowlist_snapshot().expect("raw handle allowlist");
    assert_eq!(allowlist.count, 3);
    assert_eq!(
        allowlist.bytes,
        3 * std::mem::size_of::<windows::Win32::Foundation::HANDLE>()
    );
    wait_for_path(&markers.entry).await;
    assert!(!markers.bootstrap.exists());
    assert_eq!(test_support::resume_count(), 1);
    let snapshot = process.test_job_snapshot().expect("job snapshot");
    assert!(snapshot.kill_on_close);
    assert_eq!(snapshot.active_process_limit, 1);
    assert_eq!(snapshot.job_memory_limit, 512 * 1024 * 1024);
    assert!(snapshot.cpu_hard_cap);
    assert_eq!(snapshot.cpu_rate, 2500);
    let bootstrap = concat!(
        r#"{"package_id":"fixture","version":"1","hash":"hash","pid":1,"api_version":1,"#,
        r#""token":"token"}
"#,
    )
    .as_bytes();
    process
        .test_send_line(bootstrap)
        .await
        .expect("bootstrap write");
    let hello = process.test_read_line().await.expect("hello");
    let hello = String::from_utf8(hello).expect("hello utf8");
    assert!(hello.contains("worker.hello"));
    process
        .test_send_line(bootstrap)
        .await
        .expect("second bootstrap write");
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        std::fs::read_to_string(&markers.entry).unwrap(),
        "entry:1\n"
    );
    assert_eq!(
        std::fs::read_to_string(&markers.bootstrap).unwrap(),
        "bootstrap:1\n"
    );
    assert!(process.stop().await.is_ok());
    assert!(test_support::take_captured_process()
        .expect("captured process")
        .wait_object_0());
    assert_handle_count_stable(baseline);
}

#[tokio::test]
async fn dropping_launch_at_pipe_connect_reaps_child_and_closes_handles() {
    let (_directory, _markers) = markers();
    let baseline = handle_baseline().await;
    test_support::capture_next_process();
    let mut barrier = test_support::pause_next_before_pipe_connect();
    let owner = LaunchCleanupOwner::new();
    let launch = tokio::spawn(WorkerProcess::launch_with_owner(fixture(), owner.clone()));
    barrier.suspended_ready().await;
    assert!(!launch.is_finished());

    launch.abort();
    assert!(launch
        .await
        .expect_err("launch must be cancelled")
        .is_cancelled());
    owner
        .cleanup_until(Instant::now() + Duration::from_secs(10))
        .await
        .expect("retry cleanup");
    assert!(
        test_support::take_captured_process()
            .expect("captured process")
            .wait_object_0(),
        "cancelled launch left child unreaped"
    );
    assert_handle_count_stable(baseline);
}

#[tokio::test]
async fn dropping_launch_before_resume_reaps_child_and_closes_handles() {
    let (_directory, _markers) = markers();
    let baseline = handle_baseline().await;
    test_support::capture_next_process();
    let mut barrier = test_support::pause_next_before_resume();
    let owner = LaunchCleanupOwner::new();
    let launch = tokio::spawn(WorkerProcess::launch_with_owner(fixture(), owner.clone()));
    barrier.suspended_ready().await;
    assert!(!launch.is_finished());

    launch.abort();
    assert!(launch
        .await
        .expect_err("launch must be cancelled")
        .is_cancelled());
    owner
        .cleanup_until(Instant::now() + Duration::from_secs(10))
        .await
        .expect("retry cleanup");
    assert_eq!(test_support::resume_count(), 0);
    assert!(
        test_support::take_captured_process()
            .expect("captured process")
            .wait_object_0(),
        "cancelled launch left child unreaped"
    );
    assert_handle_count_stable(baseline);
}

async fn cleanup_failure(stage: FailureStage) {
    let (_directory, markers) = markers();
    let baseline = handle_baseline().await;
    test_support::reset_resume_count();
    test_support::capture_next_process();
    test_support::fail_next(FailureStage::PreResume);
    test_support::fail_cleanup_next(stage);
    let owner = LaunchCleanupOwner::new();
    let result = WorkerProcess::launch_with_owner(fixture(), owner.clone()).await;
    assert!(
        matches!(result, Err(WorkerProcessError::Cleanup)),
        "result={result:?}"
    );
    assert!(!markers.entry.exists());
    assert!(!markers.bootstrap.exists());
    assert_eq!(test_support::resume_count(), 0);
    owner
        .cleanup_until(Instant::now() + Duration::from_secs(10))
        .await
        .expect("retry cleanup");
    assert!(test_support::take_captured_process()
        .expect("captured process")
        .wait_object_0());
    assert_handle_count_stable(baseline);
}

#[tokio::test]
async fn terminate_failure_returns_cleanup() {
    cleanup_failure(FailureStage::Terminate).await;
}

#[tokio::test]
async fn wait_timeout_returns_cleanup() {
    cleanup_failure(FailureStage::WaitTimeout).await;
}

#[tokio::test]
async fn wait_failed_returns_cleanup() {
    cleanup_failure(FailureStage::WaitFailed).await;
}

#[tokio::test]
async fn expired_deadline_does_not_invoke_termination() {
    let (_directory, _markers) = markers();
    test_support::capture_next_process();
    let owner = LaunchCleanupOwner::new();
    let mut process = WorkerProcess::launch_with_owner(fixture(), owner)
        .await
        .expect("launch");
    let result = process.stop_until(Instant::now()).await;
    assert!(matches!(result, Err(WorkerProcessError::Cleanup)));
    // The expired deadline must have skipped TerminateJobObject entirely; a
    // terminated child is reaped in milliseconds, so a 1 s negative wait is
    // already conclusive.
    assert!(!test_support::take_captured_process()
        .expect("captured process")
        .wait_object_0_within(Duration::from_secs(1)));
    process
        .stop_until(Instant::now() + Duration::from_secs(10))
        .await
        .expect("retry stop");
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_cleanup_future_retains_owner_for_retry() {
    let (_directory, _markers) = markers();
    let mut gate = test_support::pause_next_cleanup_wait();
    test_support::fail_next(FailureStage::PreResume);
    let owner = LaunchCleanupOwner::new();
    let launch = tokio::spawn(WorkerProcess::launch_with_owner(fixture(), owner.clone()));
    gate.ready().await;
    launch.abort();
    assert!(launch.await.expect_err("launch task").is_cancelled());
    gate.release();
    owner
        .cleanup_until(Instant::now() + Duration::from_secs(10))
        .await
        .expect("owner retry");
}

#[tokio::test]
async fn take_all_pipes_is_atomic_on_a_real_worker_process() {
    let (_directory, _markers) = markers();
    let mut process = launch(fixture()).await.expect("worker launch");
    assert!(test_support::take_all_pipes(&mut process));
    assert!(!test_support::take_all_pipes(&mut process));
    process.stop().await.expect("worker stop");
}
