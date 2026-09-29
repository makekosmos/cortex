//! Client impersonation helpers for the privileged pipe server.
//!
//! `ImpersonateNamedPipeClient` requires a message to have been read from the
//! pipe first — callers must parse the request before asking for the
//! client's identity. `client_profile_dir` resolves the connecting user's
//! profile path *under* their identity; it is the basis of the NTFS-scan
//! cross-user disclosure filter (`ntfs_scan::retain_visible_to`).

#![cfg(windows)]

use std::path::PathBuf;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{RevertToSelf, TOKEN_QUERY};
use windows::Win32::System::Pipes::ImpersonateNamedPipeClient;
use windows::Win32::System::Threading::{GetCurrentThread, OpenThreadToken};
use windows::Win32::UI::Shell::GetUserProfileDirectoryW;

/// The caller's profile directory, resolved while impersonating the pipe
/// client. `None` when the client cannot be identified — the scan then fails
/// closed rather than leaking file names.
pub fn client_profile_dir(pipe: HANDLE) -> Result<PathBuf, String> {
    unsafe {
        ImpersonateNamedPipeClient(pipe).map_err(|e| format!("impersonation failed: {e}"))?;
        let result = client_profile_dir_impersonated();
        let _ = RevertToSelf();
        result
    }
}

/// Run `f` under the connected client's identity; always reverts to self.
pub fn with_client_impersonation(pipe: HANDLE, f: impl FnOnce() -> bool) -> bool {
    unsafe {
        let ok = ImpersonateNamedPipeClient(pipe).is_ok() && f();
        let _ = RevertToSelf();
        ok
    }
}

fn client_profile_dir_impersonated() -> Result<PathBuf, String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, true, &mut token)
            .map_err(|e| format!("open client token failed: {e}"))?;
        let result = (|| {
            let mut size: u32 = 0;
            let _ = GetUserProfileDirectoryW(token, windows::core::PWSTR::null(), &mut size);
            if size == 0 || size > 32768 {
                return Err("client profile directory unavailable".to_string());
            }
            let mut buf = vec![0u16; size as usize];
            GetUserProfileDirectoryW(token, windows::core::PWSTR(buf.as_mut_ptr()), &mut size)
                .map_err(|e| format!("client profile directory failed: {e}"))?;
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Ok(PathBuf::from(String::from_utf16_lossy(&buf[..len])))
        })();
        let _ = CloseHandle(token);
        result
    }
}
