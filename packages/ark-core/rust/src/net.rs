//! Network-address filtering helpers — Syncthing-style.
//!
//! The crate itself doesn't enumerate interfaces (that's host-specific and
//! should be done by the caller via `std::net` / `if-addrs` / NDK / NWPath).
//! These helpers take an already-collected list of addresses and strip
//! non-routable entries before they're advertised in beacons / hello /
//! peer_list exchanges.
//!
//! Unfiltered addresses (link-local, VPN tunnels, virtual NICs) cause the
//! classic "All addresses failed" spam: they're reachable only from the
//! local host and every remote connect attempt fails.

/// Interface-name prefixes that belong to virtual / VPN / container adapters.
pub const VIRTUAL_IFACE_PREFIXES: &[&str] = &[
    // macOS
    "utun",
    "awdl",
    "llw",
    "ap",
    "bridge",
    "anpi",
    // Linux
    "docker",
    "br-",
    "veth",
    "virbr",
    "vboxnet",
    "vmnet",
    "tun",
    "tap",
    "wg",
    "tailscale",
    // Windows
    "vethernet",
    "vmware",
    "virtualbox",
    // Android
    "rmnet",
    "dummy",
];

/// Test whether a parsed IPv4 address is routable on the LAN. Shared with
/// `filter_routable_addresses` callers that already own an `Ipv4Addr`.
pub fn is_routable_v4_addr(addr: std::net::Ipv4Addr) -> bool {
    is_routable_v4(&addr.to_string())
}

/// Test whether a parsed IPv6 address is routable on the LAN.
pub fn is_routable_v6_addr(addr: std::net::Ipv6Addr) -> bool {
    is_routable_v6(&addr.to_string())
}

/// Check whether an interface name belongs to a virtual / VPN adapter.
pub fn is_virtual_interface(name: &str) -> bool {
    let lower = name.to_lowercase();
    VIRTUAL_IFACE_PREFIXES.iter().any(|p| lower.starts_with(p))
}

/// Check whether an IPv4 string is routable on the LAN.
/// Rejects link-local (169.254/16) and loopback (127/8).
pub fn is_routable_v4(addr: &str) -> bool {
    if addr.starts_with("169.254.") {
        return false;
    }
    if addr.starts_with("127.") {
        return false;
    }
    true
}

/// Check whether an IPv6 string is routable on the LAN.
/// Rejects link-local (fe80::/10), unique-local (fc00::/7), loopback (::1).
pub fn is_routable_v6(addr: &str) -> bool {
    let lower = addr.to_lowercase();
    // Link-local fe80::/10
    if lower.starts_with("fe8")
        || lower.starts_with("fe9")
        || lower.starts_with("fea")
        || lower.starts_with("feb")
    {
        return false;
    }
    // Unique-local fc00::/7
    if lower.starts_with("fc") || lower.starts_with("fd") {
        return false;
    }
    // Loopback
    if lower == "::1" {
        return false;
    }
    true
}

/// Filter a list of `host:port` / `[ipv6]:port` addresses, keeping only the
/// routable ones. The caller is responsible for supplying the list; this
/// function doesn't enumerate interfaces.
pub fn filter_routable_addresses(addresses: &[String]) -> Vec<String> {
    addresses
        .iter()
        .filter(|addr| is_address_routable(addr))
        .cloned()
        .collect()
}

/// Return `true` if a single `host:port` (or `[ipv6]:port`) string is routable.
pub fn is_address_routable(addr: &str) -> bool {
    let trimmed = addr.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Bracketed IPv6: [addr]:port
    if let Some(close) = trimmed
        .strip_prefix('[')
        .and_then(|s| s.find(']').map(|i| (s, i)))
    {
        let (inner, end) = close;
        let host = &inner[..end];
        // Strip zone id if present (interface suffix for link-local)
        let host = host.split('%').next().unwrap_or(host);
        return is_routable_v6(host);
    }

    // IPv4: host:port — take everything before the last colon.
    let host = match trimmed.rfind(':') {
        Some(idx) => &trimmed[..idx],
        None => trimmed,
    };

    // Could still be a bare IPv6 (no brackets) — distinguish by colon count.
    if host.chars().filter(|c| *c == ':').count() >= 1 {
        let host = host.split('%').next().unwrap_or(host);
        return is_routable_v6(host);
    }

    is_routable_v4(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_link_local_v4() {
        assert!(!is_routable_v4("169.254.10.1"));
        assert!(!is_address_routable("169.254.10.1:21531"));
    }

    #[test]
    fn rejects_loopback() {
        assert!(!is_routable_v4("127.0.0.1"));
        assert!(!is_routable_v6("::1"));
    }

    #[test]
    fn rejects_link_local_v6() {
        assert!(!is_routable_v6("fe80::1"));
        assert!(!is_address_routable("[fe80::1%en0]:21531"));
    }

    #[test]
    fn rejects_unique_local_v6() {
        assert!(!is_routable_v6("fc00::1"));
        assert!(!is_routable_v6("fd00::1"));
    }

    #[test]
    fn accepts_lan_v4() {
        assert!(is_routable_v4("192.168.1.70"));
        assert!(is_address_routable("192.168.1.70:21531"));
        assert!(is_routable_v4("10.0.0.1"));
    }

    #[test]
    fn accepts_global_v6() {
        assert!(is_routable_v6("2001:db8::1"));
        assert!(is_address_routable("[2001:db8::1]:21531"));
    }

    #[test]
    fn filters_mixed_list() {
        let input = vec![
            "192.168.1.70:21531".to_string(),
            "169.254.1.1:21531".to_string(),
            "[fe80::1%en0]:21531".to_string(),
            "[2001:db8::1]:21531".to_string(),
            "127.0.0.1:21531".to_string(),
        ];
        let out = filter_routable_addresses(&input);
        assert_eq!(
            out,
            vec![
                "192.168.1.70:21531".to_string(),
                "[2001:db8::1]:21531".to_string(),
            ]
        );
    }

    #[test]
    fn detects_virtual_interfaces() {
        assert!(is_virtual_interface("utun0"));
        assert!(is_virtual_interface("docker0"));
        assert!(is_virtual_interface("tailscale0"));
        assert!(is_virtual_interface("vmnet8"));
        assert!(!is_virtual_interface("en0"));
        assert!(!is_virtual_interface("eth0"));
        assert!(!is_virtual_interface("wlan0"));
    }
}
