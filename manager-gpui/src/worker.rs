//! Blocking Engine calls live on a worker thread; the UI drains replies on a
//! 100ms poll (same shape as agenda-gpui's store worker).
use serde_json::{json, Value};
use std::sync::mpsc::{Receiver, Sender};

use kosmos_gpui_kit::engine::Engine;

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
    /// Engine broadcast events (`{"event": ...}`) from the WS subscription —
    /// drained on the same UI poll as `replies`.
    pub events: Receiver<Value>,
}

impl Worker {
    pub fn start(data_dir: Option<std::path::PathBuf>) -> Self {
        let (commands, requests) = std::sync::mpsc::channel::<Command>();
        let (results, replies) = std::sync::mpsc::channel::<Reply>();
        let (event_sink, events) = std::sync::mpsc::channel::<Value>();
        let events_dir = data_dir.clone();
        std::thread::spawn(move || {
            let engine = Engine { data_dir };
            for request in requests {
                let reply = match request {
                    Command::Rpc { slot, op, params } => Reply {
                        slot,
                        result: engine.rpc(op, params),
                    },
                    Command::Get { slot, path } => Reply {
                        slot,
                        result: engine.status(path),
                    },
                    Command::UsageReport { slot } => Reply {
                        slot,
                        result: usage_report(&engine),
                    },
                };
                if results.send(reply).is_err() {
                    break;
                }
            }
        });
        // Engine broadcast events (dictation hotkey triggers, state and
        // download progress) — ws_server pushes them to every hello'd client.
        // Blocking socket reads stay off the UI thread; reconnect with
        // backoff so an Engine restart resubscribes on its own.
        std::thread::spawn(move || {
            let engine = Engine {
                data_dir: events_dir,
            };
            let mut backoff = std::time::Duration::from_millis(500);
            loop {
                if let Ok(mut stream) = engine.subscribe() {
                    backoff = std::time::Duration::from_millis(500);
                    while let Some(event) = stream.next_event() {
                        if event_sink.send(event).is_err() {
                            return;
                        }
                    }
                }
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(std::time::Duration::from_secs(10));
            }
        });
        Self {
            commands,
            replies,
            events,
        }
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
fn usage_report(engine: &Engine) -> Result<Value, String> {
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
    for row in rows.iter_mut() {
        let normalized_path = row
            .get("normalizedPath")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let icon = ids_by_path
            .get(&normalize_path(&normalized_path))
            .and_then(|id| engine.rpc("app_index.icon_path", json!({ "id": id })).ok())
            .and_then(|v| v.get("path").and_then(Value::as_str).map(str::to_string))
            .filter(|p| std::path::Path::new(p).is_file())
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
