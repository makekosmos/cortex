const AUTO_RETRY_DELAYS_SEC: [u64; 5] = [1, 5, 10, 20, 40];

fn is_dictation_test_mode() -> bool {
    matches!(std::env::var("MUNDUS_TEST_MODE").as_deref(), Ok("1"))
        || matches!(std::env::var("MUNDUS_HEADLESS").as_deref(), Ok("1"))
}

fn mock_dictation_transcript_override(provider: &str, fallback: Option<&str>) -> Option<String> {
    if !is_dictation_test_mode() || provider != "mock" {
        return None;
    }
    if let Ok(transcript) = std::env::var("MUNDUS_TEST_DICTATION_TRANSCRIPT") {
        let transcript = transcript.trim();
        if !transcript.is_empty() {
            return Some(transcript.to_string());
        }
    }
    let transcript = fallback?.trim();
    if transcript.is_empty() {
        return None;
    }
    Some(transcript.to_string())
}

fn resolve_attempt_inject_mode(raw_mode: &str, mock_transcript: Option<&str>) -> InjectMode {
    if mock_transcript.is_some() {
        return InjectMode::ClipboardOnly;
    }
    match raw_mode {
        "clipboard_only" => InjectMode::ClipboardOnly,
        _ => InjectMode::AutoPaste,
    }
}

struct TextOnlyInjector;

impl inject::Injector for TextOnlyInjector {
    fn inject(
        &self,
        _text: &str,
        _mode: InjectMode,
        _prev_hwnd: Option<isize>,
    ) -> Result<inject::DeliveryResult, InjectError> {
        Ok(inject::DeliveryResult {
            delivery: inject::Delivery::TextOnly,
        })
    }
}

fn emit_contract_transcription(host: &DictationHost, request_id: &str, text: &str) {
    if !host
        .contract_events
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return;
    }
    let _ = host.events_tx.send(json!({
        "event": "dictation.transcription_ready",
        "requestId": request_id,
        "text": text,
    }));
}

/// Результат одной попытки `process_one_attempt`.
#[derive(Debug, PartialEq)]
enum AttemptOutcome {
    /// Транскрибировано + inject выполнен (или fallback'нут в clipboard).
    Success {
        text: String,
        injected: bool,
        delivery: inject::Delivery,
    },
    /// Транскрибировано, но clipboard не принял текст. Pending сохраняется.
    DeliveryFailed { reason: &'static str },
    /// Пользователь отменил активную попытку; не inject'им и не удаляем pending.
    Cancelled,
    /// Retryable error — стоит повторить через delay.
    Retryable,
    /// Fatal error — не повторяем (401/400/конфиг).
    Fatal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttemptDelivery {
    Active,
    Background,
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
    delivery: AttemptDelivery,
) -> AttemptOutcome {
    process_one_attempt_with_injector(
        host,
        uuid,
        api_key,
        record_seconds,
        delivery,
        std::sync::Arc::new(inject::SystemInjector),
    )
    .await
}

async fn process_one_attempt_with_injector(
    host: &Arc<DictationHost>,
    uuid: &str,
    api_key: &str,
    record_seconds: f32,
    delivery: AttemptDelivery,
    injector: std::sync::Arc<dyn inject::Injector>,
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
    let cfg = host.snapshot_config().await;
    let mock_transcript =
        mock_dictation_transcript_override(&cfg.provider, Some(cfg.transcription_prompt.as_str()));
    let inject_mode =
        resolve_attempt_inject_mode(item.opts.inject_mode.as_str(), mock_transcript.as_deref());

    if let Some(text) = mock_transcript {
        tracing::info!(%uuid, "dictation: using mock transcript override");
        let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(%uuid, "dictation: skipping OS inject for mock transcript");

        let delivery_result = inject::DeliveryResult {
            delivery: inject::Delivery::ClipboardOnly,
        };

        if let Err(error) = super::pending::drop_item(&host.data_dir, uuid) {
            tracing::error!(%uuid, %error, "dictation: cleanup after mock delivery failed");
        }

        {
            let mut stats_guard = host.stats.lock().await;
            stats_guard.record_session(&text, record_seconds.round() as u64);
            if let Err(e) = save_stats_in(&host.data_dir, &stats_guard) {
                tracing::warn!(error = %e, "dictation: stats save failed");
            }
        }

        let _ = host.events_tx.send(json!({
            "event": "dictation_transcript",
            "text": text,
            "language": language,
            "durationMs": duration_ms,
            "uuid": uuid,
            "injected": false,
            "delivery": delivery_result.delivery.as_str(),
        }));
        emit_contract_transcription(host, uuid, &text);
        let _ = host
            .events_tx
            .send(json!({ "event": "dictation_stats_changed" }));
        host.emit_pending_changed();

        let mut s = host.state.lock().await;
        if s.active_uuid.as_deref() == Some(uuid) {
            *s = HostState::idle();
            let snap = s.clone();
            drop(s);
            host.emit_state(&snap).await;
        }
        return AttemptOutcome::Success {
            text,
            injected: false,
            delivery: delivery_result.delivery,
        };
    }

    let result: Result<String, SubmitError> = if provider_uses_local_runtime(&cfg.provider) {
        if !local_config_is_ready(&host.data_dir, &cfg) {
            tracing::warn!(
                %uuid,
                provider = %cfg.provider,
                local_engine = %cfg.local_engine,
                local_model = ?cfg.local_model,
                "dictation: local model became unavailable before transcribe"
            );
            let _ = reconcile_unready_local_config(host).await;
            let _ = super::pending::bump_attempt(&host.data_dir, uuid, LOCAL_MODEL_NOT_READY_MSG);
            host.emit_pending_changed();
            host.fail_session(uuid, LOCAL_MODEL_NOT_READY_MSG, false)
                .await;
            return AttemptOutcome::Fatal;
        }
        local::transcribe(local::LocalRequest {
            wav_bytes: &wav,
            language: &language,
            prompt: &prompt,
            engine: &cfg.local_engine,
            model_id: cfg.local_model.as_deref().or(Some(model.as_str())),
            model_path: cfg.local_model_path.as_deref(),
            command_path: cfg.local_command_path.as_deref(),
            idle_unload_ms: cfg.local_idle_unload_ms,
        })
        .await
        .map(|transcript| transcript.text)
        .map_err(SubmitError::from)
    } else {
        let client = match network::build_client(&cfg.network_profile, cfg.http_proxy.as_deref()) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(%uuid, error = %e, "dictation: build_client failed");
                let _ = super::pending::bump_attempt(&host.data_dir, uuid, &e.to_string());
                return AttemptOutcome::Fatal; // конфиг сломан — retry не поможет
            }
        };

        groq::transcribe(
            &client,
            &host.groq_endpoint,
            api_key,
            wav,
            &language,
            &model,
            &prompt,
        )
        .await
        .map(|transcript| transcript.text)
        .map_err(SubmitError::from)
    };

    match result {
        Ok(text) => {
            let is_active_attempt = {
                let s = host.state.lock().await;
                s.active_uuid.as_deref() == Some(uuid)
            };
            if matches!(delivery, AttemptDelivery::Active) && !is_active_attempt {
                tracing::info!(
                    %uuid,
                    "dictation: transcription finished after cancel; skipping inject",
                );
                return AttemptOutcome::Cancelled;
            }
            let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
            tracing::info!(%uuid, duration_ms, "dictation: transcribed");

            let inject_mode = if matches!(delivery, AttemptDelivery::Background) {
                InjectMode::ClipboardOnly
            } else {
                inject_mode
            };
            // `injected` reports an OS paste, not merely writing the
            // transcript to clipboard. ClipboardOnly is a successful
            // delivery path but must remain false in the API/event result.
            let os_inject_requested = matches!(inject_mode, InjectMode::AutoPaste);
            let text_for_inject = text.clone();
            let injector = injector.clone();
            let inject_res = tokio::task::spawn_blocking(move || {
                injector.inject(&text_for_inject, inject_mode, prev_hwnd)
            })
            .await;

            let delivery_result = match inject_res {
                Ok(Ok(result)) => result,
                Ok(Err(error)) => {
                    let reason = error.safe_reason();
                    tracing::warn!(%uuid, error = %error, "dictation: transcript delivery failed");
                    let _ = super::pending::bump_attempt(&host.data_dir, uuid, reason);
                    host.emit_pending_changed();
                    if matches!(delivery, AttemptDelivery::Active) {
                        host.fail_session(
                            uuid,
                            "Не удалось доставить текст — диктовка осталась в очереди",
                            true,
                        )
                        .await;
                    }
                    let _ = host.events_tx.send(json!({
                        "event": "dictation_transcript",
                        "text": text,
                        "language": language,
                        "durationMs": duration_ms,
                        "uuid": uuid,
                        "injected": false,
                        "delivery": "failed",
                        "deliveryReason": reason,
                    }));
                    return AttemptOutcome::DeliveryFailed { reason };
                }
                Err(error) => {
                    let reason = "inject_task_failed";
                    tracing::warn!(
                        %uuid,
                        error = %error,
                        "dictation: transcript delivery task failed",
                    );
                    let _ = super::pending::bump_attempt(&host.data_dir, uuid, reason);
                    host.emit_pending_changed();
                    if matches!(delivery, AttemptDelivery::Active) {
                        host.fail_session(
                            uuid,
                            "Не удалось доставить текст — диктовка осталась в очереди",
                            true,
                        )
                        .await;
                    }
                    let _ = host.events_tx.send(json!({
                        "event": "dictation_transcript",
                        "text": text,
                        "language": language,
                        "durationMs": duration_ms,
                        "uuid": uuid,
                        "injected": false,
                        "delivery": "failed",
                        "deliveryReason": reason,
                    }));
                    return AttemptOutcome::DeliveryFailed { reason };
                }
            };

            if let Err(error) = super::pending::drop_item(&host.data_dir, uuid) {
                tracing::error!(%uuid, %error, "dictation: cleanup after delivery failed");
            }

            {
                let mut stats_guard = host.stats.lock().await;
                stats_guard.record_session(&text, record_seconds.round() as u64);
                if let Err(e) = save_stats_in(&host.data_dir, &stats_guard) {
                    tracing::warn!(error = %e, "dictation: stats save failed");
                }
            }

            let injected = delivery_result.delivery.injected() && os_inject_requested;
            let _ = host.events_tx.send(json!({
                "event": "dictation_transcript",
                "text": text,
                "language": language,
                "durationMs": duration_ms,
                "uuid": uuid,
                "injected": injected,
                "delivery": delivery_result.delivery.as_str(),
                "deliveryReason": delivery_result.delivery.reason(),
            }));
            emit_contract_transcription(host, uuid, &text);
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
            AttemptOutcome::Success {
                text,
                injected,
                delivery: delivery_result.delivery,
            }
        }
        Err(e) => {
            {
                let s = host.state.lock().await;
                if s.active_uuid.is_some() && s.active_uuid.as_deref() != Some(uuid) {
                    tracing::info!(
                        %uuid,
                        error = %e,
                        "dictation: transcription failed after cancel; ignoring result",
                    );
                    return AttemptOutcome::Cancelled;
                }
            }
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
                    tracing::info!(
                        %uuid,
                        error = %e,
                        "dictation: retryable — scheduling next attempt",
                    );
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

        match process_one_attempt(
            &host,
            &uuid,
            &api_key,
            record_seconds,
            AttemptDelivery::Background,
        )
        .await
        {
            AttemptOutcome::Success { .. } => return,
            AttemptOutcome::Cancelled => return,
            AttemptOutcome::DeliveryFailed { .. } => return,
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
    let cfg = host.snapshot_config().await;
    let api_key = if provider_needs_api_key(&cfg) {
        match config::get_api_key() {
            Some(k) => k,
            None => return DictationResponse::err("retry: API key не задан"),
        }
    } else {
        String::new()
    };
    let host_clone = host.clone();
    let uuid_for_task = uuid.clone();
    let dur = item.duration_sec;
    // Ручной retry: одна попытка + если retryable — повторное auto-retry-расписание.
    tokio::spawn(async move {
        match process_one_attempt(
            &host_clone,
            &uuid_for_task,
            &api_key,
            dur,
            AttemptDelivery::Background,
        )
        .await
        {
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
            AttemptOutcome::Success { .. }
            | AttemptOutcome::Cancelled
            | AttemptOutcome::DeliveryFailed { .. }
            | AttemptOutcome::Fatal => {}
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

async fn op_discard_all(host: &DictationHost) -> DictationResponse {
    let items = match super::pending::list(&host.data_dir) {
        Ok(v) => v,
        Err(e) => return DictationResponse::err(format!("discard_all: list: {e}")),
    };
    let active_uuid = {
        let s = host.state.lock().await;
        s.active_uuid.clone()
    };
    let mut discarded = 0usize;
    let mut active_discarded = false;

    for item in items {
        if let Err(e) = super::pending::drop_item(&host.data_dir, &item.uuid) {
            return DictationResponse::err(format!("discard_all: {}: {e}", item.uuid));
        }
        if active_uuid.as_deref() == Some(item.uuid.as_str()) {
            active_discarded = true;
        }
        discarded += 1;
    }

    host.emit_pending_changed();
    if active_discarded {
        let mut s = host.state.lock().await;
        *s = HostState::idle();
        let snap = s.clone();
        drop(s);
        host.emit_state(&snap).await;
    }
    DictationResponse::ok(json!({ "discarded": discarded }))
}

async fn op_retry_all(host: &Arc<DictationHost>) -> DictationResponse {
    let items = match super::pending::list(&host.data_dir) {
        Ok(v) => v,
        Err(e) => return DictationResponse::err(format!("retry_all: list: {e}")),
    };
    let cfg = host.snapshot_config().await;
    let api_key = if provider_needs_api_key(&cfg) {
        match config::get_api_key() {
            Some(k) => k,
            None => return DictationResponse::err("retry_all: API key не задан"),
        }
    } else {
        String::new()
    };
    let count = items.len();
    for item in items {
        let host_clone = host.clone();
        let uuid = item.uuid.clone();
        let dur = item.duration_sec;
        let api_key_clone = api_key.clone();
        tokio::spawn(async move {
            if matches!(
                process_one_attempt(
                    &host_clone,
                    &uuid,
                    &api_key_clone,
                    dur,
                    AttemptDelivery::Background,
                )
                .await,
                AttemptOutcome::Retryable
            ) {
                auto_retry_loop(host_clone, uuid, api_key_clone, dur, &AUTO_RETRY_DELAYS_SEC).await;
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
    if let Err(e) = save_stats_in(&host.data_dir, &s) {
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

async fn op_local_status() -> DictationResponse {
    match local::status().await {
        Ok(status) => DictationResponse::ok(json!({
            "warm": status.warm,
            "loadedModel": status.loaded_model,
            "backend": status.backend,
            "accelerator": status.accelerator,
            "device": status.device,
            "profile": status.profile,
            "idleUnloadAfterMs": status.idle_unload_after_ms,
        })),
        Err(e) => DictationResponse::ok(json!({
            "warm": false,
            "loadedModel": null,
            "backend": null,
            "accelerator": "auto",
            "device": null,
            "profile": "fast",
            "idleUnloadAfterMs": null,
            "error": e.to_string(),
        })),
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
