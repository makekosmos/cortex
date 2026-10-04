use super::*;

pub(crate) fn write_state(
    path: &Path,
    status: &str,
    crash_streak: usize,
    core_pid: Option<u32>,
    last_exit: Option<&ExitMetadata>,
) {
    let state = SupervisorState {
        format_version: 1,
        component: "engine-core",
        version: crate::build_info::engine_version(),
        correlation_id: crate::observability::correlation_id(),
        supervisor_pid: std::process::id(),
        core_pid,
        status,
        crash_streak,
        last_exit_code: last_exit.and_then(|exit| exit.code),
        last_run_ms: last_exit.map(|exit| exit.run_ms),
        updated_at: Utc::now().to_rfc3339(),
    };
    let Ok(json) = serde_json::to_vec_pretty(&state) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let temp = path.with_extension(format!("tmp.{}", std::process::id()));
    if std::fs::write(&temp, json).is_ok() {
        let _ = std::fs::rename(temp, path);
    }
}

pub fn diagnostics_snapshot(data_dir: &Path) -> serde_json::Value {
    std::fs::read(data_dir.join(SUPERVISOR_STATE_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(serde_json::Value::Null)
}

pub(crate) fn cleanup_state(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(windows)]
pub(crate) fn process_matches_current_executable(pid: u32) -> bool {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let expected = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(path) => path,
        Err(_) => return false,
    };
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut buffer = vec![0u16; 2048];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        if result.is_err() {
            return false;
        }
        let actual = PathBuf::from(String::from_utf16_lossy(&buffer[..size as usize]));
        std::fs::canonicalize(actual)
            .map(|path| path == expected)
            .unwrap_or(false)
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn process_matches_current_executable(pid: u32) -> bool {
    let expected = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(path) => path,
        Err(_) => return false,
    };
    std::fs::canonicalize(format!("/proc/{pid}/exe"))
        .map(|path| path == expected)
        .unwrap_or(false)
}

// macOS has no /proc; libproc's proc_pidpath is the equivalent.
#[cfg(target_os = "macos")]
pub(crate) fn process_matches_current_executable(pid: u32) -> bool {
    let expected = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(path) => path,
        Err(_) => return false,
    };
    let mut buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let len = unsafe {
        libc::proc_pidpath(
            pid as libc::c_int,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
        )
    };
    if len <= 0 {
        return false;
    }
    buffer.truncate(len as usize);
    let actual = match String::from_utf8(buffer) {
        Ok(path) => std::path::PathBuf::from(path),
        Err(_) => return false,
    };
    std::fs::canonicalize(actual)
        .map(|path| path == expected)
        .unwrap_or(false)
}

#[cfg(unix)]
pub(crate) fn exit_status_failure() -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(1 << 8)
}

#[cfg(windows)]
pub(crate) fn exit_status_failure() -> std::process::ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(1)
}
