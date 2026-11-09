//! What each page asks the Engine for, and how the answer lands in the state.
//!
//! The pages never name an op. They ask the bridge to open a route once, and the
//! table here decides which Engine calls that costs. Every answer is mapped
//! defensively: the Engine states more than the Manager shows, and a field that
//! moves must not blank a page.

use std::sync::{mpsc, Arc, Mutex};

use imago_nsh::ComponentContext;
use serde_json::{json, Map, Value};

use crate::engine::Engine;

/// One Engine call and where its result goes in the state.
type Call = (&'static str, &'static str, Value);

/// route → the calls that page needs.
fn calls(route: &str) -> Vec<Call> {
    // One plugin's page: its manifest and its stored settings.
    if let Some(id) = route.strip_prefix("plugin/") {
        return vec![(
            "plugins.current",
            "plugins.settings.get",
            json!({ "id": id }),
        )];
    }
    match route {
        "models" => vec![("models", "dictation.list_local_models", json!({}))],
        "keys" => vec![("keys", "dictation.get_config", json!({}))],
        "devices" => vec![("sync", "get_sync_snapshot", json!({}))],
        "storage" => vec![
            ("backups", "manager.db_backups.list", json!({})),
            ("storage", "manager.data.storage", json!({})),
        ],
        "activity" => vec![(
            "usage",
            "get_usage_analytics",
            json!({ "range_days": 21, "top_apps_limit": 8 }),
        )],
        "integrations" => vec![("connectors", "integrations.list", json!({}))],
        // The product's own page also answers "what can be updated": a version
        // is a fact about this computer, and it has no page of its own.
        "server" => vec![("updates", "updater.status", json!({}))],
        "appearance" => vec![("appearance", "appearance.get", json!({}))],
        "plugins" => vec![
            ("plugins.catalog", "plugins.catalog", json!({})),
            ("plugins.installed", "plugins.list", json!({})),
        ],
        "settings" => vec![
            ("settings", "engine.settings.get", json!({})),
            ("settings.autostart", "engine.autostart.get", json!({})),
        ],
        _ => Vec::new(),
    }
}

enum Job {
    Route(String),
    Rpc {
        key: String,
        op: String,
        params: Value,
        transform: Option<fn(Value) -> Value>,
        /// Ask the page to re-read itself once the call is done (a download
        /// changes a row the call's own answer does not carry).
        reload: Option<String>,
    },
}

/// Runs the table on one thread of its own: an Engine that is slow or absent
/// must never stall the window, and its answers arrive as `state` events.
#[derive(Clone)]
pub struct Loader {
    jobs: mpsc::Sender<Job>,
}

impl Loader {
    pub fn start(engine: Engine, context: ComponentContext, state: Arc<Mutex<Value>>) -> Self {
        let (jobs, queue) = mpsc::channel::<Job>();
        std::thread::spawn(move || {
            for job in queue {
                match job {
                    Job::Route(route) => {
                        for (key, op, params) in calls(&route) {
                            run(&engine, &context, &state, key, op, params, None);
                        }
                    }
                    Job::Rpc {
                        key,
                        op,
                        params,
                        transform,
                        reload,
                    } => {
                        run(&engine, &context, &state, &key, &op, params, transform);
                        if let Some(route) = reload {
                            for (key, op, params) in calls(&route) {
                                run(&engine, &context, &state, key, op, params, None);
                            }
                        }
                    }
                }
            }
        });
        Self { jobs }
    }

    /// Re-reads what a page shows. Called when the sidebar opens it.
    pub fn open(&self, route: &str) {
        let _ = self.jobs.send(Job::Route(route.to_owned()));
    }

    /// One call whose answer belongs back in the state (`key` is the path).
    pub fn call(&self, key: &str, op: &str, params: Value, transform: Option<fn(Value) -> Value>) {
        let _ = self.jobs.send(Job::Rpc {
            key: key.to_owned(),
            op: op.to_owned(),
            params,
            transform,
            reload: None,
        });
    }

    /// A call after which `route` must be read again (downloads, restores).
    pub fn then_reload(&self, key: &str, op: &str, params: Value, route: &str) {
        let _ = self.jobs.send(Job::Rpc {
            key: key.to_owned(),
            op: op.to_owned(),
            params,
            transform: None,
            reload: Some(route.to_owned()),
        });
    }
}

fn run(
    engine: &Engine,
    context: &ComponentContext,
    state: &Arc<Mutex<Value>>,
    key: &str,
    op: &str,
    params: Value,
    transform: Option<fn(Value) -> Value>,
) {
    let answer = engine.rpc(op, params);
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(_) => return,
    };
    match answer {
        Ok(data) => apply(&mut state, key, data, transform),
        Err(message) => {
            // Keep what the page shows: a failed refresh is not an empty page.
            tracing::warn!(op, error = %message, "engine call failed");
        }
    }
    context.emit("state", state.clone());
}

/// Narrows one answer and writes it where the pages read it. Split out of `run`
/// so the mapping that reaches the state can be tested without an Engine.
pub(crate) fn apply(
    state: &mut Value,
    key: &str,
    answer: Value,
    transform: Option<fn(Value) -> Value>,
) {
    let mapped = match transform {
        Some(transform) => transform(answer),
        None => shape(key, answer),
    };
    write(state, key, mapped);
}

/// Writes `a.b` (or `a`) in the state. An answer that carries a piece of an
/// object merges into it: `engine.settings.get` must not drop the
/// `settings.autostart` another call already wrote.
fn write(state: &mut Value, path: &str, value: Value) {
    let Some(object) = state.as_object_mut() else {
        return;
    };
    let mut cursor = object;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            match value {
                Value::Object(fresh) => match cursor.get_mut(part) {
                    Some(Value::Object(existing)) => {
                        existing.extend(fresh);
                    }
                    _ => {
                        cursor.insert(part.to_owned(), Value::Object(fresh));
                    }
                },
                other => {
                    cursor.insert(part.to_owned(), other);
                }
            }
            return;
        }
        let entry = cursor
            .entry(part.to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        cursor = entry.as_object_mut().expect("just made an object");
    }
}

/// The Engine's answer, narrowed to what the pages render.
fn shape(key: &str, data: Value) -> Value {
    // Plugins are shaped in a module of their own: nothing here knows them.
    if let Some(shaped) = crate::plugins::shape(key, &data) {
        return shaped;
    }
    match key {
        // `engine.autostart.get` answers `{available, enabled, reason}`; the
        // screen wants the switch's position, not the object it arrives in.
        // The Engine keeps no installed-app catalog here: the Apps page has
        // nothing to ask for and renders its empty state.
        "settings.autostart" => Value::Bool(
            data.pointer("/enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
        "models" => Value::Array(
            array(&data, &["/models", "/data/models"])
                .iter()
                .map(model_row)
                .collect(),
        ),
        "keys" => keys(&data),
        "sync" => sync(&data),
        "connectors" => Value::Array(
            array(&data, &["/providers", "/data/providers"])
                .iter()
                .map(connector_row)
                .collect(),
        ),
        "backups" => Value::Array(
            array(&data, &["/backups", "/data/backups"])
                .iter()
                .map(backup_row)
                .collect(),
        ),
        "updates" => updates(&data),
        "appearance" => appearance(&data),
        "usage" => usage(&data),
        "storage" => storage(&data),
        "settings" => settings(&data),
        _ => data,
    }
}

/// The whole sync surface: peers for the Devices page and the pairing requests
/// the modal shows on whatever page the user is on.
fn sync(data: &Value) -> Value {
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

fn first(value: &Value, paths: &[&str]) -> Option<Value> {
    paths.iter().find_map(|path| value.pointer(path)).cloned()
}

fn bool_at(value: &Value, paths: &[&str]) -> bool {
    paths
        .iter()
        .find_map(|path| value.pointer(path).and_then(Value::as_bool))
        .unwrap_or(false)
}

fn model_row(model: &Value) -> Value {
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
fn keys(data: &Value) -> Value {
    json!({
        "hasApiKey": bool_at(data, &["/hasApiKey", "/has_api_key"]),
        "engine": text(data, &["/config/localEngine"]).unwrap_or_default(),
        "language": text(data, &["/config/language"]).unwrap_or_default(),
        "model": text(data, &["/config/localModelId", "/config/localModel"]).unwrap_or_default(),
    })
}

fn device_row(peer: &Value) -> Value {
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

fn connector_row(provider: &Value) -> Value {
    json!({
        "id": text(provider, &["/id", "/provider", "/name"]).unwrap_or_default(),
        "name": text(provider, &["/name", "/display_name", "/label"]).unwrap_or_default(),
        "connected": bool_at(provider, &["/connected", "/authorized", "/has_credential"]),
        "kind": text(provider, &["/kind", "/type", "/category"]).unwrap_or_default(),
        "detail": text(provider, &["/status", "/detail", "/account"]).unwrap_or_default(),
    })
}

fn backup_row(backup: &Value) -> Value {
    let bytes = first(backup, &["/size_bytes", "/bytes"]).and_then(|value| value.as_u64());
    json!({
        "id": text(backup, &["/id"]).unwrap_or_default(),
        "size": bytes.map(size).unwrap_or_default(),
        "modified": backup.get("modified_ms").and_then(Value::as_u64).unwrap_or_default(),
    })
}

/// `updater.status`: what the release channel offers and how far a download is.
fn updates(data: &Value) -> Value {
    json!({
        "state": text(data, &["/state"]).unwrap_or_else(|| "idle".into()),
        "currentVersion": text(data, &["/currentVersion", "/current_version"]).unwrap_or_default(),
        "newVersion": text(data, &["/newVersion", "/new_version"]).unwrap_or_default(),
        "channel": text(data, &["/channel"]).unwrap_or_default(),
        "canInstall": bool_at(data, &["/canInstall", "/can_install"]),
        "percent": data.get("percent").and_then(Value::as_f64).unwrap_or(0.0),
    })
}

/// `appearance.get` answers `{settings, capabilities, wallpaper_accent}`; the
/// settings are device-wide and live in the Engine.
fn appearance(data: &Value) -> Value {
    let settings = data.get("settings").unwrap_or(data);
    json!({
        "mode": text(settings, &["/mode"]).unwrap_or_else(|| "dark".into()),
        "light": text(settings, &["/light_theme", "/lightTheme"])
            .unwrap_or_else(|| "merino-light".into()),
        "dark": text(settings, &["/dark_theme", "/darkTheme"])
            .unwrap_or_else(|| "cobalt-foundry".into()),
        "followApps": bool_at(settings, &["/follow_apps", "/followApps"]),
        "accentSource": text(settings, &["/accent_source"]).unwrap_or_else(|| "theme".into()),
    })
}

fn settings(data: &Value) -> Value {
    // `autostart` is deliberately absent: `engine.settings.get` does not carry
    // it, and a `false` invented here would overwrite the real position the
    // `engine.autostart.get` call writes under `settings.autostart`.
    json!({
        "telemetry": data
            .pointer("/usage_tracker/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// `get_usage_analytics` answers a snapshot with a per-day trend and the
/// busiest programs; milliseconds become minutes here.
fn usage(data: &Value) -> Value {
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

fn storage(data: &Value) -> Value {
    let bytes = first(data, &["/size_bytes", "/total_bytes", "/bytes"]).and_then(|v| v.as_u64());
    json!({
        "dataDir": text(data, &["/data_dir", "/path", "/root"])
            .unwrap_or_else(crate::bridge::data_directory),
        "size": match bytes {
            Some(bytes) => size(bytes),
            None => text(data, &["/size_label"]).unwrap_or_default(),
        },
    })
}

/// Bytes as the page shows them: a decimal unit and one significance step.
fn size(bytes: u64) -> String {
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

#[cfg(test)]
mod tests;
