use std::env;
use std::fs;
use std::net::TcpListener;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use reqwest::header::CONTENT_TYPE;
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde_json::Value;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout, Command as TokioCommand};
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams, TimestampGranularity};
use transcribe_rs::onnx::Quantization;

use crate::process_tree::ProcessTree;

use super::groq::{filter_segments, VerboseResponse};
use super::local_sidecar_protocol::{
    LocalSttAccelerator, LocalSttModelSpec, LocalSttProfile, LocalSttRequest,
    LocalSttRequestEnvelope, LocalSttResponse, LocalSttResponseEnvelope, LocalSttStatus,
};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub const DEFAULT_LOCAL_ENGINE: &str = "whisper.cpp";
pub const PARAKEET_LOCAL_ENGINE: &str = "parakeet";

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
    /// Через сколько мс простоя выгружать whisper-server (из config).
    /// `None` = никогда. Env `MUNDUS_LOCAL_STT_IDLE_UNLOAD_MS` переопределяет.
    pub idle_unload_ms: Option<u64>,
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
    /// Разрешённое значение: env override → config_value.
    idle_unload_ms: Option<u64>,
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
            idle_unload_ms: resolve_direct_idle_unload_ms(req.idle_unload_ms),
        }
    }
}

struct LocalSttSidecarClient {
    // ProcessTree, not a bare Child: the sidecar is long-lived and must not
    // outlive the Engine — the job kills it even on a hard kill (KOS-312).
    child: ProcessTree,
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
    matches!(env::var("MUNDUS_TEST_MODE").as_deref(), Ok("1"))
        || matches!(env::var("MUNDUS_HEADLESS").as_deref(), Ok("1"))
}

fn test_override_transcript() -> Option<String> {
    if !cfg!(test) && !is_dictation_test_mode() {
        return None;
    }
    for key in [
        "MUNDUS_TEST_LOCAL_DICTATION_TRANSCRIPT",
        "MUNDUS_TEST_DICTATION_TRANSCRIPT",
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
    // Юнит-тесты проверяют контракт sidecar напрямую (см.
    // `submit_audio_local_sidecar_unavailable_keeps_pending`) — там fallback
    // выключен, включается только явным `=1`.
    if cfg!(test) {
        return matches!(
            env::var("MUNDUS_LOCAL_STT_ALLOW_DIRECT_FALLBACK").as_deref(),
            Ok("1")
        );
    }
    // Пакетирование sidecar убрано (commit "drop stale local stt sidecar
    // packaging"), поэтому в проде whisper.cpp обязан исполняться напрямую
    // через user/managed command path. Без этого SidecarUnavailable
    // классифицируется как Retryable и pill бесконечно показывает «Жду сеть»
    // для ЛОКАЛЬНОГО провайдера. Явный opt-out — `=0`.
    !matches!(
        env::var("MUNDUS_LOCAL_STT_ALLOW_DIRECT_FALLBACK").as_deref(),
        Ok("0")
    )
}

fn local_stt_accelerator() -> LocalSttAccelerator {
    match env::var("MUNDUS_LOCAL_STT_ACCELERATOR")
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
    match env::var("MUNDUS_LOCAL_STT_PROFILE")
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
    if let Ok(value) = env::var("MUNDUS_LOCAL_STT_IDLE_UNLOAD_MS") {
        return value.trim().parse::<u64>().ok();
    }
    let _ = engine;
    Some(5 * 60 * 1000)
}

/// Разрешает idle-unload для DIRECT пути: env var переопределяет config.
/// `config_value` берётся из `DictationConfig.local_idle_unload_ms`.
fn resolve_direct_idle_unload_ms(config_value: Option<u64>) -> Option<u64> {
    if let Ok(value) = env::var("MUNDUS_LOCAL_STT_IDLE_UNLOAD_MS") {
        return value.trim().parse::<u64>().ok();
    }
    config_value
}

fn local_stt_server_ready_timeout() -> Duration {
    let millis = env::var("MUNDUS_LOCAL_STT_SERVER_READY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(30_000)
        .clamp(1_000, 300_000);
    Duration::from_millis(millis)
}

fn local_stt_sidecar_binary_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["mundus-local-stt.exe", "Mundus Local STT.exe"]
    } else {
        &["mundus-local-stt"]
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
    if let Ok(value) = env::var("MUNDUS_LOCAL_STT_SIDECAR_PATH") {
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

include!("local/sidecar.rs");
include!("local/backend.rs");
#[cfg(test)]
include!("local/tests.rs");
