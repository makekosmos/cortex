async fn apply_local_model_selection(
    host: &DictationHost,
    model_id: &str,
    model_path: std::path::PathBuf,
    command_path: Option<std::path::PathBuf>,
) -> Result<DictationConfig, String> {
    apply_local_model_selection_to_config(
        &host.config,
        &host.events_tx,
        &host.data_dir,
        model_id,
        model_path,
        command_path,
    )
    .await
}

async fn apply_local_model_selection_to_config(
    config_state: &Arc<Mutex<DictationConfig>>,
    events_tx: &broadcast::Sender<Value>,
    data_dir: &std::path::Path,
    model_id: &str,
    model_path: std::path::PathBuf,
    command_path: Option<std::path::PathBuf>,
) -> Result<DictationConfig, String> {
    apply_local_model_selection_values_to_config(
        config_state,
        events_tx,
        data_dir,
        model_id,
        model_path.to_string_lossy().to_string(),
        command_path.map(|path| path.to_string_lossy().to_string()),
    )
    .await
}

async fn apply_local_model_selection_values_to_config(
    config_state: &Arc<Mutex<DictationConfig>>,
    events_tx: &broadcast::Sender<Value>,
    data_dir: &std::path::Path,
    model_id: &str,
    model_path: String,
    command_path: Option<String>,
) -> Result<DictationConfig, String> {
    let mut cfg = config_state.lock().await;
    cfg.provider = "local".into();
    cfg.provider_enabled = true;
    cfg.local_engine = local_engine_for_model(model_id).into();
    cfg.local_model = Some(model_id.to_owned());
    cfg.local_model_path = Some(model_path);
    cfg.local_command_path = command_path;
    save_config_in(data_dir, &cfg).map_err(|e| format!("save failed: {e}"))?;
    let snapshot = cfg.clone();
    drop(cfg);
    apply_ptt_hook(&snapshot, events_tx);
    let _ = events_tx.send(json!({ "event": "dictation_config_changed" }));
    Ok(snapshot)
}

fn local_engine_for_model(model_id: &str) -> &'static str {
    if model_id == "parakeet-tdt-0.6b-v3" {
        "parakeet"
    } else {
        platform_local_engine()
    }
}

fn platform_local_engine() -> &'static str {
    #[cfg(test)]
    {
        DEFAULT_LOCAL_ENGINE
    }
    #[cfg(not(test))]
    {
        platform_local_engine_for_os(std::env::consts::OS)
    }
}

fn platform_local_engine_for_os(os: &str) -> &'static str {
    match os {
        "macos" => DEFAULT_LOCAL_ENGINE,
        _ => DEFAULT_LOCAL_ENGINE,
    }
}

fn normalize_platform_local_engine(cfg: &mut DictationConfig) -> bool {
    // Parakeet — полноценный in-process движок (новая фича): никогда не
    // даунгрейдим его до whisper.cpp, иначе выбор Parakeet (вместе с путём к
    // модели) затирается на каждом старте/`update_config`, и provider
    // отключается через `clear_local_selection`.
    if cfg.local_engine == local::PARAKEET_LOCAL_ENGINE {
        return false;
    }
    let local_engine = platform_local_engine();
    if cfg.local_engine == local_engine {
        return false;
    }
    // Сюда попадают только legacy/unknown движки (например старый
    // faster-whisper) — их мигрируем на платформенный default и сбрасываем
    // несовместимый выбор модели.
    cfg.local_engine = local_engine.to_owned();
    cfg.local_model = None;
    cfg.local_model_path = None;
    cfg.local_command_path = None;
    true
}

async fn op_list_local_models(host: &DictationHost) -> DictationResponse {
    let _ = clear_unready_local_config(host).await;
    let cfg = host.snapshot_config().await;
    DictationResponse::ok(json!(local_models::snapshot(&host.data_dir, &cfg)))
}

async fn op_download_local_model(params: Value, host: &DictationHost) -> DictationResponse {
    let model_id = match params.get("modelId").and_then(|v| v.as_str()) {
        Some(value) if !value.trim().is_empty() => value.trim().to_owned(),
        _ => return DictationResponse::err("download_local_model: missing modelId"),
    };
    let select = params
        .get("select")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let cfg = host.snapshot_config().await;
    let client =
        match network::build_download_client(&cfg.network_profile, cfg.http_proxy.as_deref()) {
            Ok(client) => client,
            Err(e) => return DictationResponse::err(format!("download_local_model: {e}")),
        };
    let events_tx = host.events_tx.clone();
    let data_dir = host.data_dir.clone();
    let config_state = host.config.clone();
    let model_id_for_task = model_id.clone();
    tokio::spawn(async move {
        let model_id = model_id_for_task;
        let mut progress = |progress: local_models::DownloadProgress| {
            let _ = events_tx.send(json!({
                "event": "dictation_local_model_download_progress",
                "modelId": model_id,
                "phase": progress.phase,
                "downloadedBytes": progress.downloaded_bytes,
                "totalBytes": progress.total_bytes,
                "percent": progress.percent,
            }));
        };
        let _ = events_tx.send(json!({
            "event": "dictation_local_model_download_started",
            "modelId": model_id,
        }));
        let model_path = match local_models::ensure_model_with_progress(
            &client,
            &data_dir,
            &model_id,
            &mut progress,
        )
        .await
        {
            Ok(path) => path,
            Err(e) => {
                let msg = format!("download_local_model: {e}");
                let _ = events_tx.send(json!({
                    "event": "dictation_local_model_download_failed",
                    "modelId": model_id,
                    "error": msg,
                }));
                return;
            }
        };
        let command_path = if local_engine_for_model(&model_id) == DEFAULT_LOCAL_ENGINE {
            match local_models::ensure_whisper_cpp_with_progress(&client, &data_dir, &mut progress)
                .await
            {
                Ok(path) => Some(path),
                Err(e) => {
                    let msg = format!("download_local_model: {e}");
                    let _ = events_tx.send(json!({
                        "event": "dictation_local_model_download_failed",
                        "modelId": model_id,
                        "error": msg,
                    }));
                    return;
                }
            }
        } else {
            None
        };

        if select {
            if let Err(e) = apply_local_model_selection_to_config(
                &config_state,
                &events_tx,
                &data_dir,
                &model_id,
                model_path,
                command_path,
            )
            .await
            {
                let msg = format!("download_local_model: {e}");
                let _ = events_tx.send(json!({
                    "event": "dictation_local_model_download_failed",
                    "modelId": model_id,
                    "error": msg,
                }));
                return;
            }
        }
        let cfg = config_state.lock().await.clone();
        let _ = events_tx.send(json!({
            "event": "dictation_local_model_download_complete",
            "modelId": model_id,
            "config": select.then(|| config_to_value(&cfg)),
            "localModels": local_models::snapshot(&data_dir, &cfg),
        }));
    });
    DictationResponse::ok(json!({
        "started": true,
        "modelId": model_id,
    }))
}

async fn op_use_local_model(params: Value, host: &DictationHost) -> DictationResponse {
    let model_id = match params.get("modelId").and_then(|v| v.as_str()) {
        Some(value) if !value.trim().is_empty() => value.trim(),
        _ => return DictationResponse::err("use_local_model: missing modelId"),
    };
    let model_path = match local_models::MODEL_CATALOG
        .iter()
        .find(|model| model.id == model_id)
        .map(|spec| (spec, local_models::model_path(&host.data_dir, spec)))
    {
        Some((_, path)) if path.is_file() => path,
        Some((spec, path)) if spec.directory && path.is_dir() => path,
        Some((_, _)) => {
            return DictationResponse::err(format!(
                "use_local_model: model is not downloaded: {model_id}"
            ))
        }
        None => {
            return DictationResponse::err(format!("use_local_model: model not found: {model_id}"))
        }
    };
    if !local_models::model_supports_transcription(model_id).unwrap_or(false) {
        return DictationResponse::err(format!(
            "use_local_model: model cannot be used by whisper.cpp: {model_id}"
        ));
    }
    let command_path = if local_engine_for_model(model_id) == DEFAULT_LOCAL_ENGINE {
        match local_models::command_path(&host.data_dir) {
            Some(path) if path.is_file() => Some(path),
            _ => return DictationResponse::err("use_local_model: whisper.cpp is not installed"),
        }
    } else {
        None
    };
    match apply_local_model_selection(host, model_id, model_path, command_path).await {
        Ok(cfg) => DictationResponse::ok(json!({
            "config": config_to_value(&cfg),
            "localModels": local_models::snapshot(&host.data_dir, &cfg),
        })),
        Err(e) => DictationResponse::err(format!("use_local_model: {e}")),
    }
}

async fn op_delete_local_model(params: Value, host: &DictationHost) -> DictationResponse {
    let model_id = match params.get("modelId").and_then(|v| v.as_str()) {
        Some(value) if !value.trim().is_empty() => value.trim(),
        _ => return DictationResponse::err("delete_local_model: missing modelId"),
    };

    if let Err(e) = local::unload_sidecar().await {
        tracing::warn!(error = %e, "dictation: local STT unload before model delete failed");
    }

    let deleted_path = match local_models::delete_model(&host.data_dir, model_id) {
        Ok(path) => path,
        Err(e) => return DictationResponse::err(format!("delete_local_model: {e}")),
    };

    let mut changed_config = false;
    {
        let mut cfg = host.config.lock().await;
        let deleted_path_text = deleted_path.to_string_lossy();
        let selected_by_id = cfg.local_model.as_deref() == Some(model_id);
        let selected_by_path = cfg.local_model_path.as_deref() == Some(deleted_path_text.as_ref());
        let no_local_assets = !local_models::has_downloaded_model_assets(&host.data_dir);
        if selected_by_id || selected_by_path || no_local_assets {
            cfg.provider_enabled = false;
            cfg.local_model = None;
            cfg.local_model_path = None;
            if no_local_assets {
                cfg.local_command_path = None;
            }
            if let Err(e) = save_config_in(&host.data_dir, &cfg) {
                return DictationResponse::err(format!("delete_local_model: save failed: {e}"));
            }
            changed_config = true;
        }
    }
    if changed_config {
        host.emit_config_changed();
    }

    let cfg = host.snapshot_config().await;
    DictationResponse::ok(json!({
        "config": config_to_value(&cfg),
        "localModels": local_models::snapshot(&host.data_dir, &cfg),
    }))
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
    let was_using_local_runtime =
        cfg.provider_enabled && provider_uses_local_runtime(&cfg.provider);
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
    if params.get("localModelPath").is_some() {
        cfg.local_model_path = params
            .get("localModelPath")
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
    if params.get("localCommandPath").is_some() {
        cfg.local_command_path = params
            .get("localCommandPath")
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
    if params.get("localModel").is_some() || params.get("localModelId").is_some() {
        let value = params
            .get("localModel")
            .or_else(|| params.get("localModelId"))
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
        cfg.local_model = value;
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
    if let Some(enabled) = params
        .get("duckAudioDuringRecording")
        .and_then(|v| v.as_bool())
    {
        cfg.duck_audio_during_recording = enabled;
    }
    if params.get("localIdleUnloadMs").is_some() {
        // null → None (никогда не выгружать); число → Some(ms).
        cfg.local_idle_unload_ms = params
            .get("localIdleUnloadMs")
            .and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    v.as_u64().map(Some)
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
    normalize_platform_local_engine(&mut cfg);
    if !local_config_is_ready(&host.data_dir, &cfg) {
        clear_local_selection(&mut cfg);
    }
    if let Err(e) = save_config_in(&host.data_dir, &cfg) {
        return DictationResponse::err(format!("update_config: save failed: {e}"));
    }
    let snapshot = cfg.clone();
    let should_unload_local_runtime = was_using_local_runtime
        && !(snapshot.provider_enabled && provider_uses_local_runtime(&snapshot.provider));
    drop(cfg);
    if should_unload_local_runtime {
        tokio::spawn(async {
            if let Err(e) = local::unload_sidecar().await {
                tracing::warn!(
                    error = %e,
                    "dictation: local STT unload after provider change failed"
                );
            }
        });
    } else if snapshot.provider_enabled && provider_uses_local_runtime(&snapshot.provider) {
        preload_local_runtime_for_recording(host).await;
    }
    // Перерегистрируем PTT hook (на случай смены trigger_mode или hotkey).
    apply_ptt_hook(&snapshot, &host.events_tx);
    host.emit_config_changed();
    DictationResponse::ok(json!({ "config": config_to_value(&snapshot) }))
}
