//! Process-tree ownership for long-lived spawned children.
//!
//! `ProcessTree::spawn` must be used instead of spawning the command directly:
//! it creates a private Unix process group or assigns the child to a Windows
//! Job Object before returning the child to its caller. Dropping the
//! `ProcessTree` — or the death of the process holding it — kills the whole
//! tree, so a child can never outlive its owner.

use std::io;
use std::process::ExitStatus;
use std::time::Duration;

use tokio::process::{Child, Command};

#[cfg(windows)]
pub mod win32;
#[cfg(windows)]
mod windows_program;
#[cfg(all(test, windows))]
mod windows_program_tests;

/// A spawned child together with the OS primitive that contains its tree.
pub struct ProcessTree {
    child: Child,
    #[cfg(unix)]
    process_group: i32,
    #[cfg(windows)]
    job: win32::JobHandle,
}

impl ProcessTree {
    /// Spawn a command with process-tree containment installed.
    ///
    /// `creation_flags` are the Windows process-creation flags the caller wants
    /// on the child (e.g. `CREATE_NO_WINDOW`); `ProcessTree` ORs its own
    /// `CREATE_SUSPENDED` into them — `Command` offers no flag getter, so the
    /// caller's flags travel through this parameter instead of being silently
    /// overwritten. On Unix the value is ignored: a process group provides the
    /// same containment without it.
    ///
    /// A bare program name on Windows resolves only `<name>.exe`; pass the
    /// command through [`resolve_command`] first when it may be a `.cmd`/`.bat`
    /// shim (as npm installs one for `codex`).
    pub async fn spawn(command: &mut Command, creation_flags: u32) -> io::Result<Self> {
        command.kill_on_drop(true);

        #[cfg(unix)]
        {
            let _ = creation_flags;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command
                .as_std_mut()
                .creation_flags(win32::CREATE_SUSPENDED | creation_flags);
        }

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
            // The child is still suspended, so the job must be in place before
            // its first thread runs — after that point a grandchild could
            // escape the tree.
            let job = match win32::JobHandle::for_process(pid) {
                Ok(job) => job,
                Err(error) => {
                    reap_failed_spawn(&mut child).await;
                    return Err(error);
                }
            };
            if let Err(error) = win32::resume_primary_thread(pid) {
                reap_failed_spawn(&mut child).await;
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

/// Resolve `command`'s program the way its spawned environment will see it.
///
/// Windows `Command` only searches PATH for `<name>.exe`, so an npm shim like
/// `codex.cmd` would never spawn — and the PowerShell gate this replaced is
/// gone. This resolves a bare name over the command's `PATH` × `PATHEXT`
/// (explicit paths pass through) and rebuilds the command around the full
/// path; a resolved `.cmd`/`.bat` then gets std's own `cmd.exe` wrapping,
/// which is hardened against command injection (CVE-2024-24576).
///
/// The returned command may be rebuilt around the resolved path, so only
/// program, args, env and current_dir carry over: set stdio *after* this call
/// and pass creation flags to [`ProcessTree::spawn`], not `CommandExt`.
/// `raw_arg` arguments and `env_clear` are not observable on a built `Command`
/// and are not preserved.
#[cfg(windows)]
pub fn resolve_command(command: Command) -> io::Result<Command> {
    windows_program::resolve_command(command)
}

/// Non-Windows builds keep the command as given — `Command` resolves bare
/// names through `PATH` execvp-style on its own.
#[cfg(not(windows))]
pub fn resolve_command(command: Command) -> io::Result<Command> {
    Ok(command)
}

async fn reap_failed_spawn(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
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
