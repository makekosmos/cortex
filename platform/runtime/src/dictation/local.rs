use std::env;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use reqwest::header::CONTENT_TYPE;
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde_json::Value;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command as TokioCommand};

use super::local_sidecar_protocol::{
    LocalSttAccelerator, LocalSttModelSpec, LocalSttProfile, LocalSttRequest,
    LocalSttRequestEnvelope, LocalSttResponse, LocalSttResponseEnvelope, LocalSttStatus,
};
use super::{config, local_models};

pub const DEFAULT_LOCAL_ENGINE: &str = "whisper.cpp";
pub const FASTER_WHISPER_ENGINE: &str = "faster-whisper";

#[derive(Debug)]
pub struct TranscriptionResult {
    pub text: String,
    pub backend: String,
}

#[derive(Debug, Clone)]
pub struct LocalRequest<'a> {
    pub wav_bytes: &'a [u8],
    pub language: &'a str,
    pub prompt: &'a str,
    pub engine: &'a str,
    pub model_id: Option<&'a str>,
    pub model_path: Option<&'a str>,
    pub command_path: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct OwnedLocalRequest {
    wav_bytes: Vec<u8>,
    language: String,
    prompt: String,
    engine: String,
    model_id: Option<String>,
    model_path: Option<String>,
    command_path: Option<String>,
    accelerator: LocalSttAccelerator,
    profile: LocalSttProfile,
}

impl<'a> From<LocalRequest<'a>> for OwnedLocalRequest {
    fn from(req: LocalRequest<'a>) -> Self {
        Self {
            wav_bytes: req.wav_bytes.to_vec(),
            language: req.language.to_owned(),
            prompt: req.prompt.to_owned(),
            engine: req.engine.to_owned(),
            model_id: req.model_id.map(str::to_owned),
            model_path: req.model_path.map(str::to_owned),
            command_path: req.command_path.map(str::to_owned),
            accelerator: local_stt_accelerator(),
            profile: local_stt_profile(),
        }
    }
}

struct LocalSttSidecarClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_request_id: u64,
}

#[derive(Default)]
struct LocalSttSidecarPool {
    active: Option<LocalSttSidecarClient>,
}

static LOCAL_STT_SIDECAR_POOL: OnceLock<tokio::sync::Mutex<LocalSttSidecarPool>> = OnceLock::new();

fn local_stt_sidecar_pool() -> &'static tokio::sync::Mutex<LocalSttSidecarPool> {
    LOCAL_STT_SIDECAR_POOL.get_or_init(|| tokio::sync::Mutex::new(LocalSttSidecarPool::default()))
}

#[derive(Debug, Error)]
pub enum LocalError {
    #[error("Укажи путь к локальной Whisper-модели в настройках AI")]
    MissingModelPath,
    #[error("Локальная модель не найдена: {path}")]
    ModelPathNotFound { path: String },
    #[error("Укажи путь к локальному whisper.cpp executable в настройках AI")]
    MissingCommandPath,
    #[error("Локальный executable не найден: {path}")]
    CommandPathNotFound { path: String },
    #[error("Локальный движок '{engine}' пока не поддерживается")]
    UnsupportedEngine { engine: String },
    #[error("Не удалось подготовить временный WAV для локальной модели: {0}")]
    TempAudio(String),
    #[error("Локальный STT завершился с ошибкой: {0}")]
    CommandFailed(String),
    #[error("Локальная транскрипция завершилась без текста")]
    EmptyTranscript,
    #[error("Локальный STT sidecar недоступен: {0}")]
    SidecarUnavailable(String),
}

fn is_dictation_test_mode() -> bool {
    matches!(env::var("KOSMOS_TEST_MODE").as_deref(), Ok("1"))
        || matches!(env::var("KOSMOS_HEADLESS").as_deref(), Ok("1"))
}

fn test_override_transcript() -> Option<String> {
    if !cfg!(test) && !is_dictation_test_mode() {
        return None;
    }
    for key in [
        "KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT",
        "KOSMOS_TEST_DICTATION_TRANSCRIPT",
    ] {
        if let Ok(value) = env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

fn direct_sidecar_fallback_allowed() -> bool {
    matches!(
        env::var("KOSMOS_LOCAL_STT_ALLOW_DIRECT_FALLBACK").as_deref(),
        Ok("1")
    )
}

fn local_stt_accelerator() -> LocalSttAccelerator {
    match env::var("KOSMOS_LOCAL_STT_ACCELERATOR")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "cpu" => LocalSttAccelerator::Cpu,
        "gpu" => LocalSttAccelerator::Gpu,
        _ => LocalSttAccelerator::Auto,
    }
}

fn local_stt_profile() -> LocalSttProfile {
    match env::var("KOSMOS_LOCAL_STT_PROFILE")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "accurate" => LocalSttProfile::Accurate,
        _ => LocalSttProfile::Fast,
    }
}

fn local_stt_idle_unload_after_ms_for_engine(engine: &str) -> Option<u64> {
    if let Ok(value) = env::var("KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS") {
        return value.trim().parse::<u64>().ok();
    }
    if is_faster_whisper_engine(engine) {
        None
    } else {
        Some(5 * 60 * 1000)
    }
}

fn local_stt_server_ready_timeout() -> Duration {
    let millis = env::var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(30_000)
        .clamp(1_000, 300_000);
    Duration::from_millis(millis)
}

fn local_stt_sidecar_binary_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["kosmos-local-stt.exe", "Kosmos Local STT.exe"]
    } else {
        &["kosmos-local-stt"]
    }
}

fn local_stt_sidecar_candidate_paths(current_exe: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for name in local_stt_sidecar_binary_names() {
        candidates.push(current_exe.with_file_name(name));
        if let Some(parent) = current_exe.parent().and_then(Path::parent) {
            candidates.push(parent.join(name));
        }
    }
    candidates
}

fn local_stt_sidecar_path() -> Result<PathBuf, LocalError> {
    if let Ok(value) = env::var("KOSMOS_LOCAL_STT_SIDECAR_PATH") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    let current_exe = env::current_exe().map_err(|e| {
        LocalError::SidecarUnavailable(format!("не удалось определить путь текущего процесса: {e}"))
    })?;
    let candidates = local_stt_sidecar_candidate_paths(&current_exe);
    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }

    let searched = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(LocalError::SidecarUnavailable(format!(
        "binary not found; searched: {searched}"
    )))
}

impl LocalSttSidecarClient {
    async fn spawn() -> Result<Self, LocalError> {
        let sidecar_path = local_stt_sidecar_path()?;
        let mut child = TokioCommand::new(&sidecar_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                LocalError::SidecarUnavailable(format!(
                    "не удалось запустить {}: {e}",
                    sidecar_path.display()
                ))
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LocalError::SidecarUnavailable("sidecar stdin is unavailable".into()))?;
        let stdout = child.stdout.take().ok_or_else(|| {
            LocalError::SidecarUnavailable("sidecar stdout is unavailable".into())
        })?;

        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("[kosmos-local-stt] {line}");
                }
            });
        }

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_request_id: 1,
        })
    }

    async fn request(&mut self, request: LocalSttRequest) -> Result<LocalSttResponse, LocalError> {
        if let Some(status) = self
            .child
            .try_wait()
            .map_err(|e| LocalError::SidecarUnavailable(format!("sidecar wait failed: {e}")))?
        {
            return Err(LocalError::SidecarUnavailable(format!(
                "sidecar exited with status {status}"
            )));
        }

        let request_id = self.next_request_id;
        self.next_request_id += 1;
        let envelope = LocalSttRequestEnvelope {
            request_id,
            request,
        };
        let mut payload = serde_json::to_vec(&envelope).map_err(|e| {
            LocalError::SidecarUnavailable(format!("не удалось сериализовать запрос: {e}"))
        })?;
        payload.push(b'\n');

        self.stdin.write_all(&payload).await.map_err(|e| {
            LocalError::SidecarUnavailable(format!("не удалось отправить sidecar-запрос: {e}"))
        })?;
        self.stdin.flush().await.map_err(|e| {
            LocalError::SidecarUnavailable(format!("не удалось завершить sidecar-запрос: {e}"))
        })?;

        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).await.map_err(|e| {
            LocalError::SidecarUnavailable(format!("не удалось прочитать ответ sidecar: {e}"))
        })?;
        if read == 0 {
            return Err(LocalError::SidecarUnavailable(
                "sidecar stdout closed unexpectedly".into(),
            ));
        }
        let envelope: LocalSttResponseEnvelope =
            serde_json::from_str(line.trim()).map_err(|e| {
                LocalError::SidecarUnavailable(format!("не удалось распарсить ответ sidecar: {e}"))
            })?;
        if envelope.request_id != request_id {
            return Err(LocalError::SidecarUnavailable(format!(
                "unexpected response id {}, expected {request_id}",
                envelope.request_id
            )));
        }
        if !envelope.ok {
            return Err(LocalError::CommandFailed(
                envelope
                    .error
                    .unwrap_or_else(|| "sidecar request failed".into()),
            ));
        }
        envelope.response.ok_or_else(|| {
            LocalError::SidecarUnavailable("sidecar returned empty response payload".into())
        })
    }
}

fn model_spec_from_owned(req: &OwnedLocalRequest) -> LocalSttModelSpec {
    LocalSttModelSpec {
        engine: req.engine.clone(),
        model_id: req.model_id.clone(),
        model_path: req.model_path.clone(),
        command_path: req.command_path.clone(),
        accelerator: req.accelerator.clone(),
        profile: req.profile.clone(),
        idle_unload_after_ms: local_stt_idle_unload_after_ms_for_engine(&req.engine),
    }
}

fn owned_request_from_model(
    model: &LocalSttModelSpec,
    wav_bytes: &[u8],
    language: &str,
    prompt: &str,
) -> OwnedLocalRequest {
    OwnedLocalRequest {
        wav_bytes: wav_bytes.to_vec(),
        language: language.to_owned(),
        prompt: prompt.to_owned(),
        engine: model.engine.clone(),
        model_id: model.model_id.clone(),
        model_path: model.model_path.clone(),
        command_path: model.command_path.clone(),
        accelerator: model.accelerator.clone(),
        profile: model.profile.clone(),
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct TestSidecarMock {
    transcript: Option<String>,
    fail_error: Option<String>,
    sidecar_unavailable_remaining: u8,
    seen_ops: Vec<String>,
}

#[cfg(test)]
static TEST_SIDECAR_MOCK: OnceLock<Mutex<Option<TestSidecarMock>>> = OnceLock::new();

#[cfg(test)]
fn test_sidecar_mock_state() -> &'static Mutex<Option<TestSidecarMock>> {
    TEST_SIDECAR_MOCK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(crate) fn install_test_sidecar_mock_for_host(transcript: Option<String>) {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut guard = guard;
    *guard = Some(TestSidecarMock {
        transcript,
        fail_error: None,
        sidecar_unavailable_remaining: 0,
        seen_ops: Vec::new(),
    });
}

#[cfg(test)]
pub(crate) fn install_test_sidecar_unavailable_for_host(times: u8) {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut guard = guard;
    *guard = Some(TestSidecarMock {
        transcript: None,
        fail_error: None,
        sidecar_unavailable_remaining: times,
        seen_ops: Vec::new(),
    });
}

#[cfg(test)]
pub(crate) fn clear_test_sidecar_mock_for_host() {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut guard = guard;
    *guard = None;
}

#[cfg(test)]
pub(crate) fn has_test_sidecar_mock_for_host() -> bool {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.is_some()
}

#[cfg(test)]
pub(crate) async fn clear_test_sidecar_pool_for_host() {
    let mut pool = local_stt_sidecar_pool().lock().await;
    pool.active = None;
}

#[cfg(test)]
pub(crate) fn recorded_test_sidecar_ops_for_host() -> Vec<String> {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard
        .as_ref()
        .map(|mock| mock.seen_ops.clone())
        .unwrap_or_default()
}

#[cfg(test)]
fn test_sidecar_mock_take(op: &str) -> Option<Result<LocalSttResponse, LocalError>> {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut guard = guard;
    let mock = guard.as_mut()?;
    mock.seen_ops.push(op.to_owned());
    if mock.sidecar_unavailable_remaining > 0 {
        mock.sidecar_unavailable_remaining -= 1;
        return Some(Err(LocalError::SidecarUnavailable(
            "mock sidecar exited".into(),
        )));
    }
    if let Some(error) = mock.fail_error.clone() {
        return Some(Err(LocalError::CommandFailed(error)));
    }
    Some(Ok(match op {
        "preload" | "load_model" => LocalSttResponse::Status(LocalSttStatus {
            warm: true,
            loaded_model: None,
            backend: Some("mock_sidecar".into()),
            accelerator: LocalSttAccelerator::Auto,
            device: None,
            profile: LocalSttProfile::Fast,
            idle_unload_after_ms: local_stt_idle_unload_after_ms_for_engine(DEFAULT_LOCAL_ENGINE),
        }),
        "transcribe" => {
            LocalSttResponse::Transcription(super::local_sidecar_protocol::LocalSttTranscription {
                text: mock
                    .transcript
                    .clone()
                    .unwrap_or_else(|| "mock sidecar transcript".into()),
                backend: "mock_sidecar".into(),
            })
        }
        _ => LocalSttResponse::Ack(super::local_sidecar_protocol::LocalSttAck {
            accepted: true,
            message: None,
        }),
    }))
}

async fn send_sidecar_request(request: LocalSttRequest) -> Result<LocalSttResponse, LocalError> {
    #[cfg(test)]
    {
        let op_name = match &request {
            LocalSttRequest::Status => "status",
            LocalSttRequest::LoadModel { .. } => "load_model",
            LocalSttRequest::Preload { .. } => "preload",
            LocalSttRequest::Transcribe { .. } => "transcribe",
            LocalSttRequest::Cancel { .. } => "cancel",
            LocalSttRequest::Unload => "unload",
            LocalSttRequest::Shutdown => "shutdown",
        };
        for attempt in 0..2 {
            if let Some(response) = test_sidecar_mock_take(op_name) {
                match response {
                    Err(LocalError::SidecarUnavailable(_)) if attempt == 0 => continue,
                    other => return other,
                }
            } else {
                break;
            }
        }

        if !matches!(
            env::var("KOSMOS_TEST_ALLOW_REAL_LOCAL_STT_SIDECAR").as_deref(),
            Ok("1")
        ) {
            return Err(LocalError::SidecarUnavailable(
                "real local STT sidecar is disabled in unit tests".into(),
            ));
        }
    }

    let mut pool = local_stt_sidecar_pool().lock().await;
    let request_clone = request.clone();
    for attempt in 0..2 {
        if pool.active.is_none() {
            pool.active = Some(LocalSttSidecarClient::spawn().await?);
        }

        let response = {
            let client = pool.active.as_mut().expect("sidecar client initialized");
            client
                .request(if attempt == 0 {
                    request.clone()
                } else {
                    request_clone.clone()
                })
                .await
        };
        match response {
            Ok(value) => return Ok(value),
            Err(LocalError::SidecarUnavailable(_)) if attempt == 0 => {
                pool.active = None;
            }
            Err(error) => return Err(error),
        }
    }
    Err(LocalError::SidecarUnavailable(
        "sidecar request retry budget exhausted".into(),
    ))
}

fn is_supported_engine(engine: &str) -> bool {
    let normalized = engine.trim().to_ascii_lowercase();
    normalized.is_empty()
        || matches!(
            normalized.as_str(),
            DEFAULT_LOCAL_ENGINE
                | "whisper"
                | "whisper-cpp"
                | FASTER_WHISPER_ENGINE
                | "faster_whisper"
        )
}

pub(crate) fn is_faster_whisper_engine(engine: &str) -> bool {
    matches!(
        engine.trim().to_ascii_lowercase().as_str(),
        FASTER_WHISPER_ENGINE | "faster_whisper"
    )
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

fn temp_audio_paths() -> Result<(PathBuf, PathBuf), LocalError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| LocalError::TempAudio(e.to_string()))?
        .as_nanos();
    let base = env::temp_dir().join(format!("kosmos-local-dictation-{stamp}"));
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
    env::var("KOSMOS_LOCAL_WHISPER_THREADS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|n| n.get().clamp(1, 8))
                .unwrap_or(4)
        })
}

fn strip_whisper_timestamps(text: &str) -> String {
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
        .join("\n")
}

fn read_transcript(stdout: &[u8], out_base: &Path) -> Result<String, LocalError> {
    let txt_path = out_base.with_extension("txt");
    if let Ok(file_text) = fs::read_to_string(&txt_path) {
        let text = strip_whisper_timestamps(&file_text);
        if !text.trim().is_empty() {
            return Ok(text.trim().to_owned());
        }
    }

    let stdout_text = String::from_utf8_lossy(stdout).trim().to_owned();
    if !stdout_text.is_empty() {
        let text = strip_whisper_timestamps(&stdout_text);
        if !text.trim().is_empty() {
            return Ok(text.trim().to_owned());
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

    let text = strip_whisper_timestamps(trimmed);
    if text.trim().is_empty() {
        Err(LocalError::EmptyTranscript)
    } else {
        Ok(text.trim().to_owned())
    }
}

fn extract_transcript_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let text = strip_whisper_timestamps(text);
            (!text.trim().is_empty()).then_some(text.trim().to_owned())
        }
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
                if !joined.trim().is_empty() {
                    return Some(joined.trim().to_owned());
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

    form = form.text("response_format", "json");

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

    if let Some(language) = whisper_language_arg(&req.language) {
        command.arg("-l").arg(language);
    }
    if !req.prompt.trim().is_empty() {
        command.arg("--prompt").arg(req.prompt.trim());
    }

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

fn faster_whisper_python() -> String {
    env::var("KOSMOS_FASTER_WHISPER_PYTHON")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| local_models::faster_whisper_python_command(&config::data_dir()))
}

fn faster_whisper_timeout() -> Duration {
    env::var("KOSMOS_FASTER_WHISPER_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|millis| millis.clamp(5_000, 55_000))
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(55))
}

fn faster_whisper_runtime_dirs() -> Vec<PathBuf> {
    let data_dir = config::data_dir();
    let mut dll_dirs = Vec::<PathBuf>::new();
    for key in [
        "KOSMOS_FASTER_WHISPER_DLL_DIRS",
        "KOSMOS_FASTER_WHISPER_DLL_DIR",
    ] {
        if let Some(value) = env::var_os(key) {
            for dir in env::split_paths(&value) {
                if dir.is_dir() && !dll_dirs.iter().any(|existing| existing == &dir) {
                    dll_dirs.push(dir);
                }
            }
        }
    }
    for dir in local_models::faster_whisper_cuda_runtime_dirs(&data_dir) {
        if dir.is_dir() && !dll_dirs.iter().any(|existing| existing == &dir) {
            dll_dirs.push(dir);
        }
    }
    dll_dirs
}

fn apply_faster_whisper_environment_to_tokio(command: &mut TokioCommand) {
    let dll_dirs = faster_whisper_runtime_dirs();
    if dll_dirs.is_empty() {
        return;
    }
    if let Ok(joined) = env::join_paths(&dll_dirs) {
        command.env("KOSMOS_FASTER_WHISPER_DLL_DIRS", joined);
    }
}

fn faster_whisper_cuda_runtime_available() -> bool {
    local_models::faster_whisper_cuda_runtime_available(&config::data_dir())
}

fn faster_whisper_model_arg(req: &OwnedLocalRequest) -> Result<String, LocalError> {
    if let Ok(model) = env::var("KOSMOS_FASTER_WHISPER_MODEL") {
        let model = model.trim();
        if !model.is_empty() {
            return Ok(model.to_owned());
        }
    }

    let raw_model = req
        .model_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(LocalError::MissingModelPath)?;
    let path = Path::new(raw_model);
    if path.is_dir() {
        return Ok(raw_model.to_owned());
    }
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("bin"))
    {
        return Err(LocalError::CommandFailed(
            "faster-whisper requires a CTranslate2 model directory; whisper.cpp ggml .bin models are not compatible".into(),
        ));
    }
    Ok(raw_model.to_owned())
}

fn temp_python_script_path() -> Result<PathBuf, LocalError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| LocalError::TempAudio(e.to_string()))?
        .as_nanos();
    Ok(env::temp_dir().join(format!("kosmos-faster-whisper-worker-{stamp}.py")))
}

fn faster_whisper_runtime_args(accelerator: &LocalSttAccelerator) -> (&'static str, &'static str) {
    match accelerator {
        LocalSttAccelerator::Gpu => ("cuda", "float16"),
        LocalSttAccelerator::Auto if faster_whisper_cuda_runtime_available() => ("cuda", "float16"),
        LocalSttAccelerator::Cpu | LocalSttAccelerator::Auto => ("cpu", "int8"),
    }
}

fn run_faster_whisper(
    req: OwnedLocalRequest,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> Result<TranscriptionResult, LocalError> {
    let model_arg = faster_whisper_model_arg(&req)?;
    let (wav_path, out_base) = temp_audio_paths()?;
    fs::write(&wav_path, &req.wav_bytes).map_err(|e| LocalError::TempAudio(e.to_string()))?;

    let (device, compute_type) = faster_whisper_runtime_args(&req.accelerator);
    let beam_size = match req.profile {
        LocalSttProfile::Fast => "1",
        LocalSttProfile::Accurate => "5",
    };
    let language = whisper_language_arg(&req.language).unwrap_or_default();
    let download_root = local_models::faster_whisper_dir(&config::data_dir());
    if let Err(e) = fs::create_dir_all(&download_root) {
        cleanup_temp_outputs(&wav_path, &out_base);
        return Err(LocalError::CommandFailed(format!(
            "failed to create faster-whisper cache dir: {e}"
        )));
    }
    let script = r#"
import json
import os
import sys
import time

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

dll_dirs_raw = os.environ.get("KOSMOS_FASTER_WHISPER_DLL_DIRS", "").strip()
if not dll_dirs_raw:
    dll_dirs_raw = os.environ.get("KOSMOS_FASTER_WHISPER_DLL_DIR", "").strip()
_dll_dir_handles = []
if dll_dirs_raw and hasattr(os, "add_dll_directory"):
    for dll_dir in [part.strip() for part in dll_dirs_raw.split(os.pathsep)]:
        if dll_dir:
            _dll_dir_handles.append(os.add_dll_directory(dll_dir))

if os.environ.get("KOSMOS_FASTER_WHISPER_TRACE") == "1":
    print(
        json.dumps(
            {
                "event": "python_start",
                "executable": sys.executable,
                "version": sys.version,
                "dll_dirs": dll_dirs_raw,
            },
            ensure_ascii=False,
        ),
        file=sys.stderr,
        flush=True,
    )

try:
    from faster_whisper import WhisperModel
except Exception as exc:
    print(json.dumps({"error": f"faster-whisper Python package is not installed: {exc}"}), flush=True)
    sys.exit(2)

model_path, wav_path, language, prompt, device, compute_type, beam_size, download_root = sys.argv[1:9]
trace = os.environ.get("KOSMOS_FASTER_WHISPER_TRACE") == "1"
started_at = time.time()

def trace_event(event, **payload):
    if not trace:
        return
    payload = {"event": event, "dt": time.time() - started_at, **payload}
    print(json.dumps(payload, ensure_ascii=False), file=sys.stderr, flush=True)

try:
    trace_event("load_start", model_path=model_path, device=device, compute_type=compute_type, download_root=download_root)
    model = WhisperModel(model_path, device=device, compute_type=compute_type, download_root=download_root, local_files_only=True)
    trace_event("load_done")
    kwargs = {
        "beam_size": int(beam_size),
        "condition_on_previous_text": False,
        "vad_filter": True,
        "vad_parameters": {"min_silence_duration_ms": 500},
    }
    if language:
        kwargs["language"] = language
    if prompt:
        kwargs["initial_prompt"] = prompt
    segments, _info = model.transcribe(wav_path, **kwargs)
    trace_event("transcribe_returned")
    text = " ".join(segment.text.strip() for segment in segments).strip()
    trace_event("done", text_len=len(text))
    print(json.dumps({"text": text}), flush=True)
except Exception as exc:
    print(json.dumps({"error": str(exc)}), flush=True)
    sys.exit(1)
"#;

    let script_path = out_base.with_extension("py");
    if let Err(e) = fs::write(&script_path, script) {
        cleanup_temp_outputs(&wav_path, &out_base);
        return Err(LocalError::TempAudio(e.to_string()));
    }
    let python = faster_whisper_python();
    if matches!(env::var("KOSMOS_FASTER_WHISPER_TRACE").as_deref(), Ok("1")) {
        eprintln!(
            "[faster-whisper-trace] spawn python={} script={} model={} wav={} device={} compute={} beam={} download_root={}",
            python,
            script_path.display(),
            model_arg,
            wav_path.display(),
            device,
            compute_type,
            beam_size,
            download_root.display()
        );
    }
    let mut command = Command::new(python);
    command
        .arg(&script_path)
        .arg(&model_arg)
        .arg(&wav_path)
        .arg(language)
        .arg(req.prompt.trim())
        .arg(device)
        .arg(compute_type)
        .arg(beam_size)
        .arg(&download_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");
    augment_faster_whisper_environment(&mut command);
    let mut child = command.spawn().map_err(|e| {
        cleanup_temp_outputs(&wav_path, &out_base);
        LocalError::CommandFailed(format!("failed to launch faster-whisper python: {e}"))
    })?;
    if matches!(env::var("KOSMOS_FASTER_WHISPER_TRACE").as_deref(), Ok("1")) {
        eprintln!("[faster-whisper-trace] spawned pid={}", child.id());
    }
    let timeout = faster_whisper_timeout();
    let deadline = Instant::now() + timeout;

    loop {
        if cancel_flag
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            let _ = child.kill();
            let _ = child.wait();
            cleanup_temp_outputs(&wav_path, &out_base);
            return Err(LocalError::CommandFailed("faster-whisper cancelled".into()));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().ok();
            cleanup_temp_outputs(&wav_path, &out_base);
            let stderr = output
                .as_ref()
                .map(|output| String::from_utf8_lossy(&output.stderr).trim().to_owned())
                .filter(|value| !value.is_empty())
                .map(|value| format!("; stderr={value}"))
                .unwrap_or_default();
            return Err(LocalError::CommandFailed(format!(
                "faster-whisper timed out after {}s{}",
                timeout.as_secs(),
                stderr
            )));
        }
        if child
            .try_wait()
            .map_err(|e| LocalError::CommandFailed(format!("faster-whisper wait failed: {e}")))?
            .is_some()
        {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }

    let output = child.wait_with_output().map_err(|e| {
        cleanup_temp_outputs(&wav_path, &out_base);
        LocalError::CommandFailed(format!("faster-whisper output failed: {e}"))
    })?;

    cleanup_temp_outputs(&wav_path, &out_base);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = serde_json::from_str::<Value>(stdout.trim()).ok();
    if !output.status.success() {
        let message = parsed
            .as_ref()
            .and_then(|value| value.get("error"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                (!stderr.is_empty()).then_some(stderr)
            })
            .unwrap_or_else(|| format!("exit code {:?}", output.status.code()));
        return Err(LocalError::CommandFailed(format!(
            "faster-whisper failed: {message}"
        )));
    }

    let text = parsed
        .and_then(|value| value.get("text").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
        .trim()
        .to_owned();
    if text.is_empty() {
        return Err(LocalError::EmptyTranscript);
    }
    Ok(TranscriptionResult {
        text,
        backend: "faster_whisper".into(),
    })
}

fn augment_faster_whisper_environment(command: &mut Command) {
    let dll_dirs = faster_whisper_runtime_dirs();
    if dll_dirs.is_empty() {
        return;
    }
    if let Ok(joined) = env::join_paths(&dll_dirs) {
        command.env("KOSMOS_FASTER_WHISPER_DLL_DIRS", joined);
    }
    let current_path = env::var_os("PATH").unwrap_or_default();
    let mut paths = dll_dirs;
    paths.extend(env::split_paths(&current_path));
    if let Ok(joined) = env::join_paths(paths) {
        command.env("PATH", joined);
    }
}

pub(crate) struct FasterWhisperWorker {
    key: FasterWhisperWorkerKey,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    script_path: PathBuf,
    next_request_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FasterWhisperWorkerKey {
    model_arg: String,
    accelerator: LocalSttAccelerator,
    profile: LocalSttProfile,
}

fn faster_whisper_worker_key(
    model: &LocalSttModelSpec,
) -> Result<FasterWhisperWorkerKey, LocalError> {
    let owned = owned_request_from_model(model, &[], "", "");
    Ok(FasterWhisperWorkerKey {
        model_arg: faster_whisper_model_arg(&owned)?,
        accelerator: model.accelerator.clone(),
        profile: model.profile.clone(),
    })
}

impl FasterWhisperWorker {
    pub(crate) async fn start(model: &LocalSttModelSpec) -> Result<Self, LocalError> {
        match Self::start_inner(model).await {
            Ok(worker) => Ok(worker),
            Err(err)
                if matches!(model.accelerator, LocalSttAccelerator::Auto)
                    && faster_whisper_cuda_runtime_available() =>
            {
                tracing::warn!(
                    error = %err,
                    "dictation: faster-whisper CUDA preload failed, retrying on CPU"
                );
                let mut cpu_model = model.clone();
                cpu_model.accelerator = LocalSttAccelerator::Cpu;
                let mut worker = Self::start_inner(&cpu_model).await?;
                worker.key.accelerator = LocalSttAccelerator::Auto;
                Ok(worker)
            }
            Err(err) => Err(err),
        }
    }

    async fn start_inner(model: &LocalSttModelSpec) -> Result<Self, LocalError> {
        let start = Instant::now();
        let key = faster_whisper_worker_key(model)?;
        let model_arg = key.model_arg.clone();
        let (device, compute_type) = faster_whisper_runtime_args(&model.accelerator);
        let beam_size = match model.profile {
            LocalSttProfile::Fast => "1",
            LocalSttProfile::Accurate => "5",
        };
        let download_root = local_models::faster_whisper_dir(&config::data_dir());
        fs::create_dir_all(&download_root).map_err(|e| {
            LocalError::CommandFailed(format!("failed to create faster-whisper cache dir: {e}"))
        })?;

        let script_path = temp_python_script_path()?;
        fs::write(&script_path, FASTER_WHISPER_WORKER_SCRIPT)
            .map_err(|e| LocalError::TempAudio(e.to_string()))?;

        let mut command = TokioCommand::new(faster_whisper_python());
        command
            .arg(&script_path)
            .arg(&model_arg)
            .arg(device)
            .arg(compute_type)
            .arg(beam_size)
            .arg(&download_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8");
        apply_faster_whisper_environment_to_tokio(&mut command);

        let mut child = command.spawn().map_err(|e| {
            let _ = fs::remove_file(&script_path);
            LocalError::CommandFailed(format!("failed to launch faster-whisper worker: {e}"))
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            let _ = fs::remove_file(&script_path);
            LocalError::CommandFailed("faster-whisper worker stdin is unavailable".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            let _ = fs::remove_file(&script_path);
            LocalError::CommandFailed("faster-whisper worker stdout is unavailable".into())
        })?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("[faster-whisper-worker] {line}");
                }
            });
        }

        let mut worker = Self {
            key,
            child,
            stdin,
            stdout: BufReader::new(stdout),
            script_path,
            next_request_id: 1,
        };
        worker.wait_ready().await?;
        tracing::info!(
            engine = %model.engine,
            model_id = ?model.model_id,
            accelerator = ?model.accelerator,
            profile = ?model.profile,
            duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
            "dictation: faster-whisper worker ready"
        );
        Ok(worker)
    }

    pub(crate) fn matches_model(&self, model: &LocalSttModelSpec) -> bool {
        faster_whisper_worker_key(model).is_ok_and(|key| key == self.key)
    }

    async fn wait_ready(&mut self) -> Result<(), LocalError> {
        let mut line = String::new();
        let read = tokio::time::timeout(faster_whisper_timeout(), self.stdout.read_line(&mut line))
            .await
            .map_err(|_| {
                LocalError::CommandFailed("faster-whisper worker preload timed out".into())
            })?
            .map_err(|e| {
                LocalError::CommandFailed(format!("faster-whisper worker read failed: {e}"))
            })?;
        if read == 0 {
            return Err(LocalError::CommandFailed(
                "faster-whisper worker exited during preload".into(),
            ));
        }
        let parsed = serde_json::from_str::<Value>(line.trim()).map_err(|e| {
            LocalError::CommandFailed(format!("faster-whisper worker returned invalid JSON: {e}"))
        })?;
        if parsed.get("kind").and_then(Value::as_str) == Some("ready") {
            return Ok(());
        }
        let message = parsed
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("faster-whisper worker preload failed");
        Err(LocalError::CommandFailed(message.to_owned()))
    }

    pub(crate) async fn transcribe(
        &mut self,
        wav_bytes: &[u8],
        language: &str,
        prompt: &str,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<TranscriptionResult, LocalError> {
        let start = Instant::now();
        if cancel_flag
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            return Err(LocalError::CommandFailed("faster-whisper cancelled".into()));
        }
        let (wav_path, out_base) = temp_audio_paths()?;
        fs::write(&wav_path, wav_bytes).map_err(|e| LocalError::TempAudio(e.to_string()))?;
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        let payload = serde_json::json!({
            "id": request_id,
            "wav_path": wav_path,
            "language": whisper_language_arg(language).unwrap_or_default(),
            "prompt": prompt.trim(),
        });
        let mut bytes = serde_json::to_vec(&payload)
            .map_err(|e| LocalError::CommandFailed(format!("worker request encode failed: {e}")))?;
        bytes.push(b'\n');
        self.stdin.write_all(&bytes).await.map_err(|e| {
            cleanup_temp_outputs(&wav_path, &out_base);
            LocalError::CommandFailed(format!("faster-whisper worker write failed: {e}"))
        })?;
        self.stdin.flush().await.map_err(|e| {
            cleanup_temp_outputs(&wav_path, &out_base);
            LocalError::CommandFailed(format!("faster-whisper worker flush failed: {e}"))
        })?;

        let mut line = String::new();
        let read = tokio::time::timeout(faster_whisper_timeout(), self.stdout.read_line(&mut line))
            .await
            .map_err(|_| LocalError::CommandFailed("faster-whisper worker timed out".into()))?
            .map_err(|e| {
                LocalError::CommandFailed(format!("faster-whisper worker read failed: {e}"))
            })?;
        let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        cleanup_temp_outputs(&wav_path, &out_base);
        if read == 0 {
            return Err(LocalError::CommandFailed(
                "faster-whisper worker stdout closed".into(),
            ));
        }
        let parsed = serde_json::from_str::<Value>(line.trim()).map_err(|e| {
            LocalError::CommandFailed(format!("faster-whisper worker returned invalid JSON: {e}"))
        })?;
        if parsed.get("id").and_then(Value::as_u64) != Some(request_id) {
            return Err(LocalError::CommandFailed(
                "faster-whisper worker returned stale response".into(),
            ));
        }
        if parsed.get("ok").and_then(Value::as_bool) != Some(true) {
            let message = parsed
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("faster-whisper worker failed");
            return Err(LocalError::CommandFailed(message.to_owned()));
        }
        let text = parsed
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if text.is_empty() {
            return Err(LocalError::EmptyTranscript);
        }
        let python_duration_ms = parsed.get("duration_ms").and_then(Value::as_u64);
        tracing::info!(
            request_id,
            bytes = wav_bytes.len(),
            duration_ms = elapsed_ms,
            python_duration_ms,
            "dictation: faster-whisper worker transcribed"
        );
        Ok(TranscriptionResult {
            text,
            backend: "faster_whisper".into(),
        })
    }
}

impl Drop for FasterWhisperWorker {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = fs::remove_file(&self.script_path);
    }
}

const FASTER_WHISPER_WORKER_SCRIPT: &str = r#"
import json
import os
import sys
import tempfile
import time
import wave

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

dll_dirs_raw = os.environ.get("KOSMOS_FASTER_WHISPER_DLL_DIRS", "").strip()
if not dll_dirs_raw:
    dll_dirs_raw = os.environ.get("KOSMOS_FASTER_WHISPER_DLL_DIR", "").strip()
_dll_dir_handles = []
if dll_dirs_raw and hasattr(os, "add_dll_directory"):
    for dll_dir in [part.strip() for part in dll_dirs_raw.split(os.pathsep)]:
        if dll_dir:
            _dll_dir_handles.append(os.add_dll_directory(dll_dir))

try:
    from faster_whisper import WhisperModel
    model_path, device, compute_type, beam_size, download_root = sys.argv[1:6]
    model = WhisperModel(model_path, device=device, compute_type=compute_type, download_root=download_root, local_files_only=True)
    beam_size = int(beam_size)

    warmup = tempfile.NamedTemporaryFile(prefix="kosmos-fw-warmup-", suffix=".wav", delete=False)
    warmup.close()
    with wave.open(warmup.name, "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(16000)
        wav.writeframes(b"\x00\x00" * 1600)
    try:
        segments, _info = model.transcribe(warmup.name, language="ru", beam_size=beam_size, vad_filter=False)
        for _segment in segments:
            pass
    finally:
        try:
            os.unlink(warmup.name)
        except OSError:
            pass

    print(json.dumps({"kind": "ready"}), flush=True)
except Exception as exc:
    print(json.dumps({"kind": "error", "error": str(exc)}), flush=True)
    sys.exit(1)

for raw_line in sys.stdin:
    raw_line = raw_line.strip()
    if not raw_line:
        continue
    try:
        request = json.loads(raw_line)
        request_id = request.get("id")
        kwargs = {
            "beam_size": beam_size,
            "condition_on_previous_text": False,
            "vad_filter": True,
            "vad_parameters": {"min_silence_duration_ms": 500},
        }
        language = request.get("language") or ""
        prompt = request.get("prompt") or ""
        if language:
            kwargs["language"] = language
        if prompt:
            kwargs["initial_prompt"] = prompt
        transcribe_started = time.time()
        segments, _info = model.transcribe(request["wav_path"], **kwargs)
        text = " ".join(segment.text.strip() for segment in segments).strip()
        duration_ms = int((time.time() - transcribe_started) * 1000)
        print(json.dumps({"id": request_id, "ok": True, "text": text, "duration_ms": duration_ms, "vad_filter": True}), flush=True)
    except Exception as exc:
        print(json.dumps({"id": request.get("id") if "request" in locals() else None, "ok": False, "error": str(exc)}), flush=True)
"#;

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

    if is_faster_whisper_engine(&owned.engine) {
        return tokio::task::spawn_blocking(move || run_faster_whisper(owned, None))
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
    if !is_faster_whisper_engine(engine) && command_path.is_none_or(|path| path.trim().is_empty()) {
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
        // Explicit debug escape hatch: managed product path must use the Kosmos-owned
        // sidecar. Direct whisper-server/cli execution is available only when the
        // developer opts in with KOSMOS_LOCAL_STT_ALLOW_DIRECT_FALLBACK=1.
        Err(LocalError::SidecarUnavailable(_))
            if direct_sidecar_fallback_allowed() && is_faster_whisper_engine(engine) =>
        {
            Ok(false)
        }
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
    if let Some(text) = test_override_transcript() {
        return Ok(TranscriptionResult {
            text,
            backend: "test_override".into(),
        });
    }

    if !is_supported_engine(req.engine) {
        return Err(LocalError::UnsupportedEngine {
            engine: req.engine.to_owned(),
        });
    }
    if req.model_path.is_none_or(|path| path.trim().is_empty()) {
        return Err(LocalError::MissingModelPath);
    }
    if !is_faster_whisper_engine(req.engine)
        && req.command_path.is_none_or(|path| path.trim().is_empty())
    {
        return Err(LocalError::MissingCommandPath);
    }

    let owned = OwnedLocalRequest::from(req);
    let request = LocalSttRequest::Transcribe {
        model: model_spec_from_owned(&owned),
        wav_base64: base64::engine::general_purpose::STANDARD.encode(&owned.wav_bytes),
        language: owned.language.clone(),
        prompt: owned.prompt.clone(),
    };

    match send_sidecar_request(request).await {
        Ok(LocalSttResponse::Transcription(transcription)) => Ok(TranscriptionResult {
            text: transcription.text,
            backend: transcription.backend,
        }),
        Ok(other) => Err(LocalError::SidecarUnavailable(format!(
            "unexpected transcribe response: {other:?}"
        ))),
        // Explicit debug escape hatch: managed product path must use the Kosmos-owned
        // sidecar. Direct whisper-server/cli execution is available only when the
        // developer opts in with KOSMOS_LOCAL_STT_ALLOW_DIRECT_FALLBACK=1.
        Err(LocalError::SidecarUnavailable(_)) if direct_sidecar_fallback_allowed() => {
            transcribe_with_whisper_backend(LocalRequest {
                wav_bytes: &owned.wav_bytes,
                language: &owned.language,
                prompt: &owned.prompt,
                engine: &owned.engine,
                model_id: owned.model_id.as_deref(),
                model_path: owned.model_path.as_deref(),
                command_path: owned.command_path.as_deref(),
            })
            .await
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCAL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[test]
    fn sidecar_candidates_include_packaged_windows_name() {
        let candidates =
            local_stt_sidecar_candidate_paths(Path::new(r"C:\Kosmos\resources\Kosmos Runtime.exe"));
        let rendered = candidates
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(rendered
            .iter()
            .any(|path| path.ends_with("kosmos-local-stt.exe")));
        if cfg!(windows) {
            assert!(rendered
                .iter()
                .any(|path| path.ends_with("Kosmos Local STT.exe")));
        }
    }

    fn install_test_sidecar_mock(mock: Option<TestSidecarMock>) {
        let guard = match test_sidecar_mock_state().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut guard = guard;
        *guard = mock;
    }

    fn recorded_test_sidecar_ops() -> Vec<String> {
        let guard = match test_sidecar_mock_state().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard
            .as_ref()
            .map(|mock| mock.seen_ops.clone())
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn local_override_is_available_in_unit_tests() {
        let _guard = ENV_LOCAL_LOCK.lock().await;
        env::remove_var("KOSMOS_TEST_MODE");
        env::remove_var("KOSMOS_HEADLESS");
        env::set_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT", "local transcript");
        assert_eq!(
            test_override_transcript().as_deref(),
            Some("local transcript")
        );

        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
    }

    #[tokio::test]
    async fn faster_whisper_default_idle_keeps_worker_warm() {
        let _guard = ENV_LOCAL_LOCK.lock().await;
        env::remove_var("KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS");

        assert_eq!(
            local_stt_idle_unload_after_ms_for_engine(FASTER_WHISPER_ENGINE),
            None
        );
        assert_eq!(
            local_stt_idle_unload_after_ms_for_engine(DEFAULT_LOCAL_ENGINE),
            Some(5 * 60 * 1000)
        );

        env::set_var("KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS", "1234");
        assert_eq!(
            local_stt_idle_unload_after_ms_for_engine(FASTER_WHISPER_ENGINE),
            Some(1234)
        );
        env::remove_var("KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS");
    }

    #[test]
    fn faster_whisper_worker_key_ignores_non_runtime_metadata() {
        let preload_model = LocalSttModelSpec {
            engine: FASTER_WHISPER_ENGINE.into(),
            model_id: None,
            model_path: Some("large-v3-turbo".into()),
            command_path: None,
            accelerator: LocalSttAccelerator::Gpu,
            profile: LocalSttProfile::Fast,
            idle_unload_after_ms: None,
        };
        let transcribe_model = LocalSttModelSpec {
            engine: FASTER_WHISPER_ENGINE.into(),
            model_id: Some("whisper-large-v3-turbo".into()),
            model_path: Some("large-v3-turbo".into()),
            command_path: None,
            accelerator: LocalSttAccelerator::Gpu,
            profile: LocalSttProfile::Fast,
            idle_unload_after_ms: Some(300_000),
        };

        assert_eq!(
            faster_whisper_worker_key(&preload_model).expect("preload key"),
            faster_whisper_worker_key(&transcribe_model).expect("transcribe key")
        );
    }

    #[test]
    fn faster_whisper_runtime_args_resolve_safe_defaults() {
        assert_eq!(
            faster_whisper_runtime_args(&LocalSttAccelerator::Cpu),
            ("cpu", "int8")
        );
        assert_eq!(
            faster_whisper_runtime_args(&LocalSttAccelerator::Gpu),
            ("cuda", "float16")
        );
        let expected_auto = if faster_whisper_cuda_runtime_available() {
            ("cuda", "float16")
        } else {
            ("cpu", "int8")
        };
        assert_eq!(
            faster_whisper_runtime_args(&LocalSttAccelerator::Auto),
            expected_auto
        );
    }

    #[tokio::test]
    async fn local_transcribe_errors_without_model_path() {
        let err = transcribe_with_whisper_backend(LocalRequest {
            wav_bytes: b"wav",
            language: "ru",
            prompt: "",
            engine: DEFAULT_LOCAL_ENGINE,
            model_id: Some("whisper-base"),
            model_path: None,
            command_path: Some("C:/tools/whisper-cli.exe"),
        })
        .await
        .unwrap_err();
        assert!(matches!(err, LocalError::MissingModelPath));
    }

    fn fake_wav(sample_rate: u32, channels: u16, bits: u16, data_len: usize) -> Vec<u8> {
        let mut wav = vec![0u8; 44 + data_len];
        wav[22..24].copy_from_slice(&channels.to_le_bytes());
        wav[24..28].copy_from_slice(&sample_rate.to_le_bytes());
        wav[34..36].copy_from_slice(&bits.to_le_bytes());
        wav
    }

    #[test]
    fn wav_duration_secs_reads_header() {
        // 16kHz mono 16-bit, 2 секунды = 16000 * 2 * 2 = 64000 байт данных.
        let wav = fake_wav(16000, 1, 16, 64000);
        assert!((wav_duration_secs(&wav) - 2.0).abs() < 1e-6);
        // Слишком короткий буфер не паникует.
        assert_eq!(wav_duration_secs(b"short"), 0.0);
    }

    #[test]
    fn local_inference_timeout_scales_with_duration() {
        // Короткое аудио — не ниже минимума 60с.
        let short = fake_wav(16000, 1, 16, 16000); // 0.5с
        assert_eq!(local_inference_timeout(&short), Duration::from_secs(60));
        // Минута аудио → база 30 + 60*8 = 510с (> старых 60с, которые баговали).
        let minute = fake_wav(16000, 1, 16, 16000 * 2 * 60);
        assert_eq!(local_inference_timeout(&minute), Duration::from_secs(510));
    }

    #[tokio::test]
    async fn server_ready_timeout_is_configurable_and_clamped() {
        let _guard = ENV_LOCAL_LOCK.lock().await;
        env::remove_var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS");
        assert_eq!(
            local_stt_server_ready_timeout(),
            Duration::from_millis(30_000)
        );

        env::set_var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS", "25000");
        assert_eq!(
            local_stt_server_ready_timeout(),
            Duration::from_millis(25_000)
        );

        env::set_var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS", "1");
        assert_eq!(
            local_stt_server_ready_timeout(),
            Duration::from_millis(1_000)
        );

        env::set_var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS", "999999");
        assert_eq!(
            local_stt_server_ready_timeout(),
            Duration::from_millis(300_000)
        );
        env::remove_var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS");
    }

    #[test]
    fn strip_whisper_timestamps_removes_segment_prefixes() {
        assert_eq!(
            strip_whisper_timestamps("[00:00:00.000 --> 00:00:01.280]   Алло, алло, привет.\n"),
            "Алло, алло, привет."
        );
    }

    #[test]
    fn read_transcript_prefers_clean_txt_over_timestamp_stdout() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let out_base = tmp.path().join("dictation");
        fs::write(out_base.with_extension("txt"), "Алло, алло, привет.\n").expect("txt");

        let text = read_transcript(b"[00:00:00.000 --> 00:00:01.280]   noisy stdout", &out_base)
            .expect("transcript");

        assert_eq!(text, "Алло, алло, привет.");
    }

    #[test]
    fn whisper_server_executable_is_the_release_sibling() {
        let command = Path::new("C:/sample/local-stt/Release/whisper-cli.exe");
        assert_eq!(
            whisper_server_executable(command),
            PathBuf::from("C:/sample/local-stt/Release/whisper-server.exe")
        );
    }

    #[test]
    fn parse_server_text_prefers_json_text_field() {
        let text = parse_server_text(
            r#"{"text":"[00:00:00.000 --> 00:00:01.280] привет"}"#,
            Some("application/json"),
        )
        .expect("json transcript");
        assert_eq!(text, "привет");
    }

    #[test]
    fn parse_server_text_accepts_plain_text() {
        let text = parse_server_text(
            "[00:00:00.000 --> 00:00:01.280]   добрый день",
            Some("text/plain"),
        )
        .expect("plain transcript");
        assert_eq!(text, "добрый день");
    }

    #[test]
    fn server_request_form_includes_expected_fields() {
        let req = OwnedLocalRequest {
            wav_bytes: b"wav".to_vec(),
            language: "ru".into(),
            prompt: "term".into(),
            engine: DEFAULT_LOCAL_ENGINE.into(),
            model_id: None,
            model_path: Some("C:/models/ggml-base.bin".into()),
            command_path: Some("C:/tools/whisper-cli.exe".into()),
            accelerator: LocalSttAccelerator::Auto,
            profile: LocalSttProfile::Fast,
        };

        let form = server_request_form(&req).expect("form");
        let debug = format!("{form:?}");
        assert!(debug.contains("file"));
        assert!(debug.contains("response_format"));
        assert!(debug.contains("language"));
        assert!(debug.contains("prompt"));
    }

    #[tokio::test]
    async fn preload_server_returns_false_without_server_binary() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let model = tmp.path().join("model.bin");
        let command = tmp.path().join("whisper-cli.exe");
        fs::write(&model, b"model").expect("model");
        fs::write(&command, b"exe").expect("command");

        let warmed =
            preload_with_whisper_backend(DEFAULT_LOCAL_ENGINE, model.to_str(), command.to_str())
                .await
                .expect("preload");

        assert!(!warmed);
    }

    #[tokio::test]
    async fn preload_server_rejects_unsupported_engine() {
        let err = preload_with_whisper_backend("other", None, None)
            .await
            .unwrap_err();

        assert!(matches!(err, LocalError::UnsupportedEngine { .. }));
    }

    #[tokio::test]
    async fn preload_server_prefers_mocked_sidecar() {
        let _guard = ENV_LOCAL_LOCK.lock().await;
        install_test_sidecar_mock(Some(TestSidecarMock {
            transcript: None,
            fail_error: None,
            sidecar_unavailable_remaining: 0,
            seen_ops: Vec::new(),
        }));

        let warmed = preload_server(
            DEFAULT_LOCAL_ENGINE,
            Some("Z:/missing/model.bin"),
            Some("Z:/missing/whisper-cli.exe"),
        )
        .await
        .expect("preload through sidecar");

        assert!(warmed);
        assert_eq!(recorded_test_sidecar_ops(), vec!["preload"]);
        install_test_sidecar_mock(None);
        clear_test_sidecar_pool_for_host().await;
    }

    #[test]
    fn whisper_profile_and_accelerator_args_map_to_backend_flags() {
        let mut fast = Command::new("whisper-cli");
        apply_whisper_quality_args_blocking(&mut fast, &LocalSttProfile::Fast);
        let fast_args = fast
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(fast_args, vec!["-bo", "1", "-bs", "1"]);

        let mut accurate = Command::new("whisper-cli");
        apply_whisper_quality_args_blocking(&mut accurate, &LocalSttProfile::Accurate);
        let accurate_args = accurate
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(accurate_args, vec!["-bo", "5", "-bs", "5"]);

        let mut cpu = Command::new("whisper-cli");
        apply_whisper_accelerator_args_blocking(&mut cpu, &LocalSttAccelerator::Cpu);
        let cpu_args = cpu
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(cpu_args, vec!["-ng"]);

        let mut gpu = Command::new("whisper-cli");
        apply_whisper_accelerator_args_blocking(&mut gpu, &LocalSttAccelerator::Gpu);
        assert!(gpu.get_args().next().is_none());
    }

    #[tokio::test]
    async fn transcribe_prefers_mocked_sidecar_over_direct_whisper_binaries() {
        let _guard = ENV_LOCAL_LOCK.lock().await;
        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
        env::remove_var("KOSMOS_TEST_DICTATION_TRANSCRIPT");
        install_test_sidecar_mock(Some(TestSidecarMock {
            transcript: Some("sidecar transcript".into()),
            fail_error: None,
            sidecar_unavailable_remaining: 0,
            seen_ops: Vec::new(),
        }));

        let result = transcribe(LocalRequest {
            wav_bytes: b"wav",
            language: "ru",
            prompt: "",
            engine: DEFAULT_LOCAL_ENGINE,
            model_id: Some("small"),
            model_path: Some("Z:/missing/model.bin"),
            command_path: Some("Z:/missing/whisper-cli.exe"),
        })
        .await
        .expect("sidecar transcript");

        assert_eq!(result.text, "sidecar transcript");
        assert_eq!(recorded_test_sidecar_ops(), vec!["transcribe"]);
        install_test_sidecar_mock(None);
    }

    #[tokio::test]
    async fn transcribe_retries_once_after_sidecar_unavailable() {
        let _guard = ENV_LOCAL_LOCK.lock().await;
        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
        env::remove_var("KOSMOS_TEST_DICTATION_TRANSCRIPT");
        install_test_sidecar_mock(Some(TestSidecarMock {
            transcript: Some("recovered transcript".into()),
            fail_error: None,
            sidecar_unavailable_remaining: 1,
            seen_ops: Vec::new(),
        }));

        let result = transcribe(LocalRequest {
            wav_bytes: b"wav",
            language: "ru",
            prompt: "",
            engine: DEFAULT_LOCAL_ENGINE,
            model_id: Some("small"),
            model_path: Some("Z:/missing/model.bin"),
            command_path: Some("Z:/missing/whisper-cli.exe"),
        })
        .await
        .expect("recovered transcript");

        assert_eq!(result.text, "recovered transcript");
        assert_eq!(
            recorded_test_sidecar_ops(),
            vec!["transcribe", "transcribe"]
        );
        install_test_sidecar_mock(None);
    }

    #[tokio::test]
    async fn faster_whisper_missing_python_returns_controlled_error() {
        let _guard = ENV_LOCAL_LOCK.lock().await;
        env::set_var("KOSMOS_FASTER_WHISPER_PYTHON", "Z:/missing/python.exe");

        let err = transcribe_with_whisper_backend(LocalRequest {
            wav_bytes: &fake_wav(16_000, 1, 16, 16_000),
            language: "ru",
            prompt: "",
            engine: FASTER_WHISPER_ENGINE,
            model_id: Some("turbo"),
            model_path: Some("faster-whisper-large-v3-turbo"),
            command_path: None,
        })
        .await
        .unwrap_err();

        env::remove_var("KOSMOS_FASTER_WHISPER_PYTHON");
        assert!(
            err.to_string().contains("faster-whisper python"),
            "error: {err}"
        );
    }

    #[tokio::test]
    async fn faster_whisper_rejects_managed_ggml_model() {
        let _guard = ENV_LOCAL_LOCK.lock().await;
        env::remove_var("KOSMOS_FASTER_WHISPER_MODEL");
        let req = OwnedLocalRequest {
            wav_bytes: Vec::new(),
            language: "ru".into(),
            prompt: String::new(),
            engine: FASTER_WHISPER_ENGINE.into(),
            model_id: Some("turbo".into()),
            model_path: Some("C:/models/ggml-large-v3-turbo.bin".into()),
            command_path: None,
            accelerator: LocalSttAccelerator::Gpu,
            profile: LocalSttProfile::Fast,
        };

        let err = faster_whisper_model_arg(&req).expect_err("ggml is incompatible");
        assert!(err
            .to_string()
            .contains("ggml .bin models are not compatible"));
    }

    #[tokio::test]
    async fn faster_whisper_model_env_overrides_managed_ggml_model() {
        let _guard = ENV_LOCAL_LOCK.lock().await;
        env::set_var(
            "KOSMOS_FASTER_WHISPER_MODEL",
            "C:/models/faster-large-v3-turbo",
        );
        let req = OwnedLocalRequest {
            wav_bytes: Vec::new(),
            language: "ru".into(),
            prompt: String::new(),
            engine: FASTER_WHISPER_ENGINE.into(),
            model_id: Some("turbo".into()),
            model_path: Some("C:/models/ggml-large-v3-turbo.bin".into()),
            command_path: None,
            accelerator: LocalSttAccelerator::Gpu,
            profile: LocalSttProfile::Fast,
        };

        assert_eq!(
            faster_whisper_model_arg(&req).expect("model arg"),
            "C:/models/faster-large-v3-turbo"
        );
        env::remove_var("KOSMOS_FASTER_WHISPER_MODEL");
    }

    #[tokio::test]
    async fn test_override_transcript_bypasses_sidecar_requests() {
        let _guard = ENV_LOCAL_LOCK.lock().await;
        env::remove_var("KOSMOS_TEST_DICTATION_TRANSCRIPT");
        env::set_var(
            "KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT",
            "override transcript",
        );
        install_test_sidecar_mock(Some(TestSidecarMock {
            transcript: None,
            fail_error: Some("sidecar should not be called".into()),
            sidecar_unavailable_remaining: 0,
            seen_ops: Vec::new(),
        }));

        let result = transcribe(LocalRequest {
            wav_bytes: b"wav",
            language: "ru",
            prompt: "",
            engine: DEFAULT_LOCAL_ENGINE,
            model_id: Some("small"),
            model_path: Some("Z:/missing/model.bin"),
            command_path: Some("Z:/missing/whisper-cli.exe"),
        })
        .await
        .expect("override transcript");

        assert_eq!(result.text, "override transcript");
        assert!(recorded_test_sidecar_ops().is_empty());

        install_test_sidecar_mock(None);
        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
    }
}
