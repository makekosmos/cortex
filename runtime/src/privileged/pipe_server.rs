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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use windows::core::{HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, BOOL, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, HANDLE,
    HLOCAL, INVALID_HANDLE_VALUE,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, NAMED_PIPE_MODE, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use crate::privileged::brand;
use crate::privileged::firewall;
use crate::privileged::hosts;
use crate::privileged::ntfs_scan;
use crate::privileged::protocol::{self, Request, Response};
use crate::privileged::request_io;
use crate::privileged::token::is_user_account_sid;

const BUF_SIZE: u32 = 64 * 1024;

/// SDDL for the pipe DACL: full access for SYSTEM and the enabling user only.
/// The SID comes from the service's `--grant-sid` launch argument (written by
/// `privileged install`). Only an individual user-account SID is accepted —
/// a group/well-known SID (`S-1-1-0`, `S-1-5-32-*`, …) would grant the whole
/// group access, so anything else returns None (SYSTEM-only, fail closed).
pub fn pipe_sddl(grant_sid: &str) -> Option<String> {
    if !is_user_account_sid(grant_sid) {
        return None;
    }
    Some(format!("D:(A;;GA;;;SY)(A;;GA;;;{grant_sid})"))
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

unsafe fn create_pipe_instance(sa: *const SECURITY_ATTRIBUTES, first: bool) -> HANDLE {
    let name = wide(brand::PIPE_NAME);
    // FILE_FLAG_FIRST_PIPE_INSTANCE makes creation fail (ERROR_PIPE_BUSY)
    // when the name is already taken — without it any local user could create
    // the pipe first while the service is down and squat on it, receiving
    // the enabled user's requests or feeding fake scan results.
    let open_mode = if first {
        FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_DUPLEX.0 | FILE_FLAG_FIRST_PIPE_INSTANCE.0)
    } else {
        PIPE_ACCESS_DUPLEX
    };
    CreateNamedPipeW(
        PCWSTR(name.as_ptr()),
        open_mode,
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

    // Ownership invariant: the pipe name must never go completely unowned —
    // during a gap any local process could create the name first and squat on
    // it. The next instance is created BEFORE the connected one goes to a
    // worker, and `open_workers` counts workers still holding an instance
    // open; when it reaches zero with no listener the name may be squatted,
    // so it must be reclaimed with FILE_FLAG_FIRST_PIPE_INSTANCE.
    let open_workers = Arc::new(AtomicUsize::new(0));
    let mut listener = INVALID_HANDLE_VALUE;
    while !stop_flag.load(Ordering::SeqCst) {
        if listener == INVALID_HANDLE_VALUE {
            let first = must_reclaim_name(open_workers.load(Ordering::SeqCst));
            listener = unsafe { create_pipe_instance(sa_ptr, first) };
            if listener == INVALID_HANDLE_VALUE {
                let err = unsafe { GetLastError() };
                if first && err == ERROR_PIPE_BUSY {
                    eprintln!(
                        "pipe {} is already owned by another process — refusing to serve \
                         (possible squatting); retrying",
                        brand::PIPE_NAME
                    );
                } else {
                    eprintln!("CreateNamedPipeW failed: {}", err.0);
                }
                thread::sleep(std::time::Duration::from_millis(500));
                continue;
            }
        }

        let connected = unsafe { ConnectNamedPipe(listener, None) };
        if let Err(e) = connected {
            if e.code() != HRESULT::from_win32(ERROR_PIPE_CONNECTED.0) {
                eprintln!("ConnectNamedPipe failed: {e}");
                unsafe { CloseHandle(listener) }.ok();
                listener = INVALID_HANDLE_VALUE;
                continue;
            }
        }

        if stop_flag.load(Ordering::SeqCst) {
            unsafe {
                let _ = DisconnectNamedPipe(listener);
                CloseHandle(listener).ok();
            }
            break;
        }

        // Open the next listener BEFORE the connected handle leaves this
        // thread, so the name stays owned even if the worker exits instantly.
        let next = unsafe { create_pipe_instance(sa_ptr, false) };
        if next == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            eprintln!("CreateNamedPipeW (next instance) failed: {}", err.0);
        }

        open_workers.fetch_add(1, Ordering::SeqCst);
        let counter = open_workers.clone();
        let h = PipeHandle(listener);
        thread::spawn(move || serve_worker(h, counter));
        listener = next;
    }

    if let Some((_, sd)) = sa_pair {
        unsafe { LocalFree(HLOCAL(sd.0)) };
    }
}

/// Decide whether the next `CreateNamedPipeW` must reclaim the name with
/// `FILE_FLAG_FIRST_PIPE_INSTANCE` (i.e. possibly race a squatter): true iff
/// no instance of ours is open anywhere — no listener handle and no worker
/// still serving a connected instance.
fn must_reclaim_name(open_workers: usize) -> bool {
    open_workers == 0
}

struct PipeHandle(HANDLE);
unsafe impl Send for PipeHandle {}

fn serve_worker(pipe: PipeHandle, counter: Arc<AtomicUsize>) {
    let mut file = unsafe { std::fs::File::from_raw_handle(pipe.0 .0) };
    serve_connection(pipe.0, &mut file);
    // Decrement while this instance is still open — `open_workers > 0` must
    // always imply a live instance of ours.
    counter.fetch_sub(1, Ordering::SeqCst);
}

/// Serve one connected pipe instance; `file` wraps `pipe` and is closed by
/// the caller after this returns.
fn serve_connection(pipe: HANDLE, file: &mut std::fs::File) {
    let raw = request_io::read_request_line(file).unwrap_or_default();

    // Parse first — impersonation requires a client message to have been
    // read before ImpersonateNamedPipeClient succeeds.
    let value: serde_json::Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(e) => {
            write_response(file, &Response::err(format!("invalid request: {e}")));
            return;
        }
    };
    let version = protocol::wire_version(&value);
    if version > protocol::PROTOCOL_VERSION {
        write_response(
            file,
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
            write_response(file, &Response::err(format!("invalid request: {e}")));
            return;
        }
    };

    let resp = match req {
        Request::NtfsScan {
            root,
            exclude_noisy,
        } => ntfs_scan::handle_pipe_request(pipe, &root, exclude_noisy),
        Request::EnsureEngineAllow => match firewall::ensure_for_pipe_client(pipe) {
            Ok(_) => Response::ok(),
            Err(e) => Response::err(e),
        },
        other => protocol::dispatch(other, &hosts::default_hosts_path()),
    };
    write_response(file, &resp);
}

fn write_response(file: &mut std::fs::File, resp: &Response) {
    let json = serde_json::to_string(resp)
        .unwrap_or_else(|_| String::from(r#"{"ok":false,"error":"serialize failed"}"#));
    let _ = writeln!(file, "{json}");
    let _ = file.flush();
}

#[cfg(test)]
#[path = "pipe_server_tests.rs"]
mod tests;
