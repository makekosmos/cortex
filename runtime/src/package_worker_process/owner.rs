#[cfg(windows)]
#[derive(Default)]
struct LaunchCleanupState {
    state: OwnerState,
    epoch: u64,
    process: Option<ProcessHandle>,
    thread: Option<OwnedHandle>,
    job: Option<JobHandle>,
    terminated: bool,
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerState {
    Fresh,
    Claimed,
    Created,
    JobInstalled,
    Resumed,
    Committed,
    Cleaning,
    Clean,
}

#[cfg(windows)]
impl Default for OwnerState {
    fn default() -> Self {
        Self::Fresh
    }
}

/// Server-owned authority for a suspended launch. It never performs cleanup in
/// Drop; the Startup registry retains this Arc until cleanup is confirmed.
#[cfg(windows)]
pub struct LaunchCleanupOwner {
    state: Mutex<LaunchCleanupState>,
}

#[cfg(windows)]
impl LaunchCleanupOwner {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(LaunchCleanupState::default()),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, LaunchCleanupState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn claim(&self) -> Result<(), WorkerProcessError> {
        let mut state = self.lock();
        if state.state != OwnerState::Fresh {
            return Err(WorkerProcessError::Setup);
        }
        state.state = OwnerState::Claimed;
        state.epoch = state.epoch.wrapping_add(1);
        Ok(())
    }

    fn abandon_claim(&self) {
        let mut state = self.lock();
        if state.state == OwnerState::Claimed && state.process.is_none() {
            state.state = OwnerState::Clean;
        }
    }

    pub fn resume(&self) -> Result<(), WorkerProcessError> {
        use windows::Win32::System::Threading::ResumeThread;
        let mut state = self.lock();
        if state.state != OwnerState::JobInstalled {
            return Err(WorkerProcessError::Setup);
        }
        let thread = state
            .thread
            .as_ref()
            .ok_or(WorkerProcessError::Setup)?
            .raw();
        if should_fail_stage(FailureStage::Resume) || unsafe { ResumeThread(thread) } != 1 {
            return Err(WorkerProcessError::Setup);
        }
        state.state = OwnerState::Resumed;
        Ok(())
    }

    pub fn commit(
        &self,
        stdin: WorkerPipe,
        stdout: WorkerPipe,
        stderr: WorkerPipe,
    ) -> Result<WorkerProcess, WorkerProcessError> {
        let mut state = self.lock();
        if state.state != OwnerState::Resumed {
            return Err(WorkerProcessError::Setup);
        }
        let process = state.process.take().ok_or(WorkerProcessError::Setup)?;
        let job = state.job.take().ok_or(WorkerProcessError::Setup)?;
        drop(state.thread.take());
        state.state = OwnerState::Committed;
        Ok(WorkerProcess {
            process,
            stdin: Some(stdin),
            stdout: Some(stdout),
            stderr: Some(stderr),
            job,
        })
    }

    pub async fn cleanup_until(
        self: &Arc<Self>,
        deadline: Instant,
    ) -> Result<(), WorkerProcessError> {
        if Instant::now() >= deadline {
            return Err(WorkerProcessError::Cleanup);
        }
        let mut attempt = self.begin_cleanup(deadline)?;
        attempt.terminate()?;
        attempt.wait_until(deadline).await
    }

    fn begin_cleanup(
        self: &Arc<Self>,
        deadline: Instant,
    ) -> Result<CleanupAttempt, WorkerProcessError> {
        if Instant::now() >= deadline {
            return Err(WorkerProcessError::Cleanup);
        }
        let mut state = self.lock();
        match state.state {
            OwnerState::Created | OwnerState::JobInstalled | OwnerState::Resumed => {}
            OwnerState::Cleaning
            | OwnerState::Committed
            | OwnerState::Clean
            | OwnerState::Fresh
            | OwnerState::Claimed => return Err(WorkerProcessError::Cleanup),
        }
        let (process_raw, wait_handle) = {
            let process = state.process.as_ref().ok_or(WorkerProcessError::Cleanup)?;
            (process.handle.raw(), duplicate_handle(&process.handle)?)
        };
        let prior_state = state.state;
        state.state = OwnerState::Cleaning;
        Ok(CleanupAttempt {
            owner: self.clone(),
            epoch: state.epoch,
            prior_state,
            process: process_raw,
            job: state.job.as_ref().map(|job| job.0.raw()),
            wait_handle: Some(wait_handle),
            terminated: state.terminated,
            finalized: false,
        })
    }

    pub fn is_armed(&self) -> bool {
        matches!(
            self.lock().state,
            OwnerState::Claimed
                | OwnerState::Created
                | OwnerState::JobInstalled
                | OwnerState::Resumed
                | OwnerState::Cleaning
        )
    }
}
