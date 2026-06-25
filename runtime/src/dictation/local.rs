use std::env;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::header::CONTENT_TYPE;
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde_json::Value;
use thiserror::Error;
use tokio::process::Command as TokioCommand;

pub const DEFAULT_LOCAL_ENGINE: &str = "whisper.cpp";

#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug)]
pub struct TranscriptionResult {
    pub text: String,
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
    pub vad_model_path: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct OwnedLocalRequest {
    wav_bytes: Vec<u8>,
    language: String,
    prompt: String,
    model_id: Option<String>,
    model_path: Option<String>,
    command_path: Option<String>,
    vad_model_path: Option<String>,
}

impl<'a> From<LocalRequest<'a>> for OwnedLocalRequest {
    fn from(req: LocalRequest<'a>) -> Self {
        Self {
            wav_bytes: req.wav_bytes.to_vec(),
            language: req.language.to_owned(),
            prompt: req.prompt.to_owned(),
            model_id: req.model_id.map(str::to_owned),
            model_path: req.model_path.map(str::to_owned),
            command_path: req.command_path.map(str::to_owned),
            vad_model_path: req.vad_model_path.map(str::to_owned),
        }
    }
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
}

fn is_dictation_test_mode() -> bool {
    matches!(env::var("KOSMOS_TEST_MODE").as_deref(), Ok("1"))
        || matches!(env::var("KOSMOS_HEADLESS").as_deref(), Ok("1"))
}

pub(crate) fn test_override_transcript() -> Option<String> {
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

fn is_supported_engine(engine: &str) -> bool {
    let normalized = engine.trim().to_ascii_lowercase();
    normalized.is_empty()
        || matches!(
            normalized.as_str(),
            DEFAULT_LOCAL_ENGINE | "whisper" | "whisper-cpp"
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

fn whisper_language_arg(language: &str) -> Option<String> {
    let trimmed = language.trim();
    if trimmed.is_empty() || trimmed == "auto" {
        None
    } else if trimmed == "zh-Hans" || trimmed == "zh-Hant" {
        Some("zh".into())
    } else {
        Some(trimmed.into())
    }
}

fn append_vad_args(command: &mut Command, vad_model_path: Option<&str>) {
    let Some(vad_model_path) = vad_model_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
    else {
        return;
    };

    command
        .arg("--vad")
        .arg("-vm")
        .arg(vad_model_path)
        .arg("-vt")
        .arg("0.50")
        .arg("-vspd")
        .arg("250")
        .arg("-vsd")
        .arg("300")
        .arg("-vp")
        .arg("100")
        .arg("-vo")
        .arg("0.10")
        .arg("-vmsd")
        .arg("30");
}

fn cleanup_temp_outputs(wav_path: &Path, out_base: &Path) {
    let _ = fs::remove_file(wav_path);
    let _ = fs::remove_file(out_base.with_extension("txt"));
}

fn local_whisper_threads() -> usize {
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
        .join(" ")
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

fn server_process_key(command_path: &Path, model_path: &Path) -> ServerProcessKey {
    ServerProcessKey {
        command_path: stable_path_key(command_path),
        model_path: stable_path_key(model_path),
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
                    .join(" ");
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServerProcessKey {
    command_path: PathBuf,
    model_path: PathBuf,
}

struct WhisperServerProcess {
    key: ServerProcessKey,
    port: u16,
    child: Mutex<tokio::process::Child>,
}

impl WhisperServerProcess {
    fn is_running(&self) -> bool {
        lock_child(&self.child)
            .and_then(|mut child| match child.try_wait() {
                Ok(None) => Some(true),
                Ok(Some(_)) => Some(false),
                Err(_) => Some(false),
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

    for _ in 0..120 {
        if !server.is_running() {
            return Err(LocalError::CommandFailed(
                "whisper-server exited before becoming ready".into(),
            ));
        }

        match client
            .get(&url)
            .timeout(Duration::from_secs(1))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(_) | Err(_) => {}
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
        .arg("-bo")
        .arg("1")
        .arg("-bs")
        .arg("1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = command.spawn().map_err(|e| {
        LocalError::CommandFailed(format!("не удалось запустить whisper-server: {e}"))
    })?;

    let server = Arc::new(WhisperServerProcess {
        key: server_process_key(command_path, model_path),
        port,
        child: Mutex::new(child),
    });

    wait_for_whisper_server_ready(&server).await?;
    Ok(server)
}

async fn get_or_start_whisper_server(
    model_path: &Path,
    command_path: &Path,
) -> Result<Arc<WhisperServerProcess>, LocalError> {
    let desired_key = server_process_key(command_path, model_path);
    let mut pool = whisper_server_pool().lock().await;
    if let Some(active) = pool.active.as_ref() {
        if active.key == desired_key && active.is_running() {
            return Ok(Arc::clone(active));
        }
    }

    let server = start_whisper_server(model_path, command_path).await?;
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

    let server = get_or_start_whisper_server(&model_path, &command_path).await?;
    let form = server_request_form(&req)?;
    let url = format!("http://127.0.0.1:{}/inference", server.port);
    let response = whisper_server_client()
        .post(&url)
        .multipart(form)
        .timeout(Duration::from_secs(60))
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

    Ok(TranscriptionResult { text })
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
        .arg(local_whisper_threads().to_string())
        .arg("-bo")
        .arg("1")
        .arg("-bs")
        .arg("1");

    if let Some(language) = whisper_language_arg(&req.language) {
        command.arg("-l").arg(language);
    }
    if !req.prompt.trim().is_empty() {
        command.arg("--prompt").arg(req.prompt.trim());
    }
    append_vad_args(&mut command, req.vad_model_path.as_deref());

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
    Ok(TranscriptionResult { text })
}

async fn transcribe_one(owned: OwnedLocalRequest) -> Result<TranscriptionResult, LocalError> {
    let command_path = owned.command_path.clone();
    if owned.vad_model_path.is_none() {
        if let Some(command_path) = command_path {
            if whisper_server_executable(Path::new(&command_path)).is_file() {
                return run_whisper_server(owned).await;
            }
        }
    }

    tokio::task::spawn_blocking(move || run_whisper_cpp(owned))
        .await
        .map_err(|e| LocalError::CommandFailed(format!("worker join failed: {e}")))?
}

pub async fn transcribe(req: LocalRequest<'_>) -> Result<TranscriptionResult, LocalError> {
    if let Some(text) = test_override_transcript() {
        return Ok(TranscriptionResult { text });
    }

    if !is_supported_engine(req.engine) {
        return Err(LocalError::UnsupportedEngine {
            engine: req.engine.to_owned(),
        });
    }

    let owned = OwnedLocalRequest::from(req);
    let chunks = super::groq::split_wav_for_transcription(&owned.wav_bytes);
    if chunks.len() <= 1 {
        return transcribe_one(owned).await;
    }

    let mut parts = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut chunk_req = owned.clone();
        chunk_req.wav_bytes = chunk;
        let part = transcribe_one(chunk_req).await?;
        let trimmed = part.text.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_owned());
        }
    }

    Ok(TranscriptionResult {
        text: parts.join(" ").trim().to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_override_is_available_in_unit_tests() {
        let _guard = TEST_ENV_LOCK.lock().await;
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
    async fn local_transcribe_errors_without_model_path() {
        let _guard = TEST_ENV_LOCK.lock().await;
        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
        env::remove_var("KOSMOS_TEST_DICTATION_TRANSCRIPT");
        let err = transcribe(LocalRequest {
            wav_bytes: b"wav",
            language: "ru",
            prompt: "",
            engine: DEFAULT_LOCAL_ENGINE,
            model_id: Some("whisper-base"),
            model_path: None,
            command_path: Some("C:/tools/whisper-cli.exe"),
            vad_model_path: None,
        })
        .await
        .unwrap_err();
        assert!(matches!(err, LocalError::MissingModelPath));
        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
        env::remove_var("KOSMOS_TEST_DICTATION_TRANSCRIPT");
    }

    #[test]
    fn strip_whisper_timestamps_removes_segment_prefixes() {
        assert_eq!(
            strip_whisper_timestamps("[00:00:00.000 --> 00:00:01.280]   Алло, алло, привет.\n"),
            "Алло, алло, привет."
        );
    }

    #[test]
    fn strip_whisper_timestamps_joins_segments_with_spaces() {
        assert_eq!(
            strip_whisper_timestamps(
                "[00:00:00.000 --> 00:00:01.280]   Первая фраза.\n[00:00:01.280 --> 00:00:02.560]   Вторая фраза.",
            ),
            "Первая фраза. Вторая фраза."
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
    fn parse_server_text_joins_json_segments_with_spaces() {
        let text = parse_server_text(
            r#"{"segments":[{"text":"первая часть"},{"text":"вторая часть"}]}"#,
            Some("application/json"),
        )
        .expect("json transcript");
        assert_eq!(text, "первая часть вторая часть");
    }

    #[test]
    fn append_vad_args_enables_silero_vad_for_cli() {
        let mut command = Command::new("whisper-cli");
        append_vad_args(&mut command, Some("C:/models/ggml-silero-v6.2.0.bin"));
        let debug = format!("{command:?}");

        assert!(debug.contains("--vad"));
        assert!(debug.contains("ggml-silero-v6.2.0.bin"));
        assert!(debug.contains("-vmsd"));
        assert!(debug.contains("30"));
    }

    #[test]
    fn server_request_form_includes_expected_fields() {
        let req = OwnedLocalRequest {
            wav_bytes: b"wav".to_vec(),
            language: "ru".into(),
            prompt: "term".into(),
            model_id: None,
            model_path: Some("C:/models/ggml-base.bin".into()),
            command_path: Some("C:/tools/whisper-cli.exe".into()),
            vad_model_path: Some("C:/models/ggml-silero-v6.2.0.bin".into()),
        };

        let form = server_request_form(&req).expect("form");
        let debug = format!("{form:?}");
        assert!(debug.contains("file"));
        assert!(debug.contains("response_format"));
        assert!(debug.contains("language"));
        assert!(debug.contains("prompt"));
    }
}
