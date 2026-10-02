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
        let (fire, _) = classify_beacon(Some(&prev), "alpha", &["1.1.1.1:21531".to_string()], now);
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
