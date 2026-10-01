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
    capture: std::sync::Mutex<Option<super::native_capture::Session>>,
    contract_window_id: std::sync::Mutex<Option<String>>,
    contract_events: std::sync::atomic::AtomicBool,
}

impl DictationHost {
    pub fn new(data_dir: std::path::PathBuf) -> Arc<Self> {
        match local_models::migrate_legacy_assets(&data_dir) {
            Ok(true) => tracing::info!("migrated legacy dictation local STT assets"),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(error = %e, "failed to migrate legacy dictation local STT assets")
            }
        }
        if local_models::cleanup_obsolete_local_stt_assets(&data_dir).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to cleanup obsolete local STT assets");
            false
        }) {
            tracing::info!("removed obsolete local STT assets");
        }
        // KOS-301: crash mid-download оставлял .download/.part/.extracting
        // навсегда; no download can be in flight this early in startup.
        model_sweep::sweep_stale_downloads(&data_dir);
        let mut cfg = config::load_from(&data_dir.join("dictation-config.json"));
        let mut config_changed = normalize_platform_local_engine(&mut cfg);
        if local_models::cleanup_unused_backends(&data_dir).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to cleanup unused dictation local STT backends");
            false
        }) {
            cfg.local_model = None;
            cfg.local_model_path = None;
            cfg.local_command_path = None;
            config_changed = true;
        }
        if local_models::refresh_managed_command_path(&data_dir, &mut cfg) {
            config_changed = true;
        }
        if autoselect_local_model(&data_dir, &mut cfg) {
            config_changed = true;
        } else if !local_config_is_ready(&data_dir, &cfg) {
            clear_local_selection(&mut cfg);
            config_changed = true;
        }
        if config_changed {
            if let Err(e) = save_config_in(&data_dir, &cfg) {
                tracing::warn!(error = %e, "failed to save refreshed dictation local command path");
            }
        }
        let stats = stats::load_from(&data_dir.join("dictation-stats.json"));
        let (events_tx, _) = broadcast::channel::<Value>(64);
        let host = Arc::new(Self {
            state: Arc::new(Mutex::new(HostState::idle())),
            config: Arc::new(Mutex::new(cfg.clone())),
            stats: Arc::new(Mutex::new(stats)),
            events_tx: events_tx.clone(),
            data_dir,
            groq_endpoint: groq::GROQ_ENDPOINT.to_string(),
            capture: std::sync::Mutex::new(None),
            contract_window_id: std::sync::Mutex::new(None),
            contract_events: std::sync::atomic::AtomicBool::new(false),
        });
        // Активируем PTT hook соответственно текущему trigger_mode.
        apply_ptt_hook(&cfg, &events_tx);
        // GC pending queue + emit notifier если есть items.
        host.bootstrap_pending();
        let prewarm_host = Arc::clone(&host);
        tokio::spawn(async move {
            preload_local_runtime_for_recording(&prewarm_host).await;
        });
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
            capture: std::sync::Mutex::new(None),
            contract_window_id: std::sync::Mutex::new(None),
            contract_events: std::sync::atomic::AtomicBool::new(false),
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
        if self
            .contract_events
            .load(std::sync::atomic::Ordering::Acquire)
        {
            let _ = self.events_tx.send(json!({
            "event": "dictation.state_changed",
            "state": match state.name {
                DictationStateName::Recording => "capturing",
                DictationStateName::Transcribing => "transcribing",
                DictationStateName::Pending => "pending",
                DictationStateName::Error => "error",
                DictationStateName::Idle => "idle",
            },
            "errorCode": state.last_error,
            "requestId": state.active_uuid,
            }));
            if let Some(error) = state.last_error.as_deref() {
                let error_code = if error.contains("API key") {
                    "auth_required"
                } else if error.contains("model") || error.contains("модель") {
                    "model_unavailable"
                } else {
                    "provider_disabled"
                };
                let _ = self.events_tx.send(json!({
                    "event": "dictation.error",
                    "requestId": state.active_uuid,
                    "errorCode": error_code,
                    "retryable": state.can_retry,
                }));
            }
        }
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
    pub(crate) fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data,
            error: None,
        }
    }
    pub(crate) fn err(msg: impl Into<String>) -> Self {
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
    #[error("local: {0}")]
    Local(#[from] local::LocalError),
    #[error("groq: {0}")]
    Groq(#[from] GroqError),
    #[error("inject: {0}")]
    Inject(#[from] InjectError),
}

fn provider_uses_local_runtime(provider: &str) -> bool {
    provider == "local"
}

fn provider_needs_api_key(cfg: &DictationConfig) -> bool {
    if provider_uses_local_runtime(&cfg.provider) {
        return false;
    }
    mock_dictation_transcript_override(&cfg.provider, Some(cfg.transcription_prompt.as_str()))
        .is_none()
}

fn effective_model_for_submit(cfg: &DictationConfig) -> String {
    if provider_uses_local_runtime(&cfg.provider) {
        return cfg
            .local_model
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| cfg.model.clone());
    }
    cfg.model.clone()
}

const LOCAL_MODEL_NOT_READY_MSG: &str =
    "Локальная модель не подготовлена. Скачайте модель заново в Settings -> AI.";

/// Is the *selected* local model usable on disk — independent of
/// `provider_enabled`. A disabled provider with a surviving selection is
/// the user's own off-switch, not a broken config.
fn local_selection_valid(cfg: &DictationConfig) -> bool {
    if !provider_uses_local_runtime(&cfg.provider) {
        return true;
    }
    let model_ok = cfg.local_model_path.as_deref().is_some_and(|path| {
        let path = std::path::Path::new(path);
        path.is_file() || path.is_dir()
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

fn local_config_is_ready(_data_dir: &std::path::Path, cfg: &DictationConfig) -> bool {
    !cfg.provider_enabled
        || !provider_uses_local_runtime(&cfg.provider)
        || local_selection_valid(cfg)
}

fn clear_local_selection(cfg: &mut DictationConfig) {
    cfg.provider_enabled = false;
    cfg.local_model = None;
    cfg.local_model_path = None;
    cfg.local_command_path = None;
}

/// Runtime reconcile for a broken local selection: prefer auto-selecting a
/// downloaded model (a deleted file is recoverable when another model is
/// on disk); only with nothing downloaded does the provider get disabled.
async fn reconcile_unready_local_config(host: &DictationHost) -> bool {
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
