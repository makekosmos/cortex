//! Blocking Engine calls live on a worker thread; the UI drains replies on a
//! 100ms poll (same shape as agenda-gpui's store worker).
use serde_json::{json, Value};
use std::sync::mpsc::{Receiver, Sender};

use mundus_gpui_kit::engine::Engine;
use mundus_gpui_kit::engine_error::EngineError;

pub enum Command {
    /// POST /v1/rpc — `slot` routes the reply into `ManagerApp::slots`.
    Rpc {
        slot: String,
        op: &'static str,
        params: Value,
    },
    /// GET /v1/<path> status surface.
    Get { slot: String, path: &'static str },
    /// Usage report: get_usage_analytics merged with per-row icon paths
    /// resolved through app_index (store.ts::loadUsageRows parity).
    UsageReport { slot: String },
}

pub struct Reply {
    pub slot: String,
    pub result: Result<Value, String>,
}

pub struct Worker {
    pub commands: Sender<Command>,
    pub replies: Receiver<Reply>,
}

impl Worker {
    pub fn start(data_dir: Option<std::path::PathBuf>) -> Self {
        let (commands, requests) = std::sync::mpsc::channel::<Command>();
        let (results, replies) = std::sync::mpsc::channel::<Reply>();
        std::thread::spawn(move || {
            let engine = Engine { data_dir };
            for request in requests {
                let reply = match request {
                    Command::Rpc { slot, op, params } => {
                        let result = engine.rpc(op, params).map_err(|error| {
                            if slot == "pkg.open" {
                                package_open_message(error)
                            } else {
                                engine_error_message(error)
                            }
                        });
                        Reply { slot, result }
                    }
                    Command::Get { slot, path } => Reply {
                        slot,
                        result: engine.status(path).map_err(engine_error_message),
                    },
                    Command::UsageReport { slot } => Reply {
                        slot,
                        result: usage_report(&engine).map_err(engine_error_message),
                    },
                };
                if results.send(reply).is_err() {
                    break;
                }
            }
        });
        Self { commands, replies }
    }
}

/// Engine failures reach the UI as `message()` text; the raw wire code in
/// `detail` is what a support session needs, so it goes to the app log
/// (KOS-298 — KOS-295 could not recover why a write was rejected).
fn engine_error_message(error: EngineError) -> String {
    tracing::warn!(error = %error, "engine call failed");
    error.message()
}

/// `packages.open` answers carry typed codes (`packages.open: <code>`) that
/// mean more than the generic Engine error classes — "отключено" is an
/// instruction, not a failure. Map them to a precise Russian line; a
/// non-open Engine failure keeps the generic class text.
fn package_open_message(error: EngineError) -> String {
    let Some(code) = error.detail.strip_prefix("packages.open:").map(str::trim) else {
        return engine_error_message(error);
    };
    tracing::warn!(error = %error, "packages.open failed");
    match code {
        "not-installed" => "Приложение не установлено.".into(),
        "disabled" => "Приложение отключено. Включите его в списке.".into(),
        "at-capacity" => "Открыто слишком много приложений. Закройте одно и повторите.".into(),
        "unavailable" => "Запуск приложений недоступен. Перезапустите Engine.".into(),
        _ => "Не удалось открыть приложение. Повторите попытку.".into(),
    }
}

/// store.ts normalizePath parity: trim + '/'→'\\' + lowercase.
fn normalize_path(path: &str) -> String {
    path.trim().replace('/', "\\").to_lowercase()
}

/// `get_usage_analytics` snapshot with an `iconPath` field merged into every
/// `topApps` row. Mirrors loadUsageRows: app_index.list_all maps
/// normalized exec paths to app ids, then `app_index.icon_path` resolves the
/// cached PNG (Electron `mundus-icon://` equivalent for GPUI). A failing
/// app_index is non-fatal — rows render with letter badges.
fn usage_report(engine: &Engine) -> Result<Value, EngineError> {
    let mut snapshot = engine.rpc(
        "get_usage_analytics",
        json!({
            "range_days": 3650,
            "top_apps_limit": 500,
            "recent_sessions_limit": 1,
        }),
    )?;
    let apps = engine
        .rpc("app_index.list_all", json!({ "limit": 1000 }))
        .ok()
        .and_then(|v| v.get("apps").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    let mut ids_by_path = std::collections::HashMap::new();
    for app in &apps {
        let exec_path = app.get("exec_path").and_then(Value::as_str).unwrap_or("");
        let id = app.get("id").and_then(Value::as_str).unwrap_or("");
        if !exec_path.is_empty() && !id.is_empty() {
            ids_by_path.insert(normalize_path(exec_path), id.to_string());
        }
    }
    let Some(rows) = snapshot.get_mut("topApps").and_then(Value::as_array_mut) else {
        return Ok(snapshot);
    };

    // KOS-287: one batched `app_index.exe_info` call enriches every row with
    // the exe version-info display name (fixes legacy rows whose displayName
    // is a stale window title, e.g. a browser's last tab) and an on-demand
    // extracted icon for apps the index never saw.
    let paths: Vec<Value> = rows
        .iter()
        .filter_map(|row| {
            row.get("normalizedPath")
                .and_then(Value::as_str)
                .filter(|p| !p.is_empty())
                .map(|p| Value::String(p.to_string()))
        })
        .collect();
    let mut exe_info: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    if !paths.is_empty() {
        match engine.rpc("app_index.exe_info", json!({ "paths": paths })) {
            Ok(v) => {
                if let Some(entries) = v.get("entries").and_then(Value::as_array) {
                    for entry in entries {
                        if let Some(path) = entry.get("path").and_then(Value::as_str) {
                            exe_info.insert(normalize_path(path), entry.clone());
                        }
                    }
                }
            }
            // Rows keep their stored name and icon — degraded, not broken —
            // but the failure must not be silent.
            Err(error) => {
                tracing::warn!(%error, "app_index.exe_info failed; usage rows keep stored names")
            }
        }
    }

    for row in rows.iter_mut() {
        let normalized_path = row
            .get("normalizedPath")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let info = exe_info.get(&normalize_path(&normalized_path));

        // Version-info name wins over the stored displayName — the stored one
        // may be a window title persisted before KOS-287.
        if let Some(name) = info
            .and_then(|v| v.get("displayName"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            row["displayName"] = Value::String(name.to_string());
        }

        let icon = info
            .and_then(|v| v.get("iconPath"))
            .and_then(Value::as_str)
            .filter(|p| std::path::Path::new(p).is_file())
            .map(str::to_string)
            .or_else(|| {
                ids_by_path
                    .get(&normalize_path(&normalized_path))
                    .and_then(|id| engine.rpc("app_index.icon_path", json!({ "id": id })).ok())
                    .and_then(|v| v.get("path").and_then(Value::as_str).map(str::to_string))
                    .filter(|p| std::path::Path::new(p).is_file())
            })
            .or_else(|| {
                // tracked_apps.icon_ref — путь к PNG в icon cache (может быть
                // stale, postmortems §2026-06-09); берём только существующий файл.
                row.get("iconRef")
                    .and_then(Value::as_str)
                    .filter(|p| std::path::Path::new(p).is_file())
                    .map(str::to_string)
            });
        row["iconPath"] = icon.map_or(Value::Null, Value::String);
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The typed `packages.open` codes must each land on a specific Russian
    /// line — a bare Engine error keeps the generic class text.
    #[test]
    fn package_open_message_maps_typed_codes() {
        let message =
            |code| package_open_message(EngineError::engine(&format!("packages.open: {code}")));
        assert_eq!(message("not-installed"), "Приложение не установлено.");
        assert_eq!(
            message("disabled"),
            "Приложение отключено. Включите его в списке."
        );
        assert_eq!(
            message("at-capacity"),
            "Открыто слишком много приложений. Закройте одно и повторите."
        );
        assert_eq!(
            message("launch-failed"),
            "Не удалось открыть приложение. Повторите попытку."
        );
    }
}
