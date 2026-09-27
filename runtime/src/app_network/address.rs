//! `shared/electron/public-network-address.ts` port — literal public-address
//! check used by every app-network fetch: DNS-pinned HTTPS must land on a
//! public address so an app cannot turn the Engine into an SSRF proxy.
use std::net::{Ipv4Addr, Ipv6Addr};

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, c, _d] = address.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let first = address.segments()[0];
    !(first == 0
        || address.is_unspecified()
        || address.is_loopback()
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
        || address.is_multicast()
        || (first == 0x2001 && address.segments()[1] == 0x0db8))
}

/// `isPublicNetworkAddress` — bracketed IPv6 accepted like the TS version.
pub(crate) fn is_public_network_address(input: &str) -> bool {
    let address = input
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_lowercase();
    if let Ok(ipv4) = address.parse::<Ipv4Addr>() {
        return is_public_ipv4(ipv4);
    }
    match address.parse::<Ipv6Addr>() {
        Ok(ipv6) => is_public_ipv6(ipv6),
        Err(_) => false,
    }
}
