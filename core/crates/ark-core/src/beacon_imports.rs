// Syncthing-style UDP broadcast peer discovery.
//
// Each peer periodically sends a JSON beacon on UDP port `LAN_SYNC_PORT + 1`
// (21532) containing `{t, s, d, n, p, a}`:
//   - `t` beacon type (always `"delphi"`)
//   - `s` space id (hashed, safe on the wire)
//   - `d` device id
//   - `n` device name
//   - `p` WS server port (typically 21531)
//   - `a` array of routable `ip:port` / `[ipv6]:port` strings
//
// Listeners receive beacons from other peers in the same space, deduplicate
// by `device_id` (30 s TTL), and invoke `on_peer_discovered` only when the
// device is new or its name / address set changed. Self-beacons are
// rejected. Every address the module announces or accepts is first run
// through `net::is_address_routable`.
//
// Wire-compatible with `apps/delphi/ts/electron/broadcast-discovery.ts` and
// `apps/delphi/kotlin/.../BroadcastDiscovery.kt`.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use tokio::time::interval;

use crate::host::{get_own_addresses, strip_ipv6_zone};
use crate::net::{is_address_routable, is_routable_v4, is_virtual_interface};
use crate::protocol::LAN_SYNC_PORT;

const TAG: &str = "[Beacon]";
pub const BEACON_PORT: u16 = LAN_SYNC_PORT + 1; // 21532
pub const BEACON_INTERVAL_MS: u64 = 5_000;
pub const BEACON_TYPE: &str = "delphi";
pub const PEER_TTL_MS: u64 = 30_000;

// ---------------------------------------------------------------------------
// Wire type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconPayload {
    pub t: String,
    pub s: String,
    pub d: String,
    pub n: String,
    pub p: u16,
    pub a: Vec<String>,
}

// ---------------------------------------------------------------------------
// Callback payload
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeaconPeer {
    pub device_id: String,
    pub device_name: String,
    /// Primary `ip:wsPort` derived from the sender's UDP source address.
    pub address: String,
    /// All known addresses — merge of sender IP + beacon payload's `a`.
    pub addresses: Vec<String>,
}

pub type OnPeerDiscoveredCallback = Arc<dyn Fn(BeaconPeer) + Send + Sync>;

// ---------------------------------------------------------------------------
// Seen-peer dedup state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SeenPeer {
    device_name: String,
    sorted_addresses: Vec<String>,
    last_seen_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Decide whether an inbound beacon should fire the discovery callback.
/// Returns `(should_fire, new_state)`. Centralised so unit tests can exercise
/// the decision without touching UDP sockets.
fn classify_beacon(
    existing: Option<&SeenPeer>,
    incoming_name: &str,
    sorted_addrs: &[String],
    now_ms_val: u64,
) -> (bool, SeenPeer) {
    let new_state = SeenPeer {
        device_name: incoming_name.to_string(),
        sorted_addresses: sorted_addrs.to_vec(),
        last_seen_ms: now_ms_val,
    };

    let should_fire = match existing {
        None => true,
        Some(prev) => prev.device_name != incoming_name || prev.sorted_addresses != sorted_addrs,
    };

    (should_fire, new_state)
}

// ---------------------------------------------------------------------------
// Control messages
// ---------------------------------------------------------------------------

enum Control {
    Stop,
    UpdateOptions {
        space_id: Option<String>,
        device_id: Option<String>,
        device_name: Option<String>,
        ws_port: Option<u16>,
    },
}

// ---------------------------------------------------------------------------
