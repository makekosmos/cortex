//! Engine-owned process primitive for first-party package workers.

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
#[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
use std::sync::OnceLock;
#[cfg(windows)]
use std::sync::{Arc, Mutex};

#[cfg_attr(not(windows), allow(dead_code))]
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
    #[cfg(not(windows))]
    pub async fn launch(executable: impl Into<PathBuf>) -> Result<Self, WorkerProcessError> {
        let executable = executable.into();
        validate_executable(&executable)?;
        Err(WorkerProcessError::UnsupportedPlatform)
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

    pub fn id(&self) -> Option<u32> {
        #[cfg(windows)]
        {
            if should_fail_stage(FailureStage::PidUnavailable) {
                return None;
            }
            Some(self.process.id)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    #[cfg(windows)]
    pub(crate) fn take_stdin(&mut self) -> Option<WorkerPipe> {
        self.stdin.take()
    }
    #[cfg(windows)]
    pub(crate) fn take_stdout(&mut self) -> Option<WorkerPipe> {
        self.stdout.take()
    }
    #[cfg(windows)]
    pub(crate) fn take_stderr(&mut self) -> Option<WorkerPipe> {
        self.stderr.take()
    }
    #[cfg(windows)]
    pub(crate) fn has_all_pipes(&self) -> bool {
        self.stdin.is_some() && self.stdout.is_some() && self.stderr.is_some()
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

    #[cfg(not(windows))]
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

#[cfg(windows)]
fn hold_executable(path: &Path) -> Result<fs::File, WorkerProcessError> {
    use std::os::windows::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .map_err(|_| WorkerProcessError::InvalidExecutable)
}

fn validate_executable(path: &Path) -> Result<(), WorkerProcessError> {
    if !path.is_absolute()
        || !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        || !path.is_file()
    {
        return Err(WorkerProcessError::InvalidExecutable);
    }
    if is_package_entrypoint(path) && PackageStore::verify_immutable_entrypoint_path(path).is_err()
    {
        return Err(WorkerProcessError::InvalidExecutable);
    }
    if !is_windows_pe(path) {
        return Err(WorkerProcessError::InvalidExecutable);
    }
    Ok(())
}

fn is_package_entrypoint(path: &Path) -> bool {
    path.ancestors().any(|candidate| {
        candidate
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_hash)
            && candidate
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .is_some_and(|name| name == "unpacked")
    })
}

fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_windows_pe(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() < 64 {
        return false;
    }
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut dos = [0u8; 64];
    if file.read_exact(&mut dos).is_err() || &dos[..2] != b"MZ" {
        return false;
    }
    let pe_offset = u32::from_le_bytes([dos[0x3c], dos[0x3d], dos[0x3e], dos[0x3f]]) as u64;
    if pe_offset.checked_add(4).is_none() || pe_offset + 4 > metadata.len() {
        return false;
    }
    file.seek(SeekFrom::Start(pe_offset)).is_ok()
        && file.read_exact(&mut dos[..4]).is_ok()
        && &dos[..4] == b"PE\0\0"
}

#[cfg(windows)]
const CREATE_SUSPENDED: u32 = 0x0000_0004;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const CREATE_UNICODE_ENVIRONMENT: u32 = 0x0000_0400;
#[cfg(windows)]
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;

#[cfg(windows)]
struct ProcessHandle {
    handle: OwnedHandle,
    id: u32,
}
#[cfg(windows)]
struct OwnedHandle(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for OwnedHandle {}
#[cfg(windows)]
impl OwnedHandle {
    fn raw(&self) -> windows::Win32::Foundation::HANDLE {
        self.0
    }
}
#[cfg(windows)]
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(windows)]
unsafe impl Send for ProcessHandle {}

#[cfg(windows)]
struct JobHandle(OwnedHandle);
#[cfg(windows)]
unsafe impl Send for JobHandle {}
#[cfg(windows)]
impl JobHandle {
    fn raw(&self) -> windows::Win32::Foundation::HANDLE {
        self.0.raw()
    }
}

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

#[cfg(windows)]
fn create_process_suspended(
    executable: &Path,
    child_raw: &[windows::Win32::Foundation::HANDLE],
    owner: &LaunchCleanupOwner,
) -> Result<(), WorkerProcessError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        UpdateProcThreadAttribute, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
        STARTF_USESTDHANDLES, STARTUPINFOEXW,
    };
    let mut attribute_size = 0usize;
    unsafe {
        let _ = InitializeProcThreadAttributeList(
            windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(std::ptr::null_mut()),
            1,
            0,
            &mut attribute_size,
        );
    }
    if attribute_size == 0 {
        return Err(WorkerProcessError::Setup);
    }
    let mut attribute_storage = vec![0u8; attribute_size];
    let attribute_list = attribute_storage.as_mut_ptr() as *mut std::ffi::c_void;
    unsafe {
        InitializeProcThreadAttributeList(
            windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(attribute_list),
            1,
            0,
            &mut attribute_size,
        )
        .map_err(|_| WorkerProcessError::Setup)?;
    }
    struct AttributeGuard(*mut std::ffi::c_void);
    impl Drop for AttributeGuard {
        fn drop(&mut self) {
            unsafe {
                DeleteProcThreadAttributeList(
                    windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(self.0),
                );
            }
        }
    }
    let _attribute_guard = AttributeGuard(attribute_list);
    if should_fail_stage(FailureStage::Attribute) {
        return Err(WorkerProcessError::Setup);
    }
    if child_raw.len() != 3
        || child_raw.iter().any(|handle| handle.is_invalid())
        || child_raw
            .iter()
            .enumerate()
            .any(|(index, handle)| child_raw[index + 1..].contains(handle))
    {
        return Err(WorkerProcessError::Setup);
    }
    let child_bytes =
        handle_allowlist_byte_len(child_raw.len()).ok_or(WorkerProcessError::Setup)?;
    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    capture_handle_allowlist(child_raw.len(), child_bytes);
    unsafe {
        UpdateProcThreadAttribute(
            windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(attribute_list),
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(child_raw.as_ptr() as *const _),
            child_bytes,
            None,
            None,
        )
        .map_err(|_| WorkerProcessError::Setup)?;
    }

    let mut si = STARTUPINFOEXW::default();
    si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    si.StartupInfo.hStdInput = child_raw[0];
    si.StartupInfo.hStdOutput = child_raw[1];
    si.StartupInfo.hStdError = child_raw[2];
    si.lpAttributeList =
        windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(attribute_list);
    let mut pi = PROCESS_INFORMATION::default();
    let mut environment = std::collections::BTreeMap::<String, (String, std::ffi::OsString)>::new();
    #[cfg(any(test, feature = "package-worker-fixture"))]
    let environment_names = [
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        // Fixture markers are test-only process inputs. Production launches
        // never copy these names from the parent environment.
        "KOSMOS_FIXTURE_ENTRY_MARKER",
        "KOSMOS_FIXTURE_BOOTSTRAP_MARKER",
    ];
    #[cfg(not(any(test, feature = "package-worker-fixture")))]
    let environment_names = ["SystemRoot", "WINDIR", "TEMP", "TMP"];
    for name in environment_names {
        if let Some(value) = std::env::var_os(name) {
            environment
                .entry(name.to_ascii_lowercase())
                .or_insert_with(|| (name.into(), value));
        }
    }
    let mut env = Vec::<u16>::new();
    for (_, (name, value)) in environment {
        env.extend(std::ffi::OsStr::new(&name).encode_wide());
        env.push('=' as u16);
        env.extend(value.encode_wide());
        env.push(0);
    }
    env.push(0);
    let mut app = executable.as_os_str().encode_wide().collect::<Vec<_>>();
    app.push(0);
    let mut current_dir = executable
        .parent()
        .unwrap_or_else(|| Path::new("C:\\Windows\\System32"))
        .as_os_str()
        .encode_wide()
        .collect::<Vec<_>>();
    current_dir.push(0);
    let mut owner_state = owner.lock();
    if owner_state.state != OwnerState::Claimed
        || owner_state.process.is_some()
        || owner_state.thread.is_some()
        || owner_state.job.is_some()
    {
        return Err(WorkerProcessError::Setup);
    }
    let created = !should_fail_stage(FailureStage::CreateProcess)
        && unsafe {
            CreateProcessW(
                PCWSTR(app.as_ptr()),
                windows::core::PWSTR::null(),
                None,
                None,
                true,
                windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(
                    CREATE_SUSPENDED
                        | CREATE_NO_WINDOW
                        | CREATE_UNICODE_ENVIRONMENT
                        | EXTENDED_STARTUPINFO_PRESENT,
                ),
                Some(env.as_ptr() as *const _),
                PCWSTR(current_dir.as_ptr()),
                &si as *const STARTUPINFOEXW
                    as *const windows::Win32::System::Threading::STARTUPINFOW,
                &mut pi,
            )
            .is_ok()
        };
    if !created {
        owner_state.state = OwnerState::Clean;
        return Err(WorkerProcessError::Setup);
    }
    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    {
        CREATED_PROCESS_COUNT.fetch_add(1, Ordering::SeqCst);
        capture_process_handle(pi.hProcess);
    }
    owner_state.process = Some(ProcessHandle {
        handle: OwnedHandle(pi.hProcess),
        id: pi.dwProcessId,
    });
    owner_state.thread = Some(OwnedHandle(pi.hThread));
    owner_state.state = OwnerState::Created;
    Ok(())
}

#[cfg(any(test, windows))]
fn checked_handle_allowlist_byte_len(handle_count: usize, handle_size: usize) -> Option<usize> {
    (handle_count != 0)
        .then(|| handle_count.checked_mul(handle_size))
        .flatten()
}

#[cfg(windows)]
fn handle_allowlist_byte_len(handle_count: usize) -> Option<usize> {
    checked_handle_allowlist_byte_len(
        handle_count,
        size_of::<windows::Win32::Foundation::HANDLE>(),
    )
}

#[cfg(windows)]
async fn launch_suspended(
    executable: &Path,
    owner: Arc<LaunchCleanupOwner>,
    deadline: Instant,
) -> Result<WorkerProcess, WorkerProcessError> {
    owner.claim()?;
    let pipe_name = |suffix: &str| {
        format!(
            r"\\.\pipe\kosmos-worker-{}-{}-{}",
            std::process::id(),
            PIPE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            suffix
        )
    };

    #[cfg(windows)]
    fn create_named_pipe(
        name: &str,
        parent_reads: bool,
    ) -> Result<(WorkerPipe, OwnedHandle), WorkerProcessError> {
        use std::os::windows::io::AsRawHandle;
        use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Security::SECURITY_ATTRIBUTES;
        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: true.into(),
        };
        let mut options = ServerOptions::new();
        options
            .access_inbound(parent_reads)
            .access_outbound(!parent_reads);
        let server = options
            .create(name)
            .map_err(|_| WorkerProcessError::Setup)?;
        let mut client_options = ClientOptions::new();
        let (child_reads, child_writes) = child_pipe_access(parent_reads);
        client_options.read(child_reads).write(child_writes);
        let client = unsafe {
            client_options
                .open_with_security_attributes_raw(
                    name,
                    &mut sa as *mut SECURITY_ATTRIBUTES as *mut std::ffi::c_void,
                )
                .map_err(|_| WorkerProcessError::Setup)?
        };
        let raw = client.as_raw_handle();
        std::mem::forget(client);
        Ok((server, OwnedHandle(HANDLE(raw))))
    }

    // Parent owns the Tokio server ends; child owns the inheritable client ends.
    // The client handles are kept in RAII until CreateProcess has consumed them.
    if should_fail_stage(FailureStage::Pipe) {
        owner.abandon_claim();
        return Err(WorkerProcessError::Setup);
    }
    let (stdin, child_stdin) = match create_named_pipe(&pipe_name("stdin"), false) {
        Ok(value) => value,
        Err(error) => {
            owner.abandon_claim();
            return Err(error);
        }
    };
    let (stdout, child_stdout) = match create_named_pipe(&pipe_name("stdout"), true) {
        Ok(value) => value,
        Err(error) => {
            owner.abandon_claim();
            return Err(error);
        }
    };
    let (stderr, child_stderr) = match create_named_pipe(&pipe_name("stderr"), true) {
        Ok(value) => value,
        Err(error) => {
            owner.abandon_claim();
            return Err(error);
        }
    };
    let child_handles = [child_stdin, child_stdout, child_stderr];
    {
        let child_raw = child_handles
            .iter()
            .map(OwnedHandle::raw)
            .collect::<Vec<_>>();
        match create_process_suspended(executable, &child_raw, &owner) {
            Ok(()) => (),
            Err(error) => {
                owner.abandon_claim();
                return Err(error);
            }
        }
    }
    // This guard remains the sole owner of process/thread/job authority through
    // every await point.
    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    if let Some(barrier) = take_pipe_connect_barrier() {
        let _ = barrier.ready.send(());
        let _ = barrier.release.await;
    }
    // Establish all IOCP connections while the primary thread is still suspended.
    if stdin.connect().await.is_err()
        || stdout.connect().await.is_err()
        || stderr.connect().await.is_err()
    {
        return Err(cleanup_result_async(WorkerProcessError::Setup, owner.clone(), deadline).await);
    }
    if let Err(error) = configure_and_install_job(&owner) {
        return Err(cleanup_result_async(error, owner.clone(), deadline).await);
    }
    if should_fail_stage(FailureStage::PreResume) {
        return Err(cleanup_result_async(WorkerProcessError::Setup, owner.clone(), deadline).await);
    }
    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    if let Some(barrier) = take_resume_barrier() {
        let _ = barrier.ready.send(());
        let _ = barrier.release.await;
    }
    if let Err(error) = owner.resume() {
        return Err(cleanup_result_async(error, owner.clone(), deadline).await);
    }
    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    RESUME_COUNT.fetch_add(1, Ordering::SeqCst);
    if should_fail_stage(FailureStage::Wrapper) {
        return Err(cleanup_result_async(WorkerProcessError::Setup, owner.clone(), deadline).await);
    }
    // The child client handles are no longer needed by the parent after resume.
    drop(child_handles);
    match owner.commit(stdin, stdout, stderr) {
        Ok(process) => Ok(process),
        Err(error) => Err(cleanup_result_async(error, owner, deadline).await),
    }
}

#[cfg(windows)]
static PIPE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[cfg(windows)]
fn cleanup_result(
    setup: WorkerProcessError,
    cleanup: Result<(), WorkerProcessError>,
) -> WorkerProcessError {
    cleanup
        .map(|_| setup)
        .unwrap_or(WorkerProcessError::Cleanup)
}

#[cfg(windows)]
async fn cleanup_result_async(
    setup: WorkerProcessError,
    owner: Arc<LaunchCleanupOwner>,
    deadline: Instant,
) -> WorkerProcessError {
    cleanup_result(setup, owner.cleanup_until(deadline).await)
}

#[cfg(windows)]
fn configure_and_install_job(owner: &LaunchCleanupOwner) -> Result<(), WorkerProcessError> {
    use std::mem::size_of;
    use windows::Win32::System::JobObjects::*;
    let mut state = owner.lock();
    if state.state != OwnerState::Created || state.process.is_none() || state.job.is_some() {
        return Err(WorkerProcessError::Setup);
    }
    let process = state
        .process
        .as_ref()
        .ok_or(WorkerProcessError::Setup)?
        .handle
        .raw();
    if should_fail_stage(FailureStage::JobCreate) {
        return Err(WorkerProcessError::Setup);
    }
    let job = OwnedHandle(unsafe {
        CreateJobObjectW(None, None).map_err(|_| WorkerProcessError::Setup)?
    });
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_JOB_MEMORY;
    limits.BasicLimitInformation.ActiveProcessLimit = 1;
    limits.JobMemoryLimit = 512 * 1024 * 1024;
    if should_fail_stage(FailureStage::Limits)
        || unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut _) as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .is_err()
        }
    {
        return Err(WorkerProcessError::Setup);
    }
    let mut cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
        ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
        Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 { CpuRate: 2500 },
    };
    if should_fail_stage(FailureStage::Cpu)
        || unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectCpuRateControlInformation,
                (&mut cpu as *mut _) as *const _,
                size_of::<JOBOBJECT_CPU_RATE_CONTROL_INFORMATION>() as u32,
            )
            .is_err()
        }
    {
        return Err(WorkerProcessError::Setup);
    }
    if should_fail_stage(FailureStage::Assign)
        || unsafe { AssignProcessToJobObject(job.raw(), process).is_err() }
    {
        return Err(WorkerProcessError::Setup);
    }
    state.job = Some(JobHandle(job));
    state.state = OwnerState::JobInstalled;
    Ok(())
}

#[cfg(windows)]
fn duplicate_handle(handle: &OwnedHandle) -> Result<OwnedHandle, WorkerProcessError> {
    use windows::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS};
    use windows::Win32::System::Threading::GetCurrentProcess;
    let mut copy = windows::Win32::Foundation::HANDLE::default();
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            handle.raw(),
            GetCurrentProcess(),
            &mut copy,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
        .map_err(|_| WorkerProcessError::Setup)?;
    }
    Ok(OwnedHandle(copy))
}

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
                job.raw(),
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
                job.raw(),
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
            let result = unsafe {
                windows::Win32::System::Threading::WaitForSingleObject(self.handle, 10_000)
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
        // Initialize Tokio's process-wide named-pipe/IOCP state before
        // measuring worker ownership, including the first successful
        // launch path that can create additional lazy handles.
        reset();
        fail_next(FailureStage::Assign);
        assert!(matches!(
            WorkerProcess::launch_with_owner(
                executable.as_ref().to_path_buf(),
                LaunchCleanupOwner::new()
            )
            .await,
            Err(WorkerProcessError::Setup)
        ));
        reset();

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

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use crate::package_manifest::PackageKind;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::mem::size_of;
    use zip::{write::FileOptions, ZipWriter};

    fn minimal_pe() -> Vec<u8> {
        let mut bytes = vec![0; 68];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&(64u32).to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes
    }

    #[test]
    fn launch_state_requires_containment_before_resume() {
        assert_eq!(advance_launch_state(LaunchState::CreatedSuspended), None);
        assert_eq!(
            advance_launch_state(LaunchState::JobConfigured),
            Some(LaunchState::Assigned)
        );
        assert_eq!(
            advance_launch_state(LaunchState::Assigned),
            Some(LaunchState::Resumed)
        );
        assert_eq!(advance_launch_state(LaunchState::Resumed), None);
    }

    #[test]
    fn handle_allowlist_byte_length_requires_nonzero_count() {
        let handle_size = size_of::<usize>();
        assert_eq!(
            checked_handle_allowlist_byte_len(3, handle_size),
            Some(3 * handle_size)
        );
        assert_eq!(checked_handle_allowlist_byte_len(0, handle_size), None);
    }

    #[test]
    fn handle_allowlist_byte_length_rejects_overflow() {
        assert_eq!(
            checked_handle_allowlist_byte_len(usize::MAX, size_of::<usize>()),
            None
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_handle_allowlist_byte_length_is_three_handles() {
        assert_eq!(
            handle_allowlist_byte_len(3),
            Some(3 * size_of::<windows::Win32::Foundation::HANDLE>())
        );
    }

    #[cfg(windows)]
    #[test]
    fn named_pipe_direction_matrix_is_complementary() {
        assert_eq!(child_pipe_access(false), (true, false));
        assert_eq!(child_pipe_access(true), (false, true));
    }

    #[cfg(windows)]
    #[test]
    fn startup_attribute_list_requires_extended_startup_flag() {
        let flags = CREATE_SUSPENDED
            | CREATE_NO_WINDOW
            | CREATE_UNICODE_ENVIRONMENT
            | EXTENDED_STARTUPINFO_PRESENT;
        assert_ne!(flags & EXTENDED_STARTUPINFO_PRESENT, 0);
        assert_eq!(
            flags & EXTENDED_STARTUPINFO_PRESENT,
            EXTENDED_STARTUPINFO_PRESENT
        );
    }

    #[test]
    fn rejects_relative_non_exe_and_missing_paths() {
        assert!(matches!(
            validate_executable(Path::new("worker.exe")),
            Err(WorkerProcessError::InvalidExecutable)
        ));
        assert!(matches!(
            validate_executable(Path::new("C:\\missing-worker.exe")),
            Err(WorkerProcessError::InvalidExecutable)
        ));
    }

    #[test]
    fn rejects_non_pe_even_with_exe_suffix() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("worker.exe");
        std::fs::write(&path, b"not a PE").expect("temporary executable");
        assert!(matches!(
            validate_executable(&path),
            Err(WorkerProcessError::InvalidExecutable)
        ));
    }

    #[test]
    fn rejects_tampered_package_entrypoint_before_spawn() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let archive = directory.path().join("worker.kspkg");
        let manifest = crate::package_manifest::ManifestV2 {
            schema_version: 2,
            id: "com.kosmos.worker".into(),
            name: "Worker".into(),
            version: "1.0.0".into(),
            kind: PackageKind::Source,
            engine_api: ">=1.0.0".into(),
            entrypoint: "worker.exe".into(),
            icon: None,
            publisher: "kosmos".into(),
            permissions: vec![],
            targets: vec![crate::package_manifest::ManifestTarget {
                runtime: crate::package_manifest::TargetRuntime::Worker,
                os: vec![crate::package_manifest::TargetOs::Windows],
                arch: None,
            }],
            data: crate::package_manifest::ManifestData {
                access: vec![],
                defines: vec![],
                mappings: vec![],
            },
        };
        let pe = minimal_pe();
        let file = std::fs::File::create(&archive).expect("archive");
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default();
        zip.start_file("manifest.json", options)
            .expect("manifest entry");
        zip.write_all(&serde_json::to_vec(&manifest).expect("manifest json"))
            .expect("manifest bytes");
        zip.start_file("worker.exe", options).expect("worker entry");
        zip.write_all(&pe).expect("worker bytes");
        zip.finish().expect("archive finish");

        let bytes = std::fs::read(&archive).expect("archive bytes");
        let mut digest = Sha256::new();
        digest.update(&bytes);
        let hash = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let store =
            crate::package_store::PackageStore::new(directory.path().join("store")).expect("store");
        let installed = store
            .install_versioned(
                &archive,
                bytes.len() as u64,
                &hash,
                &crate::package_manifest::VersionedManifest::V2(manifest),
                1,
            )
            .expect("install");
        let entrypoint = store
            .immutable_entrypoint(&installed)
            .expect("immutable entrypoint");
        std::fs::write(&entrypoint, b"tampered").expect("tamper");
        assert!(matches!(
            validate_executable(&entrypoint),
            Err(WorkerProcessError::InvalidExecutable)
        ));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn reused_owner_is_rejected_before_create_process() {
        let _test_lock = windows_worker_test_lock();
        let owner = LaunchCleanupOwner::new();
        owner.claim().expect("initial claim");
        test_support::capture_next_process();
        let executable = std::env::var_os("COMSPEC").expect("COMSPEC");
        let result = WorkerProcess::launch_with_owner(executable, owner.clone()).await;
        assert!(matches!(result, Err(WorkerProcessError::Setup)));
        assert!(test_support::take_captured_process().is_none());
        assert!(owner.is_armed());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn concurrent_same_owner_launches_reject_before_second_create_process() {
        let _test_lock = windows_worker_test_lock();
        let owner = LaunchCleanupOwner::new();
        let executable = std::env::var_os("COMSPEC").expect("COMSPEC");
        let mut barrier = test_support::pause_next_before_resume();
        test_support::reset();
        let first = tokio::spawn(WorkerProcess::launch_with_owner(
            executable.clone(),
            owner.clone(),
        ));
        barrier.suspended_ready().await;
        let second = WorkerProcess::launch_with_owner(executable, owner.clone()).await;
        assert!(matches!(second, Err(WorkerProcessError::Setup)));
        assert_eq!(test_support::created_process_count(), 1);
        barrier.release();
        let mut first = first
            .await
            .expect("first launch task")
            .expect("first launch");
        first.stop().await.expect("stop first launch");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn committed_owner_reuse_is_rejected_without_create_process() {
        let _test_lock = windows_worker_test_lock();
        let executable = std::env::var_os("COMSPEC").expect("COMSPEC");
        let owner = LaunchCleanupOwner::new();
        test_support::reset();
        let mut process = WorkerProcess::launch_with_owner(executable.clone(), owner.clone())
            .await
            .expect("launch");
        let created = test_support::created_process_count();
        let result = WorkerProcess::launch_with_owner(executable, owner).await;
        assert!(matches!(result, Err(WorkerProcessError::Setup)));
        assert_eq!(test_support::created_process_count(), created);
        process.stop().await.expect("stop");
    }

    #[cfg(windows)]
    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_owner_cleanup_restores_armed_state_and_retries_reap() {
        let _test_lock = windows_worker_test_lock();
        let executable = std::env::var_os("COMSPEC").expect("COMSPEC");
        test_support::reset();
        let baseline = test_support::handle_baseline(executable.clone()).await;
        let owner = LaunchCleanupOwner::new();
        test_support::capture_next_process();
        let mut gate = test_support::pause_next_cleanup_wait();
        test_support::fail_next(FailureStage::PreResume);
        let launch = tokio::spawn(WorkerProcess::launch_with_owner(
            executable.clone(),
            owner.clone(),
        ));
        tokio::time::timeout(Duration::from_secs(5), gate.ready())
            .await
            .expect("cleanup wait gate");
        assert_eq!(owner.lock().state, OwnerState::Cleaning);
        assert!(owner.lock().process.is_some());
        assert!(owner.lock().job.is_some());
        assert!(matches!(
            owner
                .cleanup_until(Instant::now() + Duration::from_secs(10))
                .await,
            Err(WorkerProcessError::Cleanup)
        ));
        assert!(matches!(owner.claim(), Err(WorkerProcessError::Setup)));
        assert!(matches!(owner.resume(), Err(WorkerProcessError::Setup)));
        assert!(matches!(
            WorkerProcess::launch_with_owner(executable, owner.clone()).await,
            Err(WorkerProcessError::Setup)
        ));
        launch.abort();
        assert!(matches!(launch.await, Err(error) if error.is_cancelled()));
        assert_eq!(owner.lock().state, OwnerState::JobInstalled);
        assert!(owner.lock().terminated);
        assert!(owner.lock().process.is_some());
        assert!(owner.lock().job.is_some());
        gate.release();
        owner
            .cleanup_until(Instant::now() + Duration::from_secs(10))
            .await
            .expect("retry cleanup");
        assert!(test_support::take_captured_process()
            .expect("captured process")
            .wait_object_0());
        assert!(test_support::take_captured_process().is_none());
        assert!(test_support::process_handle_count() <= baseline + 2);
    }

    #[tokio::test]
    async fn non_windows_fails_closed() {
        #[cfg(not(windows))]
        {
            let directory = tempfile::tempdir().expect("temporary directory");
            let path = directory.path().join("worker.exe");
            std::fs::write(&path, minimal_pe()).expect("temporary executable");
            assert!(matches!(
                WorkerProcess::launch(path).await,
                Err(WorkerProcessError::UnsupportedPlatform)
            ));
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stopping_worker_does_not_kill_unrelated_process() {
        let _test_lock = windows_worker_test_lock();
        use std::process::Command as StdCommand;
        let comspec = std::env::var_os("COMSPEC").expect("COMSPEC");
        let mut unrelated = StdCommand::new(&comspec).spawn().expect("unrelated child");
        let mut worker = WorkerProcess::launch_with_owner(&comspec, LaunchCleanupOwner::new())
            .await
            .expect("worker child");
        worker.stop().await.expect("stop worker");
        assert!(unrelated.try_wait().expect("poll unrelated").is_none());
        let _ = unrelated.kill();
        let _ = unrelated.wait();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn launch_reaps_child_for_each_job_setup_failure() {
        let _test_lock = windows_worker_test_lock();
        use windows::Win32::Foundation::WAIT_OBJECT_0;
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::System::Threading::WaitForSingleObject;

        let executable = std::env::var_os("COMSPEC").expect("COMSPEC");
        for stage in [
            FailureStage::JobCreate,
            FailureStage::Limits,
            FailureStage::Cpu,
            FailureStage::Assign,
            FailureStage::PreResume,
            FailureStage::Resume,
        ] {
            LAST_SPAWNED_HANDLE.store(0, Ordering::SeqCst);
            JOB_SETUP_FAILURE.store(stage as u8, Ordering::SeqCst);
            CAPTURE_CHILD_HANDLE.store(true, Ordering::SeqCst);
            let result = WorkerProcess::launch_with_owner(
                PathBuf::from(executable.clone()),
                LaunchCleanupOwner::new(),
            )
            .await;
            CAPTURE_CHILD_HANDLE.store(false, Ordering::SeqCst);
            assert!(matches!(result, Err(WorkerProcessError::Setup)));
            assert_eq!(JOB_SETUP_FAILURE.load(Ordering::SeqCst), 0);
            let raw_handle = LAST_SPAWNED_HANDLE.swap(0, Ordering::SeqCst);
            assert_ne!(raw_handle, 0, "child was not spawned for {stage:?}");
            let handle = HANDLE(raw_handle as _);
            assert_eq!(unsafe { WaitForSingleObject(handle, 2_000) }, WAIT_OBJECT_0);
            unsafe {
                CloseHandle(handle).expect("close retained child handle");
            }
        }
    }
}
