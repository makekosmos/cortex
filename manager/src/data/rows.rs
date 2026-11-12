//! What a page needs from an Engine answer, and nothing more.
//!
//! One function per answer kind: the pages read these keys and no others, so a
//! change in the Engine's shape is absorbed here instead of in the layout.

use serde_json::{json, Value};

pub(crate) fn sync(data: &Value) -> Value {
    json!({
        "running": data.get("running").and_then(Value::as_bool).unwrap_or(false),
        "incoming": array(data, &["/incoming_pairing_requests"]).iter().map(|request| json!({
            "id": text(request, &["/device_id"]).unwrap_or_default(),
            "name": text(request, &["/device_name"])
                .unwrap_or_else(|| "Новое устройство".into()),
            "platform": text(request, &["/platform"]).unwrap_or_default(),
            "appVersion": text(request, &["/app_version"]).unwrap_or_default(),
        })).collect::<Vec<_>>(),
        "peers": array(data, &["/peers"]).iter().map(device_row).collect::<Vec<_>>(),
    })
}

/// The sync snapshot, as the poller writes it: peers and incoming requests
/// together, under the one `sync` key the pages read them from.
pub fn sync_view(snapshot: &Value) -> Value {
    sync(snapshot)
}

pub(crate) fn array(data: &Value, paths: &[&str]) -> Vec<Value> {
    if let Some(items) = data.as_array() {
        return items.clone();
    }
    for path in paths {
        if let Some(items) = data.pointer(path).and_then(Value::as_array) {
            return items.clone();
        }
    }
    Vec::new()
}

pub(crate) fn text(value: &Value, paths: &[&str]) -> Option<String> {
    paths
        .iter()
        .find_map(|path| value.pointer(path).and_then(Value::as_str))
        .map(str::to_owned)
}

pub(crate) fn first(value: &Value, paths: &[&str]) -> Option<Value> {
    paths.iter().find_map(|path| value.pointer(path)).cloned()
}

pub(crate) fn bool_at(value: &Value, paths: &[&str]) -> bool {
    paths
        .iter()
        .find_map(|path| value.pointer(path).and_then(Value::as_bool))
        .unwrap_or(false)
}

pub(crate) fn model_row(model: &Value) -> Value {
    // The Engine states a model's weight in megabytes (`sizeMb`), not bytes.
    let megabytes = first(model, &["/sizeMb", "/size_mb"]).and_then(|value| value.as_f64());
    json!({
        "id": text(model, &["/id", "/name", "/model_id"]).unwrap_or_default(),
        "name": text(model, &["/name", "/display_name", "/id"]).unwrap_or_default(),
        "size": match megabytes {
            Some(megabytes) => size((megabytes * 1_000_000.0) as u64),
            None => text(model, &["/size_label"]).unwrap_or_default(),
        },
        "downloaded": bool_at(model, &["/downloaded"]),
        "downloading": bool_at(model, &["/downloading"]),
        "selected": bool_at(model, &["/selected"]),
        "percent": model.get("downloadPercent").and_then(Value::as_f64).unwrap_or(0.0),
    })
}

/// `dictation.get_config` exposes no secrets — only whether one is stored.
pub(crate) fn keys(data: &Value) -> Value {
    json!({
        "hasApiKey": bool_at(data, &["/hasApiKey", "/has_api_key"]),
        "engine": text(data, &["/config/localEngine"]).unwrap_or_default(),
        "language": text(data, &["/config/language"]).unwrap_or_default(),
        "model": text(data, &["/config/localModelId", "/config/localModel"]).unwrap_or_default(),
    })
}

pub(crate) fn device_row(peer: &Value) -> Value {
    json!({
        "id": text(peer, &["/device_id", "/id"]).unwrap_or_default(),
        "name": text(peer, &["/device_name", "/name"])
            .unwrap_or_else(|| "Без имени".into()),
        "status": match text(peer, &["/status"]).as_deref() {
            Some("online") => "online",
            _ => "offline",
        },
        "platform": text(peer, &["/platform"]).unwrap_or_default(),
        "current": peer.get("current").and_then(Value::as_bool).unwrap_or(false),
    })
}

pub(crate) fn connector_row(provider: &Value) -> Value {
    json!({
        "id": text(provider, &["/id", "/provider", "/name"]).unwrap_or_default(),
        "name": text(provider, &["/name", "/display_name", "/label"]).unwrap_or_default(),
        "connected": bool_at(provider, &["/connected", "/authorized", "/has_credential"]),
        "kind": text(provider, &["/kind", "/type", "/category"]).unwrap_or_default(),
        "detail": text(provider, &["/status", "/detail", "/account"]).unwrap_or_default(),
    })
}

pub(crate) fn backup_row(backup: &Value) -> Value {
    let bytes = first(backup, &["/size_bytes", "/bytes"]).and_then(|value| value.as_u64());
    json!({
        "id": text(backup, &["/id"]).unwrap_or_default(),
        "size": bytes.map(size).unwrap_or_default(),
        "modified": backup.get("modified_ms").and_then(Value::as_u64).unwrap_or_default(),
    })
}

/// `updater.status`: what the release channel offers and how far a download is.
pub(crate) fn updates(data: &Value) -> Value {
    json!({
        "state": text(data, &["/state"]).unwrap_or_else(|| "idle".into()),
        "currentVersion": text(data, &["/currentVersion", "/current_version"]).unwrap_or_default(),
        "newVersion": text(data, &["/newVersion", "/new_version"]).unwrap_or_default(),
        "channel": text(data, &["/channel"]).unwrap_or_default(),
        "canInstall": bool_at(data, &["/canInstall", "/can_install"]),
        "percent": data.get("percent").and_then(Value::as_f64).unwrap_or(0.0),
    })
}

/// `engine.settings.get` and the two switches the Settings page shows. The
/// Engine answers `{settings: {autostart, usage_tracker_enabled}}`; one call may
/// also carry only one of them, so both are read leniently.
pub(crate) fn settings(data: &Value) -> Value {
    // `autostart` is not read here: `engine.autostart.get` holds that switch,
    // and this answer does not mention it. Writing it anyway would drop the
    // other call's answer to `false`.
    json!({
        "telemetry": bool_at(
            data,
            // `engine.settings.get` answers `{"usage_tracker": {"enabled": …}}`;
            // the flat keys are what an older file on disk holds.
            &[
                "/usage_tracker/enabled",
                "/settings/usage_tracker_enabled",
                "/usage_tracker_enabled",
                "/telemetry",
            ],
        ),
    })
}

pub(crate) fn usage(data: &Value) -> Value {
    let minutes = |milliseconds: u64| milliseconds / 60_000;
    let trend = array(data, &["/dailyTrend", "/daily_trend"]);
    let day_ms = |row: &Value| {
        first(row, &["/foregroundMs", "/foreground_ms"])
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    };
    let today = trend.last().map(day_ms).unwrap_or_default();
    let week: u64 = trend.iter().rev().take(7).map(&day_ms).sum();
    let top: Vec<Value> = array(data, &["/topApps", "/top_apps"])
        .iter()
        .filter_map(|row| {
            let name = text(row, &["/displayName", "/name", "/app_name"])?;
            let milliseconds = first(row, &["/foregroundMs", "/foreground_ms"])
                .and_then(|value| value.as_u64())?;
            Some(json!({ "name": name, "minutes": minutes(milliseconds) }))
        })
        .collect();
    json!({
        "todayMinutes": minutes(today),
        "weekMinutes": minutes(week),
        "top": top,
    })
}

pub(crate) fn storage(data: &Value) -> Value {
    // The Engine lists what the data folder holds, part by part — the page
    // draws that list as a bar and as rows, so the biggest part is obvious.
    let mut parts: Vec<Value> = array(data, &["/categories", "/parts"])
        .iter()
        .filter_map(|part| {
            let bytes = part.get("bytes").and_then(Value::as_u64)?;
            let mut detail: Vec<Value> = array(part, &["/detail"])
                .iter()
                .filter_map(|item| {
                    let bytes = item.get("bytes").and_then(Value::as_u64)?;
                    Some(json!({
                        "id": text(item, &["/id"]).unwrap_or_default(),
                        "label": text(item, &["/label", "/title"]).unwrap_or_default(),
                        "description": text(item, &["/description"]).unwrap_or_default(),
                        "bytes": bytes,
                        "size": size(bytes),
                    }))
                })
                .collect();
            // One order for the whole page: the click that opens a part shows
            // its biggest piece first.
            detail.sort_by(|left, right| {
                let bytes = |item: &Value| item.get("bytes").and_then(Value::as_u64).unwrap_or(0);
                bytes(right).cmp(&bytes(left))
            });
            Some(json!({
                "id": text(part, &["/id", "/key"]).unwrap_or_default(),
                "label": text(part, &["/label", "/title"]).unwrap_or_default(),
                "description": text(part, &["/description", "/detail_text"]).unwrap_or_default(),
                "bytes": bytes,
                "size": size(bytes),
                "detail": detail,
            }))
        })
        .collect::<Vec<_>>();
    // Biggest first: the bar reads left to right, so its widest part leads.
    parts.sort_by(|left, right| {
        let bytes = |part: &Value| part.get("bytes").and_then(Value::as_u64).unwrap_or(0);
        bytes(right).cmp(&bytes(left))
    });
    let total: u64 = parts
        .iter()
        .filter_map(|part| part.get("bytes").and_then(Value::as_u64))
        .sum();
    let bytes = first(data, &["/size_bytes", "/total_bytes", "/bytes"])
        .and_then(|v| v.as_u64())
        .or(Some(total).filter(|total| *total > 0));
    json!({
        "dataDir": text(data, &["/data_dir", "/path", "/root"])
            .unwrap_or_else(crate::bridge::data_directory),
        "size": match bytes {
            Some(bytes) => size(bytes),
            None => text(data, &["/size_label"]).unwrap_or_default(),
        },
        "parts": parts,
    })
}

/// Bytes as the page shows them: a decimal unit and one significance step.
pub(crate) fn size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} Б")
    } else if value >= 10.0 {
        // Half up: 24.5 GB is nearer 25 than 24, and `{:.0}` would say 24.
        format!("{:.0} {}", (value + 0.5).floor(), UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
