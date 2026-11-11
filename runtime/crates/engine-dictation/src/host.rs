// DictationHost — singleton в mundus-engine. State machine + broadcast events
// + dispatch для `dictation.*` operations.
//
// Arc<Mutex<State>> + `broadcast::Sender<Value>` для wire events.
// между сессиями (нет persistence — нечего сохранять между рестартами кроме
// конфига).
//
// Phase 1: audio capture на стороне renderer'а (Web Audio API), backend
// получает готовый WAV в `dictation.submit_audio`. Streaming chunks — out of
// scope (Groq не принимает streaming, см. spec).

use std::sync::Arc;

use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use thiserror::Error;
use tokio::sync::broadcast;
use tokio::sync::Mutex;

use super::config::{self, has_api_key, DictationConfig, InjectMode, NetworkProfile, TriggerMode};
use super::groq::{self, GroqError};
#[cfg(windows)]
use super::hotkey_hook;
use super::inject::{self, InjectError};
use super::local::{self, DEFAULT_LOCAL_ENGINE};
use super::local_models;
use super::model_sweep;
use super::network;
use super::stats::{self, DictationStats};

#[path = "host_state.rs"]
mod state;
pub(crate) use state::SubmitError;
use state::*;
pub use state::{DictationHost, DictationResponse, DictationStateName};
fn save_config_in(data_dir: &std::path::Path, cfg: &DictationConfig) -> std::io::Result<()> {
    config::save_to(&data_dir.join("dictation-config.json"), cfg)
}

fn save_stats_in(data_dir: &std::path::Path, value: &DictationStats) -> std::io::Result<()> {
    stats::save_to(&data_dir.join("dictation-stats.json"), value)
}
pub async fn handle_dictation_op(
    subop: &str,
    params: Value,
    host: &Arc<DictationHost>,
) -> DictationResponse {
    tracing::debug!(subop, "dictation operation received");
    match subop {
        "get_state" => DictationResponse::ok(host.current_state().await),
        "get_config" => {
            let cfg = host.snapshot_config().await;
            DictationResponse::ok(json!({
                "config": config_to_value(&cfg),
                "hasApiKey": has_api_key(),
            }))
        }
        "update_config" => op_update_config(params, host).await,
        "set_api_key" => op_set_api_key(params, host).await,
        "capture_foreground_window" => op_capture_foreground(host).await,
        "start_recording" => op_start_recording(host).await,
        "toggle" => {
            capture::handle_engine_trigger(
                host,
                &json!({"event": "dictation.trigger", "kind": "toggle", "phase": "down"}),
            )
            .await;
            DictationResponse::ok(host.current_state().await)
        }
        "cancel" => op_cancel(host).await,
        "submit_audio" => op_submit_audio(params, host).await,
        "test_connectivity" => op_test_connectivity(host).await,
        "verify_api_key" => op_verify_api_key(params, host).await,
        "list_pending" => op_list_pending(host).await,
        "retry" => op_retry(params, host).await,
        "discard" => op_discard(params, host).await,
        "discard_all" => op_discard_all(host).await,
        "retry_all" => op_retry_all(host).await,
        // The pill is on screen only while a session runs. The scope's own
        // thread keeps the last frame after the fade, so the state decides.
        "pill_frame" => {
            let state = host.current_state().await;
            let name = state
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("idle")
                .to_owned();
            let running = matches!(name.as_str(), "recording" | "transcribing" | "pending");
            DictationResponse::ok(json!({
                "active": running && host.pill.is_active(),
                "frame": host.pill.last_frame().unwrap_or(Value::Null),
                // The meter the scope is riding: without it a pill that never
                // moves looks like a drawing bug rather than a silent meter.
                "loudness": host.pill.loudness(),
            }))
        }
        // The Manager's own event tap reads key codes; this turns one into the
        // accelerator string the config stores, so the two tables cannot drift.
        "accelerator" => DictationResponse::ok(json!({
            "accelerator": op_accelerator(&params),
        })),
        "release_hotkey" => {
            #[cfg(target_os = "macos")]
            crate::macos_native::release_listeners();
            DictationResponse::ok(json!({}))
        }
        "get_stats" => op_get_stats(host).await,
        "reset_stats" => op_reset_stats(host).await,
        "begin_hotkey_capture" => op_begin_hotkey_capture(host).await,
        "end_hotkey_capture" => op_end_hotkey_capture(host).await,
        "local_status" => op_local_status().await,
        "list_local_models" => op_list_local_models(host).await,
        "capture.start" => op_capture_start(params, host).await,
        "capture.stop" => op_capture_stop(params, host).await,
        "speech.transcribe" => op_speech_transcribe(params, host).await,
        "input.insert_text" => op_insert_text(params, host).await,
        "window.foreground" => op_contract_foreground(host).await,
        "download_local_model" => op_download_local_model(params, host).await,
        "use_local_model" => op_use_local_model(params, host).await,
        "delete_local_model" => op_delete_local_model(params, host).await,
        other => DictationResponse::err(format!("dictation.{other}: unknown sub-operation")),
    }
}

#[path = "host_models.rs"]
mod models;
#[cfg(test)]
pub(crate) use models::validate_config_patch;
use models::*;
#[path = "host_capture.rs"]
mod capture;
use capture::*;
pub use capture::{handle_engine_trigger, route_trigger_event};
#[path = "host_queue.rs"]
mod queue;
use queue::*;
#[cfg(test)]
pub(crate) use queue::{probe_connectivity, verify_api_key_with};

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
