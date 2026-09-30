//! Listener-bind choice for `start_sync`: the LAN default (all interfaces)
//! or loopback-only.
//!
//! Every listener the sync stack opens is derived from this single enum:
//! the WebSocket sync server, the iroh QUIC endpoint, and the UDP discovery
//! beacon. Windows raises a firewall prompt for each new binary that listens
//! on a non-loopback address, so test runs must bind loopback on every
//! socket — not just the ones that happen to collide today.

#[cfg(feature = "iroh-spike")]
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// Where `start_sync` binds the sockets it opens.
///
/// Carried typed through `Request::StartSync` / `FfiSyncConfig` /
/// `SyncStartParams` — never a string flag — so every bind site resolves its
/// address from the same value instead of re-deriving it.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum,
)]
#[serde(rename_all = "snake_case")]
pub enum SyncBind {
    /// Production default. LAN sync and beacon discovery must be reachable
    /// by other machines, so the WS server and the iroh endpoint listen on
    /// all interfaces and the beacon binds the wildcard UDP address.
    #[default]
    AllInterfaces,
    /// Every listener binds loopback only (127.0.0.1, ::1 where IPv6 is
    /// used). Advertised own addresses and iroh tickets point at loopback
    /// so same-host peers can still connect. Used by tests: nothing in a
    /// test process may ask the OS for a firewall exception.
    Loopback,
}

impl SyncBind {
    /// TCP bind address for the sync WebSocket server.
    pub fn ws_bind_addr(self, port: u16) -> String {
        match self {
            Self::AllInterfaces => format!("0.0.0.0:{port}"),
            Self::Loopback => format!("127.0.0.1:{port}"),
        }
    }

    /// Whether LAN beacon discovery runs in this mode. Broadcast datagrams
    /// cannot traverse loopback, so a loopback-bound beacon could never see
    /// a peer — loopback mode does not bind the beacon socket at all and
    /// relies on seed addresses / iroh tickets instead.
    pub fn discovery_supported(self) -> bool {
        matches!(self, Self::AllInterfaces)
    }

    /// `host:port` strings advertised to peers (hello / peer_list /
    /// known-peers). Must match an address this node actually listens on —
    /// loopback mode cannot advertise LAN addresses it is not bound to.
    pub fn own_addresses(self, port: u16) -> Vec<String> {
        match self {
            Self::AllInterfaces => crate::host::get_own_addresses(port),
            Self::Loopback => vec![format!("127.0.0.1:{port}")],
        }
    }

    /// Whether a candidate peer address is usable in this mode. Loopback
    /// mode never dials off-host addresses: its own sockets are unreachable
    /// from the LAN anyway, so LAN peer addresses can only produce doomed
    /// connect attempts.
    pub fn accepts_peer_address(self, addr: &str) -> bool {
        match self {
            Self::AllInterfaces => crate::net::is_address_routable(addr),
            Self::Loopback => crate::net::is_loopback_address(addr),
        }
    }

    /// UDP socket addresses the iroh endpoint should bind, as
    /// `(v4_required, v6_optional)`. iroh's builder comes pre-configured
    /// with unspecified binds for both families; binding an explicit
    /// address per family replaces that default.
    ///
    /// IPv6 is always optional (port 0, not required): hosts without an
    /// IPv6 stack must still start. Callers feed these into
    /// `Endpoint::builder().bind_addr(...)` / `bind_addr_with_opts(...)`.
    #[cfg(feature = "iroh-spike")]
    pub fn iroh_bind_addrs(self) -> (SocketAddr, SocketAddr) {
        match self {
            Self::AllInterfaces => (
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
                SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
            ),
            Self::Loopback => (
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The single source of truth for "loopback mode never binds off
    /// loopback": every bind address the sync stack can produce must
    /// resolve to 127.0.0.1 / ::1, and peer/own addresses must follow.
    #[test]
    fn loopback_mode_resolves_every_bind_to_loopback() {
        let bind = SyncBind::Loopback;
        let port = 21531;

        assert_eq!(bind.ws_bind_addr(port), "127.0.0.1:21531");
        assert!(!bind.discovery_supported());
        assert_eq!(bind.own_addresses(port), vec!["127.0.0.1:21531"]);
        for addr in bind.own_addresses(port) {
            assert!(
                bind.accepts_peer_address(&addr),
                "own addr rejected: {addr}"
            );
        }
        assert!(bind.accepts_peer_address("127.0.0.1:21531"));
        assert!(bind.accepts_peer_address("[::1]:21531"));
        assert!(!bind.accepts_peer_address("192.168.1.70:21531"));
        assert!(!bind.accepts_peer_address("[2001:db8::1]:21531"));
        assert!(!bind.accepts_peer_address("0.0.0.0:21531"));

        #[cfg(feature = "iroh-spike")]
        {
            let (v4, v6) = bind.iroh_bind_addrs();
            assert_eq!(v4.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
            assert_eq!(v6.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
        }
    }

    #[test]
    fn all_interfaces_keeps_lan_behaviour() {
        let bind = SyncBind::AllInterfaces;
        assert_eq!(bind.ws_bind_addr(21531), "0.0.0.0:21531");
        assert!(bind.discovery_supported());
        assert!(bind.accepts_peer_address("192.168.1.70:21531"));
        assert!(!bind.accepts_peer_address("127.0.0.1:21531"));

        #[cfg(feature = "iroh-spike")]
        {
            let (v4, v6) = bind.iroh_bind_addrs();
            assert_eq!(v4.ip(), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
            assert_eq!(v6.ip(), IpAddr::V6(Ipv6Addr::UNSPECIFIED));
        }
    }

    #[test]
    fn serde_wire_names_are_stable() {
        assert_eq!(
            serde_json::to_value(SyncBind::Loopback).unwrap(),
            serde_json::json!("loopback")
        );
        assert_eq!(
            serde_json::from_value::<SyncBind>(serde_json::json!("all_interfaces")).unwrap(),
            SyncBind::AllInterfaces
        );
        assert!(serde_json::from_value::<SyncBind>(serde_json::json!("bogus")).is_err());
    }
}
