//! Named-pipe client transport (Engine side, user-mode).
//!
//! One request per connection: open the pipe, write one JSON line, read the
//! response until EOF (bounded). Works for the current service and for
//! legacy pre-versioning services — unknown fields are ignored by both sides,
//! and a missing `protocol_version` in the response reads as v1.

#![cfg(windows)]

use std::fs::OpenOptions;
use std::io::{Read, Write};

use crate::privileged::brand;
use crate::privileged::protocol::{self, Request, Response};

/// Upper bound on a service response. A whole-drive MFT listing is the
/// largest legitimate payload (millions of entries at ~150 B each); 1 GiB
/// exceeds it by a wide margin while still bounding a malicious/squatting
/// pipe holder streaming forever.
const MAX_RESPONSE_BYTES: u64 = 1 << 30;

/// Send `req` to the primary service pipe.
pub fn request(req: &Request) -> Result<Response, String> {
    request_on(brand::PIPE_NAME, req)
}

/// Send `req` to a specific pipe. For `ntfs_scan` callers should also try
/// `brand::LEGACY_PIPE_NAMES` on failure — the same versioned request parses
/// fine on older services (they ignore unknown fields).
pub fn request_on(pipe_name: &str, req: &Request) -> Result<Response, String> {
    let mut pipe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)
        .map_err(|e| format!("connect {pipe_name} failed: {e}"))?;

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
