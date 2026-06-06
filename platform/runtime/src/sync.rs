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

    let response = ark.request("start_sync", params).await?;
    if !response.ok {
        return Err(format!(
            "ark-core-rpc rejected start_sync: {}",
            response
                .error
                .unwrap_or_else(|| "(no error message)".to_string())
        )
        .into());
    }
    Ok(())
}
