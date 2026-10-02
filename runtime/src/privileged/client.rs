//! Engine-side (user-mode) API for the privileged service.
//!
//! `status` never elevates. `enable` runs `mundus-engine privileged install`
//! through `ShellExecuteExW` with the `runas` verb — the one and only UAC
//! prompt; afterwards the registered service is a stable copy the Engine
//! talks to over a versioned, backward-compatible pipe protocol, so Engine
//! updates never re-prompt.
//!
//! `sync_managed_block` is the hosts-file entry point callers use (e.g.
//! focus mode): it is a thin pipe client plus a test delegate seam.

use serde::Serialize;
use std::sync::{Arc, RwLock};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(windows)]
use crate::privileged::protocol::Request;

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrivilegedStatus {
    pub supported: bool,
    pub installed: bool,
    pub running: bool,
    pub service_name: Option<String>,
    pub binary_path: Option<String>,
    /// The service pipe answered a ping (service is actually serving, not
    /// just registered).
    pub pipe_ok: bool,
    /// Service protocol version observed via pipe ping; `None` when the pipe
    /// is unreachable.
    pub protocol_version: Option<u32>,
    /// True when the installed service speaks a protocol this build accepts.
    /// `false` + `installed` = the service must be reinstalled (the only path
    /// allowed to show UAC again).
    pub compatible: bool,
}

/// Query service + pipe state. Never elevates, never blocks long.
#[cfg(windows)]
pub fn status() -> PrivilegedStatus {
    use crate::privileged::{brand, pipe, protocol};

    let installed = crate::privileged::scm_status::query_status();
    let (installed, running, binary_path) = match installed {
        Ok(s) => (s.installed, s.running, s.binary_path),
        Err(_) => (false, false, None),
    };
    let (pipe_ok, protocol_version) = match pipe::ping() {
        Ok(v) => (true, Some(v)),
        Err(_) => (false, None),
    };
    PrivilegedStatus {
        supported: true,
        installed,
        running,
        service_name: Some(brand::SERVICE_NAME.into()),
        binary_path,
        pipe_ok,
        protocol_version,
        compatible: protocol_version
            .map(protocol::client_accepts)
            .unwrap_or(false),
    }
}

#[cfg(not(windows))]
pub fn status() -> PrivilegedStatus {
    PrivilegedStatus::default()
}

/// One-time grant: launch `privileged install` elevated. Returns the
/// resulting status; "elevation declined" is an error the caller may treat
/// as a user cancel.
#[cfg(windows)]
pub fn enable() -> Result<PrivilegedStatus, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_CANCELLED, HWND};
    use windows::Win32::System::Threading::WaitForSingleObject;
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_NORMAL;

    let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    // The SID of THIS (unelevated) Engine process's user — handed to the
    // elevated install as `--grant-sid`. Under over-the-shoulder UAC the
    // installer's own token is the admin's, so the SID must come from here.
    let grant_sid = crate::privileged::token::current_user_sid()
        .map_err(|e| format!("resolve caller SID failed: {e}"))?;
    let exe_w: Vec<u16> = std::ffi::OsStr::new(&exe)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let params: Vec<u16> = format!("privileged install --grant-sid {grant_sid}\0")
        .encode_utf16()
        .collect();

    unsafe {
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            hwnd: HWND::default(),
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(exe_w.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            lpDirectory: PCWSTR::null(),
            nShow: SW_NORMAL.0,
            ..Default::default()
        };
        if let Err(e) = ShellExecuteExW(&mut info) {
            let code = GetLastError();
            if code == ERROR_CANCELLED {
                return Err("elevation declined".to_string());
            }
            return Err(format!("elevation launch failed: {e}"));
        }
        // UAC + install can take a while; bound the wait so the RPC handler
        // can't hang forever.
        if !info.hProcess.is_invalid() {
            let _ = WaitForSingleObject(info.hProcess, 120_000);
            let _ = CloseHandle(info.hProcess);
        }
    }
    Ok(status())
}

#[cfg(not(windows))]
pub fn enable() -> Result<PrivilegedStatus, String> {
    Err("privileged operations are Windows-only".to_string())
}

// ---------- managed hosts block sync (Engine callers) ----------

type HostsDelegate = Arc<dyn Fn(&str, &[String]) -> Result<(), String> + Send + Sync>;
static HOSTS_DELEGATE: RwLock<Option<HostsDelegate>> = RwLock::new(None);

/// Test seam: focus tests install an in-memory delegate instead of touching
/// the real pipe.
pub fn set_hosts_delegate_for_test(delegate: Option<HostsDelegate>) {
    *HOSTS_DELEGATE.write().unwrap_or_else(|p| p.into_inner()) = delegate;
}

/// Set the named managed block to exactly `domains`; an empty list removes
/// the block. Errors are strings for RPC plumbing.
pub fn sync_managed_block(block: &str, domains: &[String]) -> Result<(), String> {
    if let Some(delegate) = HOSTS_DELEGATE
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
    {
        return delegate(block, domains);
    }
    sync_managed_block_via_pipe(block, domains)
}

#[cfg(windows)]
fn sync_managed_block_via_pipe(block: &str, domains: &[String]) -> Result<(), String> {
    use crate::privileged::pipe;
    let req = if domains.is_empty() {
        Request::HostsRemove {
            block: block.to_string(),
        }
    } else {
        Request::HostsApply {
            block: block.to_string(),
            domains: domains.to_vec(),
        }
    };
    let resp = pipe::request(&req)?;
    if resp.ok {
        Ok(())
    } else {
        Err(resp.error.unwrap_or_else(|| "service error".into()))
    }
}

#[cfg(not(windows))]
fn sync_managed_block_via_pipe(_block: &str, _domains: &[String]) -> Result<(), String> {
    Err("privileged operations are Windows-only".to_string())
}

// ---------- firewall rule for the current Engine exe (KOS-269) ----------

/// Ask the installed service to point its managed inbound allow rule at the
/// exe calling it (the service derives the path from the pipe client). An
/// older service answers "unknown variant" — that is a clean "unsupported"
/// the caller falls back from, not a failure worth retrying.
#[cfg(windows)]
pub fn ensure_engine_firewall_rule() -> Result<(), String> {
    use crate::privileged::pipe;
    let resp = pipe::request(&Request::EnsureEngineAllow)?;
    if resp.ok {
        Ok(())
    } else {
        Err(resp.error.unwrap_or_else(|| "service error".into()))
    }
}

#[cfg(not(windows))]
pub fn ensure_engine_firewall_rule() -> Result<(), String> {
    Err("privileged operations are Windows-only".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn delegate_receives_applies_and_removes() {
        let calls: Arc<Mutex<Vec<(String, Vec<String>)>>> = Arc::new(Mutex::new(Vec::new()));
        let seen = calls.clone();
        set_hosts_delegate_for_test(Some(Arc::new(move |block, domains| {
            seen.lock()
                .unwrap()
                .push((block.to_string(), domains.to_vec()));
            Ok(())
        })));
        sync_managed_block("site-block", &["a.com".into()]).unwrap();
        sync_managed_block("site-block", &[]).unwrap();
        set_hosts_delegate_for_test(None);
        let calls = calls.lock().unwrap();
        assert_eq!(
            *calls,
            vec![
                ("site-block".to_string(), vec!["a.com".to_string()]),
                ("site-block".to_string(), Vec::new()),
            ]
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_platform_reports_not_supported() {
        assert!(!status().supported);
        assert!(enable().is_err());
        assert!(sync_managed_block_via_pipe("x", &[]).is_err());
    }
}
