/// Collapse iroh's home-relay status list into a connectivity transition.
/// Returns the new connected flag only when it changed — every retry writes
/// a fresh `Disconnected` error into the watcher, so repeated updates must
/// not re-log. An empty list (no home relay chosen yet) is not a state.
fn relay_connectivity_change(prev: Option<bool>, relays: &[bool]) -> Option<bool> {
    if relays.is_empty() {
        return prev;
    }
    let connected = relays.iter().any(|connected| *connected);
    (prev != Some(connected)).then_some(connected)
}

/// Spawn the watcher that logs one WARN per relay connectivity transition
/// (up→down / down→up). iroh's relay actor retries internally and warns per
/// attempt — the Engine filters those to error (`runtime::main::init_tracing`),
/// so this transition log is the signal that survives (KOS-298).
/// No-op when `relay_mode` is `Disabled` (tests bind loopback only).
fn spawn_relay_connectivity_watch(
    endpoint: &Endpoint,
    relay_mode: Option<&RelayMode>,
    mut stop_rx: watch::Receiver<bool>,
) {
    if matches!(relay_mode, Some(RelayMode::Disabled)) {
        return;
    }
    let mut watcher = endpoint.home_relay_status();
    tokio::spawn(async move {
        let mut connected: Option<bool> = None;
        loop {
            tokio::select! {
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() { break; }
                }
                update = watcher.updated() => {
                    if update.is_err() { break; }
                    let relays: Vec<bool> = watcher
                        .get()
                        .iter()
                        .map(|status| status.is_connected())
                        .collect();
                    let Some(next) = relay_connectivity_change(connected, &relays) else {
                        continue;
                    };
                    // The first observed value only records state —
                    // connecting at startup is not a flap.
                    if connected.is_some() {
                        if next {
                            tracing::warn!("iroh relay connectivity restored");
                        } else {
                            tracing::warn!("iroh relay connectivity lost");
                        }
                    }
                    connected = Some(next);
                }
            }
        }
    });
}
