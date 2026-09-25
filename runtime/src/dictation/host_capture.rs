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

async fn op_contract_foreground(host: &DictationHost) -> DictationResponse {
    let hwnd = inject::capture_foreground_window();
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

async fn op_capture_start(params: Value, host: &Arc<DictationHost>) -> DictationResponse {
    host.contract_events
        .store(true, std::sync::atomic::Ordering::Release);
    if host.capture.lock().is_ok_and(|capture| capture.is_some()) {
        return DictationResponse::err("busy");
    }
    let device_id = params.get("deviceId").and_then(Value::as_str).map(str::to_owned);
    let capture_id = uuid::Uuid::new_v4().to_string();
    let started = op_start_recording(host).await;
    if !started.ok {
        return started;
    }
    // Live RMS-уровни микрофона → broadcast `dictation_audio_level`, чтобы
    // pill-оверлей мог рисовать waveform (паритет с Vue pill AnalyserNode).
    // broadcast::send синхронный и не блокирует — вызывается прямо из
    // capture-потока через mpsc-переходник.
    let (level_tx, level_rx) = std::sync::mpsc::channel::<f32>();
    {
        let events_tx = host.events_tx.clone();
        let level_capture_id = capture_id.clone();
        std::thread::Builder::new()
            .name("kosmos-dictation-levels".into())
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
    }
    let native_capture_id = capture_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        super::native_capture::start(device_id.as_deref(), native_capture_id, Some(level_tx))
    })
    .await;
    match result {
        Ok(Ok((session, sample_rate, channels))) => {
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
    let session = host
        .capture
        .lock()
        .expect("capture mutex poisoned")
        .take();
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
    let Some(text) = params.get("text").and_then(Value::as_str).map(str::to_owned) else {
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
    // whisper-server.exe работает только «холодно» (модель грузится заново на
    // каждую диктовку). Докачиваем обновлённый рантайм (r2) в фоне — после
    // успеха тёплый GPU-путь включается автоматически.
    if cfg.local_engine != local::PARAKEET_LOCAL_ENGINE
        && local_models::vulkan_runtime_needs_server_repair(&host.data_dir)
    {
        let data_dir = host.data_dir.clone();
        let network_profile = cfg.network_profile.clone();
        let http_proxy = cfg.http_proxy.clone();
        tokio::spawn(async move {
            let client = match network::build_download_client(
                &network_profile,
                http_proxy.as_deref(),
            ) {
                Ok(client) => client,
                Err(e) => {
                    tracing::warn!(error = %e, "dictation: whisper-server repair client build failed");
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
                tracing::info!(engine = %engine, warm, duration_ms, "dictation: local STT preload finished");
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

    // Декод base64 — fatal без retry (битое аудио переотправлять бессмысленно).
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

    // Disk-first: enqueue до любого HTTP. Crash после этой точки не теряет аудио.
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
        let _ = clear_unready_local_config(host).await;
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

    // Одна inline-попытка — pill показывает «Распознаю…» ~1s в happy-path.
    // На фейле — spawn'им auto-retry в фоне (5/10/20/40s) и сразу возвращаем
    // OK с state=idle, чтобы pill закрылся без перехвата фокуса.
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
