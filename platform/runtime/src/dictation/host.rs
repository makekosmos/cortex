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
use tokio::sync::broadcast;
use tokio::sync::Mutex;

use super::config::{self, has_api_key, DictationConfig, InjectMode, NetworkProfile, TriggerMode};
use super::groq::{self, GroqError};
#[cfg(windows)]
use super::hotkey_hook;
use super::inject::{self, InjectError};
use super::network;
use super::stats::{self, DictationStats};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictationStateName {
    Idle,
    Recording,
    Transcribing,
    /// Аудио на диске в pending-очереди, retry-loop в фоне. UI рендерит
    /// серый dot + «ждёт сети» с attempts счётчиком.
    Pending,
    /// Fatal (401/400/413 и т.п.) или исчерпан retry — pending всё ещё на
    /// диске для ручного retry. UI рендерит оранжевый `!` с user_msg и
    /// кнопкой «Повторить».
    Error,
}

#[derive(Debug, Clone)]
struct HostState {
    name: DictationStateName,
    /// HWND foreground'а, захваченный перед показом pill. Используется в
    /// inject phase для возврата фокуса.
    prev_hwnd: Option<isize>,
    /// Сообщение для UI (user-facing, локализованное).
    last_error: Option<String>,
    /// UUID активного pending item (Transcribing/Pending/Error) для retry-кнопок.
    active_uuid: Option<String>,
    /// Счётчик попыток для UI (Pending state).
    attempts: u32,
    /// Можно ли retry-ить текущую Error-сессию (true для Fatal от Groq,
    /// false для AudioDecode / NoApiKey — там аудио бесполезно).
    can_retry: bool,
}

impl HostState {
    fn idle() -> Self {
        Self {
            name: DictationStateName::Idle,
            prev_hwnd: None,
            last_error: None,
            active_uuid: None,
            attempts: 0,
            can_retry: false,
        }
    }
}

pub struct DictationHost {
    state: Arc<Mutex<HostState>>,
    config: Arc<Mutex<DictationConfig>>,
    stats: Arc<Mutex<DictationStats>>,
    events_tx: broadcast::Sender<Value>,
    /// Корневая директория для config/stats/pending. `config::data_dir()` в
    /// проде; tempdir в тестах через `new_for_test`.
    data_dir: std::path::PathBuf,
    /// Endpoint Groq transcriptions API. По умолчанию `groq::GROQ_ENDPOINT`;
    /// override для тестов на httpmock.
    groq_endpoint: String,
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
            data_dir: config::data_dir(),
            groq_endpoint: groq::GROQ_ENDPOINT.to_string(),
        });
        // Активируем PTT hook соответственно текущему trigger_mode.
        apply_ptt_hook(&cfg, &events_tx);
        // GC pending queue + emit notifier если есть items.
        host.bootstrap_pending();
        host
    }

    /// Тестовый конструктор — изолированный data_dir и custom Groq endpoint.
    /// НЕ читает реальный конфиг с диска и НЕ трогает PTT hook.
    #[cfg(test)]
    pub(crate) fn new_for_test(
        data_dir: std::path::PathBuf,
        groq_endpoint: String,
        cfg: DictationConfig,
    ) -> Arc<Self> {
        let (events_tx, _) = broadcast::channel::<Value>(64);
        Arc::new(Self {
            state: Arc::new(Mutex::new(HostState::idle())),
            config: Arc::new(Mutex::new(cfg)),
            stats: Arc::new(Mutex::new(DictationStats::default())),
            events_tx,
            data_dir,
            groq_endpoint,
        })
    }

    /// На старте: GC pending по 7d/20 items. Если что-то осталось — emit
    /// `dictation_pending_changed` чтобы UI показал нотификацию.
    fn bootstrap_pending(&self) {
        let _ = super::pending::gc(&self.data_dir, 20, chrono::Duration::days(7));
        if let Ok(items) = super::pending::list(&self.data_dir) {
            if !items.is_empty() {
                let _ = self.events_tx.send(json!({
                    "event": "dictation_pending_changed",
                    "count": items.len(),
                }));
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.events_tx.subscribe()
    }

    async fn emit_state(&self, state: &HostState) {
        let v = json!({
            "event": "dictation_state_changed",
            "state": state_name_str(state.name),
            "error": state.last_error,
            "activeUuid": state.active_uuid,
            "attempts": state.attempts,
            "canRetry": state.can_retry,
        });
        let _ = self.events_tx.send(v);
    }

    /// Helper для перехода в Error state с понятным user_msg.
    /// `can_retry` = true для retryable errors (UI покажет кнопку «Повторить»).
    async fn fail_session(&self, uuid: &str, user_msg: &str, can_retry: bool) {
        let mut s = self.state.lock().await;
        s.name = DictationStateName::Error;
        s.active_uuid = Some(uuid.to_string());
        s.last_error = Some(user_msg.to_string());
        s.can_retry = can_retry;
        s.prev_hwnd = None;
        let snap = s.clone();
        drop(s);
        self.emit_state(&snap).await;
    }

    fn emit_pending_changed(&self) {
        let count = super::pending::list(&self.data_dir)
            .map(|v| v.len())
            .unwrap_or(0);
        let _ = self.events_tx.send(json!({
            "event": "dictation_pending_changed",
            "count": count,
        }));
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
            "activeUuid": s.active_uuid,
            "attempts": s.attempts,
            "canRetry": s.can_retry,
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
        DictationStateName::Pending => "pending",
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
        "providerEnabled": cfg.provider_enabled,
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
pub(crate) enum SubmitError {
    #[error("no api key configured")]
    NoApiKey,
    #[error("audio payload base64 decode failed: {0}")]
    AudioDecode(#[from] base64::DecodeError),
    #[error("network: {0}")]
    Network(#[from] network::NetworkError),
    #[error("groq: {0}")]
    Groq(#[from] GroqError),
    #[error("inject: {0}")]
    Inject(#[from] InjectError),
}

// ---------------------------------------------------------------------------
// Dispatch: dictation.<subop>
// ---------------------------------------------------------------------------

pub async fn handle_dictation_op(
    subop: &str,
    params: Value,
    host: &Arc<DictationHost>,
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
        "verify_api_key" => op_verify_api_key(params, host).await,
        "list_pending" => op_list_pending(host).await,
        "retry" => op_retry(params, host).await,
        "discard" => op_discard(params, host).await,
        "retry_all" => op_retry_all(host).await,
        "get_stats" => op_get_stats(host).await,
        "reset_stats" => op_reset_stats(host).await,
        "begin_hotkey_capture" => op_begin_hotkey_capture(host).await,
        "end_hotkey_capture" => op_end_hotkey_capture(host).await,
        "native_status" => op_native_status().await,
        "ensure_native_permissions" => op_ensure_native_permissions(params).await,
        "native_audio_ping" => op_native_audio_ping().await,
        other => DictationResponse::err(format!("dictation.{other}: unknown sub-operation")),
    }
}

/// Валидация patch'а до применения. Возвращает первое найденное нарушение
/// с понятным русским сообщением (попадает в UI). Чистая функция — без I/O.
pub(crate) fn validate_config_patch(params: &Value) -> Result<(), String> {
    // networkProfile.kind = custom_doh — URL обязан проходить validate_custom_doh_url.
    if let Some(np_val) = params.get("networkProfile") {
        let kind = np_val.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "custom_doh" {
            let url = np_val
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            // Пустой URL разрешён транзитно: юзер только что выбрал radio
            // "Свой DoH URL", input ещё не заполнен. Реальная валидация в
            // момент использования (`build_client` вернёт CustomDohInvalid
            // при попытке транскрибировать). На @blur frontend сам валидирует
            // прежде чем посылать непустой URL.
            if !url.is_empty() {
                network::validate_custom_doh_url(url)
                    .map_err(|e| format!("Custom DoH URL: {e}"))?;
            }
        } else if !kind.is_empty() && !matches!(kind, "system" | "cloudflare_doh" | "google_doh") {
            return Err(format!("networkProfile.kind '{kind}' неизвестен"));
        }
    }
    // httpProxy — если задан и непустой, должен парситься reqwest::Proxy::all.
    if let Some(proxy_val) = params.get("httpProxy") {
        if !proxy_val.is_null() {
            if let Some(s) = proxy_val.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    reqwest::Proxy::all(trimmed)
                        .map_err(|e| format!("http proxy URL невалидный: {e}"))?;
                }
            }
        }
    }
    Ok(())
}

async fn op_update_config(params: Value, host: &DictationHost) -> DictationResponse {
    // Валидация до применения — иначе невалидный URL попал бы в JSON и
    // фейлил бы каждый последующий build_client.
    if let Err(msg) = validate_config_patch(&params) {
        return DictationResponse::err(msg);
    }
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
    if let Some(enabled) = params.get("providerEnabled").and_then(|v| v.as_bool()) {
        cfg.provider_enabled = enabled;
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

/// Активирует hotkey hook (Win32 WH_KEYBOARD_LL) для текущего hotkey + mode.
/// Hook теперь используется ВСЕГДА (не только PTT) — иначе системные shortcut'ы
/// (Win+H, Win+Space) не перехватишь через `RegisterHotKey`/Electron
/// `globalShortcut`. На non-Windows hook — no-op stub.
fn apply_ptt_hook(cfg: &DictationConfig, _tx: &broadcast::Sender<Value>) {
    #[cfg(windows)]
    {
        let mode = match cfg.trigger_mode {
            TriggerMode::PushToTalk => hotkey_hook::HookMode::PushToTalk,
            TriggerMode::Toggle => hotkey_hook::HookMode::Toggle,
        };
        if let Some(matcher) = hotkey_hook::parse_accelerator(&cfg.hotkey) {
            hotkey_hook::set_active(Some(matcher), Some(_tx.clone()), mode);
        } else {
            eprintln!(
                "[dictation::host] hotkey '{}' не парсится — hook деактивирован",
                cfg.hotkey
            );
            hotkey_hook::set_active(None, None, mode);
        }
    }
    #[cfg(not(windows))]
    {
        #[cfg(target_os = "macos")]
        {
            if let Err(e) = crate::dictation::macos_native::set_hotkey_active(
                &cfg.hotkey,
                cfg.trigger_mode,
                _tx.clone(),
            ) {
                eprintln!("[dictation::host] macOS hotkey helper unavailable: {e}");
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = cfg;
        }
    }
}

async fn op_set_api_key(params: Value, host: &DictationHost) -> DictationResponse {
    let key = match params.get("key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_owned(),
        _ => return DictationResponse::err("set_api_key: missing or empty 'key'"),
    };
    match config::set_api_key(&key) {
        Ok(()) => {
            let mut cfg = host.config.lock().await;
            cfg.provider_enabled = true;
            if let Err(e) = config::save(&cfg) {
                return DictationResponse::err(format!("set_api_key: save failed: {e}"));
            }
            drop(cfg);
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

async fn op_submit_audio(params: Value, host: &Arc<DictationHost>) -> DictationResponse {
    let audio_b64 = match params.get("audioB64").and_then(|v| v.as_str()) {
        Some(s) => s.to_owned(),
        None => return DictationResponse::err("submit_audio: missing 'audioB64'"),
    };
    let record_seconds = params
        .get("durationSec")
        .and_then(|v| v.as_f64())
        .map(|f| f.max(0.0).round() as f32)
        .unwrap_or(0.0);

    // Декод base64 — fatal без retry (битое аудио переотправлять бессмысленно).
    let wav_bytes = match base64::engine::general_purpose::STANDARD.decode(&audio_b64) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "dictation: base64 decode failed");
            return DictationResponse::err(format!("submit_audio: битое аудио ({e})"));
        }
    };

    // Validate state — должны быть в Recording.
    {
        let s = host.state.lock().await;
        if !matches!(s.name, DictationStateName::Recording) {
            return DictationResponse::err(format!(
                "submit_audio: state must be recording, got {}",
                state_name_str(s.name)
            ));
        }
    }

    // Snapshot config + prev_hwnd до enqueue.
    let cfg = host.snapshot_config().await;
    let prev_hwnd = host.state.lock().await.prev_hwnd;

    // Disk-first: enqueue до любого HTTP. Crash после этой точки не теряет аудио.
    let opts = super::pending::EnqueueOpts {
        language: cfg.language.clone(),
        prompt: cfg.transcription_prompt.clone(),
        inject_mode: match cfg.inject_mode {
            InjectMode::AutoPaste => "auto_paste".into(),
            InjectMode::ClipboardOnly => "clipboard_only".into(),
        },
        model: cfg.model.clone(),
        prev_hwnd,
    };
    let uuid = match super::pending::enqueue(&host.data_dir, &wav_bytes, record_seconds, opts) {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = %e, "dictation: pending enqueue failed");
            return DictationResponse::err(format!("submit_audio: pending enqueue: {e}"));
        }
    };
    tracing::info!(%uuid, bytes = wav_bytes.len(), "dictation: enqueued, starting transcribe");

    // Transition → Transcribing { uuid }
    {
        let mut s = host.state.lock().await;
        s.name = DictationStateName::Transcribing;
        s.active_uuid = Some(uuid.clone());
        s.attempts = 0;
        s.last_error = None;
        s.can_retry = false;
        let snap = s.clone();
        drop(s);
        host.emit_state(&snap).await;
    }

    // Проверяем API key до запуска — без ключа сразу Error.
    if !cfg.provider_enabled {
        host.fail_session(&uuid, "Поставщик диктовки выключен в настройках", false)
            .await;
        host.emit_pending_changed();
        return DictationResponse::err("submit_audio: provider disabled");
    }
    let api_key = match config::get_api_key() {
        Some(k) => k,
        None => {
            host.fail_session(&uuid, "API key не задан", false).await;
            host.emit_pending_changed();
            return DictationResponse::err("submit_audio: API key не задан");
        }
    };

    // Одна inline-попытка — pill показывает «Распознаю…» ~1s в happy-path.
    // На фейле — spawn'им auto-retry в фоне (5/10/20/40s) и сразу возвращаем
    // OK с state=idle, чтобы pill закрылся без перехвата фокуса.
    let outcome = process_one_attempt(host, &uuid, &api_key, record_seconds).await;
    match outcome {
        AttemptOutcome::Success => {
            // process_one_attempt уже перевёл state в Idle.
            DictationResponse::ok(json!({ "uuid": uuid, "state": "idle" }))
        }
        AttemptOutcome::Fatal => {
            // Фатально (401/400/403/etc) — НЕ спавним auto-retry. Возвращаем
            // pill state="error" с user_msg чтобы он показал понятную ошибку
            // и закрылся. Pending остаётся на диске — после фикса конфига
            // (DoH/proxy/новый key) можно retry'ить из Settings → Очередь.
            let user_msg = {
                let s = host.state.lock().await;
                s.last_error
                    .clone()
                    .unwrap_or_else(|| "Не удалось распознать".into())
            };
            // Reset state в Idle (чтобы новая диктовка работала); error
            // передаётся в response, не через state.
            {
                let mut s = host.state.lock().await;
                *s = HostState::idle();
                let snap = s.clone();
                drop(s);
                host.emit_state(&snap).await;
            }
            DictationResponse::ok(json!({
                "uuid": uuid,
                "state": "error",
                "error": user_msg,
                "queued": false,
            }))
        }
        AttemptOutcome::Retryable => {
            // Spawn'им фоновый auto-retry — pill не блокируется.
            let host_clone: Arc<DictationHost> = host.clone();
            let uuid_clone = uuid.clone();
            let api_key_clone = api_key.clone();
            tokio::spawn(async move {
                auto_retry_loop(
                    host_clone,
                    uuid_clone,
                    api_key_clone,
                    record_seconds,
                    &AUTO_RETRY_DELAYS_SEC,
                )
                .await;
            });
            // Сбрасываем state в Idle — pill закрывается тихо.
            {
                let mut s = host.state.lock().await;
                *s = HostState::idle();
                let snap = s.clone();
                drop(s);
                host.emit_state(&snap).await;
            }
            DictationResponse::ok(json!({
                "uuid": uuid,
                "state": "idle",
                "queued": true,
            }))
        }
    }
}

/// Полный путь обработки одного pending item: retry с backoff → inject.
/// Используется и из `op_submit_audio` (после enqueue), и из `op_retry`
/// (из UI).
/// Расписание auto-retry в фоне (после провала первой попытки в
/// `op_submit_audio`). Базовая схема «5 / 10 / 20 / 40 сек» дополнена
/// короткой первой задержкой (1с): юзер часто включает wifi сразу после
/// fail'а, и быстрый retry в первую секунду спасает от 5-секундной паузы
/// на пустом месте. После 5 retry'ев pending остаётся на диске.
const AUTO_RETRY_DELAYS_SEC: [u64; 5] = [1, 5, 10, 20, 40];

/// Результат одной попытки `process_one_attempt`.
#[derive(Debug, PartialEq)]
enum AttemptOutcome {
    /// Транскрибировано + inject выполнен (или fallback'нут в clipboard).
    Success,
    /// Retryable error — стоит повторить через delay.
    Retryable,
    /// Fatal error — не повторяем (401/400/конфиг).
    Fatal,
}

/// Одна попытка transcribe + inject. Сохраняет attempts/last_error в
/// pending JSON, emit'ит state changes, но НЕ удаляет pending (это делает
/// caller на Success). На Success возвращает Success И уже сделал
/// inject + drop + emit transcript.
async fn process_one_attempt(
    host: &Arc<DictationHost>,
    uuid: &str,
    api_key: &str,
    record_seconds: f32,
) -> AttemptOutcome {
    let start = std::time::Instant::now();

    let wav = match super::pending::read_wav(&host.data_dir, uuid) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(%uuid, error = %e, "dictation: pending read failed");
            return AttemptOutcome::Fatal;
        }
    };

    // Восстанавливаем prev_hwnd из pending JSON, НЕ из state — за время
    // retry активная сессия в state могла быть перезаписана новой диктовкой.
    let item = match super::pending::list(&host.data_dir)
        .ok()
        .and_then(|v| v.into_iter().find(|i| i.uuid == uuid))
    {
        Some(i) => i,
        None => {
            tracing::warn!(%uuid, "dictation: pending item исчез между read_wav и list");
            return AttemptOutcome::Fatal;
        }
    };
    let prev_hwnd = item.opts.prev_hwnd;
    let language = item.opts.language.clone();
    let prompt = item.opts.prompt.clone();
    let model = item.opts.model.clone();
    let inject_mode = match item.opts.inject_mode.as_str() {
        "clipboard_only" => InjectMode::ClipboardOnly,
        _ => InjectMode::AutoPaste,
    };

    let cfg = host.snapshot_config().await;
    let client = match network::build_client(&cfg.network_profile, cfg.http_proxy.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(%uuid, error = %e, "dictation: build_client failed");
            let _ = super::pending::bump_attempt(&host.data_dir, uuid, &e.to_string());
            return AttemptOutcome::Fatal; // конфиг сломан — retry не поможет
        }
    };

    let result = groq::transcribe(
        &client,
        &host.groq_endpoint,
        api_key,
        wav,
        &language,
        &model,
        &prompt,
    )
    .await
    .map_err(SubmitError::from);

    match result {
        Ok(transcript) => {
            let text = transcript.text;
            let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
            tracing::info!(%uuid, duration_ms, "dictation: transcribed");

            let text_for_inject = text.clone();
            let inject_res = tokio::task::spawn_blocking(move || {
                inject::inject_blocking(&text_for_inject, inject_mode, prev_hwnd)
            })
            .await;

            let _ = super::pending::drop_item(&host.data_dir, uuid);

            {
                let mut stats_guard = host.stats.lock().await;
                stats_guard.record_session(&text, record_seconds.round() as u64);
                if let Err(e) = stats::save(&stats_guard) {
                    tracing::warn!(error = %e, "dictation: stats save failed");
                }
            }

            let inject_failed = !matches!(inject_res, Ok(Ok(_)));
            let _ = host.events_tx.send(json!({
                "event": "dictation_transcript",
                "text": text,
                "language": language,
                "durationMs": duration_ms,
                "uuid": uuid,
                "injected": !inject_failed,
            }));
            let _ = host
                .events_tx
                .send(json!({ "event": "dictation_stats_changed" }));
            host.emit_pending_changed();

            // State → Idle. Active session завершилась успехом.
            let mut s = host.state.lock().await;
            if s.active_uuid.as_deref() == Some(uuid) {
                *s = HostState::idle();
                let snap = s.clone();
                drop(s);
                host.emit_state(&snap).await;
            }
            AttemptOutcome::Success
        }
        Err(e) => {
            let kind = super::retry::classify(&e);
            let _ = super::pending::bump_attempt(&host.data_dir, uuid, &e.to_string());
            host.emit_pending_changed();
            match kind {
                super::retry::FailureKind::Fatal { user_msg } => {
                    tracing::warn!(%uuid, error = %e, "dictation: fatal — no retry");
                    let mut s = host.state.lock().await;
                    if s.active_uuid.as_deref() == Some(uuid) {
                        s.name = DictationStateName::Error;
                        s.last_error = Some(user_msg);
                        s.can_retry = false;
                        let snap = s.clone();
                        drop(s);
                        host.emit_state(&snap).await;
                    }
                    AttemptOutcome::Fatal
                }
                super::retry::FailureKind::Retryable => {
                    tracing::info!(%uuid, error = %e, "dictation: retryable — scheduling next attempt");
                    AttemptOutcome::Retryable
                }
            }
        }
    }
}

/// Auto-retry в фоне с расписанием `delays_sec`. Вызывается из spawn'нутой
/// tokio-таски — не блокирует WS / pill. Останавливается на Success / Fatal
/// / исчерпании расписания. На исчерпании pending остаётся на диске для
/// ручного retry из Settings → Очередь.
async fn auto_retry_loop(
    host: Arc<DictationHost>,
    uuid: String,
    api_key: String,
    record_seconds: f32,
    delays_sec: &[u64],
) {
    for (i, delay) in delays_sec.iter().enumerate() {
        tracing::info!(%uuid, attempt = i + 1, delay_sec = delay, "dictation: scheduled retry");
        tokio::time::sleep(std::time::Duration::from_secs(*delay)).await;

        // Pending item мог быть discard'нут юзером — проверяем.
        let exists = super::pending::list(&host.data_dir)
            .ok()
            .map(|v| v.iter().any(|x| x.uuid == uuid))
            .unwrap_or(false);
        if !exists {
            tracing::info!(%uuid, "dictation: auto-retry прерван — item discarded");
            return;
        }

        match process_one_attempt(&host, &uuid, &api_key, record_seconds).await {
            AttemptOutcome::Success => return,
            AttemptOutcome::Fatal => return,
            AttemptOutcome::Retryable => continue,
        }
    }
    tracing::warn!(
        %uuid,
        attempts = delays_sec.len() + 1,
        "dictation: auto-retry исчерпан, pending остаётся на диске"
    );
}

async fn op_list_pending(host: &DictationHost) -> DictationResponse {
    let items = match super::pending::list(&host.data_dir) {
        Ok(v) => v,
        Err(e) => return DictationResponse::err(format!("list_pending: {e}")),
    };
    let arr: Vec<Value> = items
        .into_iter()
        .map(|i| {
            json!({
                "uuid": i.uuid,
                "createdAt": i.created_at.to_rfc3339(),
                "attempts": i.attempts,
                "lastError": i.last_error,
                "durationSec": i.duration_sec,
                "wavBytes": i.wav_bytes,
                "language": i.opts.language,
            })
        })
        .collect();
    DictationResponse::ok(json!({ "items": arr }))
}

async fn op_retry(params: Value, host: &Arc<DictationHost>) -> DictationResponse {
    let uuid = match params.get("uuid").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return DictationResponse::err("retry: missing 'uuid'"),
    };
    let items = match super::pending::list(&host.data_dir) {
        Ok(v) => v,
        Err(e) => return DictationResponse::err(format!("retry: list failed: {e}")),
    };
    let item = match items.iter().find(|i| i.uuid == uuid) {
        Some(i) => i.clone(),
        None => return DictationResponse::err(format!("retry: uuid '{uuid}' не найден")),
    };
    let api_key = match config::get_api_key() {
        Some(k) => k,
        None => return DictationResponse::err("retry: API key не задан"),
    };
    let host_clone = host.clone();
    let uuid_for_task = uuid.clone();
    let dur = item.duration_sec;
    // Ручной retry: одна попытка + если retryable — повторное auto-retry-расписание.
    tokio::spawn(async move {
        match process_one_attempt(&host_clone, &uuid_for_task, &api_key, dur).await {
            AttemptOutcome::Retryable => {
                auto_retry_loop(
                    host_clone,
                    uuid_for_task,
                    api_key,
                    dur,
                    &AUTO_RETRY_DELAYS_SEC,
                )
                .await;
            }
            AttemptOutcome::Success | AttemptOutcome::Fatal => {}
        }
    });
    DictationResponse::ok(json!({ "uuid": uuid, "started": true }))
}

async fn op_discard(params: Value, host: &DictationHost) -> DictationResponse {
    let uuid = match params.get("uuid").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return DictationResponse::err("discard: missing 'uuid'"),
    };
    if let Err(e) = super::pending::drop_item(&host.data_dir, uuid) {
        return DictationResponse::err(format!("discard: {e}"));
    }
    host.emit_pending_changed();
    // Если активная сессия — сбрасываем state.
    let mut s = host.state.lock().await;
    if s.active_uuid.as_deref() == Some(uuid) {
        *s = HostState::idle();
        let snap = s.clone();
        drop(s);
        host.emit_state(&snap).await;
    }
    DictationResponse::ok(json!({ "uuid": uuid, "discarded": true }))
}

async fn op_retry_all(host: &Arc<DictationHost>) -> DictationResponse {
    let items = match super::pending::list(&host.data_dir) {
        Ok(v) => v,
        Err(e) => return DictationResponse::err(format!("retry_all: list: {e}")),
    };
    let api_key = match config::get_api_key() {
        Some(k) => k,
        None => return DictationResponse::err("retry_all: API key не задан"),
    };
    let count = items.len();
    for item in items {
        let host_clone = host.clone();
        let uuid = item.uuid.clone();
        let dur = item.duration_sec;
        let api_key_clone = api_key.clone();
        tokio::spawn(async move {
            match process_one_attempt(&host_clone, &uuid, &api_key_clone, dur).await {
                AttemptOutcome::Retryable => {
                    auto_retry_loop(host_clone, uuid, api_key_clone, dur, &AUTO_RETRY_DELAYS_SEC)
                        .await;
                }
                _ => {}
            }
        });
    }
    DictationResponse::ok(json!({ "started": count }))
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

async fn op_begin_hotkey_capture(host: &DictationHost) -> DictationResponse {
    #[cfg(windows)]
    {
        hotkey_hook::set_capture_mode(true, Some(host.events_tx.clone()));
        DictationResponse::ok(json!({ "ok": true }))
    }
    #[cfg(target_os = "macos")]
    {
        match crate::dictation::macos_native::begin_capture(host.events_tx.clone()) {
            Ok(()) => DictationResponse::ok(json!({ "ok": true })),
            Err(e) => DictationResponse::err(format!("begin_hotkey_capture: {e}")),
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = host;
        DictationResponse::err("begin_hotkey_capture: unsupported platform")
    }
}

async fn op_end_hotkey_capture(host: &DictationHost) -> DictationResponse {
    #[cfg(windows)]
    {
        let _ = host;
        hotkey_hook::set_capture_mode(false, None);
        DictationResponse::ok(json!({ "ok": true }))
    }
    #[cfg(target_os = "macos")]
    {
        let _ = host;
        crate::dictation::macos_native::end_capture();
        DictationResponse::ok(json!({ "ok": true }))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = host;
        DictationResponse::ok(json!({ "ok": true }))
    }
}

async fn op_native_status() -> DictationResponse {
    #[cfg(target_os = "macos")]
    {
        DictationResponse::ok(crate::dictation::macos_native::helper_status())
    }
    #[cfg(not(target_os = "macos"))]
    {
        DictationResponse::ok(json!({
            "platform": std::env::consts::OS,
            "helpers": [],
            "supported": false,
        }))
    }
}

async fn op_ensure_native_permissions(params: Value) -> DictationResponse {
    let prompt = params
        .get("prompt")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    #[cfg(target_os = "macos")]
    {
        match crate::dictation::macos_native::check_permissions(prompt) {
            Ok(v) => DictationResponse::ok(v),
            Err(e) => DictationResponse::err(format!("ensure_native_permissions: {e}")),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = prompt;
        DictationResponse::ok(json!({
            "platform": std::env::consts::OS,
            "supported": false,
        }))
    }
}

async fn op_native_audio_ping() -> DictationResponse {
    #[cfg(target_os = "macos")]
    {
        match crate::dictation::macos_native::audio_ping() {
            Ok(v) => DictationResponse::ok(v),
            Err(e) => DictationResponse::err(format!("native_audio_ping: {e}")),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        DictationResponse::ok(json!({
            "platform": std::env::consts::OS,
            "supported": false,
        }))
    }
}

/// Детальный пошаговый probe: client_build → dns_resolve → tcp_connect →
/// http_head. Каждая стадия со своим временем; при первой ошибке —
/// последующие пропускаются. Возвращаемая структура — JSON для UI.
///
/// Параметры `host_str` и `head_url` извлечены для тестируемости (можно
/// натравить probe на mock-сервер).
pub(crate) async fn probe_connectivity(
    profile: &NetworkProfile,
    proxy: Option<&str>,
    host_str: &str,
    port: u16,
    head_url: &str,
) -> Value {
    let total_start = std::time::Instant::now();
    let mut stages: Vec<Value> = Vec::with_capacity(4);
    let mut first_failure: Option<String> = None;
    let mut ok_overall = true;

    // ---- 1. client_build ----
    let t = std::time::Instant::now();
    let client = match network::build_client(profile, proxy) {
        Ok(c) => {
            stages.push(json!({
                "name": "client_build",
                "ok": true,
                "ms": t.elapsed().as_millis() as u64,
            }));
            Some(c)
        }
        Err(e) => {
            stages.push(json!({
                "name": "client_build",
                "ok": false,
                "ms": t.elapsed().as_millis() as u64,
                "error": e.to_string(),
            }));
            ok_overall = false;
            first_failure = Some("client_build".into());
            None
        }
    };

    // ---- 2. dns_resolve ----
    let resolved_ip: Option<std::net::IpAddr> = if client.is_some() {
        let t = std::time::Instant::now();
        match network::resolve_host(profile, host_str).await {
            Ok(addrs) if !addrs.is_empty() => {
                let ip = addrs[0];
                stages.push(json!({
                    "name": "dns_resolve",
                    "ok": true,
                    "ms": t.elapsed().as_millis() as u64,
                    "ip": ip.to_string(),
                }));
                Some(ip)
            }
            Ok(_) => {
                stages.push(json!({
                    "name": "dns_resolve",
                    "ok": false,
                    "ms": t.elapsed().as_millis() as u64,
                    "error": "DNS не вернул IP",
                }));
                ok_overall = false;
                first_failure = Some("dns_resolve".into());
                None
            }
            Err(e) => {
                stages.push(json!({
                    "name": "dns_resolve",
                    "ok": false,
                    "ms": t.elapsed().as_millis() as u64,
                    "error": e.to_string(),
                }));
                ok_overall = false;
                first_failure = Some("dns_resolve".into());
                None
            }
        }
    } else {
        None
    };

    // ---- 3. tcp_connect (proxy не учитывается — direct probe IP:port) ----
    // Если задан proxy, TCP-стадия через прокси отдельная история; для probe
    // даём прямой connect — если он fail а HTTP works → значит proxy спасает.
    let tcp_ok = if let Some(ip) = resolved_ip {
        let t = std::time::Instant::now();
        let target = std::net::SocketAddr::new(ip, port);
        let connect = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(target),
        )
        .await;
        match connect {
            Ok(Ok(_)) => {
                stages.push(json!({
                    "name": "tcp_connect",
                    "ok": true,
                    "ms": t.elapsed().as_millis() as u64,
                }));
                true
            }
            Ok(Err(e)) => {
                stages.push(json!({
                    "name": "tcp_connect",
                    "ok": false,
                    "ms": t.elapsed().as_millis() as u64,
                    "error": e.to_string(),
                }));
                ok_overall = false;
                if first_failure.is_none() {
                    first_failure = Some("tcp_connect".into());
                }
                false
            }
            Err(_) => {
                stages.push(json!({
                    "name": "tcp_connect",
                    "ok": false,
                    "ms": t.elapsed().as_millis() as u64,
                    "error": "TCP connect timeout (5s)",
                }));
                ok_overall = false;
                if first_failure.is_none() {
                    first_failure = Some("tcp_connect".into());
                }
                false
            }
        }
    } else {
        false
    };

    // ---- 4. http_head (TLS handshake + HTTP request комбинированы) ----
    if let Some(c) = &client {
        if tcp_ok {
            let t = std::time::Instant::now();
            match c.head(head_url).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    stages.push(json!({
                        "name": "http_head",
                        "ok": true,
                        "ms": t.elapsed().as_millis() as u64,
                        "status": status,
                    }));
                }
                Err(e) => {
                    stages.push(json!({
                        "name": "http_head",
                        "ok": false,
                        "ms": t.elapsed().as_millis() as u64,
                        "error": e.to_string(),
                    }));
                    ok_overall = false;
                    if first_failure.is_none() {
                        first_failure = Some("http_head".into());
                    }
                }
            }
        }
    }

    json!({
        "ok": ok_overall,
        "totalMs": total_start.elapsed().as_millis() as u64,
        "stages": stages,
        "firstFailure": first_failure,
    })
}

/// Лёгкая проверка API key — GET /v1/models с Bearer-auth. 200 → ok, 401 →
/// неверный ключ, другие → сетевая ошибка. Не сохраняет ключ в keyring;
/// это делает следующий шаг через `set_api_key`.
///
/// Параметр `endpoint_override` для тестов через httpmock — в проде None.
pub(crate) async fn verify_api_key_with(
    api_key: &str,
    profile: &NetworkProfile,
    proxy: Option<&str>,
    endpoint: &str,
) -> Value {
    if api_key.trim().is_empty() {
        return json!({ "ok": false, "reason": "empty_key", "error": "Ключ пустой" });
    }
    let client = match network::build_client(profile, proxy) {
        Ok(c) => c,
        Err(e) => {
            return json!({
                "ok": false,
                "reason": "client_build",
                "error": e.to_string(),
            });
        }
    };
    let start = std::time::Instant::now();
    let result = client.get(endpoint).bearer_auth(api_key).send().await;
    let ms = start.elapsed().as_millis() as u64;
    match result {
        Ok(resp) => {
            let status = resp.status().as_u16();
            if status == 200 {
                json!({ "ok": true, "status": status, "latencyMs": ms })
            } else if status == 401 {
                json!({
                    "ok": false,
                    "reason": "invalid_key",
                    "status": status,
                    "error": "Неверный ключ",
                    "latencyMs": ms,
                })
            } else {
                json!({
                    "ok": false,
                    "reason": "provider_error",
                    "status": status,
                    "error": format!("Провайдер вернул {status}"),
                    "latencyMs": ms,
                })
            }
        }
        Err(e) => json!({
            "ok": false,
            "reason": "network",
            "error": e.to_string(),
            "latencyMs": ms,
        }),
    }
}

async fn op_verify_api_key(params: Value, host: &DictationHost) -> DictationResponse {
    let key = match params.get("key").and_then(|v| v.as_str()) {
        Some(s) => s.to_owned(),
        None => return DictationResponse::err("verify_api_key: missing 'key'"),
    };
    let cfg = host.snapshot_config().await;
    let report = verify_api_key_with(
        &key,
        &cfg.network_profile,
        cfg.http_proxy.as_deref(),
        "https://api.groq.com/openai/v1/models",
    )
    .await;
    DictationResponse::ok(report)
}

async fn op_test_connectivity(host: &DictationHost) -> DictationResponse {
    let cfg = host.snapshot_config().await;
    let report = probe_connectivity(
        &cfg.network_profile,
        cfg.http_proxy.as_deref(),
        "api.groq.com",
        443,
        "https://api.groq.com/openai/v1/models",
    )
    .await;
    DictationResponse::ok(report)
}

// ---------------------------------------------------------------------------
// Tests — state machine transitions + dispatch.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- validate_config_patch ----

    #[test]
    fn validate_empty_patch_is_ok() {
        assert!(validate_config_patch(&json!({})).is_ok());
    }

    #[test]
    fn validate_unrelated_fields_ok() {
        let patch = json!({
            "hotkey": "Ctrl+Shift+;",
            "language": "ru",
            "triggerMode": "toggle",
        });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_system_profile_ok() {
        let patch = json!({ "networkProfile": { "kind": "system" } });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_cloudflare_doh_ok() {
        let patch = json!({ "networkProfile": { "kind": "cloudflare_doh" } });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_google_doh_ok() {
        let patch = json!({ "networkProfile": { "kind": "google_doh" } });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_unknown_profile_kind_errors() {
        let patch = json!({ "networkProfile": { "kind": "tor_hidden" } });
        let err = validate_config_patch(&patch).unwrap_err();
        assert!(err.contains("tor_hidden"));
    }

    #[test]
    fn validate_custom_doh_with_valid_url_ok() {
        let patch = json!({
            "networkProfile": {
                "kind": "custom_doh",
                "url": "https://comss.dns.controld.com/dns-query"
            }
        });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_custom_doh_with_literal_ip_ok() {
        let patch = json!({
            "networkProfile": { "kind": "custom_doh", "url": "https://1.1.1.1/dns-query" }
        });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_custom_doh_empty_url_ok_transient() {
        // Транзитное состояние: юзер только что переключил radio на custom_doh,
        // URL ещё не введён. Валидация пропускается; build_client при
        // транскрибе вернёт CustomDohInvalid если URL так и не появится.
        let patch = json!({ "networkProfile": { "kind": "custom_doh", "url": "" } });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_custom_doh_whitespace_url_ok_transient() {
        let patch = json!({ "networkProfile": { "kind": "custom_doh", "url": "   " } });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_custom_doh_without_https_errors() {
        let patch = json!({
            "networkProfile": { "kind": "custom_doh", "url": "comss.dns.controld.com/dns-query" }
        });
        let err = validate_config_patch(&patch).unwrap_err();
        assert!(err.contains("Custom DoH"));
        assert!(err.contains("https://"));
    }

    #[test]
    fn validate_custom_doh_http_scheme_errors() {
        let patch = json!({
            "networkProfile": { "kind": "custom_doh", "url": "http://1.1.1.1/dns-query" }
        });
        assert!(validate_config_patch(&patch).is_err());
    }

    #[test]
    fn validate_custom_doh_userinfo_errors() {
        let patch = json!({
            "networkProfile": { "kind": "custom_doh", "url": "https://user:pass@1.1.1.1/dns-query" }
        });
        assert!(validate_config_patch(&patch).is_err());
    }

    #[test]
    fn validate_custom_doh_missing_url_field_ok_transient() {
        let patch = json!({ "networkProfile": { "kind": "custom_doh" } });
        // url field отсутствует — тоже транзитно ok (фронт мог не передать).
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_http_proxy_valid_ok() {
        let patch = json!({ "httpProxy": "http://127.0.0.1:8080" });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_http_proxy_socks5_ok() {
        let patch = json!({ "httpProxy": "socks5://127.0.0.1:1080" });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_http_proxy_invalid_errors() {
        let patch = json!({ "httpProxy": "not a url" });
        let err = validate_config_patch(&patch).unwrap_err();
        assert!(err.contains("proxy"));
    }

    #[test]
    fn validate_http_proxy_null_ok() {
        let patch = json!({ "httpProxy": null });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_http_proxy_empty_string_ok() {
        let patch = json!({ "httpProxy": "" });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_http_proxy_whitespace_ok() {
        let patch = json!({ "httpProxy": "   " });
        assert!(validate_config_patch(&patch).is_ok());
    }

    // ---- probe_connectivity ----

    #[tokio::test]
    async fn probe_reports_all_stages_on_success() {
        // httpmock — настоящий HTTP сервер, robust для probe testing.
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method("HEAD").path("/");
                then.status(200);
            })
            .await;
        let port = server.port();
        let report = probe_connectivity(
            &NetworkProfile::System,
            None,
            "127.0.0.1",
            port,
            &format!("http://127.0.0.1:{port}/"),
        )
        .await;
        assert_eq!(report["ok"], true, "report: {report}");
        let stages = report["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 4, "должно быть 4 стадии");
        let names: Vec<_> = stages.iter().map(|s| s["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            vec!["client_build", "dns_resolve", "tcp_connect", "http_head"]
        );
        for s in stages {
            assert_eq!(s["ok"], true, "stage failed: {s}");
        }
        let http_stage = stages.iter().find(|s| s["name"] == "http_head").unwrap();
        assert_eq!(http_stage["status"], 200);
        assert!(report["firstFailure"].is_null());
        assert!(report["totalMs"].as_u64().unwrap() < 60_000);
    }

    #[tokio::test]
    async fn probe_first_failure_on_tcp_refused() {
        // Несуществующий локальный порт → resolve OK, TCP connect fail.
        let report = probe_connectivity(
            &NetworkProfile::System,
            None,
            "127.0.0.1",
            1, // reserved, не принимает коннекты
            "http://127.0.0.1:1/",
        )
        .await;
        assert_eq!(report["ok"], false);
        assert_eq!(report["firstFailure"], "tcp_connect");
        let stages = report["stages"].as_array().unwrap();
        // client_build + dns_resolve OK, tcp_connect FAIL, http_head не выполнялся
        assert_eq!(stages[0]["name"], "client_build");
        assert_eq!(stages[0]["ok"], true);
        assert_eq!(stages[1]["name"], "dns_resolve");
        assert_eq!(stages[1]["ok"], true);
        assert_eq!(stages[2]["name"], "tcp_connect");
        assert_eq!(stages[2]["ok"], false);
        assert!(
            stages.len() == 3,
            "http_head должен быть skipped: {stages:?}"
        );
    }

    #[tokio::test]
    async fn probe_first_failure_on_invalid_proxy() {
        // Невалидный proxy → client_build fail; остальные стадии skipped.
        let report = probe_connectivity(
            &NetworkProfile::System,
            Some("not a url"),
            "127.0.0.1",
            443,
            "http://127.0.0.1/",
        )
        .await;
        assert_eq!(report["ok"], false);
        assert_eq!(report["firstFailure"], "client_build");
        let stages = report["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 1, "только client_build, остальное skipped");
        assert_eq!(stages[0]["ok"], false);
    }

    #[tokio::test]
    async fn probe_first_failure_on_dns_resolve_unknown_host() {
        let report = probe_connectivity(
            &NetworkProfile::System,
            None,
            "definitely-not-a-real-host-12345.invalid",
            443,
            "https://definitely-not-a-real-host-12345.invalid/",
        )
        .await;
        assert_eq!(report["ok"], false);
        assert_eq!(report["firstFailure"], "dns_resolve");
        let stages = report["stages"].as_array().unwrap();
        // client_build OK, dns_resolve FAIL, tcp/http skipped
        assert!(stages
            .iter()
            .any(|s| s["name"] == "dns_resolve" && s["ok"] == false));
    }

    #[tokio::test]
    async fn probe_includes_resolved_ip_on_success() {
        let report = probe_connectivity(
            &NetworkProfile::System,
            None,
            "127.0.0.1",
            65535, // даже не открываем TCP, нам нужна только DNS стадия
            "http://127.0.0.1:65535/",
        )
        .await;
        let stages = report["stages"].as_array().unwrap();
        let dns_stage = stages.iter().find(|s| s["name"] == "dns_resolve").unwrap();
        assert_eq!(dns_stage["ok"], true);
        assert_eq!(dns_stage["ip"], "127.0.0.1");
    }

    // ---- process_pending integration ----

    fn test_cfg() -> DictationConfig {
        DictationConfig {
            hotkey: "Ctrl+Shift+;".into(),
            trigger_mode: TriggerMode::Toggle,
            language: "ru".into(),
            inject_mode: InjectMode::ClipboardOnly, // не трогаем реальный clipboard
            network_profile: NetworkProfile::System,
            provider: "groq".into(),
            provider_enabled: true,
            model: "whisper-large-v3".into(),
            http_proxy: None,
            transcription_prompt: String::new(),
            microphone_device_id: None,
        }
    }

    fn make_wav() -> Vec<u8> {
        // Minimal valid WAV header + 1 sample silence. Достаточно для теста
        // что pending::enqueue/read_wav круглим без потерь.
        let mut wav = Vec::with_capacity(44);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&36u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&16000u32.to_le_bytes()); // sample rate
        wav.extend_from_slice(&32000u32.to_le_bytes()); // byte rate
        wav.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&0u32.to_le_bytes());
        wav
    }

    #[tokio::test]
    async fn process_pending_success_transitions_to_idle_and_drops_item() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(200).body(
                    r#"{"text":"привет","segments":[{"text":"привет","no_speech_prob":0.05,"avg_logprob":-0.3}]}"#,
                );
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());

        // pre-enqueue
        let wav = make_wav();
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &wav,
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "clipboard_only".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();

        // Симулируем активную сессию — иначе process_one_attempt не трогает state.
        {
            let mut s = host.state.lock().await;
            s.active_uuid = Some(uuid.clone());
            s.name = DictationStateName::Transcribing;
        }

        let outcome = process_one_attempt(&host, &uuid, "fake-key", 1.0).await;
        assert_eq!(outcome, AttemptOutcome::Success);

        // pending удалён
        assert!(super::super::pending::list(&host.data_dir)
            .unwrap()
            .is_empty());
        // state → Idle
        let snap = host.current_state().await;
        assert_eq!(snap["state"], "idle");
    }

    #[tokio::test]
    async fn process_pending_401_keeps_item_and_marks_error() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(401).body(r#"{"error":"invalid key"}"#);
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());

        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "clipboard_only".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();

        // Активная сессия — state.active_uuid должен matching, иначе state не обновляется.
        {
            let mut s = host.state.lock().await;
            s.active_uuid = Some(uuid.clone());
            s.name = DictationStateName::Transcribing;
        }

        let outcome = process_one_attempt(&host, &uuid, "fake-key", 1.0).await;
        assert_eq!(outcome, AttemptOutcome::Fatal, "401 must be Fatal");

        // Pending item остался на диске + attempts++
        let items = super::super::pending::list(&host.data_dir).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].attempts, 1);
        // State → Error с user_msg про API key
        let snap = host.current_state().await;
        assert_eq!(snap["state"], "error");
        assert!(
            snap["lastError"].as_str().unwrap().contains("API key"),
            "msg: {}",
            snap["lastError"]
        );
        // 401 — fatal, can_retry=false (retry без смены ключа бесполезен).
        assert_eq!(snap["canRetry"], false);
    }

    #[tokio::test]
    async fn process_one_attempt_5xx_is_retryable_keeps_pending() {
        // Одна попытка vs 503 — Retryable. Auto-retry-loop в проде дальше
        // запустит ретраи. Здесь тестим что: pending остался, attempts++,
        // outcome=Retryable, state НЕ Error (background retry в работе).
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(503).body("temporarily down");
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "clipboard_only".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();

        let outcome = process_one_attempt(&host, &uuid, "fake-key", 1.0).await;
        assert_eq!(outcome, AttemptOutcome::Retryable);

        let items = super::super::pending::list(&host.data_dir).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].attempts, 1);
    }

    #[tokio::test]
    async fn auto_retry_loop_gives_up_on_persistent_retryable() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(503).body("forever down");
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "clipboard_only".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();

        // 3 ретрая по 0ms = 4 попытки. Все 503 → retryable исчерпан.
        auto_retry_loop(
            host.clone(),
            uuid.clone(),
            "fake-key".into(),
            1.0,
            &[0u64, 0, 0],
        )
        .await;

        let items = super::super::pending::list(&host.data_dir).unwrap();
        assert_eq!(items.len(), 1, "pending остался");
        assert_eq!(items[0].attempts, 3, "3 attempts через auto_retry_loop");
    }

    #[tokio::test]
    async fn auto_retry_loop_stops_on_discard() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(503);
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "clipboard_only".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();

        // Удаляем pending до начала retry-loop.
        super::super::pending::drop_item(&host.data_dir, &uuid).unwrap();
        // Loop должен сразу выйти увидев что item исчез.
        auto_retry_loop(
            host.clone(),
            uuid.clone(),
            "fake-key".into(),
            1.0,
            &[0u64, 0, 0],
        )
        .await;
        // Никаких новых файлов не появилось — discard выдержан.
        assert!(super::super::pending::list(&host.data_dir)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn op_list_pending_returns_items() {
        let td = tempfile::TempDir::new().unwrap();
        let host =
            DictationHost::new_for_test(td.path().into(), "http://localhost/".into(), test_cfg());
        super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            2.5,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: "".into(),
                inject_mode: "auto_paste".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();
        let resp = op_list_pending(&host).await;
        assert!(resp.ok);
        let items = resp.data["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["language"], "ru");
        assert_eq!(items[0]["durationSec"], 2.5);
    }

    #[tokio::test]
    async fn op_discard_removes_item_and_resets_state() {
        let td = tempfile::TempDir::new().unwrap();
        let host =
            DictationHost::new_for_test(td.path().into(), "http://localhost/".into(), test_cfg());
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: "".into(),
                inject_mode: "auto_paste".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();
        // Подделаем active session.
        {
            let mut s = host.state.lock().await;
            s.name = DictationStateName::Error;
            s.active_uuid = Some(uuid.clone());
            s.last_error = Some("test".into());
        }
        let resp = op_discard(json!({ "uuid": uuid }), &host).await;
        assert!(resp.ok);
        assert!(super::super::pending::list(&host.data_dir)
            .unwrap()
            .is_empty());
        let snap = host.current_state().await;
        assert_eq!(snap["state"], "idle");
    }

    #[tokio::test]
    async fn op_discard_unknown_uuid_errors() {
        let td = tempfile::TempDir::new().unwrap();
        let host =
            DictationHost::new_for_test(td.path().into(), "http://localhost/".into(), test_cfg());
        let resp = op_discard(json!({ "uuid": "nope" }), &host).await;
        assert!(!resp.ok);
    }

    // ---- verify_api_key ----

    #[tokio::test]
    async fn verify_api_key_empty_returns_ok_false_empty_key() {
        let r = verify_api_key_with("", &NetworkProfile::System, None, "http://unused").await;
        assert_eq!(r["ok"], false);
        assert_eq!(r["reason"], "empty_key");
    }

    #[tokio::test]
    async fn verify_api_key_whitespace_returns_empty_key() {
        let r = verify_api_key_with("  \t  ", &NetworkProfile::System, None, "http://unused").await;
        assert_eq!(r["ok"], false);
        assert_eq!(r["reason"], "empty_key");
    }

    #[tokio::test]
    async fn verify_api_key_200_returns_ok_true() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/v1/models")
                    .header("authorization", "Bearer fake-key");
                then.status(200).body(r#"{"data":[]}"#);
            })
            .await;
        let endpoint = format!("{}/v1/models", server.base_url());
        let r = verify_api_key_with("fake-key", &NetworkProfile::System, None, &endpoint).await;
        assert_eq!(r["ok"], true);
        assert_eq!(r["status"], 200);
    }

    #[tokio::test]
    async fn verify_api_key_401_returns_invalid_key() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/v1/models");
                then.status(401).body(r#"{"error":"unauthorized"}"#);
            })
            .await;
        let endpoint = format!("{}/v1/models", server.base_url());
        let r = verify_api_key_with("bad-key", &NetworkProfile::System, None, &endpoint).await;
        assert_eq!(r["ok"], false);
        assert_eq!(r["reason"], "invalid_key");
        assert_eq!(r["status"], 401);
    }

    #[tokio::test]
    async fn verify_api_key_5xx_returns_provider_error() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/v1/models");
                then.status(503);
            })
            .await;
        let endpoint = format!("{}/v1/models", server.base_url());
        let r = verify_api_key_with("any", &NetworkProfile::System, None, &endpoint).await;
        assert_eq!(r["ok"], false);
        assert_eq!(r["reason"], "provider_error");
        assert_eq!(r["status"], 503);
    }

    #[tokio::test]
    async fn verify_api_key_network_fail_returns_network_reason() {
        // Закрытый порт → connect refused → reason: network.
        let r = verify_api_key_with(
            "key",
            &NetworkProfile::System,
            None,
            "http://127.0.0.1:1/v1/models",
        )
        .await;
        assert_eq!(r["ok"], false);
        assert_eq!(r["reason"], "network");
    }

    // ---- existing tests ----

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
    static ENV_DATA_DIR_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn update_config_persists_and_emits_event() {
        let _guard = ENV_DATA_DIR_LOCK.lock().await;
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
