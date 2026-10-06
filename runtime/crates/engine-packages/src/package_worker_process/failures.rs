#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureStage {
    Pipe = 1,
    Attribute = 2,
    CreateProcess = 3,
    JobCreate = 4,
    Limits = 5,
    Cpu = 6,
    Assign = 7,
    PreResume = 8,
    Resume = 9,
    Wrapper = 10,
    Terminate = 11,
    WaitTimeout = 12,
    WaitFailed = 13,
    #[cfg(windows)]
    PidUnavailable = 14,
}
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static JOB_SETUP_FAILURE: AtomicU8 = AtomicU8::new(0);
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static CLEANUP_FAILURE: AtomicU8 = AtomicU8::new(0);
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static LAST_SPAWNED_HANDLE: AtomicIsize = AtomicIsize::new(0);
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static LAST_SPAWNED_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static CREATED_PROCESS_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static TERMINATION_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static CAPTURE_CHILD_HANDLE: AtomicBool = AtomicBool::new(false);
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static WINDOWS_WORKER_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static NEXT_RESUME_BARRIER: OnceLock<Mutex<Option<ResumeBarrierParts>>> = OnceLock::new();
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static NEXT_PIPE_CONNECT_BARRIER: OnceLock<Mutex<Option<PipeConnectBarrierParts>>> =
    OnceLock::new();

#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static NEXT_CLEANUP_WAIT_GATE: OnceLock<Mutex<Option<CleanupWaitGateParts>>> = OnceLock::new();
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
struct CleanupWaitGateParts {
    ready: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static RESUME_COUNT: AtomicU64 = AtomicU64::new(0);

#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static LAST_ALLOWLIST_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
static LAST_ALLOWLIST_BYTES: AtomicUsize = AtomicUsize::new(0);
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
struct ResumeBarrierParts {
    ready: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
struct PipeConnectBarrierParts {
    ready: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
fn windows_worker_test_lock() -> std::sync::MutexGuard<'static, ()> {
    WINDOWS_WORKER_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
fn should_fail_stage(stage: FailureStage) -> bool {
    JOB_SETUP_FAILURE
        .compare_exchange(stage as u8, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
fn should_fail_cleanup(stage: FailureStage) -> bool {
    CLEANUP_FAILURE
        .compare_exchange(stage as u8, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}
#[cfg(all(windows, not(any(test, feature = "package-worker-fixture"))))]
fn should_fail_cleanup(_: FailureStage) -> bool {
    false
}
#[cfg(all(windows, not(any(test, feature = "package-worker-fixture"))))]
fn should_fail_stage(_: FailureStage) -> bool {
    false
}
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
fn take_cleanup_wait_gate() -> Option<CleanupWaitGateParts> {
    NEXT_CLEANUP_WAIT_GATE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
}
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
fn take_resume_barrier() -> Option<ResumeBarrierParts> {
    NEXT_RESUME_BARRIER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
}
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
fn take_pipe_connect_barrier() -> Option<PipeConnectBarrierParts> {
    NEXT_PIPE_CONNECT_BARRIER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
}

#[cfg(test)]
fn advance_launch_state(state: LaunchState) -> Option<LaunchState> {
    match state {
        LaunchState::JobConfigured => Some(LaunchState::Assigned),
        LaunchState::Assigned => Some(LaunchState::Resumed),
        _ => None,
    }
}
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
fn capture_process_handle(process: windows::Win32::Foundation::HANDLE) {
    if CAPTURE_CHILD_HANDLE.load(Ordering::SeqCst) {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS};
        use windows::Win32::System::Threading::GetCurrentProcess;
        let mut copy = HANDLE::default();
        unsafe {
            let _ = DuplicateHandle(
                GetCurrentProcess(),
                process,
                GetCurrentProcess(),
                &mut copy,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            );
        }
        LAST_SPAWNED_HANDLE.store(copy.0 as isize, Ordering::SeqCst);
        LAST_SPAWNED_PID.store(
            unsafe { windows::Win32::System::Threading::GetProcessId(process) },
            Ordering::SeqCst,
        );
    }
}

#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
fn capture_handle_allowlist(count: usize, bytes: usize) {
    LAST_ALLOWLIST_COUNT.store(count, Ordering::SeqCst);
    LAST_ALLOWLIST_BYTES.store(bytes, Ordering::SeqCst);
}
