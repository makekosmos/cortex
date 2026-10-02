#[cfg(windows)]
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
#[cfg(windows)]
use std::io::{Cursor, Seek, SeekFrom};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;
use std::time::Duration;

#[cfg(windows)]
use base64::{engine::general_purpose::STANDARD, Engine as _};
#[cfg(windows)]
use chrono::{DateTime, Duration as ChronoDuration, Utc};
#[cfg(windows)]
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use flate2::read::GzDecoder;
use reqwest::header::{
    ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
};
use reqwest::Client;
#[cfg(windows)]
use serde::Deserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tar::Archive;
use thiserror::Error;

use super::config;

const MODELS_DIR: &str = "models/dictation";
const TOOLS_DIR: &str = "tools/dictation/whisper.cpp";
const CUDA_TOOLS_DIR: &str = "tools/dictation/whisper.cpp-cublas";
const VULKAN_TOOLS_DIR: &str = "tools/dictation/whisper.cpp-vulkan";
const OLD_FASTER_WHISPER_TOOLS_DIR: &str = "tools/dictation/faster-whisper";
const OLD_FASTER_WHISPER_MODEL_CACHE_DIR: &str = "models/whisper";
const OLD_FASTER_WHISPER_RUNTIME_DIR: &str = "runtimes/faster-whisper";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
const OLD_FASTER_WHISPER_CUDA_LIBS_DIR: &str = "runtimes/cuda-libs";
const DOWNLOAD_MAX_ATTEMPTS: usize = 8;
#[cfg(windows)]
const WHISPER_RUNTIME_VERSION: &str = "1.9.3";
#[cfg(windows)]
const RUNTIME_MANIFEST_URL: &str = concat!(
    "https://github.com/makekosmos/local-ai-runtimes/releases/download/runtime-v1",
    ".9.3/runtimes.manifest.json"
);
#[cfg(windows)]
const RUNTIME_ENVELOPE_URL: &str = concat!(
    "https://github.com/makekosmos/local-ai-runtimes/releases/download/runtime-v1",
    ".9.3/runtimes.manifest.envelope.json"
);
#[cfg(windows)]
const RUNTIME_KEY_ID: &str = "runtime-prod-2026-1";
#[cfg(windows)]
const RUNTIME_PUBLIC_KEY_B64: &str = "ukKkVVFmhQ7wzR4ZvM3Iea83x2ptrE/+2HxHBKOk6Cc=";

#[cfg(windows)]
const WHISPER_CPP_CPU_ZIP_URL: &str = concat!(
    "https://github.com/makekosmos/local-ai-runtimes/releases/download/runtime-v1",
    ".9.3/whisper-cpu-bin-x64-v1.9.3.zip"
);
#[cfg(windows)]
const WHISPER_CPP_CPU_ZIP_SHA256: &str =
    "2464c8ecdc070ccdba079b363943e979708443180fd6dda12d7d0801beeb5954";
#[cfg(windows)]
const WHISPER_CPP_CPU_ZIP_SIZE: u64 = 1_436_012;
#[cfg(windows)]
const WHISPER_CPP_VULKAN_ZIP_URL: &str = concat!(
    "https://github.com/makekosmos/local-ai-runtimes/releases/download/runtime-v1",
    ".9.3/whisper-vulkan-bin-x64-v1.9.3.zip"
);
#[cfg(windows)]
const WHISPER_CPP_VULKAN_ZIP_SHA256: &str =
    "520ab6225f6b0afd2e8dcbc67e196a934df44bc7932f86cc2c29796a996c48f4";
#[cfg(windows)]
const WHISPER_CPP_VULKAN_ZIP_SIZE: u64 = 17_403_912;

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
    pub transcription_supported: bool,
    pub directory: bool,
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
    pub transcription_supported: bool,
    pub directory: bool,
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
        transcription_supported: true,
        directory: false,
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
        transcription_supported: true,
        directory: false,
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
        transcription_supported: true,
        directory: false,
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
        transcription_supported: true,
        directory: false,
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
        transcription_supported: true,
        directory: false,
    },
    ModelSpec {
        id: "parakeet-tdt-0.6b-v3",
        name: "Parakeet V3",
        description: "Быстрая int8-сборка NVIDIA Parakeet v3 из Handy.",
        filename: "parakeet-tdt-0.6b-v3-int8",
        url: "https://blob.handy.computer/parakeet-v3-int8.tar.gz",
        sha256: Some("43d37191602727524a7d8c6da0eef11c4ba24320f5b4730f1a2497befc2efa77"),
        size_mb: 456,
        accuracy_score: 0.80,
        speed_score: 0.85,
        recommended: false,
        transcription_supported: true,
        directory: true,
    },
];

fn model_spec(model_id: &str) -> Result<&'static ModelSpec, LocalModelsError> {
    MODEL_CATALOG
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| LocalModelsError::ModelNotFound(model_id.to_owned()))
}

pub fn model_supports_transcription(model_id: &str) -> Result<bool, LocalModelsError> {
    Ok(model_spec(model_id)?.transcription_supported)
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
    default_shared_assets_base().join("Mundus")
}

fn shared_assets_root(data_dir: &Path) -> PathBuf {
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
    let mut i = 1;
    loop {
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
        i += 1;
    }
}

pub fn migrate_legacy_assets(data_dir: &Path) -> io::Result<bool> {
    let shared_root = shared_assets_root(data_dir);
    let mut changed = false;
    let mut roots = vec![data_dir.to_path_buf()];
    if std::env::var("MUNDUS_LOCAL_STT_DIR").is_err()
        && !cfg!(test)
        && same_path_or_text(&shared_root, &default_shared_assets_root())
    {
        if let Ok(entries) = fs::read_dir(default_shared_assets_base()) {
            for entry in entries {
                let path = entry?.path();
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                // MIGRATION(KOS-267): pre-rename dev worktrees
                let is_dev_dir = name.starts_with("Mundus-dev") || name.starts_with("Kosmos-dev");
                if path.is_dir() && is_dev_dir {
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

pub fn model_is_installed(data_dir: &Path, spec: &ModelSpec) -> bool {
    let path = model_path(data_dir, spec);
    if spec.directory {
        path.is_dir()
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
fn vulkan_server_path(data_dir: &Path) -> PathBuf {
    vulkan_tools_dir(data_dir)
        .join("Release")
        .join("whisper-server.exe")
}

#[cfg(windows)]
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIntegrityRecord {
    version: String,
    files: std::collections::BTreeMap<String, String>,
}

#[cfg(windows)]
fn runtime_file_names(vulkan: bool) -> Vec<&'static str> {
    let mut files = vec![
        "LICENSE.whisper.cpp.txt",
        "Release/ggml-base.dll",
        "Release/ggml-cpu.dll",
        "Release/ggml.dll",
        "Release/whisper-cli.exe",
        "Release/whisper-server.exe",
        "Release/whisper.dll",
    ];
    if vulkan {
        files.push("Release/ggml-vulkan.dll");
    }
    files
}

#[cfg(windows)]
fn runtime_integrity_path(dir: &Path) -> PathBuf {
    dir.join(".runtime-integrity.json")
}

#[cfg(windows)]
fn runtime_transaction_path(dir: &Path) -> PathBuf {
    dir.parent().unwrap_or_else(|| Path::new(".")).join(format!(
        ".{}.transaction",
        dir.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("runtime")
    ))
}

#[cfg(windows)]
fn sha256_file(path: &Path) -> Result<String, LocalModelsError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0; 1024 * 1024];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(windows)]
fn write_runtime_integrity(dir: &Path, vulkan: bool) -> Result<(), LocalModelsError> {
    let mut files = std::collections::BTreeMap::new();
    for name in runtime_file_names(vulkan) {
        let path = dir.join(name);
        files.insert(name.to_owned(), sha256_file(&path)?);
    }
    let bytes = serde_json::to_vec(&RuntimeIntegrityRecord {
        version: WHISPER_RUNTIME_VERSION.to_owned(),
        files,
    })
    .map_err(|error| LocalModelsError::Download(format!("runtime integrity: {error}")))?;
    fs::write(runtime_integrity_path(dir), bytes)?;
    Ok(())
}

#[cfg(windows)]
fn runtime_is_current(dir: &Path) -> bool {
    if !fs::read_to_string(dir.join(".runtime-version"))
        .is_ok_and(|version| version.trim() == WHISPER_RUNTIME_VERSION)
    {
        return false;
    }
    let vulkan = dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "whisper.cpp-vulkan");
    let Ok(bytes) = fs::read(runtime_integrity_path(dir)) else {
        return false;
    };
    let Ok(record) = serde_json::from_slice::<RuntimeIntegrityRecord>(&bytes) else {
        return false;
    };
    if record.version != WHISPER_RUNTIME_VERSION {
        return false;
    }
    let expected = runtime_file_names(vulkan);
    record.files.len() == expected.len()
        && expected.iter().all(|name| {
            let path = dir.join(name);
            path.is_file()
                && record
                    .files
                    .get(*name)
                    .is_some_and(|hash| sha256_file(&path).is_ok_and(|actual| actual == *hash))
        })
}

#[cfg(windows)]
fn write_runtime_transaction(dir: &Path) -> Result<(), LocalModelsError> {
    let marker = runtime_transaction_path(dir);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(marker)?;
    file.write_all(b"1")?;
    file.sync_all()?;
    Ok(())
}

#[cfg(windows)]
fn recover_runtime_transaction(dir: &Path) -> Result<(), LocalModelsError> {
    let marker = runtime_transaction_path(dir);
    if !marker.is_file() {
        return Ok(());
    }
    let parent = dir
        .parent()
        .ok_or_else(|| LocalModelsError::Download("invalid runtime path".into()))?;
    let stem = dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| LocalModelsError::Download("invalid runtime path".into()))?;
    let staging = parent.join(format!(".{stem}.staging"));
    let previous = parent.join(format!(".{stem}.previous"));
    if dir.exists() {
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        if previous.exists() {
            fs::remove_dir_all(&previous)?;
        }
    } else if previous.exists() {
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::rename(&previous, dir)?;
    } else if staging.exists() {
        fs::rename(&staging, dir)?;
    }
    let _ = fs::remove_file(marker);
    Ok(())
}

/// `true`, если установлен Vulkan whisper-cli, но рядом нет whisper-server.exe.
/// Такой рантайм работает только в «холодном» режиме (перезагрузка модели на
/// каждую диктовку); требуется перекачать обновлённый архив с server'ом.
#[cfg(windows)]
pub fn vulkan_runtime_needs_server_repair(data_dir: &Path) -> bool {
    vulkan_tools_enabled()
        && vulkan_command_path(data_dir).is_file()
        && !vulkan_server_path(data_dir).is_file()
}

#[cfg(not(windows))]
pub fn vulkan_runtime_needs_server_repair(_data_dir: &Path) -> bool {
    false
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
    if std::env::var("MUNDUS_DICTATION_DISABLE_VULKAN").as_deref() == Ok("1") {
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
    system_vulkan || {
        let mut command = Command::new("where");
        command.arg("vulkan-1.dll");
        command.creation_flags(CREATE_NO_WINDOW);
        command.output()
    }
    .map(|output| output.status.success())
    .unwrap_or(false)
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
            let downloaded = model_is_installed(data_dir, spec);
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
                transcription_supported: spec.transcription_supported,
                directory: spec.directory,
                downloaded,
                selected,
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

async fn download_file(
    client: &Client,
    url: &str,
    destination: &Path,
    phase: &'static str,
    max_bytes: Option<u64>,
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
        if max_bytes.is_some_and(|limit| downloaded_bytes > limit) {
            fs::remove_file(&part_path)?;
            downloaded_bytes = 0;
        }
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
        #[cfg(windows)]
        if phase == "tool" && !trusted_runtime_response(response.url()) {
            return Err(LocalModelsError::Download(format!(
                "{request_url}: untrusted runtime redirect"
            )));
        }
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
                        "{request_url}: stream read failed after {downloaded_bytes} bytes \
                             ({content_type}, encoding {content_encoding}): {e}"
                    );
                    tracing::warn!(
                        phase,
                        attempt,
                        error = %message,
                        "dictation local model download chunk failed",
                    );
                    last_error = Some(message);
                    break;
                }
            };
            let Some(chunk) = chunk else {
                file.flush()?;
                fs::rename(&part_path, destination)?;
                return Ok(());
            };
            if max_bytes.is_some_and(|limit| downloaded_bytes + chunk.len() as u64 > limit) {
                drop(file);
                let _ = fs::remove_file(&part_path);
                return Err(LocalModelsError::Download(format!(
                    "{request_url}: download exceeds verified size limit"
                )));
            }
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
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0; 1024 * 1024];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
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
    if spec.directory {
        let archive_path = path.with_extension("tar.gz");
        download_file(client, spec.url, &archive_path, "model", None, progress).await?;
        if let Some(expected) = spec.sha256 {
            verify_sha256(&archive_path, expected)?;
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
        download_file(client, spec.url, &path, "model", None, progress).await?;
        if let Some(expected) = spec.sha256 {
            verify_sha256(&path, expected)?;
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
    recover_runtime_transaction(&vulkan_tools_dir(data_dir))?;
    recover_runtime_transaction(&tools_dir(data_dir))?;
    if vulkan_tools_enabled() {
        let vulkan = vulkan_command_path(data_dir);
        // Vulkan-рантайм считается установленным только если есть И whisper-cli,
        // И whisper-server.exe: без server тёплый путь (warm GPU) не работает и
        // модель грузится заново на каждую диктовку. Отсутствие server.exe →
        // перекачиваем обновлённый архив (r2), который его содержит.
        if vulkan.is_file()
            && vulkan_server_path(data_dir).is_file()
            && runtime_is_current(&vulkan_tools_dir(data_dir))
        {
            return Ok(vulkan);
        }
        match install_whisper_cpp_zip(
            client,
            &vulkan_tools_dir(data_dir),
            "whisper-vulkan-bin-x64.zip",
            WHISPER_CPP_VULKAN_ZIP_URL,
            WHISPER_CPP_VULKAN_ZIP_SHA256,
            WHISPER_CPP_VULKAN_ZIP_SIZE,
            true,
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
    let dir = tools_dir(data_dir);
    let archive_name = "whisper-cpu-bin-x64.zip";
    let cpu_command = cpu_command_path(data_dir);
    if cpu_command.is_file() && runtime_is_current(&dir) {
        return Ok(cpu_command);
    }
    install_whisper_cpp_zip(
        client,
        &dir,
        archive_name,
        WHISPER_CPP_CPU_ZIP_URL,
        WHISPER_CPP_CPU_ZIP_SHA256,
        WHISPER_CPP_CPU_ZIP_SIZE,
        false,
        progress,
    )
    .await?;
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeEnvelope {
    schema_version: u32,
    payload_type: String,
    payload_sha256: String,
    payload_size: u64,
    key_id: String,
    signature: String,
}

#[cfg(windows)]
#[derive(Deserialize)]
struct SignedRuntimeManifest {
    schema_version: u32,
    sequence: u64,
    generated_at: String,
    status: String,
    signing_key_id: String,
    runtimes: Vec<SignedRuntime>,
}

#[cfg(windows)]
#[derive(Deserialize)]
struct SignedRuntime {
    id: String,
    version: String,
    platform: String,
    architecture: String,
    backend: String,
    entrypoints: Vec<String>,
    archive: SignedRuntimeArchive,
    source: serde_json::Value,
    build: serde_json::Value,
    licences: Vec<serde_json::Value>,
}

#[cfg(windows)]
#[derive(Deserialize)]
struct SignedRuntimeArchive {
    name: String,
    url: String,
    sha256: String,
    size: u64,
    format: String,
    files: Vec<String>,
}

#[cfg(windows)]
async fn verify_published_runtime(
    client: &Client,
    runtime_id: &str,
    expected_url: &str,
    expected_sha256: &str,
    expected_size: u64,
) -> Result<(), LocalModelsError> {
    let manifest = download_runtime_metadata(client, RUNTIME_MANIFEST_URL).await?;
    let envelope = download_runtime_metadata(client, RUNTIME_ENVELOPE_URL).await?;
    verify_runtime_metadata(
        &manifest,
        &envelope,
        runtime_id,
        expected_url,
        expected_sha256,
        expected_size,
    )
}

#[cfg(windows)]
async fn download_runtime_metadata(
    client: &Client,
    url: &str,
) -> Result<Vec<u8>, LocalModelsError> {
    const MAX_METADATA_BYTES: usize = 1024 * 1024;
    let mut response = client
        .get(url)
        .header(ACCEPT_ENCODING, "identity")
        .send()
        .await
        .map_err(|error| LocalModelsError::Download(format!("runtime metadata: {error}")))?;
    if !response.status().is_success() || !trusted_runtime_response(response.url()) {
        return Err(LocalModelsError::Download(
            "untrusted runtime metadata response".into(),
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_METADATA_BYTES as u64)
    {
        return Err(LocalModelsError::Download(
            "runtime metadata exceeds size limit".into(),
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| LocalModelsError::Download(format!("runtime metadata: {error}")))?
    {
        if bytes.len() + chunk.len() > MAX_METADATA_BYTES {
            return Err(LocalModelsError::Download(
                "runtime metadata exceeds size limit".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(windows)]
fn trusted_runtime_response(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("github.com" | "release-assets.githubusercontent.com")
        )
}

#[cfg(windows)]
fn verify_runtime_metadata(
    manifest_bytes: &[u8],
    envelope_bytes: &[u8],
    runtime_id: &str,
    expected_url: &str,
    expected_sha256: &str,
    expected_size: u64,
) -> Result<(), LocalModelsError> {
    let envelope: RuntimeEnvelope = serde_json::from_slice(envelope_bytes)
        .map_err(|_| LocalModelsError::Download("invalid runtime envelope".into()))?;
    let manifest_hash = format!("{:x}", Sha256::digest(manifest_bytes));
    if envelope.schema_version != 1
        || envelope.payload_type != "application/vnd.makekosmos.runtime-manifest+json"
        || envelope.payload_sha256 != manifest_hash
        || envelope.payload_size != manifest_bytes.len() as u64
        || envelope.key_id != RUNTIME_KEY_ID
    {
        return Err(LocalModelsError::Download(
            "runtime manifest envelope verification failed".into(),
        ));
    }
    let key = STANDARD
        .decode(RUNTIME_PUBLIC_KEY_B64)
        .ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
        .ok_or_else(|| LocalModelsError::Download("invalid runtime trust root".into()))?;
    let signature = STANDARD
        .decode(&envelope.signature)
        .ok()
        .and_then(|bytes| Signature::from_slice(&bytes).ok())
        .ok_or_else(|| LocalModelsError::Download("invalid runtime signature".into()))?;
    key.verify(manifest_bytes, &signature)
        .map_err(|_| LocalModelsError::Download("invalid runtime signature".into()))?;

    let manifest: SignedRuntimeManifest = serde_json::from_slice(manifest_bytes)
        .map_err(|_| LocalModelsError::Download("invalid signed runtime manifest".into()))?;
    let generated_at = DateTime::parse_from_rfc3339(&manifest.generated_at)
        .map_err(|_| LocalModelsError::Download("invalid runtime manifest timestamp".into()))?
        .with_timezone(&Utc);
    let unique_ids: BTreeSet<&str> = manifest
        .runtimes
        .iter()
        .map(|runtime| runtime.id.as_str())
        .collect();
    if manifest.schema_version != 1
        || manifest.sequence != 2
        || manifest.status != "release"
        || manifest.signing_key_id != RUNTIME_KEY_ID
        || generated_at > Utc::now() + ChronoDuration::minutes(5)
        || unique_ids.len() != manifest.runtimes.len()
    {
        return Err(LocalModelsError::Download(
            "invalid signed runtime manifest".into(),
        ));
    }
    let runtime = manifest
        .runtimes
        .iter()
        .find(|runtime| runtime.id == runtime_id)
        .ok_or_else(|| {
            LocalModelsError::Download("runtime is absent from signed manifest".into())
        })?;
    let expected_backend = if runtime_id == "whisper-vulkan" {
        "vulkan"
    } else {
        "cpu"
    };
    if runtime.version != WHISPER_RUNTIME_VERSION
        || runtime.platform != "windows"
        || runtime.architecture != "x64"
        || runtime.backend != expected_backend
        || runtime.archive.url != expected_url
        || runtime.archive.sha256 != expected_sha256
        || runtime.archive.size != expected_size
        || runtime.archive.format != "zip"
        || runtime.archive.name.is_empty()
        || !runtime.source.is_object()
        || !runtime.build.is_object()
        || runtime.licences.is_empty()
        || !runtime
            .entrypoints
            .iter()
            .any(|path| path == "Release/whisper-cli.exe")
        || !runtime
            .archive
            .files
            .iter()
            .any(|path| path == "LICENSE.whisper.cpp.txt")
    {
        return Err(LocalModelsError::Download(
            "signed runtime coordinate mismatch".into(),
        ));
    }
    Ok(())
}

#[cfg(windows)]
async fn install_whisper_cpp_zip(
    client: &Client,
    dir: &Path,
    archive_name: &str,
    url: &str,
    expected_sha256: &str,
    expected_size: u64,
    vulkan: bool,
    progress: &mut ProgressCallback<'_>,
) -> Result<(), LocalModelsError> {
    let runtime_id = if vulkan {
        "whisper-vulkan"
    } else {
        "whisper-cpu"
    };
    verify_published_runtime(client, runtime_id, url, expected_sha256, expected_size).await?;
    let parent = dir
        .parent()
        .ok_or_else(|| LocalModelsError::Download("invalid runtime path".into()))?;
    fs::create_dir_all(parent)?;
    let stem = dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| LocalModelsError::Download("invalid runtime path".into()))?;
    let archive_path = parent.join(format!(".{stem}.{archive_name}.download"));
    let quarantine_archive = parent.join(format!(".{stem}.quarantine.zip"));
    if let Err(error) = download_file(
        client,
        url,
        &archive_path,
        "tool",
        Some(expected_size),
        progress,
    )
    .await
    {
        let partial = archive_path.with_extension("part");
        let _ = fs::remove_file(&quarantine_archive);
        if partial.exists() {
            fs::rename(partial, &quarantine_archive)?;
        }
        return Err(error);
    }
    let integrity = if archive_path.metadata()?.len() != expected_size {
        Err(LocalModelsError::Download(
            "runtime archive size mismatch".into(),
        ))
    } else {
        verify_sha256(&archive_path, expected_sha256)
    };
    if let Err(error) = integrity {
        let _ = fs::remove_file(&quarantine_archive);
        fs::rename(&archive_path, &quarantine_archive)?;
        return Err(error);
    }
    progress(DownloadProgress {
        phase: "extract",
        downloaded_bytes: 0,
        total_bytes: None,
        percent: None,
    });
    let staging = parent.join(format!(".{stem}.staging"));
    let previous = parent.join(format!(".{stem}.previous"));
    let quarantine = parent.join(format!(".{stem}.quarantine"));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir(&staging)?;
    let result = (|| {
        extract_whisper_runtime(&archive_path, &staging, vulkan)?;
        fs::write(
            staging.join(".runtime-version"),
            format!("{WHISPER_RUNTIME_VERSION}\n"),
        )?;
        write_runtime_integrity(&staging, vulkan)?;
        let command = staging.join("Release").join("whisper-cli.exe");
        let status = Command::new(&command).arg("--version").status()?;
        if !status.success() {
            return Err(LocalModelsError::Download(
                "runtime --version smoke test failed".into(),
            ));
        }
        if previous.exists() {
            fs::remove_dir_all(&previous)?;
        }
        write_runtime_transaction(dir)?;
        if dir.exists() {
            fs::rename(dir, &previous)?;
        }
        if let Err(error) = fs::rename(&staging, dir) {
            if previous.exists() {
                let _ = fs::rename(&previous, dir);
            }
            return Err(error.into());
        }
        fs::remove_file(runtime_transaction_path(dir))?;
        Ok(())
    })();
    if result.is_err() {
        if quarantine.exists() {
            fs::remove_dir_all(&quarantine)?;
        }
        if staging.exists() {
            fs::rename(&staging, &quarantine)?;
        }
    }
    let _ = fs::remove_file(&archive_path);
    result
}

#[cfg(windows)]
fn extract_whisper_runtime(
    archive_path: &Path,
    destination: &Path,
    vulkan: bool,
) -> Result<(), LocalModelsError> {
    let mut allowed = BTreeSet::from([
        "LICENSE.whisper.cpp.txt",
        "Release/ggml-base.dll",
        "Release/ggml-cpu.dll",
        "Release/ggml.dll",
        "Release/whisper-cli.exe",
        "Release/whisper-server.exe",
        "Release/whisper.dll",
    ]);
    if vulkan {
        allowed.insert("Release/ggml-vulkan.dll");
    }
    let bytes = fs::read(archive_path)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() != allowed.len() {
        return Err(LocalModelsError::Download(
            "runtime archive contains unexpected entries".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut unpacked = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        let folded = name.to_ascii_lowercase();
        let file_type = entry.unix_mode().unwrap_or(0) & 0o170000;
        unpacked = unpacked.saturating_add(entry.size());
        if !allowed.contains(name.as_str())
            || !seen.insert(folded)
            || name.contains(['\\', ':', '\0'])
            || entry.enclosed_name().is_none()
            || !entry.is_file()
            || !matches!(file_type, 0 | 0o100000)
            || entry.size() > 256 * 1024 * 1024
            || unpacked > 512 * 1024 * 1024
            || (entry.compressed_size() == 0 && entry.size() > 0)
            || (entry.compressed_size() > 0 && entry.size() / entry.compressed_size() > 100)
        {
            return Err(LocalModelsError::Download("unsafe runtime archive".into()));
        }
        let target = destination.join(entry.enclosed_name().expect("checked enclosed path"));
        fs::create_dir_all(target.parent().expect("archive file parent"))?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&target)?;
        io::copy(&mut entry, &mut output)?;
        output.sync_all()?;
        if matches!(
            target.extension().and_then(|value| value.to_str()),
            Some("exe" | "dll")
        ) && !is_x64_pe(&target)?
        {
            return Err(LocalModelsError::Download(
                "runtime archive contains non-x64 executable".into(),
            ));
        }
    }
    if seen.len() != allowed.len() {
        return Err(LocalModelsError::Download(
            "runtime archive is incomplete".into(),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_x64_pe(path: &Path) -> io::Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut header = [0_u8; 64];
    file.read_exact(&mut header)?;
    if &header[..2] != b"MZ" {
        return Ok(false);
    }
    let offset = u32::from_le_bytes(header[60..64].try_into().expect("four bytes")) as u64;
    file.seek(SeekFrom::Start(offset))?;
    let mut pe = [0_u8; 6];
    file.read_exact(&mut pe)?;
    Ok(&pe[..4] == b"PE\0\0" && u16::from_le_bytes([pe[4], pe[5]]) == 0x8664)
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
    fn whisper_cpp_runtime_sources_are_pinned() {
        assert!(WHISPER_CPP_CPU_ZIP_URL
            .contains("github.com/makekosmos/local-ai-runtimes/releases/download/runtime-v1.9.3"));
        assert!(WHISPER_CPP_CPU_ZIP_URL.contains("whisper-cpu-bin-x64-v1.9.3"));
        assert_eq!(WHISPER_CPP_CPU_ZIP_SHA256.len(), 64);
        assert_eq!(WHISPER_CPP_CPU_ZIP_SIZE, 1_436_012);
        assert!(WHISPER_CPP_VULKAN_ZIP_URL
            .contains("github.com/makekosmos/local-ai-runtimes/releases/download/runtime-v1.9.3"));
        assert!(WHISPER_CPP_VULKAN_ZIP_URL.contains("whisper-vulkan-bin-x64-v1.9.3"));
        assert_eq!(WHISPER_CPP_VULKAN_ZIP_SHA256.len(), 64);
        assert_eq!(WHISPER_CPP_VULKAN_ZIP_SIZE, 17_403_912);
    }

    #[cfg(windows)]
    #[test]
    fn runtime_reuse_rejects_tampered_files() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let dir = tools_dir(tmp.path());
        for name in runtime_file_names(false) {
            touch(&dir.join(name));
        }
        fs::write(
            dir.join(".runtime-version"),
            format!("{WHISPER_RUNTIME_VERSION}\n"),
        )
        .expect("version");
        write_runtime_integrity(&dir, false).expect("integrity");
        assert!(runtime_is_current(&dir));
        fs::write(dir.join("Release/whisper-cli.exe"), b"tampered").expect("tamper");
        assert!(!runtime_is_current(&dir));
    }

    #[cfg(windows)]
    #[test]
    fn interrupted_runtime_swap_recovers_previous_install() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let dir = tools_dir(tmp.path());
        let parent = dir.parent().expect("parent");
        let stem = dir.file_name().expect("stem").to_str().expect("stem");
        let previous = parent.join(format!(".{stem}.previous"));
        let staging = parent.join(format!(".{stem}.staging"));
        fs::create_dir_all(previous.join("Release")).expect("previous");
        fs::create_dir_all(&staging).expect("staging");
        write_runtime_transaction(&dir).expect("transaction");
        recover_runtime_transaction(&dir).expect("recovery");
        assert!(dir.join("Release").is_dir());
        assert!(!previous.exists());
        assert!(!staging.exists());
        assert!(!runtime_transaction_path(&dir).exists());
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "requires the immutable production runtime release"]
    async fn signed_runtime_release_installs_atomically() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let client = Client::new();
        install_whisper_cpp_zip(
            &client,
            &tools_dir(tmp.path()),
            "whisper-cpu-bin-x64.zip",
            WHISPER_CPP_CPU_ZIP_URL,
            WHISPER_CPP_CPU_ZIP_SHA256,
            WHISPER_CPP_CPU_ZIP_SIZE,
            false,
            &mut |_| {},
        )
        .await
        .expect("signed CPU runtime install");
        assert!(cpu_command_path(tmp.path()).is_file());
        assert!(runtime_is_current(&tools_dir(tmp.path())));

        let command = ensure_whisper_cpp(&client, tmp.path())
            .await
            .expect("signed Vulkan runtime install");

        assert!(command.is_file());
        assert!(vulkan_server_path(tmp.path()).is_file());
        assert!(runtime_is_current(&vulkan_tools_dir(tmp.path())));
    }

    #[cfg(windows)]
    #[test]
    fn runtime_extractor_rejects_traversal_before_writing() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let archive_path = tmp.path().join("runtime.zip");
        let file = fs::File::create(&archive_path).expect("archive");
        let mut archive = zip::ZipWriter::new(file);
        for name in [
            "../LICENSE.whisper.cpp.txt",
            "Release/ggml-base.dll",
            "Release/ggml-cpu.dll",
            "Release/ggml.dll",
            "Release/whisper-cli.exe",
            "Release/whisper-server.exe",
            "Release/whisper.dll",
        ] {
            archive
                .start_file(name, zip::write::FileOptions::default())
                .expect("entry");
            archive.write_all(b"x").expect("body");
        }
        archive.finish().expect("finish");
        let destination = tmp.path().join("staging");
        fs::create_dir(&destination).expect("staging");

        let error = extract_whisper_runtime(&archive_path, &destination, false)
            .expect_err("traversal must fail");

        assert!(error.to_string().contains("unsafe runtime archive"));
        assert!(!tmp.path().join("LICENSE.whisper.cpp.txt").exists());
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
            None,
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
            None,
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
    async fn download_file_enforces_hard_size_limit() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/oversized.bin");
                then.status(200)
                    .header("content-type", "application/octet-stream")
                    .body(vec![7_u8; 17]);
            })
            .await;
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("oversized.bin");

        let error = download_file(
            &Client::new(),
            &server.url("/oversized.bin"),
            &path,
            "model",
            Some(16),
            &mut |_| {},
        )
        .await
        .expect_err("oversized body must fail");

        assert!(error.to_string().contains("size limit"));
        assert!(!path.exists());
        assert!(!path.with_extension("part").exists());
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
            None,
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
    fn extract_tar_gz_installs_directory_model() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let archive_path = tmp.path().join("model.tar.gz");
        let tar_gz = fs::File::create(&archive_path).expect("archive");
        let encoder = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        let body = b"config";
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(&mut header, "nested/config.json", &body[..])
            .expect("append tar");
        archive
            .into_inner()
            .expect("finish tar")
            .finish()
            .expect("gzip");

        let destination = tmp.path().join("parakeet-tdt-0.6b-v3-int8");
        extract_tar_gz(&archive_path, &destination).expect("extract");

        assert_eq!(
            fs::read(destination.join("config.json")).expect("extracted file"),
            body
        );
        assert!(!destination.with_extension("extracting").exists());
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
