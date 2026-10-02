fn is_supported_engine(engine: &str) -> bool {
    let normalized = engine.trim().to_ascii_lowercase();
    normalized.is_empty()
        || matches!(
            normalized.as_str(),
            DEFAULT_LOCAL_ENGINE | "whisper" | "whisper-cpp" | PARAKEET_LOCAL_ENGINE
        )
}

fn is_parakeet_engine(engine: &str) -> bool {
    engine.trim().eq_ignore_ascii_case(PARAKEET_LOCAL_ENGINE)
}

fn ensure_existing_file(path: &str, missing: LocalError) -> Result<PathBuf, LocalError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(missing);
    }
    let path = PathBuf::from(trimmed);
    if !path.is_file() {
        return match missing {
            LocalError::MissingModelPath => Err(LocalError::ModelPathNotFound {
                path: trimmed.to_owned(),
            }),
            LocalError::MissingCommandPath => Err(LocalError::CommandPathNotFound {
                path: trimmed.to_owned(),
            }),
            other => Err(other),
        };
    }
    Ok(path)
}

fn ensure_existing_model_path(path: &str) -> Result<PathBuf, LocalError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(LocalError::MissingModelPath);
    }
    let path = PathBuf::from(trimmed);
    if path.is_file() || path.is_dir() {
        return Ok(path);
    }
    Err(LocalError::ModelPathNotFound {
        path: trimmed.to_owned(),
    })
}

fn temp_audio_paths() -> Result<(PathBuf, PathBuf), LocalError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| LocalError::TempAudio(e.to_string()))?
        .as_nanos();
    let base = env::temp_dir().join(format!("mundus-local-dictation-{stamp}"));
    Ok((base.with_extension("wav"), base))
}

pub(crate) fn whisper_language_arg(language: &str) -> Option<String> {
    let trimmed = language.trim();
    if trimmed.is_empty() || trimmed == "auto" {
        None
    } else if trimmed == "zh-Hans" || trimmed == "zh-Hant" {
        Some("zh".into())
    } else {
        Some(trimmed.into())
    }
}

fn cleanup_temp_outputs(wav_path: &Path, out_base: &Path) {
    let _ = fs::remove_file(wav_path);
    let _ = fs::remove_file(out_base.with_extension("txt"));
    let _ = fs::remove_file(out_base.with_extension("py"));
}

pub(crate) fn local_whisper_threads() -> usize {
    env::var("MUNDUS_LOCAL_WHISPER_THREADS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|n| n.get().clamp(1, 8))
                .unwrap_or(4)
        })
}

pub(crate) fn strip_whisper_timestamps(text: &str) -> String {
    text.lines()
        .map(|line| {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix('[') {
                if let Some((_, after)) = rest.split_once(']') {
                    return after.trim();
                }
            }
            trimmed
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn collapse_exact_repeated_transcript(text: &str) -> &str {
    let text = text.trim();
    if text.len() < 32 {
        return text;
    }

    let mid = text.len() / 2;
    for split in mid.saturating_sub(4)..=(mid + 4).min(text.len()) {
        if !text.is_char_boundary(split) {
            continue;
        }
        let first = text[..split].trim_end();
        if first.len() < 16 {
            continue;
        }
        if first == text[split..].trim_start() {
            return first;
        }
    }

    text
}

fn is_known_silence_hallucination(text: &str) -> bool {
    let text = text.trim().to_lowercase();
    [
        "продолжение следует",
        "субтитры сделал",
        "субтитры создавал",
        "субтитры предоставил",
        "спасибо за просмотр",
        "подписывайтесь на канал",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

pub(crate) fn clean_whisper_transcript(text: &str) -> Option<String> {
    let text = strip_whisper_timestamps(text);
    let text = collapse_exact_repeated_transcript(&text);
    if text.is_empty() || is_known_silence_hallucination(text) {
        None
    } else {
        Some(text.to_owned())
    }
}

fn read_transcript(stdout: &[u8], out_base: &Path) -> Result<String, LocalError> {
    let txt_path = out_base.with_extension("txt");
    if let Ok(file_text) = fs::read_to_string(&txt_path) {
        if let Some(text) = clean_whisper_transcript(&file_text) {
            return Ok(text);
        }
    }

    let stdout_text = String::from_utf8_lossy(stdout).trim().to_owned();
    if !stdout_text.is_empty() {
        if let Some(text) = clean_whisper_transcript(&stdout_text) {
            return Ok(text);
        }
    }

    let file_text = fs::read_to_string(&txt_path)
        .map_err(|e| {
            LocalError::CommandFailed(format!("не удалось прочитать {:?}: {e}", txt_path))
        })?
        .trim()
        .to_owned();
    if file_text.is_empty() {
        Err(LocalError::EmptyTranscript)
    } else {
        Ok(file_text)
    }
}

fn whisper_server_executable(command_path: &Path) -> PathBuf {
    command_path.with_file_name("whisper-server.exe")
}

fn stable_path_key(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn server_process_key(
    command_path: &Path,
    model_path: &Path,
    accelerator: &LocalSttAccelerator,
    profile: &LocalSttProfile,
) -> ServerProcessKey {
    ServerProcessKey {
        command_path: stable_path_key(command_path),
        model_path: stable_path_key(model_path),
        accelerator: accelerator.clone(),
        profile: profile.clone(),
    }
}

fn whisper_server_port() -> Result<u16, LocalError> {
    TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .map_err(|e| {
            LocalError::CommandFailed(format!("не удалось выбрать порт для whisper-server: {e}"))
        })
}

fn lock_child<'a>(
    child: &'a Mutex<tokio::process::Child>,
) -> Option<std::sync::MutexGuard<'a, tokio::process::Child>> {
    match child.lock() {
        Ok(guard) => Some(guard),
        Err(poisoned) => Some(poisoned.into_inner()),
    }
}

fn parse_server_text(body: &str, content_type: Option<&str>) -> Result<String, LocalError> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(LocalError::EmptyTranscript);
    }

    let content_type_is_json = content_type
        .map(|value| value.to_ascii_lowercase().contains("json"))
        .unwrap_or(false);
    let looks_like_json =
        trimmed.starts_with('{') || (trimmed.starts_with('[') && !trimmed.starts_with("[00:"));

    if content_type_is_json || looks_like_json {
        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => {
                if let Some(filtered) = extract_filtered_verbose_transcript(&value) {
                    return filtered;
                }
                if let Some(text) = extract_transcript_value(&value) {
                    return Ok(text);
                }
            }
            Err(e) if content_type_is_json => {
                return Err(LocalError::CommandFailed(format!(
                    "не удалось распарсить ответ whisper-server: {e}"
                )));
            }
            Err(_) => {}
        }
    }

    clean_whisper_transcript(trimmed).ok_or(LocalError::EmptyTranscript)
}

fn extract_filtered_verbose_transcript(value: &Value) -> Option<Result<String, LocalError>> {
    let segments = value.get("segments").and_then(Value::as_array)?;
    if segments.is_empty() {
        return None;
    }
    let resp = serde_json::from_value::<VerboseResponse>(value.clone()).ok()?;
    let text = filter_segments(&resp);
    Some(clean_whisper_transcript(&text).ok_or(LocalError::EmptyTranscript))
}

fn extract_transcript_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => clean_whisper_transcript(text),
        Value::Object(map) => {
            for key in ["text", "transcription", "transcript", "result", "content"] {
                if let Some(text) = map.get(key).and_then(extract_transcript_value) {
                    return Some(text);
                }
            }
            if let Some(Value::Array(segments)) = map.get("segments") {
                let joined = segments
                    .iter()
                    .filter_map(|segment| {
                        segment
                            .get("text")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|text| !text.is_empty())
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let joined = strip_whisper_timestamps(&joined);
                if let Some(text) = clean_whisper_transcript(&joined) {
                    return Some(text);
                }
            }
            None
        }
        Value::Array(values) => {
            for item in values {
                if let Some(text) = extract_transcript_value(item) {
                    return Some(text);
                }
            }
            None
        }
        _ => None,
    }
}

/// Длительность аудио в секундах из стандартного 44-байтного PCM WAV-заголовка
/// (его пишет pill renderer). Используется для расчёта таймаута инференса.
fn wav_duration_secs(wav_bytes: &[u8]) -> f64 {
    if wav_bytes.len() < 44 {
        return 0.0;
    }
    let channels = u16::from_le_bytes([wav_bytes[22], wav_bytes[23]]).max(1) as f64;
    let sample_rate =
        u32::from_le_bytes([wav_bytes[24], wav_bytes[25], wav_bytes[26], wav_bytes[27]]) as f64;
    let bits = u16::from_le_bytes([wav_bytes[34], wav_bytes[35]]).max(8) as f64;
    let bytes_per_sec = sample_rate * channels * (bits / 8.0);
    if bytes_per_sec <= 0.0 {
        return 0.0;
    }
    wav_bytes.len().saturating_sub(44) as f64 / bytes_per_sec
}

/// Таймаут на запрос к whisper-server. Локальный STT (особенно CPU-сборка)
/// работает ~1× реального времени, поэтому фиксированные 60с обрезали длинные
/// записи (баг на записях ~минута и более). Масштабируем от длительности с
/// большим запасом на медленное железо, с разумным потолком.
fn local_inference_timeout(wav_bytes: &[u8]) -> Duration {
    let dur = wav_duration_secs(wav_bytes);
    let secs = (30.0 + dur * 8.0).ceil() as u64;
    Duration::from_secs(secs.clamp(60, 1800))
}

fn server_request_form(req: &OwnedLocalRequest) -> Result<Form, LocalError> {
    let mut form = Form::new().part(
        "file",
        Part::bytes(req.wav_bytes.clone())
            .file_name("dictation.wav")
            .mime_str("audio/wav")
            .map_err(|e| {
                LocalError::CommandFailed(format!("не удалось подготовить multipart audio: {e}"))
            })?,
    );

    form = form.text("response_format", "verbose_json");

    if let Some(language) = whisper_language_arg(&req.language) {
        form = form.text("language", language);
    }
    if !req.prompt.trim().is_empty() {
        form = form.text("prompt", req.prompt.trim().to_owned());
    }

    Ok(form
        .text("temperature", "0.0")
        .text("temperature_inc", "0.2")
        .text("no_speech_thold", "0.6"))
}

fn apply_whisper_quality_args(command: &mut TokioCommand, profile: &LocalSttProfile) {
    match profile {
        LocalSttProfile::Fast => {
            command.arg("-bo").arg("1").arg("-bs").arg("1");
        }
        LocalSttProfile::Accurate => {
            command.arg("-bo").arg("5").arg("-bs").arg("5");
        }
    };
}

fn apply_whisper_quality_args_blocking(command: &mut Command, profile: &LocalSttProfile) {
    match profile {
        LocalSttProfile::Fast => {
            command.arg("-bo").arg("1").arg("-bs").arg("1");
        }
        LocalSttProfile::Accurate => {
            command.arg("-bo").arg("5").arg("-bs").arg("5");
        }
    };
}

fn apply_whisper_accelerator_args(command: &mut TokioCommand, accelerator: &LocalSttAccelerator) {
    if matches!(accelerator, LocalSttAccelerator::Cpu) {
        command.arg("-ng");
    }
}

fn apply_whisper_accelerator_args_blocking(
    command: &mut Command,
    accelerator: &LocalSttAccelerator,
) {
    if matches!(accelerator, LocalSttAccelerator::Cpu) {
        command.arg("-ng");
    }
}

fn whisper_cpp_vad_model_path(command_path: &Path) -> Option<PathBuf> {
    if let Ok(path) = env::var("MUNDUS_WHISPER_CPP_VAD_MODEL") {
        let path = PathBuf::from(path.trim());
        if path.is_file() {
            return Some(path);
        }
    }
    command_path
        .parent()
        .map(|dir| dir.join("ggml-silero-v6.2.0.bin"))
        .filter(|path| path.is_file())
}

fn apply_whisper_vad_args(command: &mut TokioCommand, command_path: &Path) {
    if let Some(vad_model_path) = whisper_cpp_vad_model_path(command_path) {
        command.arg("--vad").arg("-vm").arg(vad_model_path);
    }
}

fn apply_whisper_vad_args_blocking(command: &mut Command, command_path: &Path) {
    if let Some(vad_model_path) = whisper_cpp_vad_model_path(command_path) {
        command.arg("--vad").arg("-vm").arg(vad_model_path);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServerProcessKey {
    command_path: PathBuf,
    model_path: PathBuf,
    accelerator: LocalSttAccelerator,
    profile: LocalSttProfile,
}

struct WhisperServerProcess {
    key: ServerProcessKey,
    port: u16,
    child: Mutex<tokio::process::Child>,
}

impl WhisperServerProcess {
    fn is_running(&self) -> bool {
        lock_child(&self.child)
            .map(|mut child| match child.try_wait() {
                Ok(None) => true,
                Ok(Some(_)) => false,
                Err(_) => false,
            })
            .unwrap_or(false)
    }
}

impl Drop for WhisperServerProcess {
    fn drop(&mut self) {
        if let Some(mut child) = lock_child(&self.child) {
            let _ = child.start_kill();
        }
    }
}

#[derive(Default)]
struct WhisperServerPool {
    active: Option<Arc<WhisperServerProcess>>,
}

static WHISPER_SERVER_POOL: OnceLock<tokio::sync::Mutex<WhisperServerPool>> = OnceLock::new();
static WHISPER_SERVER_CLIENT: OnceLock<Client> = OnceLock::new();
/// Монотонный счётчик активности whisper-server пути. Инкрементируется в
/// начале каждого `run_whisper_server` вызова. Idle-unload таймер проверяет,
/// что значение не изменилось с момента его постановки — это гарантирует, что
/// мы не выгрузим сервер пока идёт транскрипция или сразу после неё.
static WHISPER_SERVER_ACTIVITY_ID: AtomicU64 = AtomicU64::new(0);

fn whisper_server_pool() -> &'static tokio::sync::Mutex<WhisperServerPool> {
    WHISPER_SERVER_POOL.get_or_init(|| tokio::sync::Mutex::new(WhisperServerPool::default()))
}

fn whisper_server_client() -> &'static Client {
    WHISPER_SERVER_CLIENT.get_or_init(Client::new)
}

async fn wait_for_whisper_server_ready(server: &WhisperServerProcess) -> Result<(), LocalError> {
    let client = whisper_server_client();
    let url = format!("http://127.0.0.1:{}/", server.port);
    let deadline = Instant::now() + local_stt_server_ready_timeout();

    while Instant::now() < deadline {
        if !server.is_running() {
            return Err(LocalError::CommandFailed(
                "whisper-server exited before becoming ready".into(),
            ));
        }

        if client
            .get(&url)
            .timeout(Duration::from_secs(1))
            .send()
            .await
            .is_ok()
        {
            return Ok(());
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Err(LocalError::CommandFailed(format!(
        "whisper-server did not become ready at {url}"
    )))
}

async fn start_whisper_server(
    model_path: &Path,
    command_path: &Path,
    accelerator: &LocalSttAccelerator,
    profile: &LocalSttProfile,
) -> Result<Arc<WhisperServerProcess>, LocalError> {
    let server_path = whisper_server_executable(command_path);
    let port = whisper_server_port()?;
    let mut command = TokioCommand::new(&server_path);
    command
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--inference-path")
        .arg("/inference")
        .arg("-m")
        .arg(model_path)
        .arg("-t")
        .arg(local_whisper_threads().to_string())
        .arg("-nt")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_whisper_quality_args(&mut command, profile);
    apply_whisper_accelerator_args(&mut command, accelerator);
    apply_whisper_vad_args(&mut command, command_path);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let child = command.spawn().map_err(|e| {
        LocalError::CommandFailed(format!("не удалось запустить whisper-server: {e}"))
    })?;

    let server = Arc::new(WhisperServerProcess {
        key: server_process_key(command_path, model_path, accelerator, profile),
        port,
        child: Mutex::new(child),
    });

    wait_for_whisper_server_ready(&server).await?;
    Ok(server)
}

async fn get_or_start_whisper_server(
    model_path: &Path,
    command_path: &Path,
    accelerator: &LocalSttAccelerator,
    profile: &LocalSttProfile,
) -> Result<Arc<WhisperServerProcess>, LocalError> {
    let desired_key = server_process_key(command_path, model_path, accelerator, profile);
    let mut pool = whisper_server_pool().lock().await;
    if let Some(active) = pool.active.as_ref() {
        if active.key == desired_key && active.is_running() {
            return Ok(Arc::clone(active));
        }
    }

    let server = start_whisper_server(model_path, command_path, accelerator, profile).await?;
    pool.active = Some(Arc::clone(&server));
    Ok(server)
}

async fn run_whisper_server(req: OwnedLocalRequest) -> Result<TranscriptionResult, LocalError> {
    // Bump activity ID сразу — любой ранее поставленный idle-unload таймер
    // увидит другое поколение и откажется выгружать пока мы в работе.
    let generation = WHISPER_SERVER_ACTIVITY_ID.fetch_add(1, Ordering::Relaxed) + 1;

    let model_path = ensure_existing_file(
        req.model_path
            .as_deref()
            .ok_or(LocalError::MissingModelPath)?,
        LocalError::MissingModelPath,
    )?;
    let command_path = ensure_existing_file(
        req.command_path
            .as_deref()
            .ok_or(LocalError::MissingCommandPath)?,
        LocalError::MissingCommandPath,
    )?;

    let server_executable = whisper_server_executable(&command_path);
    if !server_executable.is_file() {
        return Err(LocalError::CommandFailed(format!(
            "whisper-server.exe не найден рядом с {:?}",
            command_path
        )));
    }

    let server =
        get_or_start_whisper_server(&model_path, &command_path, &req.accelerator, &req.profile)
            .await?;
    let timeout = local_inference_timeout(&req.wav_bytes);
    let form = server_request_form(&req)?;
    let url = format!("http://127.0.0.1:{}/inference", server.port);
    let response = whisper_server_client()
        .post(&url)
        .multipart(form)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| LocalError::CommandFailed(format!("whisper-server request failed: {e}")))?;

    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response.text().await.map_err(|e| {
        LocalError::CommandFailed(format!("не удалось прочитать ответ whisper-server: {e}"))
    })?;

    if !status.is_success() {
        let message = parse_server_text(&body, content_type.as_deref())
            .unwrap_or_else(|_| body.trim().to_owned());
        return Err(LocalError::CommandFailed(format!(
            "whisper-server HTTP {status}: {message}"
        )));
    }

    let text = parse_server_text(&body, content_type.as_deref())?;
    if text.trim().is_empty() {
        return Err(LocalError::EmptyTranscript);
    }

    // Ставим (или переставляем) idle-unload таймер. Если следующая
    // транскрипция придёт раньше дедлайна — она инкрементирует generation
    // и наш таймер тихо сдётся.
    if let Some(ms) = req.idle_unload_ms {
        if ms > 0 {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(ms)).await;
                if WHISPER_SERVER_ACTIVITY_ID.load(Ordering::Relaxed) == generation {
                    unload_whisper_backend().await;
                    tracing::info!(
                        idle_ms = ms,
                        "dictation: whisper-server idle unload triggered"
                    );
                }
            });
        }
    }

    Ok(TranscriptionResult {
        text,
        backend: "whisper_server".into(),
    })
}

pub(crate) async fn preload_with_whisper_backend(
    engine: &str,
    model_path: Option<&str>,
    command_path: Option<&str>,
) -> Result<bool, LocalError> {
    let model = LocalSttModelSpec {
        engine: engine.to_owned(),
        model_id: None,
        model_path: model_path.map(str::to_owned),
        command_path: command_path.map(str::to_owned),
        accelerator: local_stt_accelerator(),
        profile: local_stt_profile(),
        idle_unload_after_ms: local_stt_idle_unload_after_ms_for_engine(engine),
    };
    preload_with_whisper_backend_model(&model).await
}

pub(crate) async fn preload_with_whisper_backend_model(
    model: &LocalSttModelSpec,
) -> Result<bool, LocalError> {
    if !is_supported_engine(&model.engine) {
        return Err(LocalError::UnsupportedEngine {
            engine: model.engine.clone(),
        });
    }
    let model_path = ensure_existing_file(
        model
            .model_path
            .as_deref()
            .ok_or(LocalError::MissingModelPath)?,
        LocalError::MissingModelPath,
    )?;
    let command_path = ensure_existing_file(
        model
            .command_path
            .as_deref()
            .ok_or(LocalError::MissingCommandPath)?,
        LocalError::MissingCommandPath,
    )?;
    if !whisper_server_executable(&command_path).is_file() {
        return Ok(false);
    }
    get_or_start_whisper_server(
        &model_path,
        &command_path,
        &model.accelerator,
        &model.profile,
    )
    .await?;
    Ok(true)
}

fn run_whisper_cpp(req: OwnedLocalRequest) -> Result<TranscriptionResult, LocalError> {
    let _ = req.model_id;
    let model_path = ensure_existing_file(
        req.model_path
            .as_deref()
            .ok_or(LocalError::MissingModelPath)?,
        LocalError::MissingModelPath,
    )?;
    let command_path = ensure_existing_file(
        req.command_path
            .as_deref()
            .ok_or(LocalError::MissingCommandPath)?,
        LocalError::MissingCommandPath,
    )?;

    let (wav_path, out_base) = temp_audio_paths()?;
    fs::write(&wav_path, &req.wav_bytes).map_err(|e| LocalError::TempAudio(e.to_string()))?;

    let mut command = Command::new(&command_path);
    command
        .arg("-m")
        .arg(&model_path)
        .arg("-f")
        .arg(&wav_path)
        .arg("-otxt")
        .arg("-of")
        .arg(&out_base)
        .arg("-nt")
        .arg("-np")
        .arg("-t")
        .arg(local_whisper_threads().to_string());

    apply_whisper_quality_args_blocking(&mut command, &req.profile);
    apply_whisper_accelerator_args_blocking(&mut command, &req.accelerator);
    apply_whisper_vad_args_blocking(&mut command, &command_path);

    if let Some(language) = whisper_language_arg(&req.language) {
        command.arg("-l").arg(language);
    }
    if !req.prompt.trim().is_empty() {
        command.arg("--prompt").arg(req.prompt.trim());
    }
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = command.output().map_err(|e| {
        cleanup_temp_outputs(&wav_path, &out_base);
        LocalError::CommandFailed(e.to_string())
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        cleanup_temp_outputs(&wav_path, &out_base);
        return Err(LocalError::CommandFailed(if stderr.is_empty() {
            format!("exit code {:?}", output.status.code())
        } else {
            stderr
        }));
    }

    let text = read_transcript(&output.stdout, &out_base)?;
    cleanup_temp_outputs(&wav_path, &out_base);
    Ok(TranscriptionResult {
        text,
        backend: "whisper_cli".into(),
    })
}

fn run_parakeet(req: OwnedLocalRequest) -> Result<TranscriptionResult, LocalError> {
    let model_path = ensure_existing_model_path(
        req.model_path
            .as_deref()
            .ok_or(LocalError::MissingModelPath)?,
    )?;
    let (wav_path, out_base) = temp_audio_paths()?;
    fs::write(&wav_path, &req.wav_bytes).map_err(|e| LocalError::TempAudio(e.to_string()))?;

    let result = (|| {
        let samples = transcribe_rs::audio::read_wav_samples(&wav_path)
            .map_err(|e| LocalError::CommandFailed(e.to_string()))?;
        let mut model = ParakeetModel::load(&model_path, &Quantization::Int8)
            .map_err(|e| LocalError::CommandFailed(e.to_string()))?;
        model
            .transcribe_with(
                &samples,
                &ParakeetParams {
                    timestamp_granularity: Some(TimestampGranularity::Segment),
                    ..Default::default()
                },
            )
            .map_err(|e| LocalError::CommandFailed(e.to_string()))
    })();

    cleanup_temp_outputs(&wav_path, &out_base);
    let text = result?.text.trim().to_owned();
    if text.is_empty() {
        return Err(LocalError::EmptyTranscript);
    }
    Ok(TranscriptionResult {
        text,
        backend: "parakeet_onnx".into(),
    })
}

pub(crate) async fn transcribe_with_whisper_backend(
    req: LocalRequest<'_>,
) -> Result<TranscriptionResult, LocalError> {
    transcribe_owned_with_whisper_backend(OwnedLocalRequest::from(req)).await
}

pub(crate) async fn transcribe_with_whisper_backend_model(
    model: &LocalSttModelSpec,
    wav_bytes: &[u8],
    language: &str,
    prompt: &str,
) -> Result<TranscriptionResult, LocalError> {
    transcribe_owned_with_whisper_backend(owned_request_from_model(
        model, wav_bytes, language, prompt,
    ))
    .await
}

async fn transcribe_owned_with_whisper_backend(
    owned: OwnedLocalRequest,
) -> Result<TranscriptionResult, LocalError> {
    if !is_supported_engine(&owned.engine) {
        return Err(LocalError::UnsupportedEngine {
            engine: owned.engine.clone(),
        });
    }

    if is_parakeet_engine(&owned.engine) {
        return tokio::task::spawn_blocking(move || run_parakeet(owned))
            .await
            .map_err(|e| LocalError::CommandFailed(format!("worker join failed: {e}")))?;
    }

    let command_path = owned.command_path.clone();
    if let Some(command_path) = command_path {
        if whisper_server_executable(Path::new(&command_path)).is_file() {
            return run_whisper_server(owned).await;
        }
    }

    tokio::task::spawn_blocking(move || run_whisper_cpp(owned))
        .await
        .map_err(|e| LocalError::CommandFailed(format!("worker join failed: {e}")))?
}

pub(crate) async fn unload_whisper_backend() {
    let mut pool = whisper_server_pool().lock().await;
    pool.active = None;
}

pub async fn preload_server(
    engine: &str,
    model_path: Option<&str>,
    command_path: Option<&str>,
) -> Result<bool, LocalError> {
    if model_path.is_none_or(|path| path.trim().is_empty()) {
        return Err(LocalError::MissingModelPath);
    }
    if is_parakeet_engine(engine) {
        return Ok(false);
    }
    if command_path.is_none_or(|path| path.trim().is_empty()) {
        return Err(LocalError::MissingCommandPath);
    }

    let request = LocalSttRequest::Preload {
        model: LocalSttModelSpec {
            engine: engine.to_owned(),
            model_id: None,
            model_path: model_path.map(str::to_owned),
            command_path: command_path.map(str::to_owned),
            accelerator: local_stt_accelerator(),
            profile: local_stt_profile(),
            idle_unload_after_ms: local_stt_idle_unload_after_ms_for_engine(engine),
        },
    };

    match send_sidecar_request(request).await {
        Ok(LocalSttResponse::Status(status)) => Ok(status.warm),
        Ok(other) => Err(LocalError::SidecarUnavailable(format!(
            "unexpected preload response: {other:?}"
        ))),
        // Explicit debug escape hatch: managed product path must use the Mundus-owned
        // sidecar. Direct whisper-server/cli execution is available only when the
        // developer opts in with MUNDUS_LOCAL_STT_ALLOW_DIRECT_FALLBACK=1.
        Err(LocalError::SidecarUnavailable(_)) if direct_sidecar_fallback_allowed() => {
            preload_with_whisper_backend(engine, model_path, command_path).await
        }
        Err(error) => Err(error),
    }
}

pub async fn status() -> Result<LocalSttStatus, LocalError> {
    match send_sidecar_request(LocalSttRequest::Status).await {
        Ok(LocalSttResponse::Status(status)) => Ok(status),
        Ok(other) => Err(LocalError::SidecarUnavailable(format!(
            "unexpected status response: {other:?}"
        ))),
        Err(error) => Err(error),
    }
}

pub async fn cancel_sidecar() -> Result<bool, LocalError> {
    match send_sidecar_request(LocalSttRequest::Cancel {
        target_request_id: None,
    })
    .await
    {
        Ok(LocalSttResponse::Ack(ack)) => Ok(ack.accepted),
        Ok(other) => Err(LocalError::SidecarUnavailable(format!(
            "unexpected cancel response: {other:?}"
        ))),
        Err(error) => Err(error),
    }
}

pub async fn unload_sidecar() -> Result<bool, LocalError> {
    match send_sidecar_request(LocalSttRequest::Unload).await {
        Ok(LocalSttResponse::Status(status)) => Ok(!status.warm),
        Ok(other) => Err(LocalError::SidecarUnavailable(format!(
            "unexpected unload response: {other:?}"
        ))),
        Err(error) => Err(error),
    }
}

pub async fn transcribe(req: LocalRequest<'_>) -> Result<TranscriptionResult, LocalError> {
    tracing::info!(
        engine = req.engine,
        model_id = ?req.model_id,
        model_path_exists = req.model_path.is_some_and(|path| Path::new(path).is_file()),
        command_path_exists = req.command_path.is_some_and(|path| Path::new(path).is_file()),
        wav_bytes = req.wav_bytes.len(),
        device_os = std::env::consts::OS,
        device_arch = std::env::consts::ARCH,
        "dictation local transcribe started"
    );
    if let Some(text) = test_override_transcript() {
        let result = TranscriptionResult {
            text,
            backend: "test_override".into(),
        };
        tracing::info!(backend = %result.backend, "dictation local transcribe completed");
        return Ok(result);
    }

    if !is_supported_engine(req.engine) {
        return Err(LocalError::UnsupportedEngine {
            engine: req.engine.to_owned(),
        });
    }
    if req.model_path.is_none_or(|path| path.trim().is_empty()) {
        return Err(LocalError::MissingModelPath);
    }
    if !is_parakeet_engine(req.engine) && req.command_path.is_none_or(|path| path.trim().is_empty())
    {
        return Err(LocalError::MissingCommandPath);
    }

    let owned = OwnedLocalRequest::from(req);
    if is_parakeet_engine(&owned.engine) {
        return transcribe_owned_with_whisper_backend(owned).await;
    }

    let request = LocalSttRequest::Transcribe {
        model: model_spec_from_owned(&owned),
        wav_base64: base64::engine::general_purpose::STANDARD.encode(&owned.wav_bytes),
        language: owned.language.clone(),
        prompt: owned.prompt.clone(),
    };

    match send_sidecar_request(request).await {
        Ok(LocalSttResponse::Transcription(transcription)) => {
            tracing::info!(
                backend = %transcription.backend,
                "dictation local transcribe completed",
            );
            Ok(TranscriptionResult {
                // See postmortems.md 2026-07-03: sidecar output still crosses a version boundary.
                text: clean_whisper_transcript(&transcription.text)
                    .ok_or(LocalError::EmptyTranscript)?,
                backend: transcription.backend,
            })
        }
        Ok(other) => {
            tracing::warn!(response = ?other, "dictation local transcribe failed");
            Err(LocalError::SidecarUnavailable(format!(
                "unexpected transcribe response: {other:?}"
            )))
        }
        // Explicit debug escape hatch: managed product path must use the Mundus-owned
        // sidecar. Direct whisper-server/cli execution is available only when the
        // developer opts in with MUNDUS_LOCAL_STT_ALLOW_DIRECT_FALLBACK=1.
        Err(LocalError::SidecarUnavailable(_)) if direct_sidecar_fallback_allowed() => {
            transcribe_with_whisper_backend(LocalRequest {
                wav_bytes: &owned.wav_bytes,
                language: &owned.language,
                prompt: &owned.prompt,
                engine: &owned.engine,
                model_id: owned.model_id.as_deref(),
                model_path: owned.model_path.as_deref(),
                command_path: owned.command_path.as_deref(),
                idle_unload_ms: owned.idle_unload_ms,
            })
            .await
        }
        Err(error) => {
            tracing::warn!(error = %error, "dictation local transcribe failed");
            Err(error)
        }
    }
}
