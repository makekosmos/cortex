//! Executable image path of a same-user process, queried by PID —
//! trustworthy process identity, unlike self-declared client headers.
//!
//! Moved verbatim from `engine::auth` (KOS-335): `privileged::impersonate`
//! needs it to identify pipe clients without a back-edge into engine;
//! engine keeps `crate::auth::process_image_path` as a thin wrapper that
//! maps [`ProcessImageError`] onto `AuthError`.

#![cfg(windows)]

use thiserror::Error;

/// Mirrors the `AuthError` variants `process_image_path` produced in engine.
#[derive(Debug, Error)]
pub enum ProcessImageError {
    #[error("PID {pid} does not exist")]
    PidNotFound { pid: u32 },
    #[error("{0}")]
    Other(String),
}

/// Used to pin `/v1/rpc` callers that claim to be the Manager
/// (KOS-269 round 3).
pub fn process_image_path(pid: u32) -> Result<std::path::PathBuf, ProcessImageError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|_| ProcessImageError::PidNotFound { pid })?;
        let result = (|| {
            let mut buf = vec![0u16; 1024];
            let mut size = buf.len() as u32;
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_FORMAT(0),
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
            .map_err(|e| ProcessImageError::Other(format!("process image query failed: {e}")))?;
            Ok(std::path::PathBuf::from(String::from_utf16_lossy(
                &buf[..size as usize],
            )))
        })();
        let _ = CloseHandle(process);
        result
    }
}
