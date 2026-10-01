//! Windows halves of `ProcessTree`: the KILL_ON_JOB_CLOSE job the child is
//! assigned to and the thread resume that releases a suspended child into it.

use std::io;
use std::mem::size_of;

use windows::Win32::Foundation::{CloseHandle, HANDLE};

/// `ProcessTree::spawn` ORs this into the caller's creation flags so the child
/// is created with its primary thread suspended and can join the job before
/// any of its code runs.
pub(super) const CREATE_SUSPENDED: u32 = 0x0000_0004;

/// Closes a raw Win32 handle on drop. Every handle created here travels in
/// this type so `?` early returns cannot leak one.
pub(super) struct OwnedHandle(HANDLE);

unsafe impl Send for OwnedHandle {}

impl OwnedHandle {
    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// The KILL_ON_JOB_CLOSE job owning a spawned child. Dropping the handle
/// kills the whole tree, which is what `Drop for ProcessTree` relies on.
pub(crate) struct JobHandle(OwnedHandle);

unsafe impl Send for JobHandle {}

impl JobHandle {
    pub(super) fn raw(&self) -> HANDLE {
        self.0.raw()
    }

    /// Creates the job and assigns `pid` to it while the process is suspended.
    pub(crate) fn for_process(pid: u32) -> io::Result<Self> {
        use windows::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };

        let job = OwnedHandle(
            unsafe { CreateJobObjectW(None, None) }
                .map_err(|error| io::Error::other(error.to_string()))?,
        );
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut _) as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .map_err(|error| io::Error::other(error.to_string()))?;

        let raw = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid) }
            .map_err(|error| io::Error::other(error.to_string()))?;
        if raw.is_invalid() {
            return Err(io::Error::last_os_error());
        }
        let process = OwnedHandle(raw);
        unsafe { AssignProcessToJobObject(job.raw(), process.raw()) }
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(Self(job))
    }
}

/// Resumes the primary thread of a freshly CREATE_SUSPENDED process.
///
/// `std::process::Command` closes the thread handle `CreateProcess` returns,
/// so the thread is found again through a Toolhelp32 snapshot filtered on the
/// owner pid. A suspended newborn process must have exactly one thread; any
/// other count fails closed rather than resuming an ambiguous thread.
pub(crate) fn resume_primary_thread(pid: u32) -> io::Result<()> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    let snapshot = OwnedHandle(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }
            .map_err(|error| io::Error::other(error.to_string()))?,
    );
    let mut threads = Vec::new();
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    if unsafe { Thread32First(snapshot.raw(), &mut entry) }.is_ok() {
        loop {
            if entry.th32OwnerProcessID == pid {
                threads.push(entry.th32ThreadID);
            }
            if unsafe { Thread32Next(snapshot.raw(), &mut entry) }.is_err() {
                break;
            }
        }
    }
    let [thread_id] = threads.as_slice() else {
        return Err(io::Error::other(format!(
            "suspended child {pid} has {} threads, expected exactly one",
            threads.len()
        )));
    };
    let thread = OwnedHandle(
        unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, *thread_id) }
            .map_err(|error| io::Error::other(error.to_string()))?,
    );
    // ResumeThread returns the previous suspend count, (DWORD)-1 on failure.
    if unsafe { ResumeThread(thread.raw()) } == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
