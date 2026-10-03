// Mundus Engine владеет LAN sync (Phase 5 декларация). Reusable helpers.
// Изначально эта логика жила в Electron shell; в Phase 0 pivot вынесена
// в backend crate как часть extraction'а.

use std::path::Path;

use ark_core::SyncBind;
use serde_json::{json, Value};

use crate::ark_host::ArkHost;
use crate::auth;

type DynError = Box<dyn std::error::Error + Send + Sync>;

pub fn resolve_space_id() -> String {
    crate::brand::env("SPACE_ID")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "mundus-default".to_string())
}

pub fn resolve_device_name() -> String {
    if let Some(n) = crate::brand::env("DEVICE_NAME") {
        if !n.is_empty() {
            return n;
        }
    }
    crate::device_name::system_device_name()
}

pub fn resolve_device_id(lock_dir: &Path) -> std::io::Result<String> {
    if let Some(d) = crate::brand::env("DEVICE_ID") {
        if !d.is_empty() {
            return Ok(d);
        }
    }
    let id_path = lock_dir.join("mundus-device-id.txt");
    if !id_path.exists() {
        // MIGRATION(KOS-267): remove after 2026-11-01. The device id written by
        // MIGRATION(KOS-267): the Kosmos-era Engine travels with the renamed data dir under its old
        // file name; adopt it in place so the sync identity survives the
        // rebrand.
        let legacy_path = lock_dir.join("kepler-device-id.txt");
        if legacy_path.exists() {
            std::fs::rename(&legacy_path, &id_path)?;
        }
    }
    if id_path.exists() {
        let raw = std::fs::read_to_string(&id_path)?;
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    let token = auth::generate_token();
    let short: String = token.chars().take(16).collect();
    std::fs::create_dir_all(lock_dir)?;
    std::fs::write(&id_path, &short)?;
    Ok(short)
}

fn resolve_use_iroh_by_default() -> bool {
    !matches!(
        crate::brand::env("IROH").as_deref(),
        Some(flag) if flag == "0" || flag.eq_ignore_ascii_case("false")
    )
}

// Boot-time listener bind choice (KOS-269) lives in `sync::bind`.
mod bind;
pub use bind::lan_bind_at_boot;

pub async fn start_lan_sync(
    ark: &ArkHost,
    space_id: &str,
    device_id: &str,
    device_name: &str,
    bind: SyncBind,
) -> Result<(), DynError> {
    let mut params = json!({
        "space_id": space_id,
        "device_id": device_id,
        "device_name": device_name,
        "port": null,
        "seed_addresses": null,
        "use_iroh": resolve_use_iroh_by_default(),
        "bind": serde_json::to_value(bind).unwrap_or(Value::Null),
    });

    if let Some(url) = crate::brand::env("RELAY_URL") {
        if !url.is_empty() {
            params["relay_url"] = serde_json::Value::String(url);
        }
    }
    if let Some(key) = crate::brand::env("RELAY_API_KEY") {
        if !key.is_empty() {
            params["relay_api_key"] = serde_json::Value::String(key);
        }
    }
    if let Some(secret) = crate::brand::env("AUTH_SECRET") {
        if !secret.is_empty() {
            params["auth_secret"] = serde_json::Value::String(secret);
        }
    }

    // iroh transport selection, mirrors MUNDUS_RELAY_URL above. The env vars
    // are only read here (cheap — this crate talks to the in-process ARK
    // service over JSON-RPC and never links iroh); the embedded ark-core,
    // always built with the iroh transport, acts on them in start_sync.
    if let Some(ticket) = crate::brand::env("IROH_PEER_TICKET") {
        if !ticket.is_empty() {
            params["iroh_peer_ticket"] = serde_json::Value::String(ticket);
        }
    }

    let response = ark.request("start_sync", params).await?;
    if response.ok {
        return Ok(());
    }

    let error = response
        .error
        .clone()
        .unwrap_or_else(|| "(no error message)".to_string());
    Err(format!("ark-core rejected start_sync: {error}").into())
}

/// Dev-flow cross-network step: when iroh was requested (`MUNDUS_IROH=1`),
/// fetch our own pairing ticket from the service (`GetOwnIrohTicket`) and
/// print it BIG and unmistakable to stderr so the dev can copy it into the
/// other machine's `MUNDUS_IROH_PEER_TICKET`. No-op (and cheap — no RPC
/// call) when iroh wasn't requested, so the default dev flow is unaffected.
///
/// Call this only after `start_lan_sync` returns `Ok(())` — `GetOwnIrohTicket`
/// returns `null` until sync has actually started with iroh selected (see
/// the service's `handle_get_own_iroh_ticket`).
pub async fn print_iroh_pairing_code_if_enabled(ark: &ArkHost) {
    let iroh_requested = crate::brand::env("IROH")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !iroh_requested {
        return;
    }

    match ark.request("get_own_iroh_ticket", json!({})).await {
        Ok(response) if response.ok => match response.data.as_str() {
            Some(ticket) if !ticket.is_empty() => {
                let banner = "=".repeat(ticket.len().max(24) + 22);
                eprintln!("\n{banner}");
                eprintln!("=== IROH PAIRING CODE: {ticket} ===");
                eprintln!("{banner}\n");
            }
            _ => {
                tracing::warn!(
                    "MUNDUS_IROH set, but get_own_iroh_ticket returned no ticket \
                         (sync did not select the iroh transport)"
                );
            }
        },
        Ok(response) => {
            tracing::warn!(
                error = ?response.error,
                "get_own_iroh_ticket rejected by ark-core"
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "get_own_iroh_ticket request failed");
        }
    }
}
