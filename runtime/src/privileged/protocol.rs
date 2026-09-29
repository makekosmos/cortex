//! Wire protocol for the privileged-service named pipe.
//!
//! Requests and responses carry `protocol_version`. Compatibility rule:
//! the client accepts a service reporting any version in
//! `MIN_PROTOCOL_VERSION..=PROTOCOL_VERSION` (missing field = 1, the first
//! shipped version). A service refuses requests from a *newer* protocol it
//! does not understand. Versions only bump on breaking changes — a newer
//! Engine must keep talking to the installed (possibly older) service, so
//! Engine upgrades never need another UAC prompt. When a bump is unavoidable,
//! raise `PROTOCOL_VERSION`, extend `client_accepts`, and let
//! `system.privileged.status` surface the mismatch so the caller can re-run
//! `privileged install` (the only path allowed to prompt for elevation again).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::privileged::hosts;

/// Highest protocol version this build speaks.
pub const PROTOCOL_VERSION: u32 = 1;
/// Lowest service version a client accepts.
pub const MIN_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Ping,
    HostsApply {
        block: String,
        domains: Vec<String>,
    },
    HostsRemove {
        block: String,
    },
    HostsStatus,
    /// Remove every managed block (all names) and restore the backup.
    Reset,
    #[cfg(windows)]
    NtfsScan {
        root: String,
        exclude_noisy: bool,
    },
}

/// Serialize a request with the protocol version stamped in.
/// Unknown top-level fields are ignored by peers, so older services simply
/// skip `protocol_version`.
pub fn encode_request(req: &Request) -> String {
    let mut value = serde_json::to_value(req).unwrap_or_else(|_| Value::Null);
    if let Value::Object(map) = &mut value {
        map.insert("protocol_version".into(), Value::from(PROTOCOL_VERSION));
    }
    value.to_string()
}

/// Read the peer-declared `protocol_version` out of a raw JSON message;
/// absent/malformed means "first shipped version" (1).
pub fn wire_version(raw: &Value) -> u32 {
    raw.get("protocol_version")
        .and_then(Value::as_u64)
        .map(|v| v.min(u32::MAX as u64) as u32)
        .unwrap_or(MIN_PROTOCOL_VERSION)
}

/// True when a service advertising `service_version` can serve this client.
pub fn client_accepts(service_version: u32) -> bool {
    (MIN_PROTOCOL_VERSION..=PROTOCOL_VERSION).contains(&service_version)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub ok: bool,
    /// Service's protocol version. `None` on the wire = pre-versioned (v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<BTreeMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pong: Option<bool>,
    #[cfg(windows)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<crate::privileged::ntfs_scan::NtfsScanEntry>>,
}

impl Response {
    fn base(ok: bool) -> Self {
        Self {
            ok,
            protocol_version: Some(PROTOCOL_VERSION),
            active_domains: None,
            blocks: None,
            error: None,
            pong: None,
            #[cfg(windows)]
            files: None,
        }
    }

    pub fn ok_domains(domains: Vec<String>) -> Self {
        let mut r = Self::base(true);
        r.active_domains = Some(domains);
        r
    }

    pub fn ok_blocks(blocks: Vec<(String, Vec<String>)>) -> Self {
        let mut r = Self::base(true);
        r.blocks = Some(blocks.into_iter().collect());
        r
    }

    pub fn err(msg: impl Into<String>) -> Self {
        let mut r = Self::base(false);
        r.error = Some(msg.into());
        r
    }

    pub fn pong() -> Self {
        let mut r = Self::base(true);
        r.pong = Some(true);
        r
    }

    #[cfg(windows)]
    pub fn files(files: Vec<crate::privileged::ntfs_scan::NtfsScanEntry>) -> Self {
        let mut r = Self::base(true);
        r.files = Some(files);
        r
    }
}

/// Parse a raw JSON request. Never panics — errors come back as
/// `Response::err`. Requests declaring a newer protocol than this build are
/// refused with a versioned error so the client knows to re-install.
pub fn handle_raw(raw: &str, hosts_path: &Path) -> Response {
    let value: Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(e) => return Response::err(format!("invalid request: {e}")),
    };
    let version = wire_version(&value);
    if version > PROTOCOL_VERSION {
        return Response::err(format!(
            "unsupported protocol version {version} (max {PROTOCOL_VERSION})"
        ));
    }
    let req: Request = match serde_json::from_value(value) {
        Ok(r) => r,
        Err(e) => return Response::err(format!("invalid request: {e}")),
    };
    dispatch(req, hosts_path)
}

/// Execute a parsed request. `NtfsScan` never reaches here — it needs the
/// pipe connection for client impersonation, so `pipe_server` intercepts it.
pub fn dispatch(req: Request, hosts_path: &Path) -> Response {
    match req {
        Request::Ping => Response::pong(),
        Request::HostsApply { block, domains } => {
            match hosts::apply_block(hosts_path, &block, &domains) {
                Ok(active) => Response::ok_domains(active),
                Err(e) => Response::err(e.to_string()),
            }
        }
        Request::HostsRemove { block } => match hosts::remove_block(hosts_path, &block) {
            Ok(_) => Response::ok_domains(Vec::new()),
            Err(e) => Response::err(e.to_string()),
        },
        Request::HostsStatus => match hosts::list_blocks(hosts_path) {
            Ok(blocks) => Response::ok_blocks(blocks),
            Err(e) => Response::err(e.to_string()),
        },
        Request::Reset => match hosts::reset(hosts_path) {
            Ok(()) => Response::ok_domains(Vec::new()),
            Err(e) => Response::err(e.to_string()),
        },
        #[cfg(windows)]
        Request::NtfsScan { .. } => Response::err("ntfs_scan requires a pipe connection"),
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
