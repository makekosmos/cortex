use std::fs::{self, OpenOptions};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use reqwest::header::{
    ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
};
use reqwest::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::config;

const MODELS_DIR: &str = "models/dictation";
const TOOLS_DIR: &str = "tools/dictation/whisper.cpp";
const CUDA_TOOLS_DIR: &str = "tools/dictation/whisper.cpp-cublas";
const VULKAN_TOOLS_DIR: &str = "tools/dictation/whisper.cpp-vulkan";
const OLD_FASTER_WHISPER_TOOLS_DIR: &str = "tools/dictation/faster-whisper";
const OLD_FASTER_WHISPER_MODEL_CACHE_DIR: &str = "models/whisper";
const OLD_FASTER_WHISPER_RUNTIME_DIR: &str = "runtimes/faster-whisper";
const OLD_FASTER_WHISPER_CUDA_LIBS_DIR: &str = "runtimes/cuda-libs";
const DOWNLOAD_MAX_ATTEMPTS: usize = 8;

#[cfg(windows)]
const WHISPER_CPP_CPU_ZIP_URL: &str =
    "https://github.com/ggml-org/whisper.cpp/releases/download/v1.8.4/whisper-bin-x64.zip";
#[cfg(windows)]
const WHISPER_CPP_VULKAN_ZIP_URL: &str = "https://github.com/makekosmos/local-ai-runtimes/releases/download/whisper-vulkan-win-x64-v1.9.1/whisper-vulkan-bin-x64-v1.9.1.zip";
#[cfg(windows)]
const WHISPER_CPP_VULKAN_ZIP_SHA256: &str =
    "b5660319454be5f33f387262ddeef5148e78be16c0308edc190e50fda50e66a3";

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
        changed |= merge_legacy_dir_into_shared(&root.join(TOOLS_DIR), &tools_dir(data_dir))?;
        #[cfg(windows)]
        {
            changed |= merge_legacy_dir_into_shared(
                &root.join(VULKAN_TOOLS_DIR),
                &vulkan_tools_dir(data_dir),
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

#[cfg(windows)]
fn vulkan_tools_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir).join(VULKAN_TOOLS_DIR)
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
        let vulkan = vulkan_command_path(data_dir);
        if vulkan_tools_enabled() && vulkan.is_file() {
            return Some(vulkan);
        }
        let cpu = cpu_command_path(data_dir);
        if cpu.is_file() {
            return Some(cpu);
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
fn vulkan_command_path(data_dir: &Path) -> PathBuf {
    vulkan_tools_dir(data_dir)
        .join("Release")
        .join("whisper-cli.exe")
}

#[cfg(windows)]
fn preferred_command_path(data_dir: &Path) -> PathBuf {
    let vulkan = vulkan_command_path(data_dir);
    if vulkan_tools_enabled() && vulkan.is_file() {
        vulkan
    } else {
        cpu_command_path(data_dir)
    }
}

#[cfg(windows)]
fn managed_command_paths(data_dir: &Path) -> [PathBuf; 2] {
    [cpu_command_path(data_dir), vulkan_command_path(data_dir)]
}

#[cfg(windows)]
fn vulkan_tools_enabled() -> bool {
    if std::env::var("KOSMOS_DICTATION_DISABLE_VULKAN").as_deref() == Ok("1") {
        return false;
    }
    vulkan_runtime_available()
}

#[cfg(windows)]
fn vulkan_runtime_available() -> bool {
    if cfg!(test) {
        return true;
    }
    let system_vulkan = std::env::var("SystemRoot")
        .ok()
        .map(PathBuf::from)
        .map(|root| root.join("System32").join("vulkan-1.dll"))
        .is_some_and(|path| path.is_file());
    system_vulkan
        || Command::new("where")
            .arg("vulkan-1.dll")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
}

#[cfg(windows)]
fn vulkan_zip_url() -> String {
    std::env::var("KOSMOS_WHISPER_CPP_VULKAN_ZIP_URL")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| WHISPER_CPP_VULKAN_ZIP_URL.to_owned())
}

#[cfg(windows)]
fn vulkan_zip_sha256() -> Option<String> {
    std::env::var("KOSMOS_WHISPER_CPP_VULKAN_ZIP_SHA256")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            (vulkan_zip_url() == WHISPER_CPP_VULKAN_ZIP_URL)
                .then(|| WHISPER_CPP_VULKAN_ZIP_SHA256.to_owned())
        })
}

#[cfg(windows)]
fn bundled_whisper_cpp_runtime_dir(runtime_name: &str) -> Option<PathBuf> {
    std::env::var("KOSMOS_RESOURCES_DIR")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|root| root.join("local-stt").join(runtime_name))
        .filter(|path| path.is_dir())
}

#[cfg(windows)]
fn install_bundled_whisper_cpp_runtime(destination: &Path, runtime_name: &str) -> io::Result<bool> {
    let Some(source) = bundled_whisper_cpp_runtime_dir(runtime_name) else {
        return Ok(false);
    };

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let staging = destination.with_file_name(format!(
        "{}.installing",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("whisper.cpp")
    ));
    remove_path_if_exists(&staging)?;
    copy_dir_all(&source, &staging)?;
    remove_path_if_exists(destination)?;
    fs::rename(staging, destination)?;
    Ok(true)
}

#[cfg(windows)]
fn copy_dir_all(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn remove_path_if_exists(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
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

pub fn has_downloaded_model_assets(data_dir: &Path) -> bool {
    MODEL_CATALOG
        .iter()
        .any(|spec| model_path(data_dir, spec).is_file())
}

pub fn cleanup_obsolete_local_stt_assets(data_dir: &Path) -> io::Result<bool> {
    let mut changed = false;
    for path in [
        shared_assets_root(data_dir).join(OLD_FASTER_WHISPER_TOOLS_DIR),
        shared_assets_root(data_dir).join(OLD_FASTER_WHISPER_MODEL_CACHE_DIR),
        shared_assets_root(data_dir).join(OLD_FASTER_WHISPER_RUNTIME_DIR),
        shared_assets_root(data_dir).join(OLD_FASTER_WHISPER_CUDA_LIBS_DIR),
        #[cfg(windows)]
        shared_assets_root(data_dir).join(CUDA_TOOLS_DIR),
    ] {
        if path.exists() {
            fs::remove_dir_all(path)?;
            changed = true;
        }
    }
    Ok(changed)
}

pub fn cleanup_unused_backends(data_dir: &Path) -> io::Result<bool> {
    if has_downloaded_model_assets(data_dir) {
        return Ok(false);
    }

    let mut changed = false;
    for path in [
        tools_dir(data_dir),
        #[cfg(windows)]
        vulkan_tools_dir(data_dir),
    ] {
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
    let models = MODEL_CATALOG
        .iter()
        .map(|spec| {
            let path = model_path(data_dir, spec);
            let path_text = path_string(&path);
            let downloaded = path.is_file();
            let selected = downloaded
                && (cfg.local_model.as_deref() == Some(spec.id)
                    || selected_path == Some(path_text.as_str()));
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

pub fn delete_model(data_dir: &Path, model_id: &str) -> Result<PathBuf, LocalModelsError> {
    let path = model_path_by_id(data_dir, model_id)?;
    if path.is_file() {
        fs::remove_file(&path)?;
    }
    let part_path = path.with_extension("part");
    if part_path.is_file() {
        fs::remove_file(&part_path)?;
    }
    if let Err(error) = cleanup_unused_backends(data_dir) {
        tracing::warn!(
            error = %error,
            "dictation: cleanup of unused local STT backends failed after model delete"
        );
    }
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
    if vulkan_tools_enabled() {
        let vulkan = vulkan_command_path(data_dir);
        if !vulkan.is_file() {
            match install_bundled_whisper_cpp_runtime(
                &vulkan_tools_dir(data_dir),
                "whisper.cpp-vulkan",
            ) {
                Ok(true) if vulkan.is_file() => return Ok(vulkan),
                Ok(true) => tracing::warn!(
                    path = %path_string(&vulkan),
                    "dictation: bundled Vulkan whisper.cpp runtime did not contain command"
                ),
                Ok(false) => {}
                Err(error) => tracing::warn!(
                    error = %error,
                    "dictation: bundled Vulkan whisper.cpp install failed, falling back"
                ),
            }
        }
        if vulkan.is_file() {
            return Ok(vulkan);
        }
        let url = vulkan_zip_url();
        let sha256 = vulkan_zip_sha256();
        match install_whisper_cpp_zip(
            client,
            &vulkan_tools_dir(data_dir),
            "whisper-vulkan-bin-x64.zip",
            &url,
            sha256.as_deref(),
            progress,
        )
        .await
        {
            Ok(()) if vulkan.is_file() => return Ok(vulkan),
            Ok(()) => tracing::warn!(
                path = %path_string(&vulkan),
                "dictation: Vulkan whisper.cpp archive did not contain command, falling back"
            ),
            Err(error) => tracing::warn!(
                error = %error,
                "dictation: Vulkan whisper.cpp install failed, falling back"
            ),
        }
    }
    let (dir, archive_name, url) = (
        tools_dir(data_dir),
        "whisper-bin-x64.zip",
        WHISPER_CPP_CPU_ZIP_URL,
    );
    let cpu_command = cpu_command_path(data_dir);
    if !cpu_command.is_file() {
        match install_bundled_whisper_cpp_runtime(&dir, "whisper.cpp") {
            Ok(true) if cpu_command.is_file() => return Ok(cpu_command),
            Ok(true) => tracing::warn!(
                path = %path_string(&cpu_command),
                "dictation: bundled CPU whisper.cpp runtime did not contain command"
            ),
            Ok(false) => {}
            Err(error) => tracing::warn!(
                error = %error,
                "dictation: bundled CPU whisper.cpp install failed, falling back to download"
            ),
        }
    }
    install_whisper_cpp_zip(client, &dir, archive_name, url, None, progress).await?;
    let command = preferred_command_path(data_dir);
    if command.is_file() {
        Ok(command)
    } else {
        Err(LocalModelsError::Download(format!(
            "whisper.cpp archive did not contain {}",
            path_string(&command)
        )))
    }
}

#[cfg(windows)]
async fn install_whisper_cpp_zip(
    client: &Client,
    dir: &Path,
    archive_name: &str,
    url: &str,
    expected_sha256: Option<&str>,
    progress: &mut ProgressCallback<'_>,
) -> Result<(), LocalModelsError> {
    fs::create_dir_all(&dir)?;
    let archive_path = dir.join(archive_name);
    download_file(client, url, &archive_path, "tool", progress).await?;
    if let Some(expected) = expected_sha256 {
        verify_sha256(&archive_path, expected)?;
    }
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
    Ok(())
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

    static ENV_LOCAL_MODELS_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[cfg(windows)]
    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().expect("parent")).expect("parent dir");
        fs::write(path, b"exe").expect("file");
    }

    #[cfg(windows)]
    #[test]
    fn command_path_prefers_installed_vulkan_over_cpu() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        touch(&cpu_command_path(tmp.path()));
        touch(&vulkan_command_path(tmp.path()));

        assert!(same_path_or_text(
            &command_path(tmp.path()).expect("command"),
            &vulkan_command_path(tmp.path())
        ));
    }

    #[cfg(windows)]
    #[test]
    fn vulkan_runtime_source_is_pinned() {
        assert!(WHISPER_CPP_VULKAN_ZIP_URL.contains("makekosmos/local-ai-runtimes"));
        assert!(WHISPER_CPP_VULKAN_ZIP_URL.contains("whisper-vulkan-win-x64-v1.9.1"));
        assert_eq!(WHISPER_CPP_VULKAN_ZIP_SHA256.len(), 64);
    }

    #[cfg(windows)]
    #[test]
    fn installs_bundled_whisper_cpp_runtime() {
        let _guard = ENV_LOCAL_MODELS_TEST_LOCK.lock().expect("env lock");
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let resources = tmp.path().join("resources");
        let bundled_command = resources
            .join("local-stt")
            .join("whisper.cpp-vulkan")
            .join("Release")
            .join("whisper-cli.exe");
        touch(&bundled_command);
        std::env::set_var("KOSMOS_RESOURCES_DIR", &resources);

        let destination = tmp.path().join("shared").join("whisper.cpp-vulkan");
        assert!(
            install_bundled_whisper_cpp_runtime(&destination, "whisper.cpp-vulkan")
                .expect("install bundled runtime")
        );

        assert!(destination
            .join("Release")
            .join("whisper-cli.exe")
            .is_file());
        std::env::remove_var("KOSMOS_RESOURCES_DIR");
    }

    #[cfg(windows)]
    #[test]
    fn refresh_managed_command_path_updates_cpu_to_vulkan() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        touch(&cpu_command_path(tmp.path()));
        touch(&vulkan_command_path(tmp.path()));
        let mut cfg = config::DictationConfig {
            local_command_path: Some(path_string(&cpu_command_path(tmp.path()))),
            ..Default::default()
        };

        assert!(refresh_managed_command_path(tmp.path(), &mut cfg));
        assert_eq!(
            cfg.local_command_path.as_deref(),
            Some(path_string(&vulkan_command_path(tmp.path())).as_str())
        );
    }

    #[test]
    fn snapshot_does_not_mark_missing_selected_model() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let spec = MODEL_CATALOG
            .iter()
            .find(|model| model.id == "turbo")
            .expect("turbo model");
        let path = model_path(tmp.path(), spec);
        let cfg = config::DictationConfig {
            provider: "local".into(),
            provider_enabled: true,
            local_model: Some(spec.id.to_owned()),
            local_model_path: Some(path_string(&path)),
            ..Default::default()
        };

        let snapshot = snapshot(tmp.path(), &cfg);
        let model = snapshot
            .models
            .iter()
            .find(|model| model.id == spec.id)
            .expect("snapshot model");
        assert!(!model.downloaded);
        assert!(!model.selected);
    }

    #[test]
    fn delete_model_ignores_backend_cleanup_failure_after_file_delete() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let spec = MODEL_CATALOG
            .iter()
            .find(|model| model.id == "small")
            .expect("small model");
        let path = model_path(tmp.path(), spec);
        fs::create_dir_all(path.parent().expect("model parent")).expect("model dir");
        fs::write(&path, b"model").expect("model file");
        fs::create_dir_all(tools_dir(tmp.path()).parent().expect("tools parent"))
            .expect("tools parent");
        fs::write(tools_dir(tmp.path()), b"not a directory").expect("cleanup blocker");

        let deleted = delete_model(tmp.path(), spec.id).expect("delete model");
        assert_eq!(deleted, path);
        assert!(!deleted.exists());
    }

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

        let deleted = delete_model(tmp.path(), "small").expect("delete model");

        assert_eq!(deleted, model_path);
        assert!(!tools_dir(tmp.path()).exists());
    }

    #[test]
    fn cleanup_obsolete_local_stt_assets_deletes_ct2_cache_even_with_ggml_models() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let small = model_spec("small").expect("small model");
        let model_path = model_path(tmp.path(), small);
        fs::create_dir_all(model_path.parent().expect("model parent")).expect("model dir");
        fs::write(&model_path, b"ggml").expect("ggml model");

        let old_ct2 = shared_assets_root(tmp.path()).join(OLD_FASTER_WHISPER_MODEL_CACHE_DIR);
        let old_runtime = shared_assets_root(tmp.path()).join(OLD_FASTER_WHISPER_RUNTIME_DIR);
        let old_cuda = shared_assets_root(tmp.path()).join(OLD_FASTER_WHISPER_CUDA_LIBS_DIR);
        fs::create_dir_all(&old_ct2).expect("ct2 dir");
        fs::create_dir_all(&old_runtime).expect("runtime dir");
        fs::create_dir_all(&old_cuda).expect("cuda dir");

        assert!(cleanup_obsolete_local_stt_assets(tmp.path()).expect("cleanup"));
        assert!(model_path.is_file());
        assert!(!old_ct2.exists());
        assert!(!old_runtime.exists());
        assert!(!old_cuda.exists());
    }
}
