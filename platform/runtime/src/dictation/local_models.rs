use std::fs::{self, File, OpenOptions};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header::{
    ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
};
use reqwest::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::process::Command as TokioCommand;

use super::config;

const MODELS_DIR: &str = "models/dictation";
const TOOLS_DIR: &str = "tools/dictation/whisper.cpp";
const CUDA_TOOLS_DIR: &str = "tools/dictation/whisper.cpp-cublas";
const FASTER_WHISPER_LEGACY_DIR: &str = "tools/dictation/faster-whisper";
const FASTER_WHISPER_MODEL_CACHE_DIR: &str = "models/whisper";
const FASTER_WHISPER_VENV_DIR: &str = ".venv";
const FASTER_WHISPER_MARKERS_DIR: &str = "prepared-models";
const FASTER_WHISPER_RUNTIME_DIR: &str = "runtimes/faster-whisper/win-x64";
const FASTER_WHISPER_CUDA_LIBS_DIR: &str = "runtimes/cuda-libs/cuda12-cudnn9";
const FASTER_WHISPER_RUNTIME_VERSION: &str = "2026.06.23";
const FASTER_WHISPER_RUNTIME_MANIFEST: &str = "kosmos-runtime.json";
const FASTER_WHISPER_RUNTIME_ARCHIVE: &str = "kosmos-faster-whisper-runtime-win-x64.zip";
const FASTER_WHISPER_CUDA_ARCHIVE: &str = "kosmos-cuda-libs-cuda12-cudnn9.zip";
const FASTER_WHISPER_RUNTIME_DEFAULT_URL: &str = "https://github.com/makekosmos/local-ai-runtimes/releases/download/faster-whisper-win-x64-2026.06.23/kosmos-faster-whisper-runtime-win-x64.zip";
const FASTER_WHISPER_RUNTIME_DEFAULT_SHA256: &str =
    "54a954ef9d8b56bf5b863a41b148863a695e22a8f94636b441f07179ce0813b1";
const FASTER_WHISPER_CUDA_CUBLAS_URL: &str = "https://files.pythonhosted.org/packages/20/e2/fc9a0e985249d873150276d5afb02e39a66817fedbf1a385724393e505ed/nvidia_cublas_cu12-12.9.2.10-py3-none-win_amd64.whl";
const FASTER_WHISPER_CUDA_CUBLAS_SHA256: &str =
    "623f43027d40d44ceadf0043f002bd25cf353e8f13ce90b9a87057019f560661";
const FASTER_WHISPER_CUDA_CUDNN_URL: &str = "https://files.pythonhosted.org/packages/9b/93/b37f3a0fe29b1ae3bbb42c22cc25cb152971bb400a589cade336bdf5f4f3/nvidia_cudnn_cu12-9.23.2.1-py3-none-win_amd64.whl";
const FASTER_WHISPER_CUDA_CUDNN_SHA256: &str =
    "549d6eb120cdd89429997243cd2cad1e864aac3a2f887a93f17836ce72d83873";
const DOWNLOAD_MAX_ATTEMPTS: usize = 8;

#[cfg(windows)]
const WHISPER_CPP_CPU_ZIP_URL: &str =
    "https://github.com/ggml-org/whisper.cpp/releases/download/v1.8.4/whisper-bin-x64.zip";
#[cfg(windows)]
const WHISPER_CPP_CUDA_ZIP_URL: &str =
    "https://sourceforge.net/projects/whisper-cpp.mirror/files/v1.8.5/whisper-cublas-12.4.0-bin-x64.zip/download";

#[cfg(windows)]
static NVIDIA_GPU_AVAILABLE: OnceLock<bool> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub sha256: Option<&'static str>,
    pub size_mb: u64,
    pub accuracy_score: f32,
    pub speed_score: f32,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filename: String,
    pub url: String,
    pub size_mb: u64,
    pub accuracy_score: f32,
    pub speed_score: f32,
    pub recommended: bool,
    pub downloaded: bool,
    pub selected: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelsSnapshot {
    pub models_dir: String,
    pub command_path: Option<String>,
    pub command_installed: bool,
    pub faster_whisper_runtime_installed: bool,
    pub faster_whisper_cuda_installed: bool,
    pub faster_whisper_cuda_supported: bool,
    pub models: Vec<LocalModelInfo>,
}

#[derive(Debug, Error)]
pub enum LocalModelsError {
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("model is not downloaded: {0}")]
    ModelNotDownloaded(String),
    #[error("downloading local models is not supported on this platform yet")]
    UnsupportedPlatform,
    #[error("download failed: {0}")]
    Download(String),
    #[error("faster-whisper setup failed: {0}")]
    FasterWhisper(String),
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("sha256 mismatch for {path}: expected {expected}, got {actual}")]
    ShaMismatch {
        path: String,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub phase: &'static str,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f64>,
}

pub type ProgressCallback<'a> = dyn FnMut(DownloadProgress) + Send + 'a;

pub const MODEL_CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "tiny-q5_1",
        name: "Whisper Tiny",
        description: "Самая быстрая проверочная модель. Подходит для smoke-теста, качество ниже.",
        filename: "ggml-tiny-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny-q5_1.bin",
        sha256: None,
        size_mb: 31,
        accuracy_score: 0.35,
        speed_score: 0.98,
        recommended: false,
    },
    ModelSpec {
        id: "small",
        name: "Whisper Small",
        description: "Быстрая и достаточно точная модель для повседневной диктовки.",
        filename: "ggml-small.bin",
        url: "https://blob.handy.computer/ggml-small.bin",
        sha256: Some("1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b"),
        size_mb: 465,
        accuracy_score: 0.60,
        speed_score: 0.85,
        recommended: true,
    },
    ModelSpec {
        id: "medium",
        name: "Whisper Medium",
        description: "Выше качество, заметно тяжелее для CPU.",
        filename: "whisper-medium-q4_1.bin",
        url: "https://blob.handy.computer/whisper-medium-q4_1.bin",
        sha256: Some("79283fc1f9fe12ca3248543fbd54b73292164d8df5a16e095e2bceeaaabddf57"),
        size_mb: 469,
        accuracy_score: 0.75,
        speed_score: 0.60,
        recommended: false,
    },
    ModelSpec {
        id: "turbo",
        name: "Whisper Large v3 Turbo",
        description: "Лучший баланс качества и скорости для сильной машины.",
        filename: "ggml-large-v3-turbo.bin",
        url: "https://blob.handy.computer/ggml-large-v3-turbo.bin",
        sha256: Some("1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69"),
        size_mb: 1549,
        accuracy_score: 0.80,
        speed_score: 0.40,
        recommended: false,
    },
    ModelSpec {
        id: "large",
        name: "Whisper Large v3 q5",
        description: "Самое высокое качество из локального списка, но медленнее.",
        filename: "ggml-large-v3-q5_0.bin",
        url: "https://blob.handy.computer/ggml-large-v3-q5_0.bin",
        sha256: Some("d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1"),
        size_mb: 1031,
        accuracy_score: 0.85,
        speed_score: 0.30,
        recommended: false,
    },
];

fn model_spec(model_id: &str) -> Result<&'static ModelSpec, LocalModelsError> {
    MODEL_CATALOG
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| LocalModelsError::ModelNotFound(model_id.to_owned()))
}

fn default_shared_assets_base() -> PathBuf {
    std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("XDG_CONFIG_HOME").ok().map(PathBuf::from))
        .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_shared_assets_root() -> PathBuf {
    default_shared_assets_base().join("Kosmos")
}

fn shared_assets_root(data_dir: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("KOSMOS_LOCAL_STT_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if cfg!(test) {
        return data_dir.to_path_buf();
    }
    default_shared_assets_root()
}

fn faster_whisper_runtime_version() -> String {
    std::env::var("KOSMOS_FASTER_WHISPER_RUNTIME_VERSION")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| FASTER_WHISPER_RUNTIME_VERSION.to_owned())
}

fn merge_legacy_dir_into_shared(legacy: &Path, shared: &Path) -> io::Result<bool> {
    if !legacy.is_dir() || same_path_or_text(legacy, shared) {
        return Ok(false);
    }
    if !shared.exists() {
        if let Some(parent) = shared.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(legacy, shared)?;
        return Ok(true);
    }

    let mut changed = false;
    fs::create_dir_all(shared)?;
    for entry in fs::read_dir(legacy)? {
        let entry = entry?;
        let source = entry.path();
        let destination = shared.join(entry.file_name());
        if destination.exists() {
            if source.is_dir() && destination.is_dir() {
                changed |= merge_legacy_dir_into_shared(&source, &destination)?;
                if fs::read_dir(&source)?.next().is_none() {
                    fs::remove_dir(&source)?;
                    changed = true;
                }
            } else if source.is_file() && destination.is_file() {
                if same_file_contents(&source, &destination)? {
                    fs::remove_file(&source)?;
                } else {
                    fs::rename(&source, unique_legacy_destination(&destination))?;
                }
                changed = true;
            }
            continue;
        }
        fs::rename(&source, &destination)?;
        changed = true;
    }
    if fs::read_dir(legacy)?.next().is_none() {
        fs::remove_dir(legacy)?;
        changed = true;
    }
    Ok(changed)
}

fn legacy_faster_whisper_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir).join(FASTER_WHISPER_LEGACY_DIR)
}

fn same_file_contents(left: &Path, right: &Path) -> io::Result<bool> {
    if left.metadata()?.len() != right.metadata()?.len() {
        return Ok(false);
    }

    let mut left = fs::File::open(left)?;
    let mut right = fs::File::open(right)?;
    let mut left_buf = [0; 64 * 1024];
    let mut right_buf = [0; 64 * 1024];
    loop {
        let left_read = left.read(&mut left_buf)?;
        let right_read = right.read(&mut right_buf)?;
        if left_read != right_read || left_buf[..left_read] != right_buf[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

fn unique_legacy_destination(destination: &Path) -> PathBuf {
    for i in 1.. {
        let candidate = destination.with_extension(format!(
            "{}legacy-{i}",
            destination
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!("{ext}."))
                .unwrap_or_default()
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

pub fn migrate_legacy_assets(data_dir: &Path) -> io::Result<bool> {
    let shared_root = shared_assets_root(data_dir);
    let mut changed = false;
    let mut roots = vec![data_dir.to_path_buf()];
    if std::env::var("KOSMOS_LOCAL_STT_DIR").is_err()
        && !cfg!(test)
        && same_path_or_text(&shared_root, &default_shared_assets_root())
    {
        if let Ok(entries) = fs::read_dir(default_shared_assets_base()) {
            for entry in entries {
                let path = entry?.path();
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if path.is_dir() && name.starts_with("Kosmos-dev") {
                    roots.push(path);
                }
            }
        }
    }

    for root in roots {
        if same_path_or_text(&root, &shared_root) {
            continue;
        }
        changed |= merge_legacy_dir_into_shared(&root.join(MODELS_DIR), &models_dir(data_dir))?;
        let legacy_faster_whisper = root.join(FASTER_WHISPER_LEGACY_DIR);
        changed |= merge_legacy_dir_into_shared(
            &legacy_faster_whisper.join(FASTER_WHISPER_VENV_DIR),
            &faster_whisper_venv_dir(data_dir),
        )?;
        changed |=
            merge_legacy_dir_into_shared(&legacy_faster_whisper, &faster_whisper_dir(data_dir))?;
        changed |= merge_legacy_dir_into_shared(&root.join(TOOLS_DIR), &tools_dir(data_dir))?;
        #[cfg(windows)]
        {
            changed |= merge_legacy_dir_into_shared(
                &root.join(CUDA_TOOLS_DIR),
                &cuda_tools_dir(data_dir),
            )?;
        }
    }
    Ok(changed)
}

pub fn models_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir).join(MODELS_DIR)
}

pub fn tools_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir).join(TOOLS_DIR)
}

pub fn faster_whisper_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir).join(FASTER_WHISPER_MODEL_CACHE_DIR)
}

pub fn faster_whisper_runtime_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir)
        .join(FASTER_WHISPER_RUNTIME_DIR)
        .join(faster_whisper_runtime_version())
}

pub fn faster_whisper_cuda_libs_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir)
        .join(FASTER_WHISPER_CUDA_LIBS_DIR)
        .join(faster_whisper_runtime_version())
}

pub fn faster_whisper_venv_dir(data_dir: &Path) -> PathBuf {
    faster_whisper_runtime_dir(data_dir).join(FASTER_WHISPER_VENV_DIR)
}

pub fn faster_whisper_python_path(data_dir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        faster_whisper_venv_dir(data_dir)
            .join("Scripts")
            .join("python.exe")
    }
    #[cfg(not(windows))]
    {
        faster_whisper_venv_dir(data_dir).join("bin").join("python")
    }
}

fn faster_whisper_python_candidates(data_dir: &Path) -> Vec<PathBuf> {
    let runtime_dir = faster_whisper_runtime_dir(data_dir);
    let mut candidates = Vec::new();
    candidates.push(faster_whisper_python_path(data_dir));
    #[cfg(windows)]
    {
        candidates.push(runtime_dir.join("python.exe"));
        candidates.push(runtime_dir.join("python").join("python.exe"));
    }
    #[cfg(not(windows))]
    {
        candidates.push(runtime_dir.join("bin").join("python"));
        candidates.push(runtime_dir.join("python").join("bin").join("python"));
    }
    candidates
}

pub fn faster_whisper_runtime_python_path(data_dir: &Path) -> Option<PathBuf> {
    faster_whisper_python_candidates(data_dir)
        .into_iter()
        .find(|path| path.is_file())
}

pub fn faster_whisper_cuda_runtime_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let cuda_dir = faster_whisper_cuda_libs_dir(data_dir);
    let runtime_dir = faster_whisper_runtime_dir(data_dir);
    let venv_dir = faster_whisper_venv_dir(data_dir);
    if cfg!(windows) {
        vec![
            cuda_dir.join("cublas").join("bin"),
            cuda_dir.join("cudnn").join("bin"),
            cuda_dir.join("bin"),
            runtime_dir
                .join("Lib")
                .join("site-packages")
                .join("ctranslate2"),
            venv_dir
                .join("Lib")
                .join("site-packages")
                .join("ctranslate2"),
        ]
    } else {
        vec![
            cuda_dir.join("cublas").join("lib"),
            cuda_dir.join("cudnn").join("lib"),
            cuda_dir.join("lib"),
        ]
    }
}

pub fn faster_whisper_python_command(data_dir: &Path) -> String {
    std::env::var("KOSMOS_FASTER_WHISPER_PYTHON")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            faster_whisper_runtime_python_path(data_dir)
                .map(|path| path_string(&path))
                .unwrap_or_else(|| path_string(&faster_whisper_python_path(data_dir)))
        })
}

pub fn faster_whisper_model_id(model_id: &str) -> Result<&'static str, LocalModelsError> {
    match model_id.trim().to_ascii_lowercase().as_str() {
        "tiny" | "tiny-q5_1" | "whisper-tiny" => Ok("tiny"),
        "base" | "whisper-base" => Ok("base"),
        "small" | "whisper-small" => Ok("small"),
        "medium" | "whisper-medium" => Ok("medium"),
        "turbo" | "large-v3-turbo" | "whisper-large-v3-turbo" => Ok("large-v3-turbo"),
        "large" | "large-v3" | "whisper-large-v3" => Ok("large-v3"),
        _ => Err(LocalModelsError::ModelNotFound(model_id.to_owned())),
    }
}

#[cfg(windows)]
fn cuda_tools_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir).join(CUDA_TOOLS_DIR)
}

pub fn model_path(data_dir: &Path, spec: &ModelSpec) -> PathBuf {
    models_dir(data_dir).join(spec.filename)
}

pub fn model_path_by_id(data_dir: &Path, model_id: &str) -> Result<PathBuf, LocalModelsError> {
    Ok(model_path(data_dir, model_spec(model_id)?))
}

pub fn command_path(data_dir: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let preferred = preferred_command_path(data_dir);
        if preferred.is_file() {
            return Some(preferred);
        }
        let cpu = cpu_command_path(data_dir);
        if cpu.is_file() {
            return Some(cpu);
        }
        let cuda = cuda_command_path(data_dir);
        if cuda.is_file() {
            return Some(cuda);
        }
        Some(preferred)
    }
    #[cfg(not(windows))]
    {
        let _ = data_dir;
        None
    }
}

#[cfg(windows)]
fn cpu_command_path(data_dir: &Path) -> PathBuf {
    tools_dir(data_dir).join("Release").join("whisper-cli.exe")
}

#[cfg(windows)]
fn cuda_command_path(data_dir: &Path) -> PathBuf {
    cuda_tools_dir(data_dir)
        .join("Release")
        .join("whisper-cli.exe")
}

#[cfg(windows)]
fn preferred_command_path(data_dir: &Path) -> PathBuf {
    if cuda_tools_enabled() {
        cuda_command_path(data_dir)
    } else {
        cpu_command_path(data_dir)
    }
}

#[cfg(windows)]
fn managed_command_paths(data_dir: &Path) -> [PathBuf; 2] {
    [cpu_command_path(data_dir), cuda_command_path(data_dir)]
}

#[cfg(windows)]
fn nvidia_gpu_available() -> bool {
    *NVIDIA_GPU_AVAILABLE.get_or_init(|| {
        if std::env::var("KOSMOS_DICTATION_DISABLE_CUDA").as_deref() == Ok("1") {
            return false;
        }
        Command::new("nvidia-smi")
            .arg("-L")
            .output()
            .map(|output| output.status.success() && !output.stdout.is_empty())
            .unwrap_or(false)
    })
}

#[cfg(windows)]
fn cuda_tools_enabled() -> bool {
    std::env::var("KOSMOS_DICTATION_ENABLE_CUDA").as_deref() == Ok("1") && nvidia_gpu_available()
}

pub fn faster_whisper_cuda_runtime_available(data_dir: &Path) -> bool {
    let cublas = if cfg!(windows) {
        "cublas64_12.dll"
    } else {
        "libcublas.so.12"
    };
    let cudnn = if cfg!(windows) {
        "cudnn64_9.dll"
    } else {
        "libcudnn.so.9"
    };
    let dirs = faster_whisper_cuda_runtime_dirs(data_dir);
    dirs.iter().any(|dir| dir.join(cublas).is_file())
        && dirs.iter().any(|dir| dir.join(cudnn).is_file())
}

pub fn faster_whisper_cuda_supported() -> bool {
    #[cfg(windows)]
    {
        nvidia_gpu_available()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
pub fn refresh_managed_command_path(data_dir: &Path, cfg: &mut config::DictationConfig) -> bool {
    let Some(command_path) = command_path(data_dir) else {
        return false;
    };
    if !command_path.is_file() {
        return false;
    }

    let current = cfg.local_command_path.as_deref().map(PathBuf::from);
    let current_is_managed = current.as_ref().is_none_or(|path| {
        managed_command_paths(data_dir)
            .iter()
            .any(|managed| same_path_or_text(managed, path))
    });
    if !current_is_managed {
        return false;
    }

    if current
        .as_ref()
        .is_some_and(|path| same_path_or_text(path, &command_path))
    {
        return false;
    }

    cfg.local_command_path = Some(path_string(&command_path));
    true
}

#[cfg(not(windows))]
pub fn refresh_managed_command_path(_data_dir: &Path, _cfg: &mut config::DictationConfig) -> bool {
    false
}

fn same_path_or_text(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    let left_canonical = fs::canonicalize(left).ok();
    let right_canonical = fs::canonicalize(right).ok();
    match (left_canonical, right_canonical) {
        (Some(left), Some(right)) => left == right,
        _ => left
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy()),
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub fn faster_whisper_model_is_prepared(data_dir: &Path, model_id: &str) -> bool {
    faster_whisper_model_marker_path(data_dir, model_id).is_file()
        && faster_whisper_runtime_ready(data_dir)
}

fn faster_whisper_markers_dir(data_dir: &Path) -> PathBuf {
    faster_whisper_dir(data_dir).join(FASTER_WHISPER_MARKERS_DIR)
}

fn has_prepared_faster_whisper_models(data_dir: &Path) -> bool {
    if !faster_whisper_runtime_ready(data_dir) {
        return false;
    }
    fs::read_dir(faster_whisper_markers_dir(data_dir))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| entry.path().is_file())
}

pub fn has_downloaded_model_assets(data_dir: &Path) -> bool {
    MODEL_CATALOG
        .iter()
        .any(|spec| model_path(data_dir, spec).is_file())
        || has_prepared_faster_whisper_models(data_dir)
}

pub fn cleanup_unused_backends(data_dir: &Path) -> io::Result<bool> {
    if has_downloaded_model_assets(data_dir) {
        return Ok(false);
    }

    let mut changed = false;
    for path in [
        tools_dir(data_dir),
        legacy_faster_whisper_dir(data_dir),
        faster_whisper_dir(data_dir),
        faster_whisper_runtime_dir(data_dir),
        faster_whisper_cuda_libs_dir(data_dir),
    ] {
        if path.exists() {
            fs::remove_dir_all(path)?;
            changed = true;
        }
    }
    #[cfg(windows)]
    {
        let path = cuda_tools_dir(data_dir);
        if path.exists() {
            fs::remove_dir_all(path)?;
            changed = true;
        }
    }
    Ok(changed)
}

pub fn snapshot(data_dir: &Path, cfg: &config::DictationConfig) -> LocalModelsSnapshot {
    let models_dir = models_dir(data_dir);
    let command_path = command_path(data_dir);
    let command_installed = command_path.as_ref().is_some_and(|path| path.is_file());
    let selected_path = cfg.local_model_path.as_deref();
    let faster_whisper_selected = cfg.local_engine.eq_ignore_ascii_case("faster-whisper")
        || cfg.local_engine.eq_ignore_ascii_case("faster_whisper");
    let models = MODEL_CATALOG
        .iter()
        .map(|spec| {
            let path = model_path(data_dir, spec);
            let path_text = path_string(&path);
            let selected = cfg.local_model.as_deref() == Some(spec.id)
                || selected_path == Some(path_text.as_str())
                || (faster_whisper_selected
                    && cfg.local_model.as_deref() == Some(spec.id)
                    && faster_whisper_model_id(spec.id)
                        .ok()
                        .is_some_and(|model| selected_path == Some(model)));
            let faster_whisper_downloaded = faster_whisper_model_is_prepared(data_dir, spec.id);
            let downloaded = if faster_whisper_selected {
                faster_whisper_downloaded
            } else {
                path.is_file()
            };
            LocalModelInfo {
                id: spec.id.to_owned(),
                name: spec.name.to_owned(),
                description: spec.description.to_owned(),
                filename: spec.filename.to_owned(),
                url: spec.url.to_owned(),
                size_mb: spec.size_mb,
                accuracy_score: spec.accuracy_score,
                speed_score: spec.speed_score,
                recommended: spec.recommended,
                downloaded,
                selected,
                path: path.is_file().then_some(path_text),
            }
        })
        .collect();

    LocalModelsSnapshot {
        models_dir: path_string(&models_dir),
        command_path: command_path.map(|path| path_string(&path)),
        command_installed,
        faster_whisper_runtime_installed: faster_whisper_runtime_ready(data_dir),
        faster_whisper_cuda_installed: faster_whisper_cuda_runtime_available(data_dir),
        faster_whisper_cuda_supported: faster_whisper_cuda_supported(),
        models,
    }
}

async fn download_file(
    client: &Client,
    url: &str,
    destination: &Path,
    phase: &'static str,
    progress: &mut ProgressCallback<'_>,
) -> Result<(), LocalModelsError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let part_path = destination.with_extension("part");
    let request_url = url.to_string();
    let mut last_error = None;

    for attempt in 1..=DOWNLOAD_MAX_ATTEMPTS {
        let mut downloaded_bytes = part_path.metadata().map(|m| m.len()).unwrap_or(0);
        let mut request = client.get(url).header(ACCEPT_ENCODING, "identity");
        if downloaded_bytes > 0 {
            request = request.header(RANGE, format!("bytes={downloaded_bytes}-"));
        }

        let mut response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                last_error = Some(format!("{request_url}: {e}"));
                sleep_download_retry(attempt).await;
                continue;
            }
        };
        let status = response.status();
        let final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let content_encoding = response
            .headers()
            .get(CONTENT_ENCODING)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let content_range = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        tracing::info!(
            phase,
            attempt,
            status = %status,
            resumed_from = downloaded_bytes,
            url = %request_url,
            final_url = %final_url,
            content_type = %content_type,
            content_encoding = %content_encoding,
            content_range = %content_range,
            "dictation local model download response"
        );
        if downloaded_bytes > 0 && status == reqwest::StatusCode::OK {
            tracing::warn!(
                phase,
                attempt,
                resumed_from = downloaded_bytes,
                url = %request_url,
                "dictation local model download server ignored range; restarting"
            );
            let _ = fs::remove_file(&part_path);
            downloaded_bytes = 0;
        } else if downloaded_bytes > 0 && status != reqwest::StatusCode::PARTIAL_CONTENT {
            last_error = Some(format!(
                "{request_url}: resume failed with HTTP {status} ({content_type})"
            ));
            sleep_download_retry(attempt).await;
            continue;
        }
        if !status.is_success() {
            return Err(LocalModelsError::Download(format!(
                "{request_url}: HTTP {status} ({content_type})"
            )));
        }
        let lower_content_type = content_type.to_ascii_lowercase();
        if lower_content_type.contains("text/html")
            || lower_content_type.contains("application/json")
            || lower_content_type.starts_with("text/")
        {
            return Err(LocalModelsError::Download(format!(
                "{request_url}: expected binary download, got {content_type}"
            )));
        }
        let total_bytes = parse_content_range_total(&content_range)
            .or_else(|| {
                response
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(|remaining| downloaded_bytes.saturating_add(remaining))
            })
            .or_else(|| {
                response
                    .content_length()
                    .map(|remaining| downloaded_bytes + remaining)
            });
        let mut file = OpenOptions::new()
            .create(true)
            .append(downloaded_bytes > 0)
            .write(true)
            .truncate(downloaded_bytes == 0)
            .open(&part_path)?;
        progress(DownloadProgress {
            phase,
            downloaded_bytes,
            total_bytes,
            percent: progress_percent(downloaded_bytes, total_bytes),
        });
        let mut advanced = false;
        loop {
            let chunk = match response.chunk().await {
                Ok(chunk) => chunk,
                Err(e) => {
                    let message = format!(
                        "{request_url}: stream read failed after {downloaded_bytes} bytes ({content_type}, encoding {content_encoding}): {e}"
                    );
                    tracing::warn!(phase, attempt, error = %message, "dictation local model download chunk failed");
                    last_error = Some(message);
                    break;
                }
            };
            let Some(chunk) = chunk else {
                file.flush()?;
                fs::rename(&part_path, destination)?;
                return Ok(());
            };
            file.write_all(&chunk)?;
            advanced = true;
            downloaded_bytes += chunk.len() as u64;
            progress(DownloadProgress {
                phase,
                downloaded_bytes,
                total_bytes,
                percent: progress_percent(downloaded_bytes, total_bytes),
            });
        }
        file.flush()?;
        if !advanced && attempt == DOWNLOAD_MAX_ATTEMPTS {
            break;
        }
        sleep_download_retry(attempt).await;
    }

    Err(LocalModelsError::Download(last_error.unwrap_or_else(
        || format!("{request_url}: download failed"),
    )))
}

fn parse_content_range_total(value: &str) -> Option<u64> {
    value
        .rsplit_once('/')
        .and_then(|(_, total)| total.parse::<u64>().ok())
}

fn progress_percent(downloaded_bytes: u64, total_bytes: Option<u64>) -> Option<f64> {
    total_bytes
        .filter(|total| *total > 0)
        .map(|total| (downloaded_bytes as f64 / total as f64 * 100.0).min(100.0))
}

async fn sleep_download_retry(attempt: usize) {
    if attempt >= DOWNLOAD_MAX_ATTEMPTS {
        return;
    }
    let delay_ms = (250_u64 * 2_u64.saturating_pow(attempt.saturating_sub(1) as u32)).min(5_000);
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
}

fn verify_sha256(path: &Path, expected: &str) -> Result<(), LocalModelsError> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = format!("{:x}", hasher.finalize());
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(LocalModelsError::ShaMismatch {
            path: path_string(path),
            expected: expected.to_owned(),
            actual,
        })
    }
}

struct InstallLock {
    path: PathBuf,
    _file: File,
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

async fn acquire_install_lock(path: PathBuf) -> Result<InstallLock, LocalModelsError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    for _ in 0..240 {
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok(InstallLock { path, _file: file }),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(e) => return Err(LocalModelsError::Io(e)),
        }
    }
    Err(LocalModelsError::FasterWhisper(format!(
        "runtime install lock timed out: {}",
        path_string(&path)
    )))
}

fn faster_whisper_runtime_manifest_path(data_dir: &Path) -> PathBuf {
    faster_whisper_runtime_dir(data_dir).join(FASTER_WHISPER_RUNTIME_MANIFEST)
}

fn faster_whisper_cuda_manifest_path(data_dir: &Path) -> PathBuf {
    faster_whisper_cuda_libs_dir(data_dir).join(FASTER_WHISPER_RUNTIME_MANIFEST)
}

fn write_runtime_manifest(
    path: &Path,
    kind: &str,
    sha256: Option<&str>,
) -> Result<(), LocalModelsError> {
    let payload = serde_json::json!({
        "kind": kind,
        "version": faster_whisper_runtime_version(),
        "platform": "win-x64",
        "sha256": sha256,
        "installedAt": chrono::Utc::now().to_rfc3339(),
    });
    let bytes = serde_json::to_vec_pretty(&payload).map_err(io::Error::other)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn faster_whisper_runtime_ready(data_dir: &Path) -> bool {
    faster_whisper_runtime_manifest_path(data_dir).is_file()
        && faster_whisper_runtime_python_path(data_dir).is_some()
}

fn faster_whisper_dev_python_fallback_allowed() -> bool {
    if std::env::var("KOSMOS_FASTER_WHISPER_DISABLE_DEV_PYTHON").as_deref() == Ok("1") {
        return false;
    }
    if std::env::var("KOSMOS_FASTER_WHISPER_ALLOW_SYSTEM_PYTHON").as_deref() == Ok("1") {
        return true;
    }
    cfg!(debug_assertions)
        || std::env::var("KOSMOS_TEST_MODE").as_deref() == Ok("1")
        || std::env::var("KEPLER_INSTANCE")
            .map(|value| value.to_ascii_lowercase().contains("dev"))
            .unwrap_or(false)
}

fn trusted_runtime_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn default_runtime_archive_source(url_env: &str) -> Option<RuntimeArchiveSource> {
    if url_env == "KOSMOS_FASTER_WHISPER_RUNTIME_URL" {
        return Some(RuntimeArchiveSource::Remote(
            FASTER_WHISPER_RUNTIME_DEFAULT_URL.to_owned(),
            FASTER_WHISPER_RUNTIME_DEFAULT_SHA256.to_owned(),
        ));
    }
    None
}

fn bundled_runtime_archive(archive_name: &str) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))?;
    [
        exe_dir.join(archive_name),
        exe_dir.join("runtimes").join(archive_name),
        exe_dir.join("local-ai-runtimes").join(archive_name),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

enum RuntimeArchiveSource {
    Bundled(PathBuf),
    ExplicitFile(PathBuf, String),
    Remote(String, String),
}

fn resolve_runtime_archive_source(
    archive_name: &str,
    url_env: &str,
    sha_env: &str,
) -> Result<RuntimeArchiveSource, LocalModelsError> {
    if let Some(path) = bundled_runtime_archive(archive_name) {
        return Ok(RuntimeArchiveSource::Bundled(path));
    }

    if let Some(path) = trusted_runtime_env(&format!("{url_env}_FILE")).map(PathBuf::from) {
        let sha256 = trusted_runtime_env(sha_env).ok_or_else(|| {
            LocalModelsError::FasterWhisper(format!(
                "{sha_env} is required for explicit runtime archive install"
            ))
        })?;
        return Ok(RuntimeArchiveSource::ExplicitFile(path, sha256));
    }

    if let Some(source) = default_runtime_archive_source(url_env) {
        return Ok(source);
    }

    let url = trusted_runtime_env(url_env)
        .ok_or_else(|| LocalModelsError::FasterWhisper(format!("{url_env} is not configured")))?;
    let sha256 = trusted_runtime_env(sha_env).ok_or_else(|| {
        LocalModelsError::FasterWhisper(format!(
            "{sha_env} is required for trusted runtime install"
        ))
    })?;
    Ok(RuntimeArchiveSource::Remote(url, sha256))
}

fn extract_zip_atomic(
    archive_path: &Path,
    install_dir: &Path,
    manifest_kind: &str,
    sha256: Option<&str>,
) -> Result<(), LocalModelsError> {
    let parent = install_dir.parent().ok_or_else(|| {
        LocalModelsError::FasterWhisper(format!(
            "runtime install path has no parent: {}",
            path_string(install_dir)
        ))
    })?;
    fs::create_dir_all(parent)?;
    let stamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let temp_dir = parent.join(format!(".install-{stamp}"));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;
    let bytes = fs::read(archive_path)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    archive.extract(&temp_dir)?;
    if install_dir.exists() {
        fs::remove_dir_all(install_dir)?;
    }
    fs::rename(&temp_dir, install_dir)?;
    write_runtime_manifest(
        &install_dir.join(FASTER_WHISPER_RUNTIME_MANIFEST),
        manifest_kind,
        sha256,
    )?;
    Ok(())
}

async fn install_runtime_zip(
    client: &Client,
    install_dir: &Path,
    archive_name: &str,
    url_env: &str,
    sha_env: &str,
    phase: &'static str,
    manifest_kind: &str,
    progress: &mut ProgressCallback<'_>,
) -> Result<(), LocalModelsError> {
    let archive_path = install_dir
        .parent()
        .unwrap_or(install_dir)
        .join("downloads")
        .join(archive_name);

    let source = resolve_runtime_archive_source(archive_name, url_env, sha_env)?;
    let expected_sha256 = match source {
        RuntimeArchiveSource::Bundled(local_archive) => {
            fs::create_dir_all(archive_path.parent().unwrap_or(install_dir))?;
            fs::copy(local_archive, &archive_path)?;
            None
        }
        RuntimeArchiveSource::ExplicitFile(local_archive, sha256) => {
            fs::create_dir_all(archive_path.parent().unwrap_or(install_dir))?;
            fs::copy(local_archive, &archive_path)?;
            verify_sha256(&archive_path, &sha256)?;
            Some(sha256)
        }
        RuntimeArchiveSource::Remote(url, sha256) => {
            download_file(client, &url, &archive_path, phase, progress).await?;
            verify_sha256(&archive_path, &sha256)?;
            Some(sha256)
        }
    };
    progress(DownloadProgress {
        phase: "extract",
        downloaded_bytes: 0,
        total_bytes: None,
        percent: None,
    });
    extract_zip_atomic(
        &archive_path,
        install_dir,
        manifest_kind,
        expected_sha256.as_deref(),
    )?;
    let _ = fs::remove_file(&archive_path);
    Ok(())
}

fn cuda_zip_source_configured(data_dir: &Path) -> bool {
    bundled_runtime_archive(FASTER_WHISPER_CUDA_ARCHIVE).is_some()
        || trusted_runtime_env("KOSMOS_FASTER_WHISPER_CUDA_URL").is_some()
        || trusted_runtime_env("KOSMOS_FASTER_WHISPER_CUDA_URL_FILE").is_some()
        || faster_whisper_cuda_libs_dir(data_dir)
            .join(FASTER_WHISPER_CUDA_ARCHIVE)
            .is_file()
}

struct CudaWheelSpec {
    url: &'static str,
    sha256: &'static str,
    phase: &'static str,
    source_prefix: &'static str,
    target_prefix: &'static str,
}

fn filename_from_url(url: &str) -> Result<&str, LocalModelsError> {
    url.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            LocalModelsError::FasterWhisper(format!("runtime URL has no filename: {url}"))
        })
}

fn extract_zip_prefix(
    archive_path: &Path,
    source_prefix: &str,
    target_dir: &Path,
) -> Result<(), LocalModelsError> {
    let bytes = fs::read(archive_path)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let normalized_prefix = source_prefix.trim_end_matches('/');
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let name = file.name().replace('\\', "/");
        let Some(rest) = name
            .strip_prefix(normalized_prefix)
            .and_then(|value| value.strip_prefix('/'))
        else {
            continue;
        };
        if rest.is_empty() || rest.contains("..") || rest.starts_with('/') {
            continue;
        }
        let out_path = target_dir.join(rest);
        if file.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(out_path)?;
        io::copy(&mut file, &mut out)?;
    }
    Ok(())
}

async fn install_cuda_from_official_wheels(
    client: &Client,
    install_dir: &Path,
    progress: &mut ProgressCallback<'_>,
) -> Result<(), LocalModelsError> {
    let specs = [
        CudaWheelSpec {
            url: FASTER_WHISPER_CUDA_CUBLAS_URL,
            sha256: FASTER_WHISPER_CUDA_CUBLAS_SHA256,
            phase: "cuda-cublas",
            source_prefix: "nvidia/cublas/bin",
            target_prefix: "cublas/bin",
        },
        CudaWheelSpec {
            url: FASTER_WHISPER_CUDA_CUDNN_URL,
            sha256: FASTER_WHISPER_CUDA_CUDNN_SHA256,
            phase: "cuda-cudnn",
            source_prefix: "nvidia/cudnn/bin",
            target_prefix: "cudnn/bin",
        },
    ];
    let parent = install_dir.parent().ok_or_else(|| {
        LocalModelsError::FasterWhisper(format!(
            "runtime install path has no parent: {}",
            path_string(install_dir)
        ))
    })?;
    let stamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let temp_dir = parent.join(format!(".install-cuda-{stamp}"));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;
    let download_dir = parent.join("downloads");
    fs::create_dir_all(&download_dir)?;

    for spec in specs {
        let filename = filename_from_url(spec.url)?;
        let wheel_path = download_dir.join(filename);
        download_file(client, spec.url, &wheel_path, spec.phase, progress).await?;
        verify_sha256(&wheel_path, spec.sha256)?;
        progress(DownloadProgress {
            phase: "extract",
            downloaded_bytes: 0,
            total_bytes: None,
            percent: None,
        });
        extract_zip_prefix(
            &wheel_path,
            spec.source_prefix,
            &temp_dir.join(spec.target_prefix),
        )?;
        let _ = fs::remove_file(&wheel_path);
    }

    if install_dir.exists() {
        fs::remove_dir_all(install_dir)?;
    }
    fs::rename(&temp_dir, install_dir)?;
    write_runtime_manifest(
        &install_dir.join(FASTER_WHISPER_RUNTIME_MANIFEST),
        "cuda-libs-pypi",
        Some(&format!(
            "cublas={};cudnn={}",
            FASTER_WHISPER_CUDA_CUBLAS_SHA256, FASTER_WHISPER_CUDA_CUDNN_SHA256
        )),
    )?;
    Ok(())
}

pub async fn ensure_model(
    client: &Client,
    data_dir: &Path,
    model_id: &str,
) -> Result<PathBuf, LocalModelsError> {
    ensure_model_with_progress(client, data_dir, model_id, &mut |_| {}).await
}

pub async fn ensure_model_with_progress(
    client: &Client,
    data_dir: &Path,
    model_id: &str,
    progress: &mut ProgressCallback<'_>,
) -> Result<PathBuf, LocalModelsError> {
    let spec = model_spec(model_id)?;
    let path = model_path(data_dir, spec);
    if !path.is_file() {
        download_file(client, spec.url, &path, "model", progress).await?;
    }
    if let Some(expected) = spec.sha256 {
        verify_sha256(&path, expected)?;
    }
    Ok(path)
}

fn faster_whisper_model_marker_path(data_dir: &Path, model_id: &str) -> PathBuf {
    faster_whisper_dir(data_dir)
        .join(FASTER_WHISPER_MARKERS_DIR)
        .join(format!("{model_id}.json"))
}

fn write_faster_whisper_model_marker(
    data_dir: &Path,
    model_id: &str,
    backend_model: &str,
) -> Result<(), LocalModelsError> {
    let path = faster_whisper_model_marker_path(data_dir, model_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::json!({
        "modelId": model_id,
        "backendModel": backend_model,
        "preparedAt": chrono::Utc::now().to_rfc3339(),
    });
    let bytes = serde_json::to_vec_pretty(&payload).map_err(io::Error::other)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn faster_whisper_prepare_timeout() -> Duration {
    std::env::var("KOSMOS_FASTER_WHISPER_PREPARE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|millis| millis.clamp(30_000, 30 * 60 * 1000))
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(15 * 60))
}

async fn run_faster_whisper_setup_command(
    mut command: TokioCommand,
    timeout: Duration,
    label: &str,
) -> Result<(), LocalModelsError> {
    let output = match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return Err(LocalModelsError::FasterWhisper(format!(
                "{label}: failed to launch: {e}"
            )));
        }
        Err(_) => {
            return Err(LocalModelsError::FasterWhisper(format!(
                "{label}: timed out after {}s",
                timeout.as_secs()
            )));
        }
    };
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(LocalModelsError::FasterWhisper(format!(
        "{label}: exited with status {}; stdout={}; stderr={}",
        output.status,
        stdout.trim(),
        stderr.trim()
    )))
}

#[cfg(windows)]
fn detected_python_paths_for_faster_whisper() -> Vec<PathBuf> {
    let output = match Command::new("py").arg("-0p").output() {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|line| {
            line.contains("3.10")
                || line.contains("3.11")
                || line.contains("3.12")
                || line.contains("3.13")
        })
        .filter_map(|line| {
            line.find("C:\\")
                .map(|idx| PathBuf::from(line[idx..].trim()))
        })
        .filter(|path| path.is_file())
        .collect()
}

async fn create_faster_whisper_venv(
    data_dir: &Path,
    timeout: Duration,
) -> Result<(), LocalModelsError> {
    let venv_dir = faster_whisper_venv_dir(data_dir);
    fs::create_dir_all(faster_whisper_runtime_dir(data_dir))?;
    let mut attempts: Vec<(String, TokioCommand)> = Vec::new();

    #[cfg(windows)]
    {
        for path in detected_python_paths_for_faster_whisper() {
            let label = format!("create venv with {}", path_string(&path));
            let mut command = TokioCommand::new(path);
            command.arg("-m").arg("venv").arg(&venv_dir);
            attempts.push((label, command));
        }

        let mut py311 = TokioCommand::new("py");
        py311.arg("-3.11").arg("-m").arg("venv").arg(&venv_dir);
        attempts.push(("create venv with py -3.11".into(), py311));

        let mut py312 = TokioCommand::new("py");
        py312.arg("-3.12").arg("-m").arg("venv").arg(&venv_dir);
        attempts.push(("create venv with py -3.12".into(), py312));
    }

    let mut python = TokioCommand::new("python");
    python.arg("-m").arg("venv").arg(&venv_dir);
    attempts.push(("create venv with python".into(), python));

    let mut errors = Vec::new();
    for (label, command) in attempts {
        match run_faster_whisper_setup_command(command, timeout, &label).await {
            Ok(()) => return Ok(()),
            Err(e) => errors.push(e.to_string()),
        }
    }

    Err(LocalModelsError::FasterWhisper(format!(
        "failed to create managed Python venv: {}",
        errors.join(" | ")
    )))
}

async fn ensure_faster_whisper_managed_runtime(
    client: &Client,
    data_dir: &Path,
    progress: &mut ProgressCallback<'_>,
) -> Result<(), LocalModelsError> {
    if faster_whisper_runtime_ready(data_dir) {
        return Ok(());
    }
    let install_dir = faster_whisper_runtime_dir(data_dir);
    let _lock = acquire_install_lock(install_dir.join(".install.lock")).await?;
    if faster_whisper_runtime_ready(data_dir) {
        return Ok(());
    }
    install_runtime_zip(
        client,
        &install_dir,
        FASTER_WHISPER_RUNTIME_ARCHIVE,
        "KOSMOS_FASTER_WHISPER_RUNTIME_URL",
        "KOSMOS_FASTER_WHISPER_RUNTIME_SHA256",
        "runtime",
        "faster-whisper-runtime",
        progress,
    )
    .await
}

async fn ensure_faster_whisper_dev_venv(
    data_dir: &Path,
    progress: &mut ProgressCallback<'_>,
) -> Result<(), LocalModelsError> {
    let python = faster_whisper_python_path(data_dir);
    if !python.is_file() {
        progress(DownloadProgress {
            phase: "python",
            downloaded_bytes: 0,
            total_bytes: None,
            percent: None,
        });
        create_faster_whisper_venv(data_dir, faster_whisper_prepare_timeout()).await?;
    }
    if !python.is_file() {
        return Err(LocalModelsError::FasterWhisper(format!(
            "managed Python venv did not create {}",
            path_string(&python)
        )));
    }

    progress(DownloadProgress {
        phase: "package",
        downloaded_bytes: 0,
        total_bytes: None,
        percent: None,
    });
    let mut pip = TokioCommand::new(&python);
    pip.arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--upgrade")
        .arg("faster-whisper");
    run_faster_whisper_setup_command(
        pip,
        faster_whisper_prepare_timeout(),
        "install faster-whisper",
    )
    .await?;

    write_runtime_manifest(
        &faster_whisper_runtime_manifest_path(data_dir),
        "faster-whisper-dev-venv",
        None,
    )?;
    Ok(())
}

async fn ensure_faster_whisper_python(
    client: &Client,
    data_dir: &Path,
    progress: &mut ProgressCallback<'_>,
) -> Result<String, LocalModelsError> {
    if let Ok(python) = std::env::var("KOSMOS_FASTER_WHISPER_PYTHON") {
        let python = python.trim();
        if !python.is_empty() {
            return Ok(python.to_owned());
        }
    }

    if !faster_whisper_runtime_ready(data_dir) {
        match ensure_faster_whisper_managed_runtime(client, data_dir, progress).await {
            Ok(()) => {}
            Err(err) if faster_whisper_dev_python_fallback_allowed() => {
                tracing::warn!(
                    error = %err,
                    "dictation: managed faster-whisper runtime unavailable, using dev Python fallback"
                );
                ensure_faster_whisper_dev_venv(data_dir, progress).await?;
            }
            Err(err) => return Err(err),
        }
    }

    faster_whisper_runtime_python_path(data_dir)
        .map(|path| path_string(&path))
        .ok_or_else(|| {
            LocalModelsError::FasterWhisper(format!(
                "managed faster-whisper runtime is not executable: {}",
                path_string(&faster_whisper_runtime_dir(data_dir))
            ))
        })
}

pub async fn ensure_faster_whisper_cuda_with_progress(
    client: &Client,
    data_dir: &Path,
    progress: &mut ProgressCallback<'_>,
) -> Result<(), LocalModelsError> {
    if faster_whisper_cuda_runtime_available(data_dir)
        && faster_whisper_cuda_manifest_path(data_dir).is_file()
    {
        return Ok(());
    }
    if !faster_whisper_cuda_supported() {
        return Err(LocalModelsError::FasterWhisper(
            "NVIDIA GPU is not available on this machine".into(),
        ));
    }
    let install_dir = faster_whisper_cuda_libs_dir(data_dir);
    let _lock = acquire_install_lock(install_dir.join(".install.lock")).await?;
    if faster_whisper_cuda_runtime_available(data_dir)
        && faster_whisper_cuda_manifest_path(data_dir).is_file()
    {
        return Ok(());
    }
    if cuda_zip_source_configured(data_dir) {
        install_runtime_zip(
            client,
            &install_dir,
            FASTER_WHISPER_CUDA_ARCHIVE,
            "KOSMOS_FASTER_WHISPER_CUDA_URL",
            "KOSMOS_FASTER_WHISPER_CUDA_SHA256",
            "cuda",
            "cuda-libs",
            progress,
        )
        .await?;
    } else {
        install_cuda_from_official_wheels(client, &install_dir, progress).await?;
    }
    if faster_whisper_cuda_runtime_available(data_dir) {
        Ok(())
    } else {
        Err(LocalModelsError::FasterWhisper(
            "NVIDIA runtime archive did not contain cuBLAS/cuDNN DLLs".into(),
        ))
    }
}

pub async fn ensure_faster_whisper_with_progress(
    client: &Client,
    data_dir: &Path,
    model_id: &str,
    progress: &mut ProgressCallback<'_>,
) -> Result<String, LocalModelsError> {
    let backend_model = faster_whisper_model_id(model_id)?;
    let cache_dir = faster_whisper_dir(data_dir);
    fs::create_dir_all(&cache_dir)?;
    let python = ensure_faster_whisper_python(client, data_dir, progress).await?;
    progress(DownloadProgress {
        phase: "faster-whisper",
        downloaded_bytes: 0,
        total_bytes: None,
        percent: None,
    });

    let script = r#"
import json
import sys

model_id = sys.argv[1]
download_root = sys.argv[2]

try:
    from faster_whisper import WhisperModel
except Exception as exc:
    print(json.dumps({"error": f"faster-whisper Python package is not installed: {exc}"}))
    raise SystemExit(0)

try:
    WhisperModel(model_id, device="cpu", compute_type="int8", download_root=download_root)
    print(json.dumps({"ok": True, "model": model_id, "downloadRoot": download_root}))
except Exception as exc:
    print(json.dumps({"error": str(exc)}))
"#;

    let mut command = TokioCommand::new(python);
    command
        .arg("-c")
        .arg(script)
        .arg(backend_model)
        .arg(path_string(&cache_dir))
        .kill_on_drop(true);
    let timeout = faster_whisper_prepare_timeout();
    let output = match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return Err(LocalModelsError::FasterWhisper(format!(
                "failed to launch Python: {e}"
            )))
        }
        Err(_) => {
            return Err(LocalModelsError::FasterWhisper(format!(
                "model preparation timed out after {}s",
                timeout.as_secs()
            )))
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(LocalModelsError::FasterWhisper(format!(
            "python exited with status {}: {}",
            output.status,
            stderr.trim()
        )));
    }
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).map_err(|e| {
        LocalModelsError::FasterWhisper(format!(
            "invalid setup response: {e}; stdout={}; stderr={}",
            stdout.trim(),
            stderr.trim()
        ))
    })?;
    if let Some(error) = value.get("error").and_then(|value| value.as_str()) {
        return Err(LocalModelsError::FasterWhisper(error.to_owned()));
    }
    progress(DownloadProgress {
        phase: "faster-whisper",
        downloaded_bytes: 1,
        total_bytes: Some(1),
        percent: Some(100.0),
    });
    write_faster_whisper_model_marker(data_dir, model_id, backend_model)?;
    Ok(backend_model.to_owned())
}

pub fn delete_model(data_dir: &Path, model_id: &str) -> Result<PathBuf, LocalModelsError> {
    let path = model_path_by_id(data_dir, model_id)?;
    if path.is_file() {
        fs::remove_file(&path)?;
    }
    let part_path = path.with_extension("part");
    if part_path.is_file() {
        fs::remove_file(&part_path)?;
    }
    let marker_path = faster_whisper_model_marker_path(data_dir, model_id);
    if marker_path.is_file() {
        fs::remove_file(marker_path)?;
    }
    cleanup_unused_backends(data_dir)?;
    Ok(path)
}

#[cfg(windows)]
pub async fn ensure_whisper_cpp(
    client: &Client,
    data_dir: &Path,
) -> Result<PathBuf, LocalModelsError> {
    ensure_whisper_cpp_with_progress(client, data_dir, &mut |_| {}).await
}

#[cfg(windows)]
pub async fn ensure_whisper_cpp_with_progress(
    client: &Client,
    data_dir: &Path,
    progress: &mut ProgressCallback<'_>,
) -> Result<PathBuf, LocalModelsError> {
    let command = preferred_command_path(data_dir);
    if command.is_file() {
        return Ok(command);
    }
    let (dir, archive_name, url) = if cuda_tools_enabled() {
        (
            cuda_tools_dir(data_dir),
            "whisper-cublas-12.4.0-bin-x64.zip",
            WHISPER_CPP_CUDA_ZIP_URL,
        )
    } else {
        (
            tools_dir(data_dir),
            "whisper-bin-x64.zip",
            WHISPER_CPP_CPU_ZIP_URL,
        )
    };
    fs::create_dir_all(&dir)?;
    let archive_path = dir.join(archive_name);
    download_file(client, url, &archive_path, "tool", progress).await?;
    progress(DownloadProgress {
        phase: "extract",
        downloaded_bytes: 0,
        total_bytes: None,
        percent: None,
    });
    let bytes = fs::read(&archive_path)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    archive.extract(&dir)?;
    let _ = fs::remove_file(&archive_path);
    if command.is_file() {
        Ok(command)
    } else {
        Err(LocalModelsError::Download(format!(
            "whisper.cpp archive did not contain {}",
            path_string(&command)
        )))
    }
}

#[cfg(not(windows))]
pub async fn ensure_whisper_cpp(
    _client: &Client,
    _data_dir: &Path,
) -> Result<PathBuf, LocalModelsError> {
    Err(LocalModelsError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub async fn ensure_whisper_cpp_with_progress(
    _client: &Client,
    _data_dir: &Path,
    _progress: &mut ProgressCallback<'_>,
) -> Result<PathBuf, LocalModelsError> {
    Err(LocalModelsError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::Method::GET;
    use httpmock::MockServer;

    #[tokio::test]
    async fn download_file_streams_binary_and_reports_progress() {
        let server = MockServer::start_async().await;
        let body = b"local model bytes";
        server
            .mock_async(|when, then| {
                when.method(GET).path("/model.bin");
                then.status(200)
                    .header("content-type", "application/octet-stream")
                    .header("content-length", body.len().to_string())
                    .body(body.as_slice());
            })
            .await;

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("model.bin");
        let client = Client::new();
        let mut events = Vec::new();

        download_file(
            &client,
            &server.url("/model.bin"),
            &path,
            "model",
            &mut |progress| events.push(progress),
        )
        .await
        .expect("download");

        assert_eq!(fs::read(&path).expect("downloaded file"), body);
        assert!(
            !path.with_extension("part").exists(),
            "part file should be renamed away"
        );
        assert!(
            events.iter().any(|event| event.downloaded_bytes == 0
                && event.total_bytes == Some(body.len() as u64)
                && event.percent == Some(0.0)),
            "missing initial progress event: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|event| event.downloaded_bytes == body.len() as u64
                    && event.total_bytes == Some(body.len() as u64)
                    && event.percent == Some(100.0)),
            "missing complete progress event: {events:?}"
        );
    }

    #[tokio::test]
    async fn download_file_rejects_html_response() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/model.bin");
                then.status(200)
                    .header("content-type", "text/html; charset=utf-8")
                    .body("<html>login</html>");
            })
            .await;

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("model.bin");
        let client = Client::new();
        let err = download_file(
            &client,
            &server.url("/model.bin"),
            &path,
            "model",
            &mut |_| {},
        )
        .await
        .expect_err("html response must fail");

        assert!(
            err.to_string().contains("expected binary download"),
            "unexpected error: {err}"
        );
        assert!(!path.exists(), "html response must not be stored as model");
    }

    #[tokio::test]
    async fn download_file_resumes_existing_part_with_range_request() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/model.bin")
                    .header("range", "bytes=6-");
                then.status(206)
                    .header("content-type", "application/octet-stream")
                    .header("content-range", "bytes 6-10/11")
                    .header("content-length", "5")
                    .body("world");
            })
            .await;

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("model.bin");
        fs::write(path.with_extension("part"), b"hello ").expect("part file");
        let client = Client::new();
        let mut events = Vec::new();

        download_file(
            &client,
            &server.url("/model.bin"),
            &path,
            "model",
            &mut |progress| events.push(progress),
        )
        .await
        .expect("download");

        assert_eq!(fs::read(&path).expect("downloaded file"), b"hello world");
        assert!(
            events
                .iter()
                .any(|event| event.downloaded_bytes == 6 && event.total_bytes == Some(11)),
            "missing resumed initial progress event: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|event| event.downloaded_bytes == 11 && event.percent == Some(100.0)),
            "missing resumed complete progress event: {events:?}"
        );
    }

    #[test]
    fn legacy_merge_keeps_conflicting_files() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let legacy = tmp.path().join("legacy");
        let shared = tmp.path().join("shared");
        fs::create_dir_all(&legacy).expect("legacy dir");
        fs::create_dir_all(&shared).expect("shared dir");
        fs::write(legacy.join("ggml.bin"), b"old!").expect("legacy model");
        fs::write(shared.join("ggml.bin"), b"new!").expect("shared model");

        assert!(merge_legacy_dir_into_shared(&legacy, &shared).expect("merge"));
        assert_eq!(fs::read(shared.join("ggml.bin")).expect("shared"), b"new!");
        assert_eq!(
            fs::read(shared.join("ggml.bin.legacy-1")).expect("legacy copy"),
            b"old!"
        );
        assert!(!legacy.exists());
    }

    #[test]
    fn delete_last_model_removes_unused_backend_dirs() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let small = model_spec("small").expect("small model");
        let model_path = model_path(tmp.path(), small);
        fs::create_dir_all(model_path.parent().expect("model parent")).expect("model dir");
        fs::write(&model_path, b"model").expect("model file");
        fs::create_dir_all(tools_dir(tmp.path())).expect("tools dir");
        fs::create_dir_all(faster_whisper_dir(tmp.path())).expect("faster dir");

        let deleted = delete_model(tmp.path(), "small").expect("delete model");

        assert_eq!(deleted, model_path);
        assert!(!tools_dir(tmp.path()).exists());
        assert!(!faster_whisper_dir(tmp.path()).exists());
    }

    #[test]
    fn faster_whisper_marker_without_runtime_is_not_prepared() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        write_faster_whisper_model_marker(tmp.path(), "small", "small").expect("marker");

        assert!(!faster_whisper_model_is_prepared(tmp.path(), "small"));
        assert!(!has_downloaded_model_assets(tmp.path()));
        assert!(cleanup_unused_backends(tmp.path()).expect("cleanup"));
        assert!(!faster_whisper_dir(tmp.path()).exists());
    }

    #[test]
    fn faster_whisper_cuda_runtime_detects_managed_cublas() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let cublas = if cfg!(windows) {
            "cublas64_12.dll"
        } else {
            "libcublas.so.12"
        };
        let cudnn = if cfg!(windows) {
            "cudnn64_9.dll"
        } else {
            "libcudnn.so.9"
        };
        let cublas_dir = faster_whisper_cuda_runtime_dirs(tmp.path())
            .into_iter()
            .find(|path| path.to_string_lossy().contains("cublas"))
            .expect("cublas dir");
        let cudnn_dir = faster_whisper_cuda_runtime_dirs(tmp.path())
            .into_iter()
            .find(|path| path.to_string_lossy().contains("cudnn"))
            .expect("cudnn dir");
        fs::create_dir_all(&cublas_dir).expect("cublas dir");
        fs::create_dir_all(&cudnn_dir).expect("cudnn dir");
        fs::write(cublas_dir.join(cublas), b"dll").expect("cublas dll");
        fs::write(cudnn_dir.join(cudnn), b"dll").expect("cudnn dll");

        assert!(faster_whisper_cuda_runtime_available(tmp.path()));
    }

    #[test]
    fn faster_whisper_runtime_has_default_download_source() {
        match resolve_runtime_archive_source(
            FASTER_WHISPER_RUNTIME_ARCHIVE,
            "KOSMOS_FASTER_WHISPER_RUNTIME_URL",
            "KOSMOS_FASTER_WHISPER_RUNTIME_SHA256",
        )
        .expect("default source")
        {
            RuntimeArchiveSource::Remote(url, sha256) => {
                assert!(url.contains("makekosmos/local-ai-runtimes"));
                assert_eq!(sha256, FASTER_WHISPER_RUNTIME_DEFAULT_SHA256);
            }
            _ => panic!("expected default remote source"),
        }
    }

    #[test]
    fn faster_whisper_cuda_defaults_to_official_wheels_without_zip_source() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        std::env::remove_var("KOSMOS_FASTER_WHISPER_CUDA_URL");
        std::env::remove_var("KOSMOS_FASTER_WHISPER_CUDA_URL_FILE");

        assert!(!cuda_zip_source_configured(tmp.path()));
        assert!(FASTER_WHISPER_CUDA_CUBLAS_URL.contains("files.pythonhosted.org"));
        assert!(FASTER_WHISPER_CUDA_CUDNN_URL.contains("files.pythonhosted.org"));
    }

    #[test]
    fn cuda_wheel_extract_keeps_only_requested_prefix() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let archive_path = tmp.path().join("cuda.whl");
        {
            let file = File::create(&archive_path).expect("wheel");
            let mut writer = zip::ZipWriter::new(file);
            let opts = zip::write::FileOptions::default();
            writer
                .start_file("nvidia/cublas/bin/cublas64_12.dll", opts)
                .expect("cublas entry");
            writer.write_all(b"cublas").expect("cublas bytes");
            writer
                .start_file("nvidia/other/bin/ignore.dll", opts)
                .expect("ignored entry");
            writer.write_all(b"ignore").expect("ignored bytes");
            writer.finish().expect("finish zip");
        }

        let out = tmp.path().join("out");
        extract_zip_prefix(&archive_path, "nvidia/cublas/bin", &out).expect("extract prefix");

        assert_eq!(
            fs::read(out.join("cublas64_12.dll")).expect("dll"),
            b"cublas"
        );
        assert!(!out.join("ignore.dll").exists());
    }

    #[test]
    fn faster_whisper_runtime_uses_shared_versioned_root() {
        let tmp = tempfile::TempDir::new().expect("tempdir");

        assert!(faster_whisper_runtime_dir(tmp.path())
            .to_string_lossy()
            .contains("runtimes"));
        assert!(faster_whisper_runtime_dir(tmp.path())
            .to_string_lossy()
            .contains("faster-whisper"));
        assert!(faster_whisper_dir(tmp.path())
            .to_string_lossy()
            .contains("models"));
    }

    #[test]
    fn prod_like_runtime_path_does_not_allow_system_python_fallback() {
        std::env::set_var("KOSMOS_FASTER_WHISPER_DISABLE_DEV_PYTHON", "1");
        std::env::remove_var("KOSMOS_FASTER_WHISPER_ALLOW_SYSTEM_PYTHON");
        std::env::remove_var("KOSMOS_TEST_MODE");
        std::env::remove_var("KEPLER_INSTANCE");

        assert!(!faster_whisper_dev_python_fallback_allowed());

        std::env::remove_var("KOSMOS_FASTER_WHISPER_DISABLE_DEV_PYTHON");
    }
}
