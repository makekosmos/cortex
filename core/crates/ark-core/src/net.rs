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
    // Name prefixes of virtual interfaces (utun, awdl, llw, anpi, …).
    // Matched by name on every host.
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
/// Rejects link-local (169.254/16), loopback (127/8), unspecified, broadcast,
/// multicast — and any input that isn't a parseable IPv4 literal (hostnames,
/// empty strings, malformed quads all fail the filter).
pub fn is_routable_v4(addr: &str) -> bool {
    let Ok(ip) = addr.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    if ip.is_unspecified() || ip.is_loopback() || ip.is_broadcast() || ip.is_multicast() {
        return false;
    }
    let [a, b, ..] = ip.octets();
    if a == 169 && b == 254 {
        return false;
    }
    true
}

/// Check whether an IPv6 string is routable on the LAN.
/// Rejects link-local (fe80::/10), unique-local (fc00::/7), loopback (::1),
/// unspecified (::) — and any input that isn't a parseable IPv6 literal.
pub fn is_routable_v6(addr: &str) -> bool {
    let Ok(ip) = addr.parse::<std::net::Ipv6Addr>() else {
        return false;
    };
    if ip.is_unspecified() || ip.is_loopback() || ip.is_multicast() {
        return false;
    }
    let seg0 = ip.segments()[0];
    // Link-local fe80::/10
    if seg0 & 0xffc0 == 0xfe80 {
        return false;
    }
    // Unique-local fc00::/7
    if seg0 & 0xfe00 == 0xfc00 {
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

/// `":<digits>"` suffix where `<digits>` is a usable (non-zero) port.
fn is_port_suffix(suffix: &str) -> bool {
    match suffix.strip_prefix(':') {
        Some(port) => is_port_str(port),
        None => false,
    }
}

fn is_port_str(port: &str) -> bool {
    matches!(port.parse::<u16>(), Ok(p) if p != 0)
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
        // Whatever follows ']' must be empty or ":port".
        let rest = &inner[end + 1..];
        if !rest.is_empty() && !is_port_suffix(rest) {
            return false;
        }
        return is_routable_v6(host);
    }

    // Bare IPv6 without brackets or port: at least two colons and the whole
    // string (zone id stripped) must parse as an address. Checking this before
    // the host:port split keeps "2001:db8::1" routable while "fe80::1" and
    // malformed strings still fail.
    let zoneless = trimmed.split('%').next().unwrap_or(trimmed);
    if zoneless.matches(':').count() >= 2 && zoneless.parse::<std::net::Ipv6Addr>().is_ok() {
        return is_routable_v6(zoneless);
    }

    // host:port — split at the last colon; a port segment must be a valid
    // non-zero u16 (":notaport" / ":0" are not dialable).
    let (host, port_ok) = match trimmed.rfind(':') {
        Some(idx) => (&trimmed[..idx], is_port_str(&trimmed[idx + 1..])),
        None => (trimmed, true),
    };
    if !port_ok {
        return false;
    }

    // A remaining colon means an unbracketed IPv6-ish host with a port suffix
    // that didn't parse as a full bare address above.
    if host.contains(':') {
        let host = host.split('%').next().unwrap_or(host);
        return is_routable_v6(host);
    }

    is_routable_v4(host)
}

/// Return `true` if a `host:port` / `[ipv6]:port` string names a loopback
/// address. Unlike `is_address_routable`, the host must parse as an IP —
/// garbage input is rejected, not waved through. Used by
/// `SyncBind::Loopback` to keep peer candidates on-host.
pub fn is_loopback_address(addr: &str) -> bool {
    let trimmed = addr.trim();
    if trimmed.is_empty() {
        return false;
    }
    let host = if let Some(rest) = trimmed.strip_prefix('[') {
        match rest.find(']') {
            Some(end) => &rest[..end],
            None => return false,
        }
    } else {
        match trimmed.rfind(':') {
            Some(idx) => &trimmed[..idx],
            None => trimmed,
        }
    };
    let host = host.split('%').next().unwrap_or(host);
    host.parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
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
    fn rejects_non_ip_strings() {
        // Regression: the string-prefix checks used to return `true` for any
        // input not on the reject list, so garbage hosts (hostnames, empty,
        // malformed quads) passed the routable filter straight into dial lists.
        assert!(!is_routable_v4("not-an-ip"));
        assert!(!is_routable_v4("example.com"));
        assert!(!is_routable_v4("999.999.999.999"));
        assert!(!is_routable_v4(""));
        assert!(!is_routable_v6("not-an-ip"));
        assert!(!is_address_routable("garbage:1234"));
        assert!(!is_address_routable("localhost:21531"));
        assert!(!is_address_routable("[garbage]:21531"));
        assert!(!is_address_routable("999.999.999.999:1"));
        assert!(!is_address_routable("10.0.0.1:notaport"));
        assert!(!is_address_routable("10.0.0.1:0"));
        assert!(!is_address_routable("10.0.0.1:99999"));
        assert!(!is_address_routable("[2001:db8::1]:junk"));
        assert!(!is_address_routable("0.0.0.0:21531"));
        assert!(!is_address_routable("255.255.255.255:21531"));
    }

    #[test]
    fn handles_bare_ipv6() {
        // Unbracketed IPv6 (no port): evaluate the whole address instead of
        // slicing off the last segment like a port.
        assert!(is_address_routable("2001:db8::1"));
        assert!(!is_address_routable("fe80::1"));
        assert!(!is_address_routable("fd00::1"));
        assert!(!is_address_routable("::1"));
        assert!(is_address_routable("2001:db8::1%eth0"));
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
    fn detects_loopback_addresses() {
        assert!(is_loopback_address("127.0.0.1:21531"));
        assert!(is_loopback_address("[::1]:21531"));
        assert!(is_loopback_address("127.0.0.1"));
        assert!(!is_loopback_address("192.168.1.70:21531"));
        assert!(!is_loopback_address("[2001:db8::1]:21531"));
        assert!(!is_loopback_address("0.0.0.0:21531"));
        assert!(!is_loopback_address("[::]:21531"));
        assert!(!is_loopback_address("garbage"));
        assert!(!is_loopback_address(""));
        assert!(!is_loopback_address("[fe80::1%en0]:21531"));
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
