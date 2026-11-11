//! The shapes the wire and the pages read: state names, config, stats and the
//! predicates that decide whether the saved provider can transcribe at all.

use serde_json::{json, Value};

use super::*;

pub fn state_name_str(n: DictationStateName) -> &'static str {
    match n {
        DictationStateName::Idle => "idle",
        DictationStateName::Recording => "recording",
        DictationStateName::Transcribing => "transcribing",
        DictationStateName::Pending => "pending",
        DictationStateName::Error => "error",
    }
}

pub fn config_to_value(cfg: &DictationConfig) -> Value {
    json!({
        "hotkey": cfg.hotkey,
        "triggerMode": match cfg.trigger_mode {
            TriggerMode::Toggle => "toggle",
            TriggerMode::PushToTalk => "push_to_talk",
        },
        "language": cfg.language,
        "injectMode": match cfg.inject_mode {
            InjectMode::AutoPaste => "auto_paste",
            InjectMode::ClipboardOnly => "clipboard_only",
        },
        "networkProfile": network_profile_to_value(&cfg.network_profile),
        "httpProxy": cfg.http_proxy,
        "transcriptionPrompt": cfg.transcription_prompt,
        "provider": cfg.provider,
        "providerEnabled": cfg.provider_enabled,
        "duckAudioDuringRecording": cfg.duck_audio_during_recording,
        "model": cfg.model,
        "localEngine": cfg.local_engine,
        "localModelPath": cfg.local_model_path,
        "localCommandPath": cfg.local_command_path,
        "localModel": cfg.local_model,
        "localModelId": cfg.local_model,
        "microphoneDeviceId": cfg.microphone_device_id,
        "localIdleUnloadMs": cfg.local_idle_unload_ms,
    })
}

pub fn stats_to_value(s: &DictationStats) -> Value {
    json!({
        "totalWords": s.total_words,
        "totalRecordSeconds": s.total_record_seconds,
        "totalSessions": s.total_sessions,
        "wpm": s.wpm(),
        "timeSavedSeconds": s.time_saved_seconds(),
        "days": s.days,
    })
}

pub fn network_profile_to_value(profile: &NetworkProfile) -> Value {
    match profile {
        NetworkProfile::System => json!({ "kind": "system" }),
        NetworkProfile::CloudflareDoh => json!({ "kind": "cloudflare_doh" }),
        NetworkProfile::GoogleDoh => json!({ "kind": "google_doh" }),
        NetworkProfile::CustomDoh { url } => json!({ "kind": "custom_doh", "url": url }),
    }
}

// ---------------------------------------------------------------------------
// Response wrapper — same shape as the other host dispatchers for a uniform
// wire response.
// ---------------------------------------------------------------------------

pub struct DictationResponse {
    pub ok: bool,
    pub data: Value,
    pub error: Option<String>,
}

impl DictationResponse {
    pub fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data,
            error: None,
        }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: Value::Null,
            error: Some(msg.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Errors (internal). Конвертируются в DictationResponse::err строки.
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SubmitError {
    #[error("audio payload base64 decode failed: {0}")]
    AudioDecode(#[from] base64::DecodeError),
    #[error("network: {0}")]
    Network(#[from] network::NetworkError),
    #[error("local: {0}")]
    Local(#[from] local::LocalError),
    #[error("groq: {0}")]
    Groq(#[from] GroqError),
    #[error("inject: {0}")]
    Inject(#[from] InjectError),
}

pub fn provider_uses_local_runtime(provider: &str) -> bool {
    provider == "local"
}

pub fn provider_needs_api_key(cfg: &DictationConfig) -> bool {
    if provider_uses_local_runtime(&cfg.provider) {
        return false;
    }
    mock_dictation_transcript_override(&cfg.provider, Some(cfg.transcription_prompt.as_str()))
        .is_none()
}

pub fn effective_model_for_submit(cfg: &DictationConfig) -> String {
    if provider_uses_local_runtime(&cfg.provider) {
        return cfg
            .local_model
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| cfg.model.clone());
    }
    cfg.model.clone()
}

pub const LOCAL_MODEL_NOT_READY_MSG: &str =
    "Локальная модель не подготовлена. Скачайте модель заново в Settings -> AI.";

/// Is the *selected* local model usable on disk — independent of
/// `provider_enabled`. A disabled provider with a surviving selection is
/// the user's own off-switch, not a broken config.
pub fn local_selection_valid(cfg: &DictationConfig) -> bool {
    if !provider_uses_local_runtime(&cfg.provider) {
        return true;
    }
    let model_ok = cfg.local_model_path.as_deref().is_some_and(|path| {
        let path = std::path::Path::new(path);
        if path.is_file() {
            return true;
        }
        if !path.is_dir() {
            return false;
        }
        // Multi-file directory models count only once every declared file
        // exists — a half-downloaded dir is not a usable selection.
        cfg.local_model
            .as_deref()
            .and_then(|id| {
                local_models::MODEL_CATALOG
                    .iter()
                    .find(|spec| spec.id == id)
            })
            .filter(|spec| spec.directory && !spec.files.is_empty())
            .map(|spec| {
                spec.files
                    .iter()
                    .all(|(_, local)| path.join(local).is_file())
            })
            .unwrap_or(true)
    });
    if cfg.local_engine == "parakeet" {
        return model_ok;
    }
    let command_ok = cfg
        .local_command_path
        .as_deref()
        .is_some_and(|path| std::path::Path::new(path).is_file());
    model_ok && command_ok
}

pub fn local_config_is_ready(_data_dir: &std::path::Path, cfg: &DictationConfig) -> bool {
    !cfg.provider_enabled
        || !provider_uses_local_runtime(&cfg.provider)
        || local_selection_valid(cfg)
}

pub fn clear_local_selection(cfg: &mut DictationConfig) {
    cfg.provider_enabled = false;
    cfg.local_model = None;
    cfg.local_model_path = None;
    cfg.local_command_path = None;
}

/// Runtime reconcile for a broken local selection: prefer auto-selecting a
/// downloaded model (a deleted file is recoverable when another model is
/// on disk); only with nothing downloaded does the provider get disabled.
pub async fn reconcile_unready_local_config(host: &DictationHost) -> bool {
    let mut cfg = host.config.lock().await;
    if local_config_is_ready(&host.data_dir, &cfg) {
        return false;
    }
    if !autoselect_local_model(&host.data_dir, &mut cfg) {
        clear_local_selection(&mut cfg);
    }
    if let Err(e) = save_config_in(&host.data_dir, &cfg) {
        tracing::warn!(error = %e, "failed to save cleared dictation local config");
    }
    drop(cfg);
    host.emit_config_changed();
    true
}

// ---------------------------------------------------------------------------
// Dispatch: dictation.<subop>
// ---------------------------------------------------------------------------
