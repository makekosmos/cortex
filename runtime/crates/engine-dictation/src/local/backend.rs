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

#[cfg(feature = "local-dictation")]
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

fn lock_child<'a>(child: &'a Mutex<ProcessTree>) -> Option<std::sync::MutexGuard<'a, ProcessTree>> {
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
    // ProcessTree, not a bare Child: the KILL_ON_JOB_CLOSE job (Unix: process
    // group) is what kills whisper-server when the Engine dies — installer
    // kill, crash, Task Manager — instead of leaving it holding the tools dir
    // and its port (KOS-312). Dropping it also covers our own stop path.
    child: Mutex<ProcessTree>,
}

impl WhisperServerProcess {
    fn is_running(&self) -> bool {
        lock_child(&self.child)
            .map(|mut tree| matches!(tree.child_mut().try_wait(), Ok(None)))
            .unwrap_or(false)
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
    // Creation flags go through ProcessTree::spawn, not CommandExt: it ORs
    // CREATE_SUSPENDED in and assigns the job before the primary thread runs,
    // so the child can never execute a single instruction outside the job.
    #[cfg(windows)]
    let creation_flags = CREATE_NO_WINDOW;
    #[cfg(not(windows))]
    let creation_flags = 0;
    let tree = ProcessTree::spawn(&mut command, creation_flags)
        .await
        .map_err(|e| {
            LocalError::CommandFailed(format!("не удалось запустить whisper-server: {e}"))
        })?;

    let server = Arc::new(WhisperServerProcess {
        key: server_process_key(command_path, model_path, accelerator, profile),
        port,
        child: Mutex::new(tree),
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
    local_dictation_feature_gate()?;
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

/// ONNX Runtime статически слинкован (ort-sys download-binaries в build
/// script) — искать/грузить dylib в рантайме не нужно. `ort::init()` лишь
/// коммитит Env; повторный commit — no-op, так что это дёшево.
#[cfg(feature = "local-dictation")]
fn ensure_onnxruntime() -> Result<(), LocalError> {
    ort::init().commit();
    Ok(())
}

/// Резидентная Parakeet-модель: ONNX session init стоит ~600мс на каждой
/// диктовке, поэтому модель живёт в процессе между запросами (аналог пула
/// whisper-server). Mutex сериализует инференс — одна транскрипция за раз.
#[cfg(feature = "local-dictation")]
struct ParakeetCache {
    path: PathBuf,
    model: ParakeetModel,
    last_used: Instant,
    generation: u64,
}

#[cfg(feature = "local-dictation")]
static PARAKEET: OnceLock<Mutex<Option<ParakeetCache>>> = OnceLock::new();

#[cfg(feature = "local-dictation")]
fn parakeet_slot() -> &'static Mutex<Option<ParakeetCache>> {
    PARAKEET.get_or_init(|| Mutex::new(None))
}

/// Кэш протух, если модели нет или каталог сменился. Чистая функция —
/// покрыта тестом без модельных файлов.
#[cfg(feature = "local-dictation")]
fn parakeet_cache_is_stale(cached: Option<&Path>, requested: &Path) -> bool {
    cached != Some(requested)
}

/// Блокирует кэш и гарантирует загруженную модель для `model_dir`; держать
/// guard на время инференса — это и есть сериализация.
#[cfg(feature = "local-dictation")]
fn with_parakeet<R>(
    model_dir: &Path,
    f: impl FnOnce(&mut ParakeetModel) -> Result<R, LocalError>,
) -> Result<R, LocalError> {
    let mut guard = parakeet_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let requested = stable_path_key(model_dir);
    if parakeet_cache_is_stale(
        guard.as_ref().map(|cached| cached.path.as_path()),
        &requested,
    ) {
        let started = Instant::now();
        let model = ParakeetModel::load(model_dir, &Quantization::Int8)
            .map_err(|e| LocalError::CommandFailed(e.to_string()))?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(duration_ms, "dictation: parakeet model loaded");
        eprintln!("[dictation::local] parakeet model loaded duration_ms={duration_ms}");
        let generation = guard
            .as_ref()
            .map(|cached| cached.generation + 1)
            .unwrap_or(1);
        *guard = Some(ParakeetCache {
            path: requested,
            model,
            last_used: Instant::now(),
            generation,
        });
    }
    let cached = guard.as_mut().expect("parakeet cache just populated");
    let result = f(&mut cached.model)?;
    cached.last_used = Instant::now();
    let generation = cached.generation;
    drop(guard);
    schedule_parakeet_idle_unload(generation);
    Ok(result)
}

/// Idle-unload в отдельном потоке (этот код вызывается из spawn_blocking —
/// tokio::spawn недоступен). Выгружает только если с момента постановки не
/// было новой генерации и last_used старше таймаута. `None`/0 — никогда.
#[cfg(feature = "local-dictation")]
fn schedule_parakeet_idle_unload(generation: u64) {
    let Some(ms) = local_stt_idle_unload_after_ms_for_engine(PARAKEET_LOCAL_ENGINE) else {
        return;
    };
    if ms == 0 {
        return;
    }
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(ms));
        let mut guard = parakeet_slot()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let stale = guard.as_ref().is_some_and(|cached| {
            cached.generation == generation
                && cached.last_used.elapsed() >= Duration::from_millis(ms)
        });
        if stale {
            guard.take();
            tracing::info!(idle_ms = ms, "dictation: parakeet idle unload triggered");
            eprintln!("[dictation::local] parakeet idle unload triggered idle_ms={ms}");
        }
    });
}

/// Выгрузить резидентную модель — вызывается из всех путей смены/удаления
/// модели вместе с whisper unload'ами.
#[cfg(feature = "local-dictation")]
pub(crate) fn unload_parakeet() {
    let mut guard = parakeet_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.take().is_some() {
        tracing::info!("dictation: parakeet model unloaded");
        eprintln!("[dictation::local] parakeet model unloaded");
    }
}

// ---- Streaming-транскриба во время записи -------------------------------

/// Текст уже транскрибированных во время записи кусков. `committed` — длина
/// префикса в сэмплах i16, `prefix_hash` — хэш этого префикса для проверки,
/// что финальный wav — та же запись.
#[cfg(feature = "local-dictation")]
pub(crate) struct PartialTranscript {
    pub committed: usize,
    pub prefix_hash: u64,
    pub texts: Vec<String>,
}

/// Активный stream-транскрибёр: join'им при остановке записи.
#[cfg(feature = "local-dictation")]
struct StreamingParakeet {
    join: thread::JoinHandle<StreamingState>,
}

#[cfg(feature = "local-dictation")]
struct StreamingState {
    samples: Vec<i16>,
    committed: usize,
    texts: Vec<String>,
}

#[cfg(feature = "local-dictation")]
static PARAKEET_STREAM: OnceLock<Mutex<Option<StreamingParakeet>>> = OnceLock::new();
#[cfg(feature = "local-dictation")]
static STREAM_PARTIAL: OnceLock<Mutex<Option<PartialTranscript>>> = OnceLock::new();

#[cfg(feature = "local-dictation")]
fn stream_slot() -> &'static Mutex<Option<StreamingParakeet>> {
    PARAKEET_STREAM.get_or_init(|| Mutex::new(None))
}

#[cfg(feature = "local-dictation")]
fn stream_partial_slot() -> &'static Mutex<Option<PartialTranscript>> {
    STREAM_PARTIAL.get_or_init(|| Mutex::new(None))
}

/// i16-сэмплы из wav-байтов (16kHz mono s16 — формат capture'а). Поиск
/// data-чанка, а не слепой offset 44 — запас на чужие wav'и.
#[cfg(feature = "local-dictation")]
fn wav_pcm16(wav: &[u8]) -> Option<Vec<i16>> {
    if wav.len() < 12 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12usize;
    while pos + 8 <= wav.len() {
        let size = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().ok()?) as usize;
        if &wav[pos..pos + 4] == b"data" {
            let end = (pos + 8 + size).min(wav.len());
            return Some(
                wav[pos + 8..end]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| i16::from_le_bytes(*c))
                    .collect(),
            );
        }
        pos += 8 + size + (size & 1);
    }
    None
}

/// Хэш префикса i16-сэмплов — проверка, что wav из capture.stop та же
/// запись, что стримил транскрибёр (иначе частичный текст чужой).
#[cfg(feature = "local-dictation")]
fn hash_pcm16(samples: &[i16]) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for s in samples {
        h.write_i16(*s);
    }
    h.finish()
}

/// Решение «reuse partial»: Some(committed) только если wav длиннее
/// committed И хэш префикса совпал.
#[cfg(feature = "local-dictation")]
fn apply_partial(samples: &[i16], partial: &PartialTranscript) -> Option<usize> {
    if partial.committed == 0 || samples.len() < partial.committed {
        return None;
    }
    if hash_pcm16(&samples[..partial.committed]) != partial.prefix_hash {
        return None;
    }
    Some(partial.committed)
}

/// Стартует фоновую транскрибу pcm-потока (вызывается при capture.start
/// для parakeet). `rx` — канал, куда capture-драйвер пишет drain-блоки.
#[cfg(feature = "local-dictation")]
pub(crate) fn start_parakeet_stream(model_dir: PathBuf, rx: mpsc::Receiver<Vec<i16>>) {
    // Новая запись: старый stream и его partial сбрасываем.
    finish_parakeet_stream();
    *stream_partial_slot()
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = None;

    let join = thread::Builder::new()
        .name("mundus-parakeet-stream".into())
        .spawn(move || {
            let mut state = StreamingState {
                samples: Vec::new(),
                committed: 0,
                texts: Vec::new(),
            };
            while let Ok(block) = rx.recv() {
                state.samples.extend_from_slice(&block);
                let uncommitted = state.samples.len() - state.committed;
                // Чанк по тихому месту: ждём ≥10с некоммиченного аудио и
                // режем в [7с, 12с] — не рвём посреди фразы.
                if uncommitted >= 10 * 16_000 {
                    let tail_f32: Vec<f32> = state.samples[state.committed..]
                        .iter()
                        .map(|s| *s as f32 / i16::MAX as f32)
                        .collect();
                    let cut = state.committed + best_cut(&tail_f32, 16_000, 7.0, 12.0);
                    let region_f32: Vec<f32> = state.samples[state.committed..cut]
                        .iter()
                        .map(|s| *s as f32 / i16::MAX as f32)
                        .collect();
                    let seconds = (cut - state.committed) as f64 / 16_000.0;
                    match with_parakeet(&model_dir, |model| {
                        model
                            .transcribe_with(
                                &region_f32,
                                &ParakeetParams {
                                    timestamp_granularity: Some(TimestampGranularity::Segment),
                                    ..Default::default()
                                },
                            )
                            .map_err(|e| LocalError::CommandFailed(e.to_string()))
                    }) {
                        Ok(chunk) => {
                            let text = chunk.text.trim().to_owned();
                            eprintln!(
                                "[dictation::local] stream chunk {:.1}s committed={:.1}s len={}",
                                seconds,
                                cut as f64 / 16_000.0,
                                text.len()
                            );
                            if !text.is_empty() {
                                state.texts.push(text);
                            }
                            state.committed = cut;
                        }
                        Err(e) => {
                            eprintln!("[dictation::local] stream chunk failed: {e}");
                            // Не коммитим — кусок останется в хвосте для
                            // полного прохода после stop.
                            break;
                        }
                    }
                }
            }
            state
        })
        .expect("spawn parakeet stream thread");
    *stream_slot().lock().unwrap_or_else(|p| p.into_inner()) = Some(StreamingParakeet { join });
}

/// Завершить stream-транскрибу: ждать конца потока (in-flight чанк
/// дозавершится), сохранить PartialTranscript для run_parakeet.
#[cfg(feature = "local-dictation")]
pub(crate) fn finish_parakeet_stream() {
    let streamer = stream_slot()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take();
    let Some(streamer) = streamer else { return };
    let state = match streamer.join.join() {
        Ok(state) => state,
        Err(_) => {
            eprintln!("[dictation::local] stream thread panicked");
            return;
        }
    };
    if state.committed > 0 && !state.texts.is_empty() {
        *stream_partial_slot()
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(PartialTranscript {
            committed: state.committed,
            prefix_hash: hash_pcm16(&state.samples[..state.committed]),
            texts: state.texts,
        });
    }
}

/// Сбросить partial (cancel записи / новая диктовка до transcribe).
#[cfg(feature = "local-dictation")]
pub(crate) fn discard_parakeet_partial() {
    *stream_partial_slot()
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = None;
}

/// Самый тихий срез внутри `samples` в окне [min_secs, max_secs] от начала —
/// центр минимального по энергии 300мс прогона 30мс RMS-фреймов. Возвращает
/// индекс сэмпла в `samples`. Чистая функция.
#[cfg(feature = "local-dictation")]
fn best_cut(samples: &[f32], sample_rate: u32, min_secs: f64, max_secs: f64) -> usize {
    let rate = sample_rate as f64;
    let frame = (0.030 * rate) as usize;
    let run_frames = 10usize; // самое тихое окно = 10 фреймов = 300мс
    let lo = ((min_secs * rate) as usize).min(samples.len());
    let hi = ((max_secs * rate) as usize).min(samples.len());
    if hi <= lo {
        return hi.max(lo.min(1));
    }
    // Энергия каждого фрейма в окне поиска.
    let mut energies: Vec<f64> = Vec::new();
    let mut p = lo;
    while p < hi {
        let end = (p + frame).min(hi);
        let sum_sq: f64 = samples[p..end]
            .iter()
            .map(|s| (*s as f64) * (*s as f64))
            .sum();
        energies.push(sum_sq / (end - p) as f64);
        p = end;
    }
    if energies.len() < run_frames {
        // Окно короче 300мс (тишину искать негде) — режем по max.
        return hi;
    }
    // Центр самого тихого региона: крайние позиции минимального 300мс
    // прогона (пауза может быть длиннее окна — резать надо по середине,
    // а не по первому совпадению).
    let mut best_e = f64::MAX;
    let (mut first, mut last) = (0usize, 0usize);
    for i in 0..=(energies.len() - run_frames) {
        let e: f64 = energies[i..i + run_frames].iter().sum();
        if e < best_e {
            best_e = e;
            first = i;
            last = i;
        } else if e == best_e {
            last = i;
        }
    }
    lo + (first + last + run_frames) * frame / 2
}

/// Точки разреза (индексы сэмплов) для длинного аудио: жадный TDT/RNNT
/// декодер Parakeet вырождается в повторы на длинных проходах (>20с), поэтому
/// режем на куски ≤ max_secs по самому тихому 300мс окну в диапазоне
/// [min_secs, max_secs] от начала каждого куска. Чистая функция.
#[cfg(feature = "local-dictation")]
fn split_points(samples: &[f32], sample_rate: u32, min_secs: f64, max_secs: f64) -> Vec<usize> {
    let rate = sample_rate as f64;
    let max_len = (max_secs * rate) as usize;
    let mut points = Vec::new();
    let mut start = 0usize;
    while samples.len().saturating_sub(start) > max_len {
        let window_end = (start + max_len).min(samples.len());
        let cut = start + best_cut(&samples[start..window_end], sample_rate, min_secs, max_secs);
        let cut = cut.clamp(start + 1, window_end);
        points.push(cut);
        start = cut;
    }
    points
}

/// Страховка от вырождения жадного декодера: схлопывает n-граммы (n=1..4,
/// токены по whitespace, сравнение без регистра и конечной пунктуации),
/// повторённые подряд ≥3 раз, до одного экземпляра. Легитимные дубли
/// («очень очень») не трогаем — порог именно 3+.
#[cfg(feature = "local-dictation")]
fn collapse_repeats(text: &str) -> String {
    fn norm(token: &str) -> String {
        token
            .trim_end_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    }
    let toks: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<&str> = Vec::with_capacity(toks.len());
    let mut i = 0usize;
    while i < toks.len() {
        let mut collapsed = false;
        for n in 1..=4usize {
            if i + n * 3 > toks.len() {
                continue;
            }
            let gram: Vec<String> = toks[i..i + n].iter().map(|t| norm(t)).collect();
            let mut run = 1usize;
            while i + (run + 1) * n <= toks.len()
                && toks[i + run * n..i + (run + 1) * n]
                    .iter()
                    .map(|t| norm(t))
                    .eq(gram.iter().cloned())
            {
                run += 1;
            }
            if run >= 3 {
                out.extend_from_slice(&toks[i..i + n]);
                i += run * n;
                collapsed = true;
                break;
            }
        }
        if !collapsed {
            out.push(toks[i]);
            i += 1;
        }
    }
    out.join(" ")
}

#[cfg(feature = "local-dictation")]
fn run_parakeet(req: OwnedLocalRequest) -> Result<TranscriptionResult, LocalError> {
    let model_path = ensure_existing_model_path(
        req.model_path
            .as_deref()
            .ok_or(LocalError::MissingModelPath)?,
    )?;
    ensure_onnxruntime()?;
    let (wav_path, out_base) = temp_audio_paths()?;
    fs::write(&wav_path, &req.wav_bytes).map_err(|e| LocalError::TempAudio(e.to_string()))?;

    let result = (|| {
        let samples = transcribe_rs::audio::read_wav_samples(&wav_path)
            .map_err(|e| LocalError::CommandFailed(e.to_string()))?;
        // Streaming-транскриба во время записи: если partial совпал по
        // хэшу i16-префикса — транскрибируем только хвост. i16 читаем
        // напрямую из wav-байтов (f32-конверсия теряет точность хэша).
        let wav_i16 = wav_pcm16(&req.wav_bytes);
        let mut texts: Vec<String> = Vec::new();
        let mut tail_start = 0usize;
        if let Some(pcm) = wav_i16.as_deref() {
            let partial = stream_partial_slot()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take();
            if let Some(partial) = partial {
                match apply_partial(pcm, &partial) {
                    Some(committed) if pcm.len() == samples.len() => {
                        eprintln!(
                            "[dictation::local] stream partial hit committed={:.1}s \
                             chunks={} tail={:.1}s",
                            committed as f64 / 16_000.0,
                            partial.texts.len(),
                            (samples.len() - committed) as f64 / 16_000.0,
                        );
                        texts = partial.texts;
                        tail_start = committed;
                    }
                    Some(_) => {
                        eprintln!("[dictation::local] stream partial miss: len mismatch");
                    }
                    None => {
                        eprintln!("[dictation::local] stream partial miss: hash/len");
                    }
                }
            }
        }
        // Хвост <0.3с или почти тишина — транскрибировать нечего.
        let tail = &samples[tail_start..];
        let tail_secs = tail.len() as f64 / 16_000.0;
        let tail_rms = if tail.is_empty() {
            0.0f64
        } else {
            (tail.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / tail.len() as f64).sqrt()
        };
        if tail_secs < 0.3 || (tail_start > 0 && tail_rms < 0.001) {
            return Ok(texts.join(" "));
        }
        // >20с за один проход жадный TDT-декодер зацикливается — режем по
        // тихим местам и склеиваем текст.
        let cuts = split_points(tail, 16_000, 12.0, 20.0);
        let mut bounds = Vec::with_capacity(cuts.len() + 1);
        let mut prev = 0usize;
        for cut in &cuts {
            bounds.push(prev..*cut);
            prev = *cut;
        }
        bounds.push(prev..tail.len());
        eprintln!(
            "[dictation::local] parakeet audio={:.1}s chunks={} (tail from {:.1}s)",
            samples.len() as f64 / 16_000.0,
            bounds.len(),
            tail_start as f64 / 16_000.0,
        );
        with_parakeet(&model_path, |model| {
            let mut new_texts: Vec<String> = Vec::with_capacity(bounds.len());
            for (idx, range) in bounds.iter().enumerate() {
                let started = Instant::now();
                let chunk = model
                    .transcribe_with(
                        &tail[range.clone()],
                        &ParakeetParams {
                            timestamp_granularity: Some(TimestampGranularity::Segment),
                            ..Default::default()
                        },
                    )
                    .map_err(|e| LocalError::CommandFailed(e.to_string()))?;
                let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                eprintln!(
                    "[dictation::local] parakeet chunk {}/{} {:.1}s duration_ms={elapsed}",
                    idx + 1,
                    bounds.len(),
                    (range.end - range.start) as f64 / 16_000.0,
                );
                let trimmed = chunk.text.trim().to_owned();
                if !trimmed.is_empty() {
                    new_texts.push(trimmed);
                }
            }
            Ok(new_texts)
        })
        .map(|new_texts| {
            texts.extend(new_texts);
            texts.join(" ")
        })
    })();

    cleanup_temp_outputs(&wav_path, &out_base);
    let text = collapse_repeats(result?.trim());
    let text = text.trim().to_owned();
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
    local_dictation_feature_gate()?;
    transcribe_owned_with_whisper_backend(OwnedLocalRequest::from(req)).await
}

pub(crate) async fn transcribe_with_whisper_backend_model(
    model: &LocalSttModelSpec,
    wav_bytes: &[u8],
    language: &str,
    prompt: &str,
) -> Result<TranscriptionResult, LocalError> {
    local_dictation_feature_gate()?;
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
        #[cfg(feature = "local-dictation")]
        return tokio::task::spawn_blocking(move || run_parakeet(owned))
            .await
            .map_err(|e| LocalError::CommandFailed(format!("worker join failed: {e}")))?;
        #[cfg(not(feature = "local-dictation"))]
        return Err(LocalError::NotBuiltWithLocalDictation);
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
    #[cfg(feature = "local-dictation")]
    unload_parakeet();
    let mut pool = whisper_server_pool().lock().await;
    pool.active = None;
}

pub async fn preload_server(
    engine: &str,
    model_path: Option<&str>,
    command_path: Option<&str>,
) -> Result<bool, LocalError> {
    local_dictation_feature_gate()?;
    if model_path.is_none_or(|path| path.trim().is_empty()) {
        return Err(LocalError::MissingModelPath);
    }
    if is_parakeet_engine(engine) {
        #[cfg(feature = "local-dictation")]
        {
            // Parakeet живёт в процессе (не sidecar) — прогрев = загрузка в
            // резидентный кэш. Вызывается на старте и на recording start.
            let model_dir = ensure_existing_model_path(model_path.unwrap_or_default())?;
            let warm_dir = model_dir.clone();
            tokio::task::spawn_blocking(move || with_parakeet(&warm_dir, |_| Ok(())))
                .await
                .map_err(|e| LocalError::CommandFailed(format!("worker join failed: {e}")))??;
            return Ok(true);
        }
        #[cfg(not(feature = "local-dictation"))]
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
    // Parakeet-модель живёт в этом процессе (не в sidecar) — выгружаем кэш
    // безусловно при delete/switch модели, даже если sidecar недоступен.
    #[cfg(feature = "local-dictation")]
    unload_parakeet();
    match send_sidecar_request(LocalSttRequest::Unload).await {
        Ok(LocalSttResponse::Status(status)) => Ok(!status.warm),
        Ok(other) => Err(LocalError::SidecarUnavailable(format!(
            "unexpected unload response: {other:?}"
        ))),
        Err(error) => Err(error),
    }
}

pub async fn transcribe(req: LocalRequest<'_>) -> Result<TranscriptionResult, LocalError> {
    local_dictation_feature_gate()?;
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
