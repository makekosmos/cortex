use super::*;

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
pub(super) struct HostState {
    pub(super) name: DictationStateName,
    /// HWND foreground'а, захваченный перед показом pill. Используется в
    /// inject phase для возврата фокуса.
    pub(super) prev_hwnd: Option<isize>,
    /// Сообщение для UI (user-facing, локализованное).
    pub(super) last_error: Option<String>,
    /// UUID активного pending item (Transcribing/Pending/Error) для retry-кнопок.
    pub(super) active_uuid: Option<String>,
    /// Счётчик попыток для UI (Pending state).
    pub(super) attempts: u32,
    /// Можно ли retry-ить текущую Error-сессию (true для Fatal от Groq,
    /// false для AudioDecode — там аудио бесполезно).
    pub(super) can_retry: bool,
}

impl HostState {
    pub(super) fn idle() -> Self {
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
    pub(super) state: Arc<Mutex<HostState>>,
    pub(super) config: Arc<Mutex<DictationConfig>>,
    pub(super) stats: Arc<Mutex<DictationStats>>,
    pub(super) events_tx: broadcast::Sender<Value>,
    /// Корневая директория для config/stats/pending. `config::data_dir()` в
    /// проде; tempdir в тестах через `new_for_test`.
    pub(super) data_dir: std::path::PathBuf,
    /// Endpoint Groq transcriptions API. По умолчанию `groq::GROQ_ENDPOINT`;
    /// override для тестов на httpmock.
    pub(super) groq_endpoint: String,
    pub(super) capture: std::sync::Mutex<Option<crate::native_capture::Session>>,
    pub(super) contract_window_id: std::sync::Mutex<Option<String>>,
    pub(super) contract_events: std::sync::atomic::AtomicBool,
    /// In-flight `download_local_model` ops: model_id → last percent
    /// (`None` = indeterminate phase). Polled via `list_local_models` —
    /// Manager has no WS subscription.
    pub(super) downloads: Arc<std::sync::Mutex<local_models::ModelDownloads>>,
    /// The pill's dot scope, stepped on the display's cadence while a session
    /// runs. Frames go out as `dictation_pill_frame`; see `pill_frame.rs`.
    pub(super) pill: Arc<crate::pill_frame::PillTicker>,
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
        // crash mid-download оставлял.download/.part/.extracting
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
            downloads: Arc::new(std::sync::Mutex::new(local_models::ModelDownloads::new())),
            pill: Arc::new(crate::pill_frame::PillTicker::new()),
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
    #[cfg(any(test, feature = "test-support"))]
    pub fn new_for_test(
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
            downloads: Arc::new(std::sync::Mutex::new(local_models::ModelDownloads::new())),
            pill: Arc::new(crate::pill_frame::PillTicker::new()),
        })
    }

    /// На старте: GC pending по 7d/20 items. Если что-то осталось — emit
    /// `dictation_pending_changed` чтобы UI показал нотификацию.
    pub(super) fn bootstrap_pending(&self) {
        // История диктовок живёт 30 дней, потолок — 200 записей.
        let _ = crate::pending::gc(&self.data_dir, 200, chrono::Duration::days(30));
        if let Ok(items) = crate::pending::list(&self.data_dir) {
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

    pub(super) async fn emit_state(&self, state: &HostState) {
        self.drive_pill(state.name);
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

    /// Пилюля — тот же state, только в терминах сцены: запись, обработка,
    /// готово, ошибка. Первый кадр появляется только с первой записью, чтобы
    /// при старте движка пилюля не мигала.
    fn drive_pill(&self, name: DictationStateName) {
        use crate::pill_scope::Stage;
        let stage = match name {
            DictationStateName::Recording => Stage::Recording,
            DictationStateName::Transcribing | DictationStateName::Pending => Stage::Processing,
            DictationStateName::Error => Stage::Failed,
            DictationStateName::Idle => Stage::Done,
        };
        self.pill.set_stage(stage);
        if matches!(stage, Stage::Recording) {
            self.pill.begin(self.events_tx.clone());
        }
    }

    /// Helper для перехода в Error state с понятным user_msg.
    /// `can_retry` = true для retryable errors (UI покажет кнопку «Повторить»).
    pub(super) async fn fail_session(&self, uuid: &str, user_msg: &str, can_retry: bool) {
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

    pub(super) fn emit_pending_changed(&self) {
        let count = crate::pending::list(&self.data_dir)
            .map(|v| v.len())
            .unwrap_or(0);
        let _ = self.events_tx.send(json!({
            "event": "dictation_pending_changed",
            "count": count,
        }));
    }

    pub(super) fn emit_config_changed(&self) {
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

    pub(super) async fn snapshot_config(&self) -> DictationConfig {
        self.config.lock().await.clone()
    }

    pub(super) fn downloads_snapshot(&self) -> local_models::ModelDownloads {
        self.downloads
            .lock()
            .map(|downloads| downloads.clone())
            .unwrap_or_default()
    }

    /// `localModels` snapshot for op replies — config plus live download
    /// progress so poll-only clients (Manager) can render a progress bar.
    pub(super) fn local_models_snapshot(
        &self,
        cfg: &DictationConfig,
    ) -> local_models::LocalModelsSnapshot {
        local_models::snapshot(&self.data_dir, cfg, &self.downloads_snapshot())
    }
}

/// Value shapes and provider predicates live in their own file; the host keeps
/// the state machine and re-exports them, so call sites stay as they were.
#[path = "host_state/values.rs"]
mod values;
pub use values::*;
