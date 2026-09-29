//! Named-pipe accept loop for the privileged service (LocalSystem).
//!
//! Security boundary:
//! * the pipe DACL allows exactly SYSTEM and the user who enabled privileged
//!   mode (`--grant-sid`, captured at `privileged install` time);
//! * `PIPE_REJECT_REMOTE_CLIENTS` — no network access;
//! * requests are a closed enum (`protocol::Request`) — no arbitrary paths,
//!   commands, or file writes;
//! * request size is bounded (`request_io::MAX_REQUEST_BYTES`);
//! * `ntfs_scan` additionally impersonates the client to prove it can list
//!   the requested drive root, and filters results through the caller's own
//!   profile directory so other users' file names are never returned.

#![cfg(windows)]

use std::ffi::OsStr;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use windows::core::{HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, BOOL, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL,
    INVALID_HANDLE_VALUE,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, NAMED_PIPE_MODE, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use crate::privileged::brand;
use crate::privileged::hosts;
use crate::privileged::impersonate;
use crate::privileged::ntfs_scan;
use crate::privileged::protocol::{self, Request, Response};
use crate::privileged::request_io;

const BUF_SIZE: u32 = 64 * 1024;

/// SDDL for the pipe DACL: full access for SYSTEM and the enabling user only.
/// The SID comes from the service's `--grant-sid` launch argument (written by
/// `privileged install`). Returns None for malformed SIDs — a garbage string
/// must never be pasted into a security descriptor.
pub fn pipe_sddl(grant_sid: &str) -> Option<String> {
    if !is_sid_literal(grant_sid) {
        return None;
    }
    Some(format!("D:(A;;GA;;;SY)(A;;GA;;;{grant_sid})"))
}

/// Well-formed SID literal: `S-<digits>(-<digits>)*`.
fn is_sid_literal(sid: &str) -> bool {
    let rest = match sid.strip_prefix("S-") {
        Some(r) => r,
        None => return false,
    };
    !rest.is_empty()
        && rest.len() <= 64
        && rest
            .split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// SECURITY_ATTRIBUTES carrying the DACL. The returned descriptor must stay
/// alive for as long as pipes are created from it — the caller owns it and
/// frees with LocalFree.
unsafe fn build_security_attributes(
    sddl: &str,
) -> Option<(SECURITY_ATTRIBUTES, PSECURITY_DESCRIPTOR)> {
    let sddl_w = wide(sddl);
    let mut sd = PSECURITY_DESCRIPTOR(std::ptr::null_mut());
    ConvertStringSecurityDescriptorToSecurityDescriptorW(
        PCWSTR(sddl_w.as_ptr()),
        SDDL_REVISION_1,
        &mut sd,
        None,
    )
    .ok()?;
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd.0,
        bInheritHandle: BOOL(0),
    };
    Some((sa, sd))
}

unsafe fn create_pipe_instance(sa: *const SECURITY_ATTRIBUTES) -> HANDLE {
    let name = wide(brand::PIPE_NAME);
    CreateNamedPipeW(
        PCWSTR(name.as_ptr()),
        PIPE_ACCESS_DUPLEX,
        NAMED_PIPE_MODE(
            PIPE_TYPE_BYTE.0 | PIPE_READMODE_BYTE.0 | PIPE_WAIT.0 | PIPE_REJECT_REMOTE_CLIENTS.0,
        ),
        PIPE_UNLIMITED_INSTANCES,
        BUF_SIZE,
        BUF_SIZE,
        0,
        if sa.is_null() { None } else { Some(sa) },
    )
}

/// Main accept loop. `grant_sid` is the enabling user's SID; when it is
/// malformed the pipe is created SYSTEM-only (fail closed — nobody but
/// LocalSystem can talk to the service until it is reinstalled properly).
pub fn accept_loop(stop_flag: Arc<AtomicBool>, grant_sid: &str) {
    let sddl = pipe_sddl(grant_sid).unwrap_or_else(|| {
        eprintln!("[privileged-service] invalid --grant-sid; pipe is SYSTEM-only");
        // "D:(A;;GA;;;SY)" — SYSTEM only.
        String::from("D:(A;;GA;;;SY)")
    });
    let sa_pair = unsafe { build_security_attributes(&sddl) };
    let sa_ptr: *const SECURITY_ATTRIBUTES = match &sa_pair {
        Some((sa, _)) => sa,
        None => std::ptr::null(),
    };

    while !stop_flag.load(Ordering::SeqCst) {
        let pipe = unsafe { create_pipe_instance(sa_ptr) };
        if pipe == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            eprintln!("CreateNamedPipeW failed: {}", err.0);
            thread::sleep(std::time::Duration::from_millis(500));
            continue;
        }

        let connected = unsafe { ConnectNamedPipe(pipe, None) };
        if let Err(e) = connected {
            if e.code() != HRESULT::from_win32(ERROR_PIPE_CONNECTED.0) {
                eprintln!("ConnectNamedPipe failed: {e}");
                unsafe { CloseHandle(pipe) }.ok();
                continue;
            }
        }

        if stop_flag.load(Ordering::SeqCst) {
            unsafe {
                let _ = DisconnectNamedPipe(pipe);
                CloseHandle(pipe).ok();
            }
            break;
        }

        // Raw handle into the worker thread.
        let h = PipeHandle(pipe);
        thread::spawn(move || {
            handle_connection(h);
        });
    }

    if let Some((_, sd)) = sa_pair {
        unsafe { LocalFree(HLOCAL(sd.0)) };
    }
}

struct PipeHandle(HANDLE);
unsafe impl Send for PipeHandle {}

fn handle_connection(pipe: PipeHandle) {
    let mut file = unsafe { std::fs::File::from_raw_handle(pipe.0 .0) };
    let raw = request_io::read_request_line(&mut file).unwrap_or_default();

    // Parse first — impersonation requires a client message to have been
    // read before ImpersonateNamedPipeClient succeeds.
    let value: serde_json::Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(e) => {
            write_response(&mut file, &Response::err(format!("invalid request: {e}")));
            return;
        }
    };
    let version = protocol::wire_version(&value);
    if version > protocol::PROTOCOL_VERSION {
        write_response(
            &mut file,
            &Response::err(format!(
                "unsupported protocol version {version} (max {})",
                protocol::PROTOCOL_VERSION
            )),
        );
        return;
    }
    let req: Request = match serde_json::from_value(value) {
        Ok(r) => r,
        Err(e) => {
            write_response(&mut file, &Response::err(format!("invalid request: {e}")));
            return;
        }
    };

    let resp = match req {
        Request::NtfsScan {
            root,
            exclude_noisy,
        } => handle_ntfs_scan(pipe.0, &root, exclude_noisy),
        other => protocol::dispatch(other, &hosts::default_hosts_path()),
    };
    write_response(&mut file, &resp);
}

fn write_response(file: &mut std::fs::File, resp: &Response) {
    let json = serde_json::to_string(resp)
        .unwrap_or_else(|_| String::from(r#"{"ok":false,"error":"serialize failed"}"#));
    let _ = writeln!(file, "{json}");
    let _ = file.flush();
}

/// `ntfs_scan` with caller gating:
/// 1. impersonate the client and prove it can enumerate the requested drive
///    root itself;
/// 2. scan as SYSTEM;
/// 3. drop entries under other users' profile directories.
fn handle_ntfs_scan(pipe: HANDLE, root: &str, exclude_noisy: bool) -> Response {
    let root = match ntfs_scan::validate_scan_root(root) {
        Ok(r) => r,
        Err(e) => return Response::err(e),
    };
    let profile = match impersonate::client_profile_dir(pipe) {
        Ok(p) => p,
        Err(e) => return Response::err(e),
    };

    // Access check under the client's identity: it must be able to enumerate
    // the requested root on its own.
    let readable =
        impersonate::with_client_impersonation(pipe, || std::fs::read_dir(&root).is_ok());
    if !readable {
        return Response::err(format!("scan root {root} is not readable by the caller"));
    }

    match ntfs_scan::scan_drive_root(&root, exclude_noisy) {
        Ok(files) => Response::files(ntfs_scan::retain_visible_to(files, &profile)),
        Err(e) => Response::err(e),
    }
}

#[cfg(test)]
#[path = "pipe_server_tests.rs"]
mod tests;
