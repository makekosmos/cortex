#[cfg(target_os = "macos")]
pub struct LaunchCleanupOwner;

#[cfg(target_os = "macos")]
impl LaunchCleanupOwner {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
    pub fn is_armed(&self) -> bool {
        false
    }
    pub async fn cleanup_until(
        self: &Arc<Self>,
        _deadline: Instant,
    ) -> Result<(), WorkerProcessError> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn validate_macos_executable(path: &Path) -> Result<(), WorkerProcessError> {
    use std::os::unix::fs::PermissionsExt;
    if !path.is_absolute()
        || !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        || fs::symlink_metadata(path).map_or(true, |m| {
            !m.is_file() || m.file_type().is_symlink() || m.permissions().mode() & 0o111 == 0
        })
    {
        return Err(WorkerProcessError::InvalidExecutable);
    }
    if PackageStore::verify_immutable_entrypoint_path(path).is_err() {
        return Err(WorkerProcessError::InvalidExecutable);
    }
    let mut header = [0; 4];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut header))
        .map_err(|_| WorkerProcessError::InvalidExecutable)?;
    if !matches!(
        header,
        [0xcf, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe] | [0xbe, 0xba, 0xfe, 0xca]
    ) {
        return Err(WorkerProcessError::InvalidExecutable);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn launch_macos(
    executable: &Path,
    state_root: Option<&Path>,
) -> Result<WorkerProcess, WorkerProcessError> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    let mut command = tokio::process::Command::new(executable);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.kill_on_drop(true);
    command.as_std_mut().process_group(0);
    command.env_clear().env("PATH", "/usr/bin:/bin");
    if let Some(root) = state_root {
        command.env("MUNDUS_DATA_DIR", root);
    }
    command.current_dir(
        executable
            .parent()
            .ok_or(WorkerProcessError::InvalidExecutable)?,
    );
    let child = command.spawn().map_err(|_| WorkerProcessError::Setup)?;
    let group = child.id().ok_or(WorkerProcessError::Setup)? as i32;
    let mut process = WorkerProcess {
        child,
        group,
        stdin: None,
        stdout: None,
        stderr: None,
    };
    process.stdin = process.child.stdin.take();
    process.stdout = process.child.stdout.take();
    process.stderr = process.child.stderr.take();
    if !process.has_all_pipes() {
        return Err(WorkerProcessError::Setup);
    }
    Ok(process)
}

#[cfg(target_os = "macos")]
impl WorkerProcess {
    fn kill_group(&mut self) -> Result<(), WorkerProcessError> {
        if self.group > 0 {
            // The child was placed in a fresh group before exec. The negative
            // PID addresses that group, including descendants still in it.
            if unsafe { libc::kill(-self.group, libc::SIGKILL) } != 0 {
                let errno = std::io::Error::last_os_error().raw_os_error();
                // Darwin reports EPERM for a group containing only a zombie
                // leader. Reap our child and inspect the group; only an empty
                // group proves that no descendant remains to clean up.
                let child_exited = self.child.try_wait().ok().flatten().is_some();
                let empty = errno == Some(libc::EPERM) && group_is_empty(self.group);
                if errno != Some(libc::ESRCH)
                    && !(errno == Some(libc::EPERM) && child_exited && empty)
                {
                    return Err(WorkerProcessError::Cleanup);
                }
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn group_is_empty(group: i32) -> bool {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PGRP, group];
    let mut buffer = [0u8; 4096];
    let mut size = buffer.len();
    unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        ) == 0
            && size == 0
    }
}

#[cfg(target_os = "macos")]
impl Drop for WorkerProcess {
    fn drop(&mut self) {
        let _ = self.kill_group();
        let _ = self.child.start_kill();
    }
}
