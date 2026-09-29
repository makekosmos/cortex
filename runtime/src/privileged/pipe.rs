//! Named-pipe client transport (Engine side, user-mode).
//!
//! One request per connection: open the pipe, write one JSON line, read the
//! response until EOF (bounded). Works for the current service and for
//! legacy pre-versioning services — unknown fields are ignored by both sides,
//! and a missing `protocol_version` in the response reads as v1.
//!
//! Anti-squatting: before anything is written, `GetNamedPipeServerProcessId`
//! must return the PID of the owning service's process (SCM query). While the
//! service is stopped a random local process could otherwise create the pipe
//! name first and harvest requests or feed fake scan results.

#![cfg(windows)]

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::io::AsRawHandle;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;

use crate::privileged::brand;
use crate::privileged::protocol::{self, Request, Response};
use crate::privileged::scm_status;

/// Upper bound on a service response. A whole-drive MFT listing is the
/// largest legitimate payload (millions of entries at ~150 B each); 1 GiB
/// exceeds it by a wide margin while still bounding a malicious/squatting
/// pipe holder streaming forever.
const MAX_RESPONSE_BYTES: u64 = 1 << 30;

/// Service expected to own each pipe name we talk to (anti-squatting).
/// `None` for unknown names → the caller must not use them.
fn service_name_for_pipe(pipe_name: &str) -> Option<&'static str> {
    if pipe_name == brand::PIPE_NAME {
        return Some(brand::SERVICE_NAME);
    }
    // MIGRATION(KOS-267): drop with the legacy pipes after 2026-11-01.
    brand::LEGACY_PIPES
        .iter()
        .find(|(name, _)| *name == pipe_name)
        .map(|(_, svc)| *svc)
}

/// The connected server is trusted iff its PID equals the SCM-registered
/// service PID. `None` (service missing/stopped) and PID 0 → never trusted.
fn server_identity_ok(pipe_server_pid: u32, service_pid: Option<u32>) -> bool {
    pipe_server_pid != 0 && service_pid == Some(pipe_server_pid)
}

/// Verify the pipe server is our service's process before any bytes are sent.
fn verify_server_identity(pipe: &File, pipe_name: &str) -> Result<(), String> {
    let service = service_name_for_pipe(pipe_name)
        .ok_or_else(|| format!("untrusted pipe name {pipe_name}"))?;
    let expected = scm_status::service_pid(service);
    let mut pid = 0u32;
    unsafe {
        GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut pid)
            .map_err(|e| format!("pipe server pid query failed: {e}"))?;
    }
    if !server_identity_ok(pid, expected) {
        return Err(format!(
            "pipe {pipe_name} served by pid {pid}, expected service {service} \
             (pid {expected:?}) — possible squatting; refusing"
        ));
    }
    Ok(())
}

/// Send `req` to the primary service pipe.
pub fn request(req: &Request) -> Result<Response, String> {
    request_on(brand::PIPE_NAME, req)
}

/// Send `req` to a specific pipe. For `ntfs_scan` callers should also try
/// `brand::LEGACY_PIPES` names on failure — the same versioned request parses
/// fine on older services (they ignore unknown fields).
pub fn request_on(pipe_name: &str, req: &Request) -> Result<Response, String> {
    let mut pipe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)
        .map_err(|e| format!("connect {pipe_name} failed: {e}"))?;

    verify_server_identity(&pipe, pipe_name)?;

    let wire = protocol::encode_request(req);
    writeln!(pipe, "{wire}").map_err(|e| format!("write request failed: {e}"))?;
    pipe.flush()
        .map_err(|e| format!("flush request failed: {e}"))?;

    let mut raw = String::new();
    std::io::Read::take(&pipe, MAX_RESPONSE_BYTES + 1)
        .read_to_string(&mut raw)
        .map_err(|e| format!("read response failed: {e}"))?;
    if raw.len() as u64 > MAX_RESPONSE_BYTES {
        return Err("service response exceeds 1 GiB".to_string());
    }

    let value: serde_json::Value = serde_json::from_str(raw.trim())
        .map_err(|e| format!("parse service response failed: {e}"))?;
    let version = protocol::wire_version(&value);
    if !protocol::client_accepts(version) {
        return Err(format!(
            "service speaks protocol {version}, this Engine supports \
             {}..={} — reinstall the service (`privileged install`)",
            protocol::MIN_PROTOCOL_VERSION,
            protocol::PROTOCOL_VERSION,
        ));
    }
    serde_json::from_value(value).map_err(|e| format!("invalid service response: {e}"))
}

/// Reachability + version probe.
pub fn ping() -> Result<u32, String> {
    let resp = request(&Request::Ping)?;
    if !resp.ok {
        return Err(resp.error.unwrap_or_else(|| "ping failed".into()));
    }
    Ok(resp
        .protocol_version
        .unwrap_or(protocol::MIN_PROTOCOL_VERSION))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipes_map_to_their_owning_service() {
        assert_eq!(
            service_name_for_pipe(brand::PIPE_NAME),
            Some(brand::SERVICE_NAME)
        );
        for (name, svc) in brand::LEGACY_PIPES {
            assert_eq!(service_name_for_pipe(name), Some(*svc));
        }
        assert_eq!(service_name_for_pipe(r"\\.\pipe\random"), None);
    }

    #[test]
    fn server_identity_requires_the_service_pid() {
        // Squatter's pid != service pid → refuse.
        assert!(!server_identity_ok(4321, Some(1234)));
        // Service not running / unknown → refuse (squatting window).
        assert!(!server_identity_ok(4321, None));
        assert!(server_identity_ok(1234, Some(1234)));
        assert!(!server_identity_ok(0, Some(1234)));
        assert!(!server_identity_ok(0, Some(0)));
    }
}
