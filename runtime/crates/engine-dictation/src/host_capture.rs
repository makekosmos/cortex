fn apply_ptt_hook(cfg: &DictationConfig, _tx: &broadcast::Sender<Value>) {
    #[cfg(windows)]
    {
        // Режима больше нет в UI: завершение — тем же хоткеем (toggle),
        // отмена — двойной Esc. trigger_mode в конфиге игнорируется.
        let mode = hotkey_hook::HookMode::Toggle;
        if let Some(matcher) = hotkey_hook::parse_accelerator(&cfg.hotkey) {
            hotkey_hook::set_active(Some(matcher), Some(_tx.clone()), mode);
        } else {
            eprintln!(
                "[dictation::host] hotkey '{}' не парсится — \
                 hook деактивирован",
                cfg.hotkey
            );
            hotkey_hook::set_active(None, None, mode);
        }
    }
    #[cfg(not(windows))]
    {
        #[cfg(target_os = "macos")]
        {
            if let Err(e) =
                // Toggle-only: завершение — тем же хоткеем (см. коммент выше).
                crate::macos_native::set_hotkey_active(
                    &cfg.hotkey,
                    TriggerMode::Toggle,
                    _tx.clone(),
                )
            {
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
            if let Err(e) = save_config_in(&host.data_dir, &cfg) {
                return DictationResponse::err(format!("set_api_key: save failed: {e}"));
            }
            drop(cfg);
            host.emit_config_changed();
            DictationResponse::ok(json!({ "ok": true }))
        }
        Err(e) => DictationResponse::err(format!("keyring write failed: {e}")),
    }
}

async fn op_capture_foreground(host: &DictationHost) -> DictationResponse {
    let hwnd = inject::capture_foreground_window();
    eprintln!("[dictation::inject] capture_foreground op result: {hwnd:?}");
    let mut s = host.state.lock().await;
    s.prev_hwnd = hwnd;
    DictationResponse::ok(json!({ "captured": hwnd.is_some() }))
}

async fn op_contract_foreground(host: &DictationHost) -> DictationResponse {
    let hwnd = inject::capture_foreground_window();
    eprintln!("[dictation::inject] contract_foreground op result: {hwnd:?}");
    if hwnd.is_none() {
        return DictationResponse::err("device_unavailable");
    }
    let window_id = uuid::Uuid::new_v4().to_string();
    let mut state = host.state.lock().await;
    state.prev_hwnd = hwnd;
    *host
        .contract_window_id
        .lock()
        .expect("window mutex poisoned") = Some(window_id.clone());
    DictationResponse::ok(json!({ "windowId": window_id }))
}

/// Каталог Parakeet-модели, если для этой записи имеет смысл stream-
/// транскриба: local provider + engine=parakeet + модель на месте —
/// то же условие, что у preload.
#[cfg(feature = "local-dictation")]
async fn stream_model_dir_for_streaming(host: &DictationHost) -> Option<std::path::PathBuf> {
    let cfg = host.snapshot_config().await;
    let eligible = cfg.provider_enabled
        && provider_uses_local_runtime(&cfg.provider)
        && cfg.local_engine == local::PARAKEET_LOCAL_ENGINE
        && local_config_is_ready(&host.data_dir, &cfg);
    if !eligible {
        return None;
    }
    cfg.local_model_path
        .map(std::path::PathBuf::from)
        .filter(|dir| dir.is_dir())
}

#[cfg(not(feature = "local-dictation"))]
async fn stream_model_dir_for_streaming(_host: &DictationHost) -> Option<std::path::PathBuf> {
    None
}

/// Опциональный файловый источник захвата (e2e): `sourceFile` — абсолютный
/// путь к wav/mp3/m4a, `sourceSpeed` — темп подачи (по умолчанию 1.0,
/// зажато в (0, 64]).
fn parse_capture_source(
    params: &Value,
) -> Result<Option<super::native_capture::CaptureSource>, String> {
    let Some(file) = params.get("sourceFile") else {
        return Ok(None);
    };
    let Some(path_str) = file.as_str() else {
        return Err("capture.start: sourceFile must be a string".into());
    };
    let path = std::path::PathBuf::from(path_str);
    if !path.is_file() {
        return Err(format!(
            "capture.start: sourceFile does not exist: {path_str}"
        ));
    }
    let speed = match params.get("sourceSpeed") {
        None => 1.0,
        Some(v) => v
            .as_f64()
            .filter(|s| s.is_finite())
            .ok_or("capture.start: sourceSpeed must be a finite number")?,
    };
    Ok(Some(super::native_capture::CaptureSource {
        path,
        speed: speed.clamp(0.01, 64.0) as f32,
    }))
}

async fn op_capture_start(params: Value, host: &Arc<DictationHost>) -> DictationResponse {
    host.contract_events
        .store(true, std::sync::atomic::Ordering::Release);
    if host.capture.lock().is_ok_and(|capture| capture.is_some()) {
        return DictationResponse::err("busy");
    }
    let device_id = params
        .get("deviceId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let source = match parse_capture_source(&params) {
        Ok(source) => source,
        Err(error) => return DictationResponse::err(error),
    };
    let capture_id = uuid::Uuid::new_v4().to_string();
    let started = op_start_recording(host).await;
    if !started.ok {
        return started;
    }
    // Live RMS-уровни микрофона → broadcast `dictation_audio_level`, чтобы
    // pill-оверлей мог рисовать waveform (паритет с Vue pill
    // AnalyserNode). Файловый источник (e2e) — без broadcast'ов: pill в
    // GPUI открывается именно по audio_level-событиям, а тестовая запись
    // не должна мелькать на экране пользователя.
    let level_sink = if source.is_none() {
        let (level_tx, level_rx) = std::sync::mpsc::channel::<f32>();
        let events_tx = host.events_tx.clone();
        let level_capture_id = capture_id.clone();
        std::thread::Builder::new()
            .name("mundus-dictation-levels".into())
            .spawn(move || {
                for level in level_rx {
                    let _ = events_tx.send(json!({
                        "event": "dictation_audio_level",
                        "captureId": level_capture_id,
                        "level": level,
                    }));
                }
            })
            .ok();
        Some(level_tx)
    } else {
        None
    };
    let stream_model_dir = stream_model_dir_for_streaming(host).await;
    let (pcm_tx, pcm_rx) = std::sync::mpsc::channel::<Vec<i16>>();
    let pcm_sink = stream_model_dir.as_ref().map(|_| pcm_tx);
    let native_capture_id = capture_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        super::native_capture::start(
            device_id.as_deref(),
            native_capture_id,
            level_sink,
            pcm_sink,
            source,
        )
    })
    .await;
    match result {
        Ok(Ok((session, sample_rate, channels))) => {
            if let Some(model_dir) = stream_model_dir {
                #[cfg(feature = "local-dictation")]
                local::start_parakeet_stream(model_dir, pcm_rx);
                #[cfg(not(feature = "local-dictation"))]
                let _ = (model_dir, pcm_rx);
            }
            *host.capture.lock().expect("capture mutex poisoned") = Some(session);
            DictationResponse::ok(json!({
                "captureId": capture_id,
                "sampleRate": sample_rate,
                "channels": channels,
                "format": "wav"
            }))
        }
        Ok(Err(error)) => {
            let _ = op_cancel(host).await;
            DictationResponse::err(error)
        }
        Err(error) => {
            let _ = op_cancel(host).await;
            DictationResponse::err(format!("capture.start: {error}"))
        }
    }
}

async fn op_capture_stop(params: Value, host: &Arc<DictationHost>) -> DictationResponse {
    let Some(capture_id) = params.get("captureId").and_then(Value::as_str) else {
        return DictationResponse::err("capture.stop: missing captureId");
    };
    if capture_id.is_empty() {
        return DictationResponse::err("capture.stop: invalid captureId");
    }
    let session = host.capture.lock().expect("capture mutex poisoned").take();
    let Some(session) = session else {
        return DictationResponse::err("capture.stop: capture not active");
    };
    if session.capture_id != capture_id {
        *host.capture.lock().expect("capture mutex poisoned") = Some(session);
        return DictationResponse::err("capture.stop: captureId does not match active capture");
    }
    let result = tokio::task::spawn_blocking(move || super::native_capture::stop(session)).await;
    match result {
        Ok(Ok(audio)) => {
            // Keep the foreground capability token alive for the worker's
            // subsequent speech.transcribe → input.insert_text sequence.
            super::audio_duck::restore();
            // Дождаться stream-потока (in-flight чанк дозавершится) и
            // сохранить partial для transcribe. Join'им вне async-runtime.
            #[cfg(feature = "local-dictation")]
            tokio::task::spawn_blocking(local::finish_parakeet_stream)
                .await
                .ok();
            let mut state = host.state.lock().await;
            *state = HostState::idle();
            let snapshot = state.clone();
            drop(state);
            host.emit_state(&snapshot).await;
            DictationResponse::ok(json!({
                "audioB64": base64::engine::general_purpose::STANDARD.encode(audio.wav),
                "format": audio.format,
                "durationMs": audio.duration_ms
            }))
        }
        Ok(Err(error)) => {
            let _ = op_cancel(host).await;
            DictationResponse::err(error)
        }
        Err(error) => {
            let _ = op_cancel(host).await;
            DictationResponse::err(format!("capture.stop: {error}"))
        }
    }
}

async fn op_speech_transcribe(params: Value, host: &Arc<DictationHost>) -> DictationResponse {
    host.contract_events
        .store(true, std::sync::atomic::Ordering::Release);
    if params.get("audioB64").and_then(Value::as_str).is_none() {
        return DictationResponse::err("speech.transcribe: missing audioB64");
    }
    let state = host.current_state().await;
    if state.get("state").and_then(Value::as_str) == Some("idle") {
        let started = op_start_recording(host).await;
        if !started.ok {
            return started;
        }
    }
    op_submit_audio(params, host).await
}

async fn op_insert_text(params: Value, host: &DictationHost) -> DictationResponse {
    let Some(text) = params
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return DictationResponse::err("input.insert_text: missing text");
    };
    let Some(target) = params.get("targetWindow").and_then(Value::as_str) else {
        return DictationResponse::err("input.insert_text: missing targetWindow");
    };
    let matches = {
        let mut token = host
            .contract_window_id
            .lock()
            .expect("window mutex poisoned");
        let matches = token.as_deref() == Some(target);
        if matches {
            *token = None;
        }
        matches
    };
    if !matches {
        return DictationResponse::err("window_unavailable");
    }
    let prev_hwnd = host.state.lock().await.prev_hwnd;
    match tokio::task::spawn_blocking(move || {
        inject::inject_blocking(&text, InjectMode::AutoPaste, prev_hwnd)
    })
    .await
    {
        Ok(Ok(())) => DictationResponse::ok(json!({ "inserted": true, "method": "paste" })),
        Ok(Err(error)) => DictationResponse::err(format!("input.insert_text: {error}")),
        Err(error) => DictationResponse::err(format!("input.insert_text: {error}")),
    }
}

/// In-engine обработка триггеров хоткея (`dictation.trigger`,
/// `dictation_escape_cancel`): весь конвейер capture → transcribe → inject
/// живёт в этом процессе, внешний клиент нужен только для UI. Вызывается
/// из trigger-loop в engine main.rs, когда dictation worker недоступен
/// или не установлен.
pub async fn handle_engine_trigger(host: &Arc<DictationHost>, event: &Value) {
    match event.get("event").and_then(Value::as_str) {
        Some("dictation_escape_cancel") | Some("dictation_capture_cancelled") => {
            let _ = op_cancel(host).await;
        }
        Some("dictation.trigger") => {
            let kind = event
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("toggle");
            let phase = event.get("phase").and_then(Value::as_str).unwrap_or("down");
            match (kind, phase) {
                ("ptt", "down") => engine_begin_capture(host).await,
                ("ptt", "up") => engine_finish_capture(host).await,
                ("toggle", "down") => {
                    let state = host.current_state().await;
                    if state.get("state").and_then(Value::as_str) == Some("recording") {
                        engine_finish_capture(host).await;
                    } else {
                        engine_begin_capture(host).await;
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

/// Маршрутизация одного события хоткея: worker → живое dictation-приложение
/// (WS subscriber, обрабатывает тот же broadcast) → in-engine fallback.
/// `invoke_worker`, `app_alive` и `engine_handle` инжектятся, чтобы роутинг
/// тестировался без worker'а, процессов и аудио. `dictation_escape_cancel`
/// — safety cancel, он идёт в engine безусловно и идемпотентен.
pub async fn route_trigger_event<Invoke, InvokeFut, AppAlive, Engine, EngineFut, E>(
    event: &Value,
    invoke_worker: Invoke,
    app_alive: AppAlive,
    engine_handle: Engine,
) where
    Invoke: FnOnce(Value) -> InvokeFut,
    InvokeFut: std::future::Future<Output = Result<Value, E>>,
    E: std::fmt::Display,
    AppAlive: FnOnce() -> bool,
    Engine: FnOnce(Value) -> EngineFut,
    EngineFut: std::future::Future<Output = ()>,
{
    match event.get("event").and_then(Value::as_str) {
        Some("dictation.trigger") => {
            let params = json!({
                "kind": event.get("kind").and_then(Value::as_str),
                "phase": event.get("phase").and_then(Value::as_str),
            });
            match invoke_worker(params).await {
                Ok(_) => {}
                Err(error) if app_alive() => {
                    tracing::debug!(
                        error = %error,
                        "dictation trigger worker unavailable; app is running"
                    );
                }
                Err(error) => {
                    tracing::debug!(
                        error = %error,
                        "dictation trigger: no worker, no app; handling in-engine"
                    );
                    engine_handle(event.clone()).await;
                }
            }
        }
        Some("dictation_escape_cancel") => engine_handle(event.clone()).await,
        _ => {}
    }
}

/// Тот же happy path, что делает worker: захват foreground HWND, затем
/// `capture.start` на сконфигурированном микрофоне.
async fn engine_begin_capture(host: &Arc<DictationHost>) {
    let _ = op_capture_foreground(host).await;
    let device_id = host.snapshot_config().await.microphone_device_id;
    let params = match device_id {
        Some(id) => json!({ "deviceId": id }),
        None => json!({}),
    };
    let resp = op_capture_start(params, host).await;
    if !resp.ok {
        tracing::warn!(
            error = ?resp.error,
            "dictation: engine trigger capture.start failed"
        );
    }
}

/// `capture.stop` активной сессии → `speech.transcribe` (тот сам транскрибирует
/// и инжектит через SystemInjector с захваченным prev_hwnd).
async fn engine_finish_capture(host: &Arc<DictationHost>) {
    let capture_id = host
        .capture
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|s| s.capture_id.clone()));
    let Some(capture_id) = capture_id else {
        // Записи нет (например, старая сессия без native capture) — сброс.
        let _ = op_cancel(host).await;
        return;
    };
    let stopped = op_capture_stop(json!({ "captureId": capture_id }), host).await;
    if !stopped.ok {
        tracing::warn!(
            error = ?stopped.error,
            "dictation: engine trigger capture.stop failed"
        );
        return;
    }
    let audio_b64 = stopped
        .data
        .get("audioB64")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if audio_b64.is_empty() {
        return;
    }
    let duration_sec = stopped
        .data
        .get("durationMs")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        / 1000.0;
    // Транскрибация (~1s) в фоне — иначе trigger-loop не обработает
    // дабл-Esc отмену до конца распознавания.
    let host = host.clone();
    tokio::spawn(async move {
        let resp = op_speech_transcribe(
            json!({ "audioB64": audio_b64, "durationSec": duration_sec }),
            &host,
        )
        .await;
        if !resp.ok {
            tracing::warn!(
                error = ?resp.error,
                "dictation: engine trigger speech.transcribe failed"
            );
        }
    });
}

async fn preload_local_runtime_for_recording(host: &DictationHost) {
    #[cfg(test)]
    {
        if !local::has_test_sidecar_mock_for_host() {
            return;
        }
    }

    let cfg = host.snapshot_config().await;
    if !cfg.provider_enabled || !provider_uses_local_runtime(&cfg.provider) {
        return;
    }
    if !local_config_is_ready(&host.data_dir, &cfg) {
        tracing::warn!("dictation: local STT preload skipped because selected model is not ready");
        return;
    }

    // Self-heal для существующих установок: Vulkan whisper-cli без
    // whisper-server.exe работает только «холодно» (модель грузится
    // заново на каждую диктовку). Докачиваем обновлённый рантайм (r2)
    // в фоне — после
    // успеха тёплый GPU-путь включается автоматически.
    if cfg.local_engine != local::PARAKEET_LOCAL_ENGINE
        && local_models::vulkan_runtime_needs_server_repair(&host.data_dir)
    {
        let data_dir = host.data_dir.clone();
        let network_profile = cfg.network_profile.clone();
        let http_proxy = cfg.http_proxy.clone();
        tokio::spawn(async move {
            let client =
                match network::build_download_client(&network_profile, http_proxy.as_deref()) {
                    Ok(client) => client,
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "dictation: whisper-server repair client build failed",
                        );
                        return;
                    }
                };
            match local_models::ensure_whisper_cpp(&client, &data_dir).await {
                Ok(_) => tracing::info!(
                    "dictation: whisper-server runtime repaired — warm GPU path enabled"
                ),
                Err(e) => {
                    tracing::warn!(error = %e, "dictation: whisper-server runtime repair failed")
                }
            }
        });
    }

    let engine = cfg.local_engine.clone();
    let model_path = cfg.local_model_path.clone();
    let command_path = cfg.local_command_path.clone();
    tokio::spawn(async move {
        let start = std::time::Instant::now();
        match local::preload_server(&engine, model_path.as_deref(), command_path.as_deref()).await {
            Ok(warm) => {
                let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                tracing::info!(
                    engine = %engine,
                    warm,
                    duration_ms,
                    "dictation: local STT preload finished",
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "dictation: local STT preload failed");
            }
        }
    });
}

async fn op_start_recording(host: &DictationHost) -> DictationResponse {
    let cfg = host.snapshot_config().await;
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
    tracing::info!(state = "recording", "dictation recording started");
    super::audio_duck::duck_if_enabled(cfg.duck_audio_during_recording);
    preload_local_runtime_for_recording(host).await;
    DictationResponse::ok(json!({ "state": "recording" }))
}

async fn op_cancel(host: &DictationHost) -> DictationResponse {
    super::audio_duck::restore();
    // Остановить stream-транскрибу и выбросить её partial — запись
    // отменена, частичный текст непригоден.
    #[cfg(feature = "local-dictation")]
    {
        tokio::task::spawn_blocking(local::finish_parakeet_stream)
            .await
            .ok();
        local::discard_parakeet_partial();
    }
    // Глушим живой захват — без этого helper (macOS) остаётся запущенным и
    // следующий capture.start получает «busy».
    let session = host.capture.lock().expect("capture mutex poisoned").take();
    if let Some(session) = session {
        tokio::task::spawn_blocking(move || {
            let _ = super::native_capture::stop(session);
        });
    }
    *host
        .contract_window_id
        .lock()
        .expect("window mutex poisoned") = None;
    *host
        .contract_window_id
        .lock()
        .expect("window mutex poisoned") = None;
    let mut s = host.state.lock().await;
    let should_cancel_sidecar = matches!(
        s.name,
        DictationStateName::Recording | DictationStateName::Transcribing
    );
    *s = HostState::idle();
    let snapshot = s.clone();
    drop(s);
    if should_cancel_sidecar {
        tokio::spawn(async {
            if let Err(e) = local::cancel_sidecar().await {
                tracing::warn!(error = %e, "dictation: local STT cancel failed");
            }
        });
    }
    host.emit_state(&snapshot).await;
    tracing::info!(state = "idle", "dictation recording cancelled");
    DictationResponse::ok(json!({ "state": "idle" }))
}

async fn op_submit_audio(params: Value, host: &Arc<DictationHost>) -> DictationResponse {
    super::audio_duck::restore();
    let audio_b64 = match params.get("audioB64").and_then(|v| v.as_str()) {
        Some(s) => s.to_owned(),
        None => return DictationResponse::err("submit_audio: missing 'audioB64'"),
    };
    let record_seconds = params
        .get("durationSec")
        .and_then(|v| v.as_f64())
        .map(|f| f.max(0.0).round() as f32)
        .unwrap_or(0.0);

    // Декод base64 — fatal без retry (битое аудио переотправлять
    // бессмысленно).
    let wav_bytes = match base64::engine::general_purpose::STANDARD.decode(&audio_b64) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "dictation: base64 decode failed");
            return DictationResponse::err(format!("submit_audio: битое аудио ({e})"));
        }
    };
    tracing::info!(
        bytes = wav_bytes.len(),
        duration_sec = record_seconds,
        "dictation audio received"
    );

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

    // Disk-first: enqueue до любого HTTP. Crash после этой точки не
    // теряет аудио.
    let opts = super::pending::EnqueueOpts {
        language: cfg.language.clone(),
        prompt: cfg.transcription_prompt.clone(),
        inject_mode: match cfg.inject_mode {
            InjectMode::AutoPaste => "auto_paste".into(),
            InjectMode::ClipboardOnly => "clipboard_only".into(),
        },
        model: effective_model_for_submit(&cfg),
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
        let local_provider_without_model =
            provider_uses_local_runtime(&cfg.provider) && cfg.local_model.is_none();
        let user_msg = if local_provider_without_model {
            LOCAL_MODEL_NOT_READY_MSG
        } else {
            "Поставщик диктовки выключен в настройках"
        };
        tracing::warn!(
            %uuid,
            provider = %cfg.provider,
            local_engine = %cfg.local_engine,
            local_model = ?cfg.local_model,
            "dictation: provider disabled"
        );
        host.fail_session(&uuid, user_msg, false).await;
        host.emit_pending_changed();
        return DictationResponse::ok(json!({
            "uuid": uuid,
            "state": "error",
            "error": user_msg,
            "queued": false,
        }));
    }
    if provider_uses_local_runtime(&cfg.provider) && !local_config_is_ready(&host.data_dir, &cfg) {
        tracing::warn!(
            %uuid,
            provider = %cfg.provider,
            local_engine = %cfg.local_engine,
            local_model = ?cfg.local_model,
            "dictation: local model is not ready"
        );
        let _ = reconcile_unready_local_config(host).await;
        host.fail_session(&uuid, LOCAL_MODEL_NOT_READY_MSG, false)
            .await;
        host.emit_pending_changed();
        return DictationResponse::ok(json!({
            "uuid": uuid,
            "state": "error",
            "error": LOCAL_MODEL_NOT_READY_MSG,
            "queued": false,
        }));
    }
    let api_key = if provider_needs_api_key(&cfg) {
        match config::get_api_key() {
            Some(k) => k,
            None => {
                host.fail_session(&uuid, "API key не задан", false).await;
                host.emit_pending_changed();
                return DictationResponse::err("submit_audio: API key не задан");
            }
        }
    } else {
        String::new()
    };

    // Одна inline-попытка — pill показывает «Распознаю…» ~1s в
    // happy-path. На фейле — spawn'им auto-retry в фоне (5/10/20/40s)
    // и сразу возвращаем OK с state=idle, чтобы pill закрылся без
    // перехвата фокуса.
    let outcome = if params.get("delivery").and_then(Value::as_str) == Some("text_only") {
        process_one_attempt_with_injector(
            host,
            &uuid,
            &api_key,
            record_seconds,
            AttemptDelivery::Active,
            std::sync::Arc::new(TextOnlyInjector),
        )
        .await
    } else {
        process_one_attempt(
            host,
            &uuid,
            &api_key,
            record_seconds,
            AttemptDelivery::Active,
        )
        .await
    };
    match outcome {
        AttemptOutcome::Success {
            text,
            injected,
            delivery,
        } => {
            // process_one_attempt уже перевёл state в Idle.
            DictationResponse::ok(json!({
                "uuid": uuid,
                "text": text,
                "state": "idle",
                "injected": injected,
                "delivery": delivery.as_str(),
                "deliveryReason": delivery.reason(),
            }))
        }
        AttemptOutcome::Cancelled => DictationResponse::ok(json!({
            "uuid": uuid,
            "state": "idle",
            "queued": false,
            "cancelled": true,
        })),
        AttemptOutcome::Fatal => {
            // Фатально (401/400/403/etc) — НЕ спавним auto-retry.
            // Возвращаем pill state="error" с user_msg чтобы он показал
            // понятную ошибку и закрылся. Pending остаётся на диске —
            // после фикса конфига (DoH/proxy/новый key) можно retry'ить
            // из Settings → Очередь.
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
        AttemptOutcome::DeliveryFailed { reason } => DictationResponse::ok(json!({
            "uuid": uuid,
            "state": "error",
            "error": "Не удалось доставить текст — диктовка осталась в очереди",
            "queued": true,
            "injected": false,
            "delivery": "failed",
            "deliveryReason": reason,
        })),
    }
}
