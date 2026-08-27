//! Process-tree ownership for app-server children.
//!
//! `ProcessTree::spawn` must be used instead of spawning the command directly:
//! it creates a private Unix process group or assigns the child to a Windows
//! Job Object before returning the child to its caller.

use std::io;
#[cfg(windows)]
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;

use tokio::process::{Child, Command};

/// An app-server child together with the OS primitive that contains its tree.
pub struct ProcessTree {
    child: Child,
    #[cfg(unix)]
    process_group: i32,
    #[cfg(windows)]
    job: JobHandle,
}

impl ProcessTree {
    /// Spawn a command with process-tree containment installed.
    pub async fn spawn(command: &mut Command) -> io::Result<Self> {
        configure_command(command)?;
        let mut child = command.spawn()?;
        let pid = match child.id() {
            Some(pid) => pid,
            None => {
                reap_failed_spawn(&mut child).await;
                return Err(io::Error::other("spawned process has no pid"));
            }
        };

        #[cfg(unix)]
        {
            let process_group = match i32::try_from(pid) {
                Ok(process_group) => process_group,
                Err(_) => {
                    reap_failed_spawn(&mut child).await;
                    return Err(io::Error::other("spawned process pid is out of range"));
                }
            };
            if process_group <= 0 {
                reap_failed_spawn(&mut child).await;
                return Err(io::Error::other("spawned process has an invalid pid"));
            }
            return Ok(Self {
                child,
                process_group,
            });
        }

        #[cfg(windows)]
        {
            use tokio::io::AsyncWriteExt;

            let job = match JobHandle::for_process(pid) {
                Ok(job) => job,
                Err(error) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    return Err(error);
                }
            };
            if let Err(error) = async {
                let stdin = child
                    .stdin
                    .as_mut()
                    .ok_or_else(|| io::Error::other("gated process has no stdin"))?;
                stdin.write_all(b"\n").await?;
                stdin.flush().await
            }
            .await
            {
                drop(job);
                let _ = child.wait().await;
                return Err(error);
            }
            Ok(Self { child, job })
        }
    }

    /// Access the child for reading or taking its stdio handles.
    pub fn child(&self) -> &Child {
        &self.child
    }

    /// Access the child for reading or taking its stdio handles.
    pub fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    /// Kill the whole tree and reap the root process, waiting no longer than
    /// `timeout`. Repeating this after a timeout or successful wait is safe.
    pub async fn terminate_and_wait(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
        if let Some(status) = self.child.try_wait()? {
            self.terminate()?;
            return Ok(status);
        }

        if let Err(error) = self.terminate() {
            if self.child.try_wait()?.is_none() {
                return Err(error);
            }
        }
        tokio::time::timeout(timeout, self.child.wait())
            .await
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "process-tree termination timed out",
                )
            })?
    }

    fn terminate(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        {
            terminate_process_group(self.process_group)
        }

        #[cfg(windows)]
        {
            use windows::Win32::System::JobObjects::TerminateJobObject;

            unsafe { TerminateJobObject(self.job.raw(), 1) }
                .map_err(|error| io::Error::other(error.to_string()))
        }
    }
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = terminate_process_group(self.process_group);
        // Closing the Windows job handle applies KILL_ON_JOB_CLOSE.
    }
}

async fn reap_failed_spawn(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn configure_command(command: &mut Command) -> io::Result<()> {
    #[cfg(windows)]
    gate_windows_command(command)?;

    command.kill_on_drop(true);

    #[cfg(unix)]
    {
        command.process_group(0);
    }
    Ok(())
}

#[cfg(windows)]
fn gate_windows_command(command: &mut Command) -> io::Result<()> {
    use base64::Engine;

    let source = command.as_std();
    let spec = serde_json::json!({
        "program": source.get_program().to_string_lossy(),
        "args": source.get_args().map(|arg| arg.to_string_lossy()).collect::<Vec<_>>(),
    });
    let encoded = base64::engine::general_purpose::STANDARD.encode(spec.to_string());
    let script = format!(
        "$gateStream=[Console]::OpenStandardInput();$null=$gateStream.ReadByte();\
         $spec=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}'))|ConvertFrom-Json;\
         & $spec.program @($spec.args);exit $LASTEXITCODE"
    );
    let current_dir = source.get_current_dir().map(PathBuf::from);
    let environment = source
        .get_envs()
        .map(|(name, value)| (name.to_os_string(), value.map(|value| value.to_os_string())))
        .collect::<Vec<_>>();

    let mut gated = Command::new("powershell.exe");
    gated
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    if let Some(current_dir) = current_dir {
        gated.current_dir(current_dir);
    }
    for (name, value) in environment {
        if let Some(value) = value {
            gated.env(name, value);
        } else {
            gated.env_remove(name);
        }
    }
    *command = gated;
    Ok(())
}

#[cfg(unix)]
fn terminate_process_group(process_group: i32) -> io::Result<()> {
    let result = unsafe { libc::kill(-process_group, libc::SIGKILL) };
    if result == 0 {
        return Ok(());
    }

    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(windows)]
struct JobHandle(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for JobHandle {}

#[cfg(windows)]
impl JobHandle {
    fn raw(&self) -> windows::Win32::Foundation::HANDLE {
        self.0
    }

    fn for_process(pid: u32) -> io::Result<Self> {
        use std::mem::size_of;
        use windows::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };

        let job = unsafe { CreateJobObjectW(None, None) }
            .map_err(|error| io::Error::other(error.to_string()))?;
        let job = Self(job);
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

        let process = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid) }
            .map_err(|error| io::Error::other(error.to_string()))?;
        if process.is_invalid() {
            return Err(io::Error::last_os_error());
        }
        let assigned = unsafe { AssignProcessToJobObject(job.raw(), process) };
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(process);
        }
        assigned.map_err(|error| io::Error::other(error.to_string()))?;
        Ok(job)
    }
}

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
