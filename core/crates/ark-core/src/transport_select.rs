//! Step 4a: pure transport-selection rule shared by the `ark-core-rpc`
//! JSON-RPC binary (`main.rs`) and the UniFFI facade (`ffi.rs`), so both
//! start-sync entry points pick `RelaySync`'s underlying `SyncTransport`
//! (relay WebSocket vs. iroh p2p QUIC, behind `iroh-spike`) the same way.
//!
//! Deliberately free of any transport/network types — this is a sync,
//! side-effect-free decision function so it's unit-testable without
//! standing up a transport, and so both call sites can share one source of
//! truth instead of duplicating the precedence rule.

/// Which transport `RelaySync` should be driven by, derived from the
/// request/config's relay/iroh fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportChoice {
    None,
    Relay,
    Iroh,
}

/// `use_iroh` wins over `relay_url` when both are supplied — iroh is the
/// transport explicitly being tested/selected; relay stays available as a
/// fallback configuration but is not "the more specific ask" once iroh is on.
pub fn select_transport(use_iroh: bool, relay_url: &Option<String>) -> TransportChoice {
    if use_iroh {
        TransportChoice::Iroh
    } else if relay_url.is_some() {
        TransportChoice::Relay
    } else {
        TransportChoice::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_transport_picks_none_when_nothing_configured() {
        assert_eq!(select_transport(false, &None), TransportChoice::None);
    }

    #[test]
    fn select_transport_picks_relay_when_relay_url_set() {
        assert_eq!(
            select_transport(false, &Some("ws://relay.example.com".to_string())),
            TransportChoice::Relay
        );
    }

    #[test]
    fn select_transport_picks_iroh_when_use_iroh_set() {
        assert_eq!(select_transport(true, &None), TransportChoice::Iroh);
    }

    #[test]
    fn select_transport_prefers_iroh_when_both_set() {
        assert_eq!(
            select_transport(true, &Some("ws://relay.example.com".to_string())),
            TransportChoice::Iroh
        );
    }
}
