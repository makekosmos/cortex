// Kepler владеет LAN sync (Phase 5 декларация). Reusable helpers — между
// headless `kepler-backend` binary и legacy desktop `kepler` binary. Изначально
// эта логика жила в apps/kepler/src/main.rs; в Phase 0 Kepler-Electron pivot
// вынесена в backend crate как часть extraction'а.

use std::path::Path;

use serde_json::json;

use crate::ark_host::ArkHost;
use crate::auth;

type DynError = Box<dyn std::error::Error + Send + Sync>;

pub fn resolve_space_id() -> String {
    std::env::var("KOSMOS_SPACE_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "kepler-default".to_string())
}

pub fn resolve_device_name() -> String {
    if let Ok(n) = std::env::var("KOSMOS_DEVICE_NAME") {
        if !n.is_empty() {
            return n;
        }
    }
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Kosmos Host".to_string())
}

pub fn resolve_device_id(lock_dir: &Path) -> std::io::Result<String> {
    if let Ok(d) = std::env::var("KOSMOS_DEVICE_ID") {
        if !d.is_empty() {
            return Ok(d);
        }
    }
    let id_path = lock_dir.join("kepler-device-id.txt");
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
    match std::env::var("KOSMOS_IROH") {
        Ok(flag) if flag == "0" || flag.eq_ignore_ascii_case("false") => false,
        Ok(flag) if flag == "1" || flag.eq_ignore_ascii_case("true") => true,
        Ok(_) => true,
        Err(_) => true,
    }
}

fn start_sync_unsupported_iroh_error(err: &str) -> bool {
    let err = err.to_ascii_lowercase();
    err.contains("iroh-spike")
        || err.contains("unsupported iroh transport")
        || err.contains("use_iroh")
        || err.contains("compiled without")
}

pub async fn start_lan_sync(
    ark: &ArkHost,
    space_id: &str,
    device_id: &str,
    device_name: &str,
) -> Result<(), DynError> {
    let mut params = json!({
        "space_id": space_id,
        "device_id": device_id,
        "device_name": device_name,
        "port": null,
        "seed_addresses": null,
        "use_iroh": resolve_use_iroh_by_default(),
    });

    if let Ok(url) = std::env::var("KOSMOS_RELAY_URL") {
        if !url.is_empty() {
            params["relay_url"] = serde_json::Value::String(url);
        }
    }
    if let Ok(key) = std::env::var("KOSMOS_RELAY_API_KEY") {
        if !key.is_empty() {
            params["relay_api_key"] = serde_json::Value::String(key);
        }
    }
    if let Ok(secret) = std::env::var("KOSMOS_AUTH_SECRET") {
        if !secret.is_empty() {
            params["auth_secret"] = serde_json::Value::String(secret);
        }
    }

    // Step 4a: iroh transport selection, mirrors KOSMOS_RELAY_URL above.
    // Reading these env vars is unconditional (cheap, no transport
    // construction here — this crate talks to the ark-core-rpc sidecar over
    // JSON-RPC, it never links iroh directly); they are only ACTED ON by
    // ark-core-rpc when it was built with the `iroh-spike` Rust feature. A
    // non-iroh-spike sidecar rejects start_sync with an explicit error if
    // KOSMOS_IROH=1 is set, rather than silently ignoring it.
    if let Ok(ticket) = std::env::var("KOSMOS_IROH_PEER_TICKET") {
        if !ticket.is_empty() {
            params["iroh_peer_ticket"] = serde_json::Value::String(ticket);
        }
    }

    let response = ark.request("start_sync", params.clone()).await?;
    if response.ok {
        return Ok(());
    }

    let error = response
        .error
        .clone()
        .unwrap_or_else(|| "(no error message)".to_string());
    if params
        .get("use_iroh")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        && start_sync_unsupported_iroh_error(&error)
    {
        tracing::warn!(error = %error, "iroh start_sync unsupported; retrying with LAN fallback");
        let mut fallback = params;
        fallback["use_iroh"] = serde_json::Value::Bool(false);
        let retry = ark.request("start_sync", fallback).await?;
        if retry.ok {
            return Ok(());
        }
        return Err(format!(
            "ark-core-rpc rejected fallback start_sync: {}",
            retry
                .error
                .unwrap_or_else(|| "(no error message)".to_string())
        )
        .into());
    }

    Err(format!("ark-core-rpc rejected start_sync: {error}").into())
}

/// Dev-flow cross-network step: when iroh was requested (`KOSMOS_IROH=1`),
/// fetch our own pairing ticket from the sidecar (`GetOwnIrohTicket`) and
/// print it BIG and unmistakable to stderr so the dev can copy it into the
/// other machine's `KOSMOS_IROH_PEER_TICKET`. No-op (and cheap — no RPC
/// call) when iroh wasn't requested, so the default dev flow is unaffected.
///
/// Call this only after `start_lan_sync` returns `Ok(())` — `GetOwnIrohTicket`
/// returns `null` until sync has actually started with iroh selected (see
/// `ark-core-rpc`'s `handle_get_own_iroh_ticket`).
pub async fn print_iroh_pairing_code_if_enabled(ark: &ArkHost) {
    let iroh_requested = std::env::var("KOSMOS_IROH")
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
                    "KOSMOS_IROH set, but get_own_iroh_ticket returned no ticket \
                         (sidecar likely built without --features iroh-spike, or sync \
                         did not select the iroh transport)"
                );
            }
        },
        Ok(response) => {
            tracing::warn!(
                error = ?response.error,
                "get_own_iroh_ticket rejected by ark-core-rpc"
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "get_own_iroh_ticket request failed");
        }
    }
}
