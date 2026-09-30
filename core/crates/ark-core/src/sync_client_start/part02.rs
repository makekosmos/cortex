async move {
            let mut reconnect_delay = RECONNECT_BASE_MS;

            loop {
                if stopped.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                let peer_record = peer.read().await.clone();
                let mut addresses = peer_record.addresses.clone();

                // Never dial our own listener: a stale record can carry an
                // address that now belongs to us (e.g. the peer's old DHCP
                // lease). Prune it from the stored record — a pure
                // self-reference still ends the client.
                if addresses.iter().any(|a| own_addresses.contains(a)) {
                    addresses.retain(|a| !own_addresses.contains(a));
                    peer.write().await.addresses = addresses.clone();
                    if addresses.is_empty() {
                        eprintln!(
                            "{TAG} All addresses for {} are ours — evicting self-record",
                            peer_record.device_name
                        );
                        stopped.store(true, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                }

                if addresses.is_empty() {
                    eprintln!(
                        "{TAG} No addresses for peer {}, scheduling reconnect",
                        peer_record.device_name
                    );
                    tokio::time::sleep(Duration::from_millis(reconnect_delay)).await;
                    reconnect_delay =
                        (reconnect_delay as f64 * 1.5).min(RECONNECT_MAX_MS as f64) as u64;
                    continue;
                }

                // Race connections to all addresses
                let result = race_connect(&addresses).await;

                include!("part03.rs");

                // Reconnect with backoff
                let jitter = rand::random::<f64>() * 1000.0;
                let delay = (reconnect_delay as f64 + jitter).min(RECONNECT_MAX_MS as f64);
                tokio::time::sleep(Duration::from_millis(delay as u64)).await;
                reconnect_delay =
                    ((reconnect_delay as f64) * 1.5).min(RECONNECT_MAX_MS as f64) as u64;
            }
}
