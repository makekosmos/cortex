// DictationHost — singleton в kepler-backend. State machine + broadcast events
// + dispatch для `dictation.*` operations.
//
// Аналогично PomodoroHost (`pomodoro_host.rs:48-380`) — Arc<Mutex<State>> +
// `broadcast::Sender<Value>` для wire events. Разница: dictation stateless
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
use tokio::sync::Mutex;
use tokio::sync::broadcast;

use super::config::{
    self, has_api_key, DictationConfig, InjectMode, NetworkProfile, TriggerMode,
};
use super::groq::{self, GroqError};
use super::inject::{self, InjectError};
use super::network;
use super::stats::{self, DictationStats};
#[cfg(windows)]
use super::hotkey_hook;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictationStateName {
    Idle,
    Recording,
    Transcribing,
    Error,
}

#[derive(Debug, Clone)]
struct HostState {
    name: DictationStateName,
    /// HWND foreground'а, захваченный перед показом pill. Используется в
    /// inject phase для возврата фокуса.
    prev_hwnd: Option<isize>,
    /// Последнее сообщение об ошибке для UI (если name == Error).
    last_error: Option<String>,
}

impl HostState {
    fn idle() -> Self {
        Self {
            name: DictationStateName::Idle,
            prev_hwnd: None,
            last_error: None,
        }
    }
}

pub struct DictationHost {
    state: Arc<Mutex<HostState>>,
    config: Arc<Mutex<DictationConfig>>,
    stats: Arc<Mutex<DictationStats>>,
    events_tx: broadcast::Sender<Value>,
}

impl DictationHost {
    pub fn new() -> Arc<Self> {
        let cfg = config::load();
        let stats = stats::load();
        let (events_tx, _) = broadcast::channel::<Value>(64);
        let host = Arc::new(Self {
            state: Arc::new(Mutex::new(HostState::idle())),
            config: Arc::new(Mutex::new(cfg.clone())),
            stats: Arc::new(Mutex::new(stats)),
            events_tx: events_tx.clone(),
        });
        // Активируем PTT hook соответственно текущему trigger_mode.
        apply_ptt_hook(&cfg, &events_tx);
        host
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.events_tx.subscribe()
    }

    async fn emit_state(&self, state: &HostState) {
        let v = json!({
            "event": "dictation_state_changed",
            "state": state_name_str(state.name),
            "error": state.last_error,
        });
        let _ = self.events_tx.send(v);
    }

    fn emit_config_changed(&self) {
        let v = json!({ "event": "dictation_config_changed" });
        let _ = self.events_tx.send(v);
    }

    pub async fn current_state(&self) -> Value {
        let s = self.state.lock().await;
        let cfg = self.config.lock().await;
        json!({
            "state": state_name_str(s.name),
            "hasApiKey": has_api_key(),
            "lastError": s.last_error,
            "config": config_to_value(&cfg),
        })
    }

    async fn snapshot_config(&self) -> DictationConfig {
        self.config.lock().await.clone()
    }
}

fn state_name_str(n: DictationStateName) -> &'static str {
    match n {
        DictationStateName::Idle => "idle",
        DictationStateName::Recording => "recording",
        DictationStateName::Transcribing => "transcribing",
        DictationStateName::Error => "error",
    }
}

fn config_to_value(cfg: &DictationConfig) -> Value {
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
        "model": cfg.model,
        "microphoneDeviceId": cfg.microphone_device_id,
    })
}

fn stats_to_value(s: &DictationStats) -> Value {
    json!({
        "totalWords": s.total_words,
        "totalRecordSeconds": s.total_record_seconds,
        "totalSessions": s.total_sessions,
        "wpm": s.wpm(),
        "timeSavedSeconds": s.time_saved_seconds(),
    })
}

fn network_profile_to_value(profile: &NetworkProfile) -> Value {
    match profile {
        NetworkProfile::System => json!({ "kind": "system" }),
        NetworkProfile::CloudflareDoh => json!({ "kind": "cloudflare_doh" }),
        NetworkProfile::GoogleDoh => json!({ "kind": "google_doh" }),
        NetworkProfile::CustomDoh { url } => json!({ "kind": "custom_doh", "url": url }),
    }
}

// ---------------------------------------------------------------------------
// Response wrapper — та же форма что в pomodoro_host для unifrom dispatch.
// ---------------------------------------------------------------------------

pub struct DictationResponse {
    pub ok: bool,
    pub data: Value,
    pub error: Option<String>,
}

impl DictationResponse {
    fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data,
            error: None,
        }
    }
    fn err(msg: impl Into<String>) -> Self {
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
enum SubmitError {
    #[error("no api key configured")]
    NoApiKey,
    #[error("audio payload base64 decode failed: {0}")]
    AudioDecode(#[from] base64::DecodeError),
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    #[error("groq: {0}")]
    Groq(#[from] GroqError),
    #[error("inject: {0}")]
    Inject(#[from] InjectError),
    #[error("inject join: {0}")]
    InjectJoin(String),
}

// ---------------------------------------------------------------------------
// Dispatch: dictation.<subop>
// ---------------------------------------------------------------------------

pub async fn handle_dictation_op(
    subop: &str,
    params: Value,
    host: &DictationHost,
) -> DictationResponse {
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
        "clear_api_key" => op_clear_api_key(host).await,
        "capture_foreground_window" => op_capture_foreground(host).await,
        "start_recording" => op_start_recording(host).await,
        "cancel" => op_cancel(host).await,
        "submit_audio" => op_submit_audio(params, host).await,
        "test_connectivity" => op_test_connectivity(host).await,
        "get_stats" => op_get_stats(host).await,
        "reset_stats" => op_reset_stats(host).await,
        other => DictationResponse::err(format!("dictation.{other}: unknown sub-operation")),
    }
}

async fn op_update_config(params: Value, host: &DictationHost) -> DictationResponse {
    // Принимаем patch — частичный объект, мерджим поверх текущего.
    let mut cfg = host.config.lock().await;
    if let Some(s) = params.get("hotkey").and_then(|v| v.as_str()) {
        cfg.hotkey = s.to_owned();
    }
    if let Some(s) = params.get("triggerMode").and_then(|v| v.as_str()) {
        cfg.trigger_mode = match s {
            "toggle" => TriggerMode::Toggle,
            "push_to_talk" => TriggerMode::PushToTalk,
            other => {
                return DictationResponse::err(format!(
                    "update_config: invalid triggerMode '{other}'"
                ))
            }
        };
    }
    if let Some(s) = params.get("language").and_then(|v| v.as_str()) {
        cfg.language = s.to_owned();
    }
    if let Some(s) = params.get("injectMode").and_then(|v| v.as_str()) {
        cfg.inject_mode = match s {
            "auto_paste" => InjectMode::AutoPaste,
            "clipboard_only" => InjectMode::ClipboardOnly,
            other => {
                return DictationResponse::err(format!(
                    "update_config: invalid injectMode '{other}'"
                ))
            }
        };
    }
    if params.get("httpProxy").is_some() {
        cfg.http_proxy = params
            .get("httpProxy")
            .and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    v.as_str().map(|s| {
                        let trimmed = s.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_owned())
                        }
                    })
                }
            })
            .unwrap_or(None);
    }
    if let Some(s) = params.get("transcriptionPrompt").and_then(|v| v.as_str()) {
        cfg.transcription_prompt = s.to_owned();
    }
    if let Some(s) = params.get("provider").and_then(|v| v.as_str()) {
        cfg.provider = s.to_owned();
    }
    if let Some(s) = params.get("model").and_then(|v| v.as_str()) {
        cfg.model = s.to_owned();
    }
    if params.get("microphoneDeviceId").is_some() {
        cfg.microphone_device_id = params
            .get("microphoneDeviceId")
            .and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    v.as_str().map(|s| {
                        let trimmed = s.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_owned())
                        }
                    })
                }
            })
            .unwrap_or(None);
    }
    if let Some(np_val) = params.get("networkProfile") {
        let kind = np_val.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        cfg.network_profile = match kind {
            "system" => NetworkProfile::System,
            "cloudflare_doh" => NetworkProfile::CloudflareDoh,
            "google_doh" => NetworkProfile::GoogleDoh,
            "custom_doh" => {
                let url = np_val
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                NetworkProfile::CustomDoh { url }
            }
            other => {
                return DictationResponse::err(format!(
                    "update_config: invalid networkProfile.kind '{other}'"
                ))
            }
        };
    }
    if let Err(e) = config::save(&cfg) {
        return DictationResponse::err(format!("update_config: save failed: {e}"));
    }
    let snapshot = cfg.clone();
    drop(cfg);
    // Перерегистрируем PTT hook (на случай смены trigger_mode или hotkey).
    apply_ptt_hook(&snapshot, &host.events_tx);
    host.emit_config_changed();
    DictationResponse::ok(json!({ "config": config_to_value(&snapshot) }))
}

/// Включает PTT hook когда `trigger_mode == PushToTalk` и hotkey парсится.
/// Снимает (matcher=None) в остальных случаях.
fn apply_ptt_hook(cfg: &DictationConfig, _tx: &broadcast::Sender<Value>) {
    #[cfg(windows)]
    {
        if cfg.trigger_mode == TriggerMode::PushToTalk {
            if let Some(matcher) = hotkey_hook::parse_accelerator(&cfg.hotkey) {
                hotkey_hook::set_active(Some(matcher), Some(_tx.clone()));
                return;
            } else {
                eprintln!(
                    "[dictation::host] PTT mode requested but hotkey '{}' не парсится",
                    cfg.hotkey
                );
            }
        }
        hotkey_hook::set_active(None, None);
    }
    #[cfg(not(windows))]
    {
        // PTT в Phase 1.5 — Windows-only. Toggle работает на всех платформах
        // через Electron globalShortcut.
        let _ = cfg;
    }
}

async fn op_set_api_key(params: Value, host: &DictationHost) -> DictationResponse {
    let key = match params.get("key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_owned(),
        _ => return DictationResponse::err("set_api_key: missing or empty 'key'"),
    };
    match config::set_api_key(&key) {
        Ok(()) => {
            host.emit_config_changed();
            DictationResponse::ok(json!({ "ok": true }))
        }
        Err(e) => DictationResponse::err(format!("keyring write failed: {e}")),
    }
}

async fn op_clear_api_key(host: &DictationHost) -> DictationResponse {
    match config::clear_api_key() {
        Ok(()) => {
            host.emit_config_changed();
            DictationResponse::ok(json!({ "ok": true }))
        }
        Err(e) => DictationResponse::err(format!("keyring delete failed: {e}")),
    }
}

async fn op_capture_foreground(host: &DictationHost) -> DictationResponse {
    let hwnd = inject::capture_foreground_window();
    let mut s = host.state.lock().await;
    s.prev_hwnd = hwnd;
    DictationResponse::ok(json!({ "captured": hwnd.is_some() }))
}

async fn op_start_recording(host: &DictationHost) -> DictationResponse {
    let mut s = host.state.lock().await;
    if !matches!(s.name, DictationStateName::Idle | DictationStateName::Error) {
        return DictationResponse::err(format!(
            "start_recording: state must be idle, got {}",
            state_name_str(s.name)
        ));
    }
    s.name = DictationStateName::Recording;
    s.last_error = None;
    let snapshot = s.clone();
    drop(s);
    host.emit_state(&snapshot).await;
    DictationResponse::ok(json!({ "state": "recording" }))
}

async fn op_cancel(host: &DictationHost) -> DictationResponse {
    let mut s = host.state.lock().await;
    s.name = DictationStateName::Idle;
    s.prev_hwnd = None;
    s.last_error = None;
    let snapshot = s.clone();
    drop(s);
    host.emit_state(&snapshot).await;
    DictationResponse::ok(json!({ "state": "idle" }))
}

async fn op_submit_audio(params: Value, host: &DictationHost) -> DictationResponse {
    let audio_b64 = match params.get("audioB64").and_then(|v| v.as_str()) {
        Some(s) => s.to_owned(),
        None => return DictationResponse::err("submit_audio: missing 'audioB64'"),
    };
    // Длительность записи (секунды) — renderer её знает через `audioCtx`
    // sampleRate × PCM samples. Используем для агрегата stats. Если поле
    // не пришло (legacy renderer) — 0, stats просто не учтут duration.
    let record_seconds = params
        .get("durationSec")
        .and_then(|v| v.as_f64())
        .map(|f| f.max(0.0).round() as u64)
        .unwrap_or(0);

    // transition Recording → Transcribing
    {
        let mut s = host.state.lock().await;
        if !matches!(s.name, DictationStateName::Recording) {
            return DictationResponse::err(format!(
                "submit_audio: state must be recording, got {}",
                state_name_str(s.name)
            ));
        }
        s.name = DictationStateName::Transcribing;
        let snap = s.clone();
        drop(s);
        host.emit_state(&snap).await;
    }

    match do_transcribe_and_inject(&audio_b64, host).await {
        Ok((text, language, duration_ms)) => {
            // Накапливаем агрегатную stats: words/seconds/sessions →
            // diagnostic UI на странице диктации (WPM, Time Saved, Total).
            {
                let mut stats_guard = host.stats.lock().await;
                stats_guard.record_session(&text, record_seconds);
                if let Err(e) = stats::save(&stats_guard) {
                    eprintln!("[dictation::host] stats save failed: {e}");
                }
            }
            // emit transcript event
            let _ = host.events_tx.send(json!({
                "event": "dictation_transcript",
                "text": text,
                "language": language,
                "durationMs": duration_ms,
            }));
            // signal stats changed → UI обновляет карточки live.
            let _ = host
                .events_tx
                .send(json!({ "event": "dictation_stats_changed" }));
            // transition → Idle
            let mut s = host.state.lock().await;
            s.name = DictationStateName::Idle;
            s.prev_hwnd = None;
            s.last_error = None;
            let snap = s.clone();
            drop(s);
            host.emit_state(&snap).await;
            DictationResponse::ok(json!({
                "text": text,
                "durationMs": duration_ms,
            }))
        }
        Err(err) => {
            let msg = err.to_string();
            let mut s = host.state.lock().await;
            s.name = DictationStateName::Error;
            s.last_error = Some(msg.clone());
            s.prev_hwnd = None;
            let snap = s.clone();
            drop(s);
            host.emit_state(&snap).await;
            DictationResponse::err(format!("submit_audio: {msg}"))
        }
    }
}

async fn do_transcribe_and_inject(
    audio_b64: &str,
    host: &DictationHost,
) -> Result<(String, String, u64), SubmitError> {
    let start = std::time::Instant::now();

    let api_key = config::get_api_key().ok_or(SubmitError::NoApiKey)?;

    let wav_bytes = base64::engine::general_purpose::STANDARD.decode(audio_b64)?;

    let cfg = host.snapshot_config().await;
    let client = network::build_client(&cfg.network_profile, cfg.http_proxy.as_deref())?;
    let result = groq::transcribe(
        &client,
        &api_key,
        wav_bytes,
        &cfg.language,
        &cfg.model,
        &cfg.transcription_prompt,
    )
    .await?;

    let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

    // Inject в активное окно (или только clipboard).
    let prev_hwnd = {
        let s = host.state.lock().await;
        s.prev_hwnd
    };
    let text = result.text.clone();
    let inject_mode = cfg.inject_mode;
    let text_for_inject = text.clone();
    tokio::task::spawn_blocking(move || {
        inject::inject_blocking(&text_for_inject, inject_mode, prev_hwnd)
    })
    .await
    .map_err(|e| SubmitError::InjectJoin(e.to_string()))??;

    Ok((text, cfg.language, duration_ms))
}

async fn op_get_stats(host: &DictationHost) -> DictationResponse {
    let s = host.stats.lock().await;
    DictationResponse::ok(stats_to_value(&s))
}

async fn op_reset_stats(host: &DictationHost) -> DictationResponse {
    let mut s = host.stats.lock().await;
    *s = DictationStats::default();
    if let Err(e) = stats::save(&s) {
        return DictationResponse::err(format!("reset_stats: save failed: {e}"));
    }
    let snap = stats_to_value(&s);
    drop(s);
    let _ = host
        .events_tx
        .send(json!({ "event": "dictation_stats_changed" }));
    DictationResponse::ok(snap)
}

async fn op_test_connectivity(host: &DictationHost) -> DictationResponse {
    let cfg = host.snapshot_config().await;
    let client = match network::build_client(&cfg.network_profile, cfg.http_proxy.as_deref()) {
        Ok(c) => c,
        Err(e) => return DictationResponse::err(format!("client build: {e}")),
    };
    let start = std::time::Instant::now();
    match client
        .head("https://api.groq.com/openai/v1/models")
        .send()
        .await
    {
        Ok(resp) => {
            let latency_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
            DictationResponse::ok(json!({
                "ok": true,
                "status": resp.status().as_u16(),
                "latencyMs": latency_ms,
            }))
        }
        Err(e) => DictationResponse::err(format!("connectivity failed: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Tests — state machine transitions + dispatch.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_state_idle_initially() {
        let host = DictationHost::new();
        let resp = handle_dictation_op("get_state", Value::Null, &host).await;
        assert!(resp.ok);
        assert_eq!(resp.data["state"], "idle");
    }

    #[tokio::test]
    async fn start_recording_transitions_to_recording() {
        let host = DictationHost::new();
        let resp = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(resp.ok, "start_recording failed: {:?}", resp.error);
        let state = handle_dictation_op("get_state", Value::Null, &host).await;
        assert_eq!(state.data["state"], "recording");
    }

    #[tokio::test]
    async fn start_recording_from_non_idle_errors() {
        let host = DictationHost::new();
        handle_dictation_op("start_recording", Value::Null, &host).await;
        let resp = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(!resp.ok);
        let err = resp.error.unwrap_or_default();
        assert!(err.contains("idle"), "got: {err}");
    }

    #[tokio::test]
    async fn cancel_returns_to_idle() {
        let host = DictationHost::new();
        handle_dictation_op("start_recording", Value::Null, &host).await;
        let resp = handle_dictation_op("cancel", Value::Null, &host).await;
        assert!(resp.ok);
        let state = handle_dictation_op("get_state", Value::Null, &host).await;
        assert_eq!(state.data["state"], "idle");
    }

    #[tokio::test]
    async fn submit_audio_requires_recording_state() {
        let host = DictationHost::new();
        let resp =
            handle_dictation_op("submit_audio", json!({ "audioB64": "aGVsbG8=" }), &host).await;
        assert!(!resp.ok);
        assert!(resp.error.unwrap_or_default().contains("recording"));
    }

    #[tokio::test]
    async fn unknown_subop_errors() {
        let host = DictationHost::new();
        let resp = handle_dictation_op("nope", Value::Null, &host).await;
        assert!(!resp.ok);
        assert!(resp.error.unwrap_or_default().contains("unknown"));
    }

    // Global mutex для тестов которые мутируют process-wide env var
    // `KOSMOS_DATA_DIR`. `config::save` пишет по абсолютному пути из
    // env, поэтому без guard'а параллельные тесты ломают друг друга
    // ИЛИ затирают `%APPDATA%\Kosmos\dictation-config.json` пользователя.
    static ENV_DATA_DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[tokio::test]
    async fn update_config_persists_and_emits_event() {
        let _guard = ENV_DATA_DIR_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().expect("tempdir");
        std::env::set_var("KOSMOS_DATA_DIR", tmp.path());

        let host = DictationHost::new();
        let mut rx = host.subscribe();
        let resp = handle_dictation_op(
            "update_config",
            json!({ "language": "auto", "injectMode": "clipboard_only" }),
            &host,
        )
        .await;
        assert!(resp.ok, "update_config failed: {:?}", resp.error);

        let evt = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
            .await
            .expect("event timeout")
            .expect("event recv");
        assert_eq!(evt["event"], "dictation_config_changed");

        // Re-load: новый host подхватит persisted config.
        let host2 = DictationHost::new();
        let state = handle_dictation_op("get_state", Value::Null, &host2).await;
        assert_eq!(state.data["config"]["language"], "auto");
        assert_eq!(state.data["config"]["injectMode"], "clipboard_only");

        std::env::remove_var("KOSMOS_DATA_DIR");
    }
}
