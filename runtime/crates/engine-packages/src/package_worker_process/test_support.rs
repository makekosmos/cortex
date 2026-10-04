#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
#[allow(clippy::await_holding_lock, private_interfaces)]
pub mod test_support {
    use super::*;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};

    pub struct FailureGuard<'a> {
        _guard: std::sync::MutexGuard<'a, ()>,
    }

    pub fn serialized() -> FailureGuard<'static> {
        FailureGuard {
            _guard: windows_worker_test_lock(),
        }
    }

    pub fn fail_next(stage: FailureStage) {
        JOB_SETUP_FAILURE.store(stage as u8, Ordering::SeqCst);
    }

    pub fn fail_cleanup_next(stage: FailureStage) {
        CLEANUP_FAILURE.store(stage as u8, Ordering::SeqCst);
    }

    pub fn resume_count() -> u64 {
        RESUME_COUNT.load(Ordering::SeqCst)
    }

    pub fn reset_resume_count() {
        RESUME_COUNT.store(0, Ordering::SeqCst);
        NEXT_RESUME_BARRIER
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
    }

    pub struct ResumeBarrier {
        ready: tokio::sync::oneshot::Receiver<()>,
        release: Option<tokio::sync::oneshot::Sender<()>>,
    }

    pub fn pause_next_before_resume() -> ResumeBarrier {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        *NEXT_RESUME_BARRIER
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(ResumeBarrierParts {
            ready: ready_tx,
            release: release_rx,
        });
        ResumeBarrier {
            ready: ready_rx,
            release: Some(release_tx),
        }
    }

    pub fn pause_next_before_pipe_connect() -> ResumeBarrier {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        *NEXT_PIPE_CONNECT_BARRIER
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(PipeConnectBarrierParts {
            ready: ready_tx,
            release: release_rx,
        });
        ResumeBarrier {
            ready: ready_rx,
            release: Some(release_tx),
        }
    }

    impl ResumeBarrier {
        pub async fn suspended_ready(&mut self) {
            let _ = (&mut self.ready).await;
        }

        pub fn release(&mut self) {
            if let Some(release) = self.release.take() {
                let _ = release.send(());
            }
        }
    }

    impl Drop for ResumeBarrier {
        fn drop(&mut self) {
            self.release();
        }
    }

    pub struct CleanupWaitGate {
        ready: tokio::sync::oneshot::Receiver<()>,
        release: Option<tokio::sync::oneshot::Sender<()>>,
    }

    pub fn pause_next_cleanup_wait() -> CleanupWaitGate {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        *NEXT_CLEANUP_WAIT_GATE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(CleanupWaitGateParts {
            ready: ready_tx,
            release: release_rx,
        });
        CleanupWaitGate {
            ready: ready_rx,
            release: Some(release_tx),
        }
    }

    impl CleanupWaitGate {
        pub async fn ready(&mut self) {
            let _ = (&mut self.ready).await;
        }

        pub fn release(&mut self) {
            if let Some(release) = self.release.take() {
                let _ = release.send(());
            }
        }
    }

    impl Drop for CleanupWaitGate {
        fn drop(&mut self) {
            self.release();
        }
    }

    pub fn capture_next_process() {
        LAST_SPAWNED_HANDLE.store(0, Ordering::SeqCst);
        LAST_SPAWNED_PID.store(0, Ordering::SeqCst);
        CAPTURE_CHILD_HANDLE.store(true, Ordering::SeqCst);
    }

    pub fn created_process_count() -> u64 {
        CREATED_PROCESS_COUNT.load(Ordering::SeqCst)
    }

    pub fn termination_count() -> u64 {
        TERMINATION_COUNT.load(Ordering::SeqCst)
    }

    pub struct CapturedProcess {
        handle: HANDLE,
        pub pid: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct JobSnapshot {
        pub kill_on_close: bool,
        pub active_process_limit: u32,
        pub job_memory_limit: usize,
        pub cpu_hard_cap: bool,
        pub cpu_rate: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct HandleAllowlistSnapshot {
        pub count: usize,
        pub bytes: usize,
    }

    pub fn handle_allowlist_snapshot() -> Option<HandleAllowlistSnapshot> {
        let count = LAST_ALLOWLIST_COUNT.load(Ordering::SeqCst);
        (count != 0).then_some(HandleAllowlistSnapshot {
            count,
            bytes: LAST_ALLOWLIST_BYTES.load(Ordering::SeqCst),
        })
    }

    pub(crate) fn query_job(job: &JobHandle) -> Result<JobSnapshot, WorkerProcessError> {
        use std::mem::size_of;
        use windows::Win32::System::JobObjects::*;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        let mut returned = 0;
        unsafe {
            QueryInformationJobObject(
                Some(job.raw()),
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION)
                    .cast::<std::ffi::c_void>(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                Some(&mut returned),
            )
            .map_err(|_| WorkerProcessError::Setup)?;
        }
        let mut cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION::default();
        unsafe {
            QueryInformationJobObject(
                Some(job.raw()),
                JobObjectCpuRateControlInformation,
                (&mut cpu as *mut JOBOBJECT_CPU_RATE_CONTROL_INFORMATION)
                    .cast::<std::ffi::c_void>(),
                size_of::<JOBOBJECT_CPU_RATE_CONTROL_INFORMATION>() as u32,
                Some(&mut returned),
            )
            .map_err(|_| WorkerProcessError::Setup)?;
        }
        Ok(JobSnapshot {
            kill_on_close: limits.BasicLimitInformation.LimitFlags
                & JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                != windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT(0),
            active_process_limit: limits.BasicLimitInformation.ActiveProcessLimit,
            job_memory_limit: limits.JobMemoryLimit,
            cpu_hard_cap: cpu.ControlFlags & JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP
                != windows::Win32::System::JobObjects::JOB_OBJECT_CPU_RATE_CONTROL(0),
            cpu_rate: unsafe { cpu.Anonymous.CpuRate },
        })
    }

    impl CapturedProcess {
        pub fn wait_object_0(self) -> bool {
            self.wait_object_0_within(Duration::from_secs(10))
        }

        /// Bounded wait for tests that assert a process did NOT exit: a
        /// terminated child is reaped by the kernel within milliseconds, so a
        /// short timeout already proves non-termination without a 10 s sleep.
        pub fn wait_object_0_within(self, timeout: Duration) -> bool {
            let millis = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
            let result = unsafe {
                windows::Win32::System::Threading::WaitForSingleObject(self.handle, millis)
            } == WAIT_OBJECT_0;
            unsafe {
                let _ = CloseHandle(self.handle);
            }
            result
        }
    }

    pub fn take_captured_process() -> Option<CapturedProcess> {
        let raw = LAST_SPAWNED_HANDLE.swap(0, Ordering::SeqCst);
        (raw != 0).then_some(CapturedProcess {
            handle: HANDLE(raw as *mut std::ffi::c_void),
            pid: LAST_SPAWNED_PID.load(Ordering::SeqCst),
        })
    }

    pub fn process_handle_count() -> u32 {
        let mut count = 0;
        unsafe {
            let _ = windows::Win32::System::Threading::GetProcessHandleCount(
                windows::Win32::System::Threading::GetCurrentProcess(),
                &mut count,
            );
        }
        count
    }

    pub async fn handle_baseline(executable: impl AsRef<Path>) -> u32 {
        reset();
        fail_next(FailureStage::Assign);
        assert!(matches!(
            WorkerProcess::launch_with_owner(
                executable.as_ref().to_path_buf(),
                LaunchCleanupOwner::new(),
            )
            .await,
            Err(WorkerProcessError::Setup)
        ));
        let mut worker = WorkerProcess::launch_with_owner(
            executable.as_ref().to_path_buf(),
            LaunchCleanupOwner::new(),
        )
        .await
        .expect("worker handle warmup");
        worker.stop().await.expect("worker handle warmup stop");
        let baseline = process_handle_count();
        reset_resume_count();
        baseline
    }

    pub fn take_all_pipes(process: &mut WorkerProcess) -> bool {
        process.take_all_pipes().is_some()
    }

    pub fn reset() {
        JOB_SETUP_FAILURE.store(0, Ordering::SeqCst);
        CLEANUP_FAILURE.store(0, Ordering::SeqCst);
        CAPTURE_CHILD_HANDLE.store(false, Ordering::SeqCst);
        LAST_ALLOWLIST_COUNT.store(0, Ordering::SeqCst);
        LAST_ALLOWLIST_BYTES.store(0, Ordering::SeqCst);
        CREATED_PROCESS_COUNT.store(0, Ordering::SeqCst);
        TERMINATION_COUNT.store(0, Ordering::SeqCst);
        NEXT_PIPE_CONNECT_BARRIER
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        if let Some(process) = take_captured_process() {
            let _ = process.wait_object_0();
        }
    }
}
