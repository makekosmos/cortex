//! Local dictation models and their download/install lifecycle. Split into
//! submodules: `download` (resumable transport) and, on Windows, `runtime`
//! (the pinned whisper.cpp zip install stack).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use reqwest::Client;
use serde::Serialize;
use tar::Archive;
use thiserror::Error;

use super::config;

mod catalog;
mod download;
mod legacy;
use catalog::model_spec;
pub use catalog::{model_supports_transcription, ModelSpec, MODEL_CATALOG};
pub use download::{DownloadProgress, ProgressCallback};
pub use legacy::migrate_legacy_assets;
#[cfg(windows)]
mod runtime;
#[cfg(windows)]
pub use runtime::{
    ensure_whisper_cpp, ensure_whisper_cpp_with_progress, refresh_managed_command_path,
    vulkan_runtime_needs_server_repair,
};

const MODELS_DIR: &str = "models/dictation";
const TOOLS_DIR: &str = "tools/dictation/whisper.cpp";
const CUDA_TOOLS_DIR: &str = "tools/dictation/whisper.cpp-cublas";
const OLD_FASTER_WHISPER_TOOLS_DIR: &str = "tools/dictation/faster-whisper";
const OLD_FASTER_WHISPER_MODEL_CACHE_DIR: &str = "models/whisper";
const OLD_FASTER_WHISPER_RUNTIME_DIR: &str = "runtimes/faster-whisper";
const OLD_FASTER_WHISPER_CUDA_LIBS_DIR: &str = "runtimes/cuda-libs";

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
    pub transcription_supported: bool,
    pub directory: bool,
    pub downloaded: bool,
    pub selected: bool,
    /// A download op for this model is in flight; `downloadPercent` is the
    /// last reported value (None = indeterminate, e.g. the extract phase).
    pub downloading: bool,
    pub download_percent: Option<f32>,
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

pub(super) fn default_shared_assets_base() -> PathBuf {
    std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("XDG_CONFIG_HOME").ok().map(PathBuf::from))
        .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(super) fn default_shared_assets_root() -> PathBuf {
    default_shared_assets_base().join("Mundus")
}

pub(super) fn shared_assets_root(data_dir: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("MUNDUS_LOCAL_STT_DIR") {
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

pub fn models_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir).join(MODELS_DIR)
}

pub fn tools_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir).join(TOOLS_DIR)
}

pub fn model_path(data_dir: &Path, spec: &ModelSpec) -> PathBuf {
    models_dir(data_dir).join(spec.filename)
}

pub fn model_is_installed(data_dir: &Path, spec: &ModelSpec) -> bool {
    let path = model_path(data_dir, spec);
    if spec.directory {
        if spec.files.is_empty() {
            // Tarball directory models: extraction is atomic, dir-exists is enough.
            path.is_dir()
        } else {
            // Multi-file models download into the dir piecemeal — count as
            // installed only once every declared file is present.
            spec.files
                .iter()
                .all(|(_, local)| path.join(local).is_file())
        }
    } else {
        path.is_file()
    }
}

pub fn model_path_by_id(data_dir: &Path, model_id: &str) -> Result<PathBuf, LocalModelsError> {
    Ok(model_path(data_dir, model_spec(model_id)?))
}

pub fn command_path(data_dir: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let preferred = runtime::preferred_command_path(data_dir);
        if preferred.is_file() {
            return Some(preferred);
        }
        let vulkan = runtime::vulkan_command_path(data_dir);
        if runtime::vulkan_tools_enabled() && vulkan.is_file() {
            return Some(vulkan);
        }
        let cpu = runtime::cpu_command_path(data_dir);
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

#[cfg(not(windows))]
pub fn vulkan_runtime_needs_server_repair(_data_dir: &Path) -> bool {
    false
}

#[cfg(not(windows))]
pub fn refresh_managed_command_path(_data_dir: &Path, _cfg: &mut config::DictationConfig) -> bool {
    false
}

/// Map the shared size/sha256 verification result onto this module's error
/// type — the `ShaMismatch` variant keeps the path for the UI message.
pub(super) fn verify_error(path: &Path, error: crate::file_hash::VerifyError) -> LocalModelsError {
    match error {
        crate::file_hash::VerifyError::Hash { expected, actual } => LocalModelsError::ShaMismatch {
            path: path_string(path),
            expected,
            actual,
        },
        crate::file_hash::VerifyError::Size { expected, actual } => {
            LocalModelsError::Download(format!(
                "{}: size mismatch: expected {expected} bytes, got {actual}",
                path_string(path)
            ))
        }
        crate::file_hash::VerifyError::Io(error) => error.into(),
    }
}

pub(super) fn same_path_or_text(left: &Path, right: &Path) -> bool {
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

pub(super) fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub fn has_downloaded_model_assets(data_dir: &Path) -> bool {
    MODEL_CATALOG
        .iter()
        .any(|spec| model_is_installed(data_dir, spec))
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
        runtime::vulkan_tools_dir(data_dir),
    ] {
        if path.exists() {
            fs::remove_dir_all(path)?;
            changed = true;
        }
    }
    Ok(changed)
}

/// In-flight model downloads, `model_id` → last reported percent
/// (`None` while the phase is indeterminate). `DictationHost` owns the map;
/// `snapshot` renders it into `LocalModelInfo` for poll-only clients.
pub type ModelDownloads = std::collections::HashMap<String, Option<f32>>;

pub fn snapshot(
    data_dir: &Path,
    cfg: &config::DictationConfig,
    downloads: &ModelDownloads,
) -> LocalModelsSnapshot {
    let models_dir = models_dir(data_dir);
    let command_path = command_path(data_dir);
    let command_installed = command_path.as_ref().is_some_and(|path| path.is_file());
    let selected_path = cfg.local_model_path.as_deref();
    let models = MODEL_CATALOG
        .iter()
        .map(|spec| {
            let path = model_path(data_dir, spec);
            let path_text = path_string(&path);
            let downloaded = model_is_installed(data_dir, spec);
            let selected = downloaded
                && (cfg.local_model.as_deref() == Some(spec.id)
                    || selected_path == Some(path_text.as_str()));
            let download_percent = downloads.get(spec.id).copied().flatten();
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
                transcription_supported: spec.transcription_supported,
                directory: spec.directory,
                downloaded,
                selected,
                downloading: downloads.contains_key(spec.id),
                download_percent,
                path: downloaded.then_some(path_text),
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

fn extract_tar_gz(archive_path: &Path, destination: &Path) -> Result<(), LocalModelsError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let extracting = destination.with_extension("extracting");
    let _ = fs::remove_dir_all(&extracting);
    fs::create_dir_all(&extracting)?;

    let file = fs::File::open(archive_path)?;
    Archive::new(GzDecoder::new(file)).unpack(&extracting)?;

    let dirs = fs::read_dir(&extracting)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ty| ty.is_dir()).unwrap_or(false))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();

    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    if dirs.len() == 1 {
        fs::rename(&dirs[0], destination)?;
        let _ = fs::remove_dir_all(&extracting);
    } else {
        fs::create_dir_all(parent)?;
        fs::rename(&extracting, destination)?;
    }
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
    if model_is_installed(data_dir, spec) {
        return Ok(path);
    }
    if spec.directory && !spec.files.is_empty() {
        // Multi-file model (e.g. Parakeet Ultra ONNX): `url` is the base,
        // each `files` pair downloads straight into the model dir. Already
        // present files are skipped; per-file `.part` resume stays inside
        // `download_file`.
        let base = spec.url.trim_end_matches('/');
        let expected_total = spec.size_mb.saturating_mul(1024 * 1024);
        let mut per_file = vec![0_u64; spec.files.len()];
        for (index, (remote, local)) in spec.files.iter().enumerate() {
            let destination = path.join(local);
            if destination.is_file() {
                continue;
            }
            let url = format!("{base}/{remote}");
            let per_file = &mut per_file;
            let progress = &mut *progress;
            let mut inner = move |file_progress: DownloadProgress| {
                per_file[index] = file_progress.downloaded_bytes;
                let downloaded: u64 = per_file.iter().sum();
                progress(DownloadProgress {
                    phase: "model",
                    downloaded_bytes: downloaded,
                    total_bytes: Some(expected_total),
                    percent: Some((downloaded as f64 / expected_total as f64 * 100.0).min(100.0)),
                });
            };
            download::download_file(client, &url, &destination, "model", None, &mut inner).await?;
        }
        progress(DownloadProgress {
            phase: "model",
            downloaded_bytes: expected_total,
            total_bytes: Some(expected_total),
            percent: Some(100.0),
        });
    } else if spec.directory {
        let archive_path = path.with_extension("tar.gz");
        download::download_file(client, spec.url, &archive_path, "model", None, progress).await?;
        if let Some(expected) = spec.sha256 {
            crate::file_hash::verify_sha256(&archive_path, expected)
                .map_err(|error| verify_error(&archive_path, error))?;
        }
        progress(DownloadProgress {
            phase: "extract",
            downloaded_bytes: 0,
            total_bytes: None,
            percent: None,
        });
        extract_tar_gz(&archive_path, &path)?;
        let _ = fs::remove_file(&archive_path);
    } else {
        download::download_file(client, spec.url, &path, "model", None, progress).await?;
        if let Some(expected) = spec.sha256 {
            crate::file_hash::verify_sha256(&path, expected)
                .map_err(|error| verify_error(&path, error))?;
        }
    }
    Ok(path)
}

pub fn delete_model(data_dir: &Path, model_id: &str) -> Result<PathBuf, LocalModelsError> {
    let spec = model_spec(model_id)?;
    let path = model_path(data_dir, spec);
    if spec.directory && path.is_dir() {
        fs::remove_dir_all(&path)?;
    } else if path.is_file() {
        fs::remove_file(&path)?;
    }
    for path in [path.with_extension("part"), path.with_extension("tar.part")] {
        if path.is_file() {
            fs::remove_file(&path)?;
        }
    }
    let _ = fs::remove_dir_all(path.with_extension("extracting"));
    if let Err(error) = cleanup_unused_backends(data_dir) {
        tracing::warn!(
            error = %error,
            "dictation: cleanup of unused local STT backends failed after model delete"
        );
    }
    Ok(path)
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
mod tests;
