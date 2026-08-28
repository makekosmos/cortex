//! Host-level helpers: device name and interface enumeration.
//!
//! The sync protocol expects a human-readable `device_name` (OS host name) and
//! a list of routable addresses for `hello` / `peer_list` / UDP beacon payloads.
//! This module centralises those so callers don't re-implement them per
//! platform.
//!
//! Interface enumeration uses `if-addrs` for portability. Address filtering
//! applies the same Syncthing-style rules as `net.rs`.

use crate::net::{is_routable_v4, is_routable_v6, is_virtual_interface};

/// Return the OS host name, stripped of a trailing `.local` (Bonjour mDNS
/// suffix) and surrounding whitespace. Falls back to a generic placeholder
/// when the host-name lookup fails or returns an empty string.
///
/// The Electron TS code used `"Delphi Electron"` as its fallback. The Rust
/// crate is embedder-agnostic so we use a neutral default — the Android caller
/// always passes an explicit `device_name` via UniFFI, which sidesteps this
/// function entirely.
pub fn get_host_device_name() -> String {
    // Android: if the caller doesn't provide a name, return the canonical
    // generic label. Real Android sync code always passes
    // `${Build.MANUFACTURER} ${Build.MODEL}` so this branch only fires in
    // tests or broken callers.
    #[cfg(target_os = "android")]
    {
        return "Android device".to_string();
    }

    #[cfg(not(target_os = "android"))]
    {
        let raw = match hostname::get() {
            Ok(os_string) => os_string.to_string_lossy().to_string(),
            Err(_) => return "Ark Device".to_string(),
        };
        let cleaned = strip_dot_local(&raw);
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            "Ark Device".to_string()
        } else {
            trimmed.to_string()
        }
    }
}

/// Strip a trailing `.local` / `.LOCAL` suffix (case-insensitive).
#[cfg_attr(target_os = "android", allow(dead_code))]
fn strip_dot_local(raw: &str) -> String {
    if raw.len() >= 6 {
        let tail = &raw[raw.len() - 6..];
        if tail.eq_ignore_ascii_case(".local") {
            return raw[..raw.len() - 6].to_string();
        }
    }
    raw.to_string()
}

/// Enumerate all routable host-local addresses, formatted with the given
/// `port`. IPv4 addresses use `a.b.c.d:<port>`, IPv6 addresses use
/// `[addr]:<port>` (IPv6 zone-id `%zone` suffix stripped).
///
/// Addresses on loopback / link-local / unique-local / virtual interfaces are
/// dropped — matching the `apps/delphi/ts/electron/broadcast-discovery.ts`
/// `collectLocalAddresses` helper and `core/ark/packages/arksync/src/node.ts`
/// `getOwnAddresses`.
pub fn get_own_addresses(port: u16) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let interfaces = match if_addrs::get_if_addrs() {
        Ok(list) => list,
        Err(_) => return out,
    };

    for iface in interfaces {
        if iface.is_loopback() {
            continue;
        }
        if is_virtual_interface(&iface.name) {
            continue;
        }
        match iface.addr {
            if_addrs::IfAddr::V4(v4) => {
                let ip = v4.ip.to_string();
                if !is_routable_v4(&ip) {
                    continue;
                }
                let entry = format!("{ip}:{port}");
                if !out.contains(&entry) {
                    out.push(entry);
                }
            }
            if_addrs::IfAddr::V6(v6) => {
                // Strip zone id if present: `fe80::1%en0` → `fe80::1`.
                let raw = v6.ip.to_string();
                let clean = raw.split('%').next().unwrap_or(&raw);
                if !is_routable_v6(clean) {
                    continue;
                }
                let entry = format!("[{clean}]:{port}");
                if !out.contains(&entry) {
                    out.push(entry);
                }
            }
        }
    }

    out
}

/// Strip the `%zone` suffix from an IPv6 address string. Public so `beacon.rs`
/// and `sync_server.rs` can reuse the same canonical form.
pub fn strip_ipv6_zone(addr: &str) -> String {
    match addr.find('%') {
        Some(idx) => addr[..idx].to_string(),
        None => addr.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_name_is_non_empty() {
        let name = get_host_device_name();
        assert!(
            !name.is_empty(),
            "host name should fall back to a non-empty string"
        );
    }

    #[test]
    fn host_name_has_no_dot_local_suffix() {
        let name = get_host_device_name();
        assert!(
            !name.to_lowercase().ends_with(".local"),
            "host name {name:?} should not retain .local suffix",
        );
    }

    #[test]
    fn strip_dot_local_case_insensitive() {
        assert_eq!(strip_dot_local("macbook.local"), "macbook");
        assert_eq!(strip_dot_local("MacBook.LOCAL"), "MacBook");
        assert_eq!(strip_dot_local("MacBook.Local"), "MacBook");
        assert_eq!(strip_dot_local("mypc"), "mypc");
    }

    #[test]
    fn strip_ipv6_zone_works() {
        assert_eq!(strip_ipv6_zone("fe80::1%en0"), "fe80::1");
        assert_eq!(strip_ipv6_zone("2001:db8::1"), "2001:db8::1");
    }

    #[test]
    fn own_addresses_are_all_routable() {
        let addrs = get_own_addresses(21531);
        for a in &addrs {
            // Loopback, link-local, unique-local, virtual ifaces all rejected.
            assert!(
                crate::net::is_address_routable(a),
                "non-routable address leaked into get_own_addresses: {a}",
            );
            assert!(a.ends_with(":21531") || a.contains("]:21531"));
        }
    }

    #[test]
    fn own_addresses_reject_loopback() {
        let addrs = get_own_addresses(21531);
        for a in &addrs {
            assert!(!a.starts_with("127."));
            assert!(!a.starts_with("[::1]"));
        }
    }

    #[test]
    fn own_addresses_strip_ipv6_zone() {
        let addrs = get_own_addresses(21531);
        for a in &addrs {
            assert!(
                !a.contains('%'),
                "address {a} still carries an IPv6 zone id"
            );
        }
    }
}
