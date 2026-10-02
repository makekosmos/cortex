//! Listener bind choice at boot (KOS-269)
//!
//! Windows Defender Firewall prompts — per exe path — as soon as a process
//! listens on a non-loopback address, and the Engine exe lives under a
//! versioned install dir, so every update would re-prompt. Two guards:
//!
//!   * privileges enabled → the service keeps an inbound allow rule pointed
//!     at the current exe (see `privileged::firewall`), and we keep the LAN
//!     bind unconditionally;
//!   * no privileges → bind loopback until sync is actually *on*: a paired
//!     device exists (ark-core persists them under `sync.peers`) or the user
//!     explicitly provided a peer ticket this boot. Pairing a device through
//!     `connect_with_pairing_code` re-binds to all interfaces inside
//!     ark-core — that is the one moment the firewall prompt is expected.
//!
//! Tests never bind off-loopback: `SyncBind::Loopback` exists for that.

use ark_core::SyncBind;
use serde_json::{json, Value};

use crate::ark_host::ArkHost;

const KNOWN_PEERS_KEY: &str = "sync.peers";
const REMOVED_PEERS_KEY: &str = "sync.removed_peers";

/// Pure decision: where the sync stack may bind at boot.
/// `sync_enabled` — a paired device exists or an explicit peer ticket was
/// configured; `firewall_rule_ok` — the privileged service refreshed the
/// managed allow rule for this exe.
pub fn boot_bind(sync_enabled: bool, firewall_rule_ok: bool) -> SyncBind {
    if sync_enabled || firewall_rule_ok {
        SyncBind::AllInterfaces
    } else {
        SyncBind::Loopback
    }
}

/// Does the persisted `sync.peers` set contain a real pairing — i.e. a peer
/// that is neither us nor one the user explicitly disconnected?
pub fn has_paired_peers(
    known_peers_raw: Option<&str>,
    removed_peers_raw: Option<&str>,
    self_device_id: &str,
) -> bool {
    let known: Vec<Value> = known_peers_raw
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    let removed: Vec<String> = removed_peers_raw
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    known.iter().any(|peer| {
        let device_id = peer.get("device_id").and_then(Value::as_str);
        device_id.is_some_and(|id| id != self_device_id && !removed.iter().any(|r| r == id))
    })
}

async fn get_sync_kv(ark: &ArkHost, key: &str) -> Option<String> {
    let resp = ark
        .request("get_sync_kv", json!({ "key": key }))
        .await
        .ok()?;
    if !resp.ok {
        return None;
    }
    resp.data.as_str().map(str::to_owned)
}

/// Ask the privileged service to refresh the firewall rule for this exe.
/// `false` covers every non-fatal outcome: not installed, older service
/// (unknown-variant error), or a refused request — the caller falls back to
/// the unprivileged bind policy.
async fn ensure_firewall_rule_via_service() -> bool {
    let result = tokio::task::spawn_blocking(|| {
        let status = crate::privileged::client::status();
        if !(status.installed && status.pipe_ok) {
            return Ok(false);
        }
        crate::privileged::client::ensure_engine_firewall_rule().map(|()| true)
    })
    .await;
    match result {
        Ok(Ok(ok)) => ok,
        Ok(Err(e)) => {
            tracing::info!(
                error = %e,
                "privileged service could not refresh the Engine firewall rule \
                 (older service?); using unprivileged bind policy"
            );
            false
        }
        Err(e) => {
            tracing::warn!(error = %e, "firewall-rule task join failed");
            false
        }
    }
}

/// Resolve where sync may bind this boot. See the module notes above.
pub async fn lan_bind_at_boot(ark: &ArkHost, self_device_id: &str) -> SyncBind {
    if ensure_firewall_rule_via_service().await {
        return SyncBind::AllInterfaces;
    }
    // An explicit iroh ticket for this boot is an explicit opt-in to
    // LAN-reachable sync right now.
    let explicit_peer = crate::brand::env("IROH_PEER_TICKET").is_some_and(|t| !t.trim().is_empty());
    let paired = has_paired_peers(
        get_sync_kv(ark, KNOWN_PEERS_KEY).await.as_deref(),
        get_sync_kv(ark, REMOVED_PEERS_KEY).await.as_deref(),
        self_device_id,
    );
    boot_bind(paired || explicit_peer, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// KOS-269 regression: an Engine whose user never paired a device and
    /// has no privileged firewall rule must not ask the OS for a firewall
    /// exception at boot.
    #[test]
    fn sync_not_enabled_binds_loopback() {
        assert_eq!(boot_bind(false, false), SyncBind::Loopback);
        assert_eq!(SyncBind::Loopback.ws_bind_addr(21531), "127.0.0.1:21531");
        assert!(!SyncBind::Loopback.discovery_supported());
    }

    #[test]
    fn paired_or_rule_ok_binds_lan() {
        assert_eq!(boot_bind(true, false), SyncBind::AllInterfaces);
        assert_eq!(boot_bind(false, true), SyncBind::AllInterfaces);
        assert_eq!(boot_bind(true, true), SyncBind::AllInterfaces);
    }

    #[test]
    fn paired_peers_ignores_self_and_removed() {
        let known = serde_json::json!([
            {"device_id": "self", "device_name": "Me"},
            {"device_id": "laptop", "device_name": "Laptop"},
        ])
        .to_string();
        assert!(has_paired_peers(Some(&known), None, "self"));

        let removed = serde_json::json!(["laptop"]).to_string();
        // The only real peer was disconnected — back to "sync not enabled".
        assert!(!has_paired_peers(Some(&known), Some(&removed), "self"));

        let only_self = serde_json::json!([{ "device_id": "self" }]).to_string();
        assert!(!has_paired_peers(Some(&only_self), None, "self"));
        assert!(!has_paired_peers(None, None, "self"));
        assert!(!has_paired_peers(Some("not json"), None, "self"));
    }
}
