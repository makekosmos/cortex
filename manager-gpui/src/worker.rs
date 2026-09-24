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
    /// Pill start: `dictation.capture_foreground_window` (inject target HWND,
    /// captured before the pill window exists) then engine-owned mic capture
    /// via `dictation.capture.start` (WASAPI 16kHz mono WAV on Windows).
    DictationStart { slot: String },
    /// Pill stop: `dictation.capture.stop {captureId}` → `{audioB64,
    /// durationMs}` payload for the follow-up transcribe call.
    DictationStop { slot: String, capture_id: String },
    /// Chained after the pill window closed: re-capture the foreground HWND
    /// (capture.stop resets `prev_hwnd`) then `dictation.speech.transcribe`,
    /// which transcribes and injects per the configured `injectMode`.
    DictationTranscribe {
        slot: String,
        audio_b64: String,
        duration_sec: f64,
    },
    /// Pill cancel: terminate the capture session (audio discarded) and reset
    /// the backend state machine via `dictation.cancel`.
    DictationCancel {
        slot: String,
        capture_id: Option<String>,
    },
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
                    Command::DictationStart { slot } => Reply {
                        slot,
                        result: dictation_start(&engine),
                    },
                    Command::DictationStop { slot, capture_id } => Reply {
                        slot,
                        result: engine
                            .rpc("dictation.capture.stop", json!({ "captureId": capture_id })),
                    },
                    Command::DictationTranscribe {
                        slot,
                        audio_b64,
                        duration_sec,
                    } => Reply {
                        slot,
                        result: dictation_transcribe(&engine, &audio_b64, duration_sec),
                    },
                    Command::DictationCancel { slot, capture_id } => Reply {
                        slot,
                        result: dictation_cancel(&engine, capture_id.as_deref()),
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

/// store.ts normalizePath parity: trim + '/'→'\\' + lowercase.
fn normalize_path(path: &str) -> String {
    path.trim().replace('/', "\\").to_lowercase()
}

/// `get_usage_analytics` snapshot with an `iconPath` field merged into every
/// `topApps` row. Mirrors loadUsageRows: app_index.list_all maps
/// normalized exec paths to app ids, then `app_index.icon_path` resolves the
/// cached PNG (Electron `kosmos-icon://` equivalent for GPUI). A failing
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

// --- Dictation pill session (Electron dictation-pill.ts parity) --------------

/// Foreground HWND is captured before the pill window exists — on Windows the
/// inject path restores focus to it and sends the paste shortcut. Failure is
/// non-fatal (clipboard-only fallback in the runtime).
fn dictation_start(engine: &Engine) -> Result<Value, String> {
    let _ = engine.rpc("dictation.capture_foreground_window", json!({}));
    engine.rpc("dictation.capture.start", json!({}))
}

/// `dictation.capture.stop` clears `prev_hwnd` together with the rest of the
/// session state, so the foreground HWND is re-captured here — the pill window
/// is already closed by the time this runs and the target app owns the
/// foreground again. `speech.transcribe` then runs the queued
/// transcribe + inject attempt inline.
fn dictation_transcribe(
    engine: &Engine,
    audio_b64: &str,
    duration_sec: f64,
) -> Result<Value, String> {
    if audio_b64.is_empty() {
        return Err("Engine не вернул аудио записи".into());
    }
    let _ = engine.rpc("dictation.capture_foreground_window", json!({}));
    engine.rpc(
        "dictation.speech.transcribe",
        json!({ "audioB64": audio_b64, "durationSec": duration_sec }),
    )
}

/// `dictation.cancel` resets the state machine but leaves a live capture
/// session marked busy, so the session is stopped (audio discarded) first.
fn dictation_cancel(engine: &Engine, capture_id: Option<&str>) -> Result<Value, String> {
    if let Some(id) = capture_id {
        let _ = engine.rpc("dictation.capture.stop", json!({ "captureId": id }));
    }
    engine.rpc("dictation.cancel", json!({}))
}
