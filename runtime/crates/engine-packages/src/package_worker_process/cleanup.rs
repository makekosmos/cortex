#[cfg(windows)]
struct CleanupAttempt {
    owner: Arc<LaunchCleanupOwner>,
    epoch: u64,
    prior_state: OwnerState,
    process: windows::Win32::Foundation::HANDLE,
    job: Option<windows::Win32::Foundation::HANDLE>,
    wait_handle: Option<OwnedHandle>,
    terminated: bool,
    finalized: bool,
}

#[cfg(windows)]
// HANDLE values are process-wide kernel objects and the cleanup operation is
// deliberately moved to Tokio worker tasks.
unsafe impl Send for CleanupAttempt {}

#[cfg(windows)]
impl CleanupAttempt {
    fn terminate(&mut self) -> Result<(), WorkerProcessError> {
        use windows::Win32::System::JobObjects::TerminateJobObject;
        use windows::Win32::System::Threading::TerminateProcess;
        if self.terminated {
            return Ok(());
        }
        if should_fail_cleanup(FailureStage::Terminate) {
            return Err(WorkerProcessError::Cleanup);
        }
        #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
        TERMINATION_COUNT.fetch_add(1, Ordering::SeqCst);
        let result = unsafe {
            match self.job {
                Some(job) => TerminateJobObject(job, 1),
                None => TerminateProcess(self.process, 1),
            }
        };
        result.map_err(|_| WorkerProcessError::Cleanup)?;
        self.terminated = true;
        Ok(())
    }

    async fn wait_until(&mut self, deadline: Instant) -> Result<(), WorkerProcessError> {
        use windows::Win32::Foundation::{WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};
        use windows::Win32::System::Threading::WaitForSingleObject;
        #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
        if let Some(gate) = take_cleanup_wait_gate() {
            let _ = gate.ready.send(());
            let _ = gate.release.await;
        }
        loop {
            if Instant::now() >= deadline {
                return Err(WorkerProcessError::Cleanup);
            }
            if should_fail_cleanup(FailureStage::WaitTimeout) {
                return Err(WorkerProcessError::Cleanup);
            }
            if should_fail_cleanup(FailureStage::WaitFailed) {
                return Err(WorkerProcessError::Cleanup);
            }
            let result = unsafe {
                WaitForSingleObject(
                    self.wait_handle
                        .as_ref()
                        .ok_or(WorkerProcessError::Cleanup)?
                        .raw(),
                    0,
                )
            };
            if result == WAIT_OBJECT_0 {
                self.finalize()?;
                return Ok(());
            }
            if result == WAIT_FAILED {
                return Err(WorkerProcessError::Cleanup);
            }
            if result != WAIT_TIMEOUT {
                return Err(WorkerProcessError::Cleanup);
            }
            tokio::time::sleep(
                Duration::from_millis(1).min(deadline.saturating_duration_since(Instant::now())),
            )
            .await;
        }
    }

    fn finalize(&mut self) -> Result<(), WorkerProcessError> {
        let mut state = self.owner.lock();
        if state.state != OwnerState::Cleaning || state.epoch != self.epoch {
            return Err(WorkerProcessError::Cleanup);
        }
        drop(state.thread.take());
        drop(state.job.take());
        drop(state.process.take());
        state.state = OwnerState::Clean;
        self.finalized = true;
        self.wait_handle.take();
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for CleanupAttempt {
    fn drop(&mut self) {
        if self.finalized {
            return;
        }
        let mut state = self.owner.lock();
        if state.state == OwnerState::Cleaning && state.epoch == self.epoch {
            state.terminated = self.terminated;
            state.state = self.prior_state;
        }
    }
}

#[cfg(windows)]
type WorkerPipe = tokio::net::windows::named_pipe::NamedPipeServer;

#[cfg(windows)]
const fn child_pipe_access(parent_reads: bool) -> (bool, bool) {
    // (child_reads, child_writes): the child has the complementary direction
    // to the parent-owned server endpoint.
    (!parent_reads, parent_reads)
}
