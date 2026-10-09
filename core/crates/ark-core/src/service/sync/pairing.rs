//! Pairing — the two explicit "turn sync on" transitions (KOS-269):
//! entering a peer's code and showing our own. Both escalate a
//! loopback-bound runtime to `AllInterfaces` and the iroh transport; the
//! restart lives here so the firewall prompt is owned by a user action, not
//! by boot.

use super::*;

/// Restart params for the explicit "turn sync on" transitions: both force
/// the iroh transport (pairing codes are iroh tickets) and the
/// all-interfaces bind — a loopback endpoint can neither reach a remote
/// ticket nor hand out one that remote devices can reach (it would
/// advertise 127.0.0.1). The escalation is also the moment the Windows
/// firewall prompt legitimately appears for an unprivileged Engine.
pub(crate) fn lan_opt_in_params(
    runtime: &SyncRuntime,
    peer_ticket: Option<String>,
) -> SyncStartParams {
    let mut params = runtime.start_params.clone();
    params.use_iroh = true;
    params.iroh_peer_ticket = peer_ticket;
    params.bind = SyncBind::AllInterfaces;
    params
}

pub(crate) fn build_pairing_restart_params(
    runtime: &SyncRuntime,
    pairing_code: &str,
) -> SyncStartParams {
    lan_opt_in_params(runtime, Some(pairing_code.trim().to_string()))
}

/// Stop the running sync and restart it with `params`; on failure restore
/// `restore`. Shared by `connect_with_pairing_code` and `show_pairing_code`.
async fn restart_sync_with_restore(
    state: &Arc<ServiceState>,
    params: SyncStartParams,
    restore: SyncStartParams,
    op_name: &str,
) -> Result<Value, String> {
    handle_stop_sync(state).await;
    match handle_start_sync_with_params(state, params).await {
        Ok(result) => Ok(result),
        Err(err) => {
            if let Err(restore_err) = handle_start_sync_with_params(state, restore).await {
                return Err(format!(
                    "{op_name} failed: {err}; restoring previous sync also failed: {restore_err}"
                ));
            }
            Err(format!("{op_name} failed: {err}; previous sync restored"))
        }
    }
}

/// How long `connect_with_pairing_code` waits for the peer's Hello before
/// reporting failure. The dial itself is async — without this wait the op
/// returned Ok the moment the transport restarted and a peer that never
/// answered left the UI showing literally nothing (KOS-367). Bounded below
/// the Manager's 15s RPC timeout: ticket decode + restart (which includes
/// the endpoint bind and the `our_ticket` relay-homing wait) can already
/// consume several seconds.
const PAIRING_CONNECT_BUDGET: std::time::Duration = std::time::Duration::from_secs(12);

pub(crate) async fn handle_connect_with_pairing_code(
    state: &Arc<ServiceState>,
    pairing_code: String,
) -> Result<Value, String> {
    let code = pairing_code.trim();
    if code.is_empty() {
        return Err("pairing code is empty".to_string());
    }

    let runtime = {
        let guard = state.sync.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return Err("Sync not running".to_string()),
        }
    };

    let restart_params = build_pairing_restart_params(&runtime, code);
    let restore_params = runtime.start_params.clone();

    let started_at = std::time::Instant::now();
    restart_sync_with_restore(
        state,
        restart_params,
        restore_params,
        "connect_with_pairing_code",
    )
    .await?;

    // Wait for the peer's Hello: the iroh dial runs in a background task,
    // so a successful restart says nothing about reachability. Poll the
    // fresh runtime's peer list until the budget expires; a timeout is a
    // real error for the UI while the dial loop keeps retrying in the
    // background, so a late peer still completes the pairing.
    loop {
        let runtime = {
            let guard = state.sync.lock().await;
            match guard.as_ref() {
                Some(r) => r.clone(),
                None => return Err("Sync not running".to_string()),
            }
        };
        let mut entries = runtime.server.get_connected_peer_entries().await;
        if let Some(relay) = runtime.relay.as_ref() {
            entries.extend(relay.get_connected_peer_entries().await);
        }
        if let Some(peer) = entries
            .into_iter()
            .find(|entry| entry.device_id != runtime.device_id)
        {
            return Ok(json!({
                "status": "connected",
                "device_id": peer.device_id,
                "device_name": peer.device_name,
            }));
        }
        if started_at.elapsed() >= PAIRING_CONNECT_BUDGET {
            return Err(
                "device did not respond — check the code and that the other device is online"
                    .to_string(),
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

/// `show_pairing_code` (KOS-269): the explicit LAN opt-in for the device
/// that *shows* its code. A loopback-booted sync runs iroh with
/// `RelayMode::Disabled` and would hand out a ticket advertising only
/// 127.0.0.1 — unreachable for the other device — so the bind is escalated
/// here first. Kept separate from `get_own_iroh_ticket`, which stays a
/// passive read for callers that only inspect the running runtime: only an
/// explicit user action may escalate the bind.
pub(crate) async fn handle_show_pairing_code(state: &Arc<ServiceState>) -> Result<Value, String> {
    let runtime = {
        let guard = state.sync.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return Err("Sync not running".to_string()),
        }
    };

    // Restart only when the current runtime cannot produce a LAN-reachable
    // ticket: loopback bind, or iroh wasn't the selected transport.
    let needs_restart =
        runtime.start_params.bind == SyncBind::Loopback || runtime.iroh_our_ticket.is_none();
    if needs_restart {
        let restore_params = runtime.start_params.clone();
        restart_sync_with_restore(
            state,
            lan_opt_in_params(&runtime, None),
            restore_params,
            "show_pairing_code",
        )
        .await?;
    }

    let ticket = state
        .sync
        .lock()
        .await
        .as_ref()
        .and_then(|r| r.iroh_our_ticket.clone());
    Ok(json!(ticket))
}
