use std::path::{Path, PathBuf};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    time::{Duration, Instant},
};

use crate::package_store::PackageStore;

use thiserror::Error;

#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicU8, AtomicUsize, Ordering};
#[cfg(any(windows, target_os = "macos"))]
use std::sync::Arc;
#[cfg(windows)]
use std::sync::Mutex;
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
use std::sync::OnceLock;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchState {
    CreatedSuspended,
    JobConfigured,
    Assigned,
    Resumed,
}

#[derive(Debug, Error)]
pub enum WorkerProcessError {
    #[error("worker executable must be an absolute regular .exe file")]
    InvalidExecutable,
    #[error("package workers are unsupported on this platform")]
    UnsupportedPlatform,
    #[error("worker process setup failed")]
    Setup,
    #[error("worker process cleanup failed")]
    Cleanup,
}

/// A spawned worker and (on Windows) the Job Object that owns its process tree.
pub struct WorkerProcess {
    #[cfg(windows)]
    process: ProcessHandle,
    #[cfg(windows)]
    stdin: Option<WorkerPipe>,
    #[cfg(windows)]
    stdout: Option<WorkerPipe>,
    #[cfg(windows)]
    stderr: Option<WorkerPipe>,
    #[cfg(windows)]
    job: JobHandle,
    #[cfg(target_os = "macos")]
    child: tokio::process::Child,
    #[cfg(target_os = "macos")]
    group: i32,
    #[cfg(target_os = "macos")]
    stdin: Option<tokio::process::ChildStdin>,
    #[cfg(target_os = "macos")]
    stdout: Option<tokio::process::ChildStdout>,
    #[cfg(target_os = "macos")]
    stderr: Option<tokio::process::ChildStderr>,
}

impl std::fmt::Debug for WorkerProcess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerProcess")
            .finish_non_exhaustive()
    }
}

impl WorkerProcess {
    /// Windows launch is server-owned: the caller must provide the durable
    /// owner retained by the Startup registry. There is intentionally no
    /// Windows local-owner convenience path.
    #[cfg(not(any(windows, target_os = "macos")))]
    pub async fn launch(executable: impl Into<PathBuf>) -> Result<Self, WorkerProcessError> {
        let executable = executable.into();
        validate_executable(&executable)?;
        Err(WorkerProcessError::UnsupportedPlatform)
    }

    #[cfg(target_os = "macos")]
    pub async fn launch(executable: impl Into<PathBuf>) -> Result<Self, WorkerProcessError> {
        let executable = executable.into();
        validate_executable(&executable)?;
        launch_macos(&executable, None)
    }

    #[cfg(target_os = "macos")]
    pub(crate) async fn launch_with_owner_until_in_state_root(
        executable: impl Into<PathBuf>,
        _owner: Arc<LaunchCleanupOwner>,
        state_root: PathBuf,
        deadline: Instant,
    ) -> Result<Self, WorkerProcessError> {
        if Instant::now() >= deadline {
            return Err(WorkerProcessError::Setup);
        }
        let executable = executable.into();
        validate_executable(&executable)?;
        launch_macos(&executable, Some(&state_root))
    }

    #[cfg(windows)]
    pub async fn launch_with_owner(
        executable: impl Into<PathBuf>,
        owner: Arc<LaunchCleanupOwner>,
    ) -> Result<Self, WorkerProcessError> {
        Self::launch_with_owner_until(executable, owner, Instant::now() + Duration::from_secs(10))
            .await
    }

    #[cfg(windows)]
    pub async fn launch_with_owner_until(
        executable: impl Into<PathBuf>,
        owner: Arc<LaunchCleanupOwner>,
        deadline: Instant,
    ) -> Result<Self, WorkerProcessError> {
        let executable = executable.into();
        let _executable_guard = hold_executable(&executable)?;
        validate_executable(&executable)?;
        launch_suspended(&executable, owner, deadline).await
    }

    #[cfg(windows)]
    pub(crate) async fn launch_with_owner_until_in_state_root(
        executable: impl Into<PathBuf>,
        owner: Arc<LaunchCleanupOwner>,
        state_root: PathBuf,
        deadline: Instant,
    ) -> Result<Self, WorkerProcessError> {
        let executable = executable.into();
        let _executable_guard = hold_executable(&executable)?;
        validate_executable(&executable)?;
        launch_suspended_in_state_root(&executable, owner, &state_root, deadline).await
    }

    pub fn id(&self) -> Option<u32> {
        #[cfg(windows)]
        {
            if should_fail_stage(FailureStage::PidUnavailable) {
                return None;
            }
            Some(self.process.id)
        }
        #[cfg(target_os = "macos")]
        {
            self.child.id()
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            None
        }
    }

    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn has_all_pipes(&self) -> bool {
        self.stdin.is_some() && self.stdout.is_some() && self.stderr.is_some()
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn take_all_pipes(
        &mut self,
    ) -> Option<(
        tokio::process::ChildStdin,
        tokio::process::ChildStdout,
        tokio::process::ChildStderr,
    )> {
        if !self.has_all_pipes() {
            return None;
        }
        Some((self.stdin.take()?, self.stdout.take()?, self.stderr.take()?))
    }

    #[cfg(windows)]
    pub(crate) fn take_all_pipes(&mut self) -> Option<(WorkerPipe, WorkerPipe, WorkerPipe)> {
        if !self.has_all_pipes() {
            return None;
        }
        Some((
            self.stdin.take().expect("validated stdin"),
            self.stdout.take().expect("validated stdout"),
            self.stderr.take().expect("validated stderr"),
        ))
    }

    pub async fn stop(&mut self) -> Result<(), WorkerProcessError> {
        self.stop_until(Instant::now() + Duration::from_secs(10))
            .await
    }

    #[cfg(windows)]
    pub async fn stop_until(&mut self, deadline: Instant) -> Result<(), WorkerProcessError> {
        use windows::Win32::Foundation::{WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};
        use windows::Win32::System::JobObjects::TerminateJobObject;
        use windows::Win32::System::Threading::WaitForSingleObject;
        if Instant::now() >= deadline {
            return Err(WorkerProcessError::Cleanup);
        }
        if should_fail_cleanup(FailureStage::Terminate) {
            return Err(WorkerProcessError::Cleanup);
        }
        unsafe {
            TerminateJobObject(self.job.0.raw(), 1).map_err(|_| WorkerProcessError::Setup)?;
        }
        let wait_handle = duplicate_handle(&self.process.handle)?;
        loop {
            let result = unsafe { WaitForSingleObject(wait_handle.raw(), 0) };
            if result == WAIT_OBJECT_0 {
                return Ok(());
            }
            if result == WAIT_FAILED {
                return Err(WorkerProcessError::Cleanup);
            }
            if should_fail_cleanup(FailureStage::WaitTimeout)
                || should_fail_cleanup(FailureStage::WaitFailed)
            {
                return Err(WorkerProcessError::Cleanup);
            }
            if result != WAIT_TIMEOUT || Instant::now() >= deadline {
                return Err(WorkerProcessError::Cleanup);
            }
            tokio::time::sleep(
                Duration::from_millis(1).min(deadline.saturating_duration_since(Instant::now())),
            )
            .await;
        }
    }

    #[cfg(target_os = "macos")]
    pub async fn stop_until(&mut self, deadline: Instant) -> Result<(), WorkerProcessError> {
        while self.kill_group().is_err() {
            if Instant::now() >= deadline {
                return Err(WorkerProcessError::Cleanup);
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        if Instant::now() >= deadline {
            return Err(WorkerProcessError::Cleanup);
        }
        tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            self.child.wait(),
        )
        .await
        .map_err(|_| WorkerProcessError::Cleanup)?
        .map_err(|_| WorkerProcessError::Cleanup)?;
        self.group = 0;
        Ok(())
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    pub async fn stop_until(&mut self, _deadline: Instant) -> Result<(), WorkerProcessError> {
        Ok(())
    }

    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    pub async fn test_send_line(&mut self, line: &[u8]) -> Result<(), WorkerProcessError> {
        use tokio::io::AsyncWriteExt;
        let stdin = self.stdin.as_mut().ok_or(WorkerProcessError::Setup)?;
        stdin
            .write_all(line)
            .await
            .map_err(|_| WorkerProcessError::Setup)?;
        stdin.flush().await.map_err(|_| WorkerProcessError::Setup)
    }

    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    pub async fn test_read_line(&mut self) -> Result<Vec<u8>, WorkerProcessError> {
        use tokio::io::AsyncReadExt;
        let stdout = self.stdout.as_mut().ok_or(WorkerProcessError::Setup)?;
        let mut line = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            stdout
                .read_exact(&mut byte)
                .await
                .map_err(|_| WorkerProcessError::Setup)?;
            if byte[0] == b'\n' {
                return Ok(line);
            }
            line.push(byte[0]);
        }
    }

    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    pub fn test_job_snapshot(&self) -> Result<test_support::JobSnapshot, WorkerProcessError> {
        test_support::query_job(&self.job)
    }
}
