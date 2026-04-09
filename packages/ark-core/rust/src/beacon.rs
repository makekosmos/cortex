//! Syncthing-style UDP broadcast peer discovery.
//!
//! Each peer periodically sends a JSON beacon on UDP port `LAN_SYNC_PORT + 1`
//! (21532) containing `{t, s, d, n, p, a}`:
//!   - `t` beacon type (always `"delphi"`)
//!   - `s` space id (hashed, safe on the wire)
//!   - `d` device id
//!   - `n` device name
//!   - `p` WS server port (typically 21531)
//!   - `a` array of routable `ip:port` / `[ipv6]:port` strings
//!
//! Listeners receive beacons from other peers in the same space, deduplicate
//! by `device_id` (30 s TTL), and invoke `on_peer_discovered` only when the
//! device is new or its name / address set changed. Self-beacons are
//! rejected. Every address the module announces or accepts is first run
//! through `net::is_address_routable`.
//!
//! Wire-compatible with `apps/delphi/ts/electron/broadcast-discovery.ts` and
//! `apps/delphi/kotlin/.../BroadcastDiscovery.kt`.

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
        Some(prev) => {
            prev.device_name != incoming_name || prev.sorted_addresses != sorted_addrs
        }
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
// BroadcastDiscovery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BroadcastDiscoveryOptions {
    pub space_id: String,
    pub device_id: String,
    pub device_name: String,
    pub ws_port: u16,
}

pub struct BroadcastDiscovery {
    control_tx: Mutex<Option<mpsc::UnboundedSender<Control>>>,
    on_peer_discovered: Mutex<Option<OnPeerDiscoveredCallback>>,
    seen_peers: Arc<Mutex<HashMap<String, SeenPeer>>>,
}

impl Default for BroadcastDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl BroadcastDiscovery {
    pub fn new() -> Self {
        Self {
            control_tx: Mutex::new(None),
            on_peer_discovered: Mutex::new(None),
            seen_peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn set_on_peer_discovered(&self, handler: OnPeerDiscoveredCallback) {
        *self.on_peer_discovered.lock().await = Some(handler);
    }

    /// Directly inject a beacon (used by unit tests and by the send loop's
    /// own self-loopback handling path). Public-for-tests.
    #[doc(hidden)]
    pub async fn process_inbound(
        &self,
        payload: &BeaconPayload,
        sender_ip: &str,
        own_space_id: &str,
        own_device_id: &str,
    ) {
        self.process_inbound_inner(payload, sender_ip, own_space_id, own_device_id, now_ms())
            .await;
    }

    async fn process_inbound_inner(
        &self,
        payload: &BeaconPayload,
        sender_ip: &str,
        own_space_id: &str,
        own_device_id: &str,
        now_val: u64,
    ) {
        if payload.t != BEACON_TYPE {
            return;
        }
        if payload.s != own_space_id {
            return;
        }
        if payload.d.is_empty() {
            return;
        }
        if payload.d == own_device_id {
            return; // self-reject
        }

        let port = if payload.p == 0 { LAN_SYNC_PORT } else { payload.p };
        let sender_addr = format!("{sender_ip}:{port}");

        // Merge: sender IP first, then beacon-provided addresses (deduplicated).
        // Also filter out any non-routable addresses the peer might have
        // accidentally advertised — we never want to act on a link-local or
        // virtual-iface address.
        let mut merged: Vec<String> = Vec::new();
        if is_address_routable(&sender_addr) {
            merged.push(sender_addr.clone());
        }
        for addr in &payload.a {
            if !is_address_routable(addr) {
                continue;
            }
            if !merged.contains(addr) {
                merged.push(addr.clone());
            }
        }

        if merged.is_empty() {
            // Nothing routable — nothing to emit.
            return;
        }

        let mut sorted = merged.clone();
        sorted.sort();

        let mut seen = self.seen_peers.lock().await;
        let existing = seen.get(&payload.d).cloned();
        let (should_fire, new_state) =
            classify_beacon(existing.as_ref(), &payload.n, &sorted, now_val);
        seen.insert(payload.d.clone(), new_state);
        drop(seen);

        if !should_fire {
            return;
        }

        let peer = BeaconPeer {
            device_id: payload.d.clone(),
            device_name: payload.n.clone(),
            address: sender_addr,
            addresses: merged,
        };

        if let Some(handler) = self.on_peer_discovered.lock().await.as_ref() {
            handler(peer);
        }
    }

    /// Evict peers whose `last_seen_ms` is older than `PEER_TTL_MS`.
    /// Returns the number of peers removed.
    pub async fn sweep_stale(&self) -> usize {
        let cutoff = now_ms().saturating_sub(PEER_TTL_MS);
        self.sweep_stale_at(cutoff).await
    }

    async fn sweep_stale_at(&self, cutoff_ms: u64) -> usize {
        let mut seen = self.seen_peers.lock().await;
        let before = seen.len();
        seen.retain(|_, peer| peer.last_seen_ms >= cutoff_ms);
        before - seen.len()
    }

    pub async fn is_running(&self) -> bool {
        self.control_tx.lock().await.is_some()
    }

    /// Current number of peers tracked in the dedup table (for tests / debug).
    pub async fn seen_peer_count(&self) -> usize {
        self.seen_peers.lock().await.len()
    }

    /// Start the UDP send + receive loops.
    pub async fn start(self: &Arc<Self>, options: BroadcastDiscoveryOptions) -> Result<(), String> {
        // Stop any running loops first.
        self.stop().await;
        self.seen_peers.lock().await.clear();

        let socket = UdpSocket::bind(SocketAddr::new(
            std::net::IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            BEACON_PORT,
        ))
        .await
        .map_err(|e| format!("Failed to bind UDP {BEACON_PORT}: {e}"))?;
        socket.set_broadcast(true).ok();
        let socket = Arc::new(socket);

        let (control_tx, mut control_rx) = mpsc::unbounded_channel::<Control>();
        *self.control_tx.lock().await = Some(control_tx);

        let options_arc = Arc::new(Mutex::new(options));
        let self_clone = self.clone();
        let options_for_loops = options_arc.clone();
        let socket_for_recv = socket.clone();
        let socket_for_send = socket.clone();
        let self_for_recv = self_clone.clone();
        let self_for_sweep = self_clone.clone();

        // Receive loop
        let recv_handle = tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            loop {
                match socket_for_recv.recv_from(&mut buf).await {
                    Ok((len, peer)) => {
                        let text = match std::str::from_utf8(&buf[..len]) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let payload: BeaconPayload = match serde_json::from_str(text) {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        let opts = options_for_loops.lock().await.clone();
                        let sender_ip = match peer.ip() {
                            std::net::IpAddr::V4(v4) => v4.to_string(),
                            std::net::IpAddr::V6(v6) => {
                                format!("[{}]", strip_ipv6_zone(&v6.to_string()))
                            }
                        };
                        self_for_recv
                            .process_inbound_inner(
                                &payload,
                                &sender_ip,
                                &opts.space_id,
                                &opts.device_id,
                                now_ms(),
                            )
                            .await;
                    }
                    Err(e) => {
                        eprintln!("{TAG} recv error: {e}");
                        break;
                    }
                }
            }
        });

        // Send loop
        let options_for_send = options_arc.clone();
        let send_handle = tokio::spawn(async move {
            // Send first beacon immediately to avoid waiting for the first tick.
            if let Err(e) = send_beacon_once(&socket_for_send, &options_for_send).await {
                eprintln!("{TAG} initial send error: {e}");
            }
            let mut ticker = interval(Duration::from_millis(BEACON_INTERVAL_MS));
            ticker.tick().await; // skip immediate first tick
            loop {
                ticker.tick().await;
                if let Err(e) = send_beacon_once(&socket_for_send, &options_for_send).await {
                    eprintln!("{TAG} send error: {e}");
                }
            }
        });

        // Sweep loop
        let sweep_handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(PEER_TTL_MS));
            ticker.tick().await;
            loop {
                ticker.tick().await;
                let _ = self_for_sweep.sweep_stale().await;
            }
        });

        // Control loop
        let options_for_control = options_arc.clone();
        tokio::spawn(async move {
            while let Some(msg) = control_rx.recv().await {
                match msg {
                    Control::Stop => {
                        recv_handle.abort();
                        send_handle.abort();
                        sweep_handle.abort();
                        break;
                    }
                    Control::UpdateOptions {
                        space_id,
                        device_id,
                        device_name,
                        ws_port,
                    } => {
                        let mut opts = options_for_control.lock().await;
                        if let Some(s) = space_id {
                            opts.space_id = s;
                        }
                        if let Some(d) = device_id {
                            opts.device_id = d;
                        }
                        if let Some(n) = device_name {
                            opts.device_name = n;
                        }
                        if let Some(p) = ws_port {
                            opts.ws_port = p;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn stop(&self) {
        if let Some(tx) = self.control_tx.lock().await.take() {
            let _ = tx.send(Control::Stop);
        }
        self.seen_peers.lock().await.clear();
    }

    pub async fn update_options(
        &self,
        space_id: Option<String>,
        device_id: Option<String>,
        device_name: Option<String>,
        ws_port: Option<u16>,
    ) {
        if let Some(tx) = self.control_tx.lock().await.as_ref() {
            let _ = tx.send(Control::UpdateOptions {
                space_id,
                device_id,
                device_name,
                ws_port,
            });
        }
        self.seen_peers.lock().await.clear();
    }
}

// ---------------------------------------------------------------------------
// Send helpers
// ---------------------------------------------------------------------------

async fn send_beacon_once(
    socket: &UdpSocket,
    options: &Mutex<BroadcastDiscoveryOptions>,
) -> Result<(), String> {
    let snapshot = options.lock().await.clone();
    let addresses = get_own_addresses(snapshot.ws_port);
    let payload = BeaconPayload {
        t: BEACON_TYPE.to_string(),
        s: snapshot.space_id,
        d: snapshot.device_id,
        n: snapshot.device_name,
        p: snapshot.ws_port,
        a: addresses,
    };
    let json = serde_json::to_string(&payload).map_err(|e| e.to_string())?;

    let targets = collect_broadcast_targets();
    for target in targets {
        let _ = socket.send_to(json.as_bytes(), target).await;
    }
    Ok(())
}

/// Enumerate broadcast targets: `255.255.255.255` plus each IPv4 interface's
/// calculated subnet broadcast address (`ip | ~netmask`).
fn collect_broadcast_targets() -> Vec<SocketAddr> {
    let mut targets: Vec<SocketAddr> = Vec::new();
    targets.push(SocketAddr::new(
        std::net::IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255)),
        BEACON_PORT,
    ));

    let ifaces = match if_addrs::get_if_addrs() {
        Ok(list) => list,
        Err(_) => return targets,
    };

    for iface in ifaces {
        if iface.is_loopback() {
            continue;
        }
        if is_virtual_interface(&iface.name) {
            continue;
        }
        if let if_addrs::IfAddr::V4(v4) = iface.addr {
            let ip = v4.ip.to_string();
            if !is_routable_v4(&ip) {
                continue;
            }
            let ip_octets = v4.ip.octets();
            let mask_octets = v4.netmask.octets();
            let mut broadcast_octets = [0u8; 4];
            for i in 0..4 {
                broadcast_octets[i] = ip_octets[i] | !mask_octets[i];
            }
            let broadcast_addr = SocketAddr::new(
                std::net::IpAddr::V4(Ipv4Addr::new(
                    broadcast_octets[0],
                    broadcast_octets[1],
                    broadcast_octets[2],
                    broadcast_octets[3],
                )),
                BEACON_PORT,
            );
            if !targets.contains(&broadcast_addr) {
                targets.push(broadcast_addr);
            }
        }
    }

    targets
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_payload(device_id: &str, name: &str, addrs: &[&str]) -> BeaconPayload {
        BeaconPayload {
            t: BEACON_TYPE.to_string(),
            s: "space-1".to_string(),
            d: device_id.to_string(),
            n: name.to_string(),
            p: 21531,
            a: addrs.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[tokio::test]
    async fn self_reject_by_device_id() {
        let disc = Arc::new(BroadcastDiscovery::new());
        let fired = Arc::new(AtomicUsize::new(0));
        let fired_clone = fired.clone();
        disc.set_on_peer_discovered(Arc::new(move |_peer| {
            fired_clone.fetch_add(1, Ordering::SeqCst);
        }))
        .await;

        let payload = make_payload("me", "self", &["192.168.1.10:21531"]);
        disc.process_inbound(&payload, "192.168.1.10", "space-1", "me")
            .await;

        assert_eq!(fired.load(Ordering::SeqCst), 0, "self beacon must not fire");
    }

    #[tokio::test]
    async fn dedup_repeat_unchanged_beacon() {
        let disc = Arc::new(BroadcastDiscovery::new());
        let fired = Arc::new(AtomicUsize::new(0));
        let fired_clone = fired.clone();
        disc.set_on_peer_discovered(Arc::new(move |_peer| {
            fired_clone.fetch_add(1, Ordering::SeqCst);
        }))
        .await;

        let payload = make_payload("peer-a", "Alpha", &["192.168.1.50:21531"]);
        disc.process_inbound(&payload, "192.168.1.50", "space-1", "me")
            .await;
        disc.process_inbound(&payload, "192.168.1.50", "space-1", "me")
            .await;
        disc.process_inbound(&payload, "192.168.1.50", "space-1", "me")
            .await;

        assert_eq!(
            fired.load(Ordering::SeqCst),
            1,
            "duplicate beacons must not re-fire callback",
        );
    }

    #[tokio::test]
    async fn re_fire_when_addresses_change() {
        let disc = Arc::new(BroadcastDiscovery::new());
        let fired = Arc::new(AtomicUsize::new(0));
        let fired_clone = fired.clone();
        disc.set_on_peer_discovered(Arc::new(move |_peer| {
            fired_clone.fetch_add(1, Ordering::SeqCst);
        }))
        .await;

        let p1 = make_payload("peer-b", "Beta", &["192.168.1.60:21531"]);
        disc.process_inbound(&p1, "192.168.1.60", "space-1", "me")
            .await;

        let p2 = make_payload("peer-b", "Beta", &["192.168.1.61:21531"]);
        disc.process_inbound(&p2, "192.168.1.61", "space-1", "me")
            .await;

        assert_eq!(
            fired.load(Ordering::SeqCst),
            2,
            "changed address list must re-fire callback",
        );
    }

    #[tokio::test]
    async fn stale_eviction_after_ttl() {
        let disc = Arc::new(BroadcastDiscovery::new());
        let fired = Arc::new(AtomicUsize::new(0));
        let fired_clone = fired.clone();
        disc.set_on_peer_discovered(Arc::new(move |_peer| {
            fired_clone.fetch_add(1, Ordering::SeqCst);
        }))
        .await;

        let payload = make_payload("peer-c", "Gamma", &["192.168.1.70:21531"]);
        disc.process_inbound(&payload, "192.168.1.70", "space-1", "me")
            .await;
        assert_eq!(disc.seen_peer_count().await, 1);

        // Force eviction by providing a cutoff far in the future.
        let evicted = disc.sweep_stale_at(u64::MAX).await;
        assert_eq!(evicted, 1);
        assert_eq!(disc.seen_peer_count().await, 0);

        // Next beacon fires again because the dedup state was cleared.
        disc.process_inbound(&payload, "192.168.1.70", "space-1", "me")
            .await;
        assert_eq!(fired.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn non_routable_addresses_are_dropped_from_merged() {
        let disc = Arc::new(BroadcastDiscovery::new());
        let captured: Arc<Mutex<Option<BeaconPeer>>> = Arc::new(Mutex::new(None));
        let captured_clone = captured.clone();
        disc.set_on_peer_discovered(Arc::new(move |peer| {
            let captured_clone = captured_clone.clone();
            tokio::spawn(async move {
                *captured_clone.lock().await = Some(peer);
            });
        }))
        .await;

        // Beacon contains a link-local, a loopback, and a routable address.
        let payload = make_payload(
            "peer-d",
            "Delta",
            &[
                "169.254.1.1:21531",
                "127.0.0.1:21531",
                "192.168.1.80:21531",
                "[fe80::1]:21531",
            ],
        );
        disc.process_inbound(&payload, "192.168.1.80", "space-1", "me")
            .await;

        // Allow the callback to run.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let guard = captured.lock().await;
        let peer = guard.as_ref().expect("callback should have fired");
        for a in &peer.addresses {
            assert!(
                is_address_routable(a),
                "non-routable address {a} leaked into merged list",
            );
        }
    }

    #[test]
    fn classify_beacon_new_peer_fires() {
        let now = 100;
        let (fire, _) = classify_beacon(None, "alpha", &["1.1.1.1:21531".to_string()], now);
        assert!(fire);
    }

    #[test]
    fn classify_beacon_same_fires_false() {
        let now = 200;
        let prev = SeenPeer {
            device_name: "alpha".to_string(),
            sorted_addresses: vec!["1.1.1.1:21531".to_string()],
            last_seen_ms: 100,
        };
        let (fire, _) = classify_beacon(
            Some(&prev),
            "alpha",
            &["1.1.1.1:21531".to_string()],
            now,
        );
        assert!(!fire);
    }

    #[test]
    fn classify_beacon_name_change_refires() {
        let now = 200;
        let prev = SeenPeer {
            device_name: "alpha".to_string(),
            sorted_addresses: vec!["1.1.1.1:21531".to_string()],
            last_seen_ms: 100,
        };
        let (fire, _) = classify_beacon(
            Some(&prev),
            "alpha-renamed",
            &["1.1.1.1:21531".to_string()],
            now,
        );
        assert!(fire);
    }

    #[test]
    fn beacon_payload_roundtrip() {
        let p = BeaconPayload {
            t: "delphi".to_string(),
            s: "space-1".to_string(),
            d: "dev-1".to_string(),
            n: "Device One".to_string(),
            p: 21531,
            a: vec!["192.168.1.1:21531".to_string()],
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"t\":\"delphi\""));
        assert!(json.contains("\"s\":\"space-1\""));
        let back: BeaconPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.d, "dev-1");
        assert_eq!(back.a, vec!["192.168.1.1:21531".to_string()]);
    }
}
