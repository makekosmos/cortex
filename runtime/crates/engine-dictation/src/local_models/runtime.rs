//! whisper.cpp runtime install stack (Windows only): pinned release zips
//! from `makekosmos/engine-addons`, transactional staging/commit, and a
//! per-file integrity record that detects tampering between installs.
//!
//! Trust model: the archive URL, byte size and sha256 are compile-time
//! constants inside the released Engine binary — the runtime manifest +
//! Ed25519 envelope used to re-check exactly those same values at download
//! time and was removed in KOS-321. HTTPS + GitHub Releases + the pinned
//! hash (the same model the self-updater and native apps already use) is
//! the trust anchor now.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::download::ProgressCallback;
use super::{path_string, same_path_or_text, shared_assets_root, tools_dir, LocalModelsError};
use crate::config;

mod install;
use install::install_whisper_cpp_zip;

pub(super) const VULKAN_TOOLS_DIR: &str = "tools/dictation/whisper.cpp-vulkan";
const CREATE_NO_WINDOW: u32 = 0x08000000;
const WHISPER_RUNTIME_VERSION: &str = "1.9.3";

const WHISPER_CPP_CPU_ZIP_URL: &str = concat!(
    "https://github.com/makekosmos/engine-addons/releases/download/runtime-v1",
    ".9.3/whisper-cpu-bin-x64-v1.9.3.zip"
);
const WHISPER_CPP_CPU_ZIP_SHA256: &str =
    "2464c8ecdc070ccdba079b363943e979708443180fd6dda12d7d0801beeb5954";
const WHISPER_CPP_CPU_ZIP_SIZE: u64 = 1_436_012;
const WHISPER_CPP_VULKAN_ZIP_URL: &str = concat!(
    "https://github.com/makekosmos/engine-addons/releases/download/runtime-v1",
    ".9.3/whisper-vulkan-bin-x64-v1.9.3.zip"
);
const WHISPER_CPP_VULKAN_ZIP_SHA256: &str =
    "520ab6225f6b0afd2e8dcbc67e196a934df44bc7932f86cc2c29796a996c48f4";
const WHISPER_CPP_VULKAN_ZIP_SIZE: u64 = 17_403_912;

pub(super) fn vulkan_tools_dir(data_dir: &Path) -> PathBuf {
    shared_assets_root(data_dir).join(VULKAN_TOOLS_DIR)
}

pub(super) fn cpu_command_path(data_dir: &Path) -> PathBuf {
    tools_dir(data_dir).join("Release").join("whisper-cli.exe")
}

pub(super) fn vulkan_command_path(data_dir: &Path) -> PathBuf {
    vulkan_tools_dir(data_dir)
        .join("Release")
        .join("whisper-cli.exe")
}

fn vulkan_server_path(data_dir: &Path) -> PathBuf {
    vulkan_tools_dir(data_dir)
        .join("Release")
        .join("whisper-server.exe")
}

pub(super) fn preferred_command_path(data_dir: &Path) -> PathBuf {
    let vulkan = vulkan_command_path(data_dir);
    if vulkan_tools_enabled() && vulkan.is_file() {
        vulkan
    } else {
        cpu_command_path(data_dir)
    }
}

fn managed_command_paths(data_dir: &Path) -> [PathBuf; 2] {
    [cpu_command_path(data_dir), vulkan_command_path(data_dir)]
}

pub(super) fn vulkan_tools_enabled() -> bool {
    if std::env::var("MUNDUS_DICTATION_DISABLE_VULKAN").as_deref() == Ok("1") {
        return false;
    }
    vulkan_runtime_available()
}

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

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIntegrityRecord {
    version: String,
    files: std::collections::BTreeMap<String, String>,
}

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

fn runtime_integrity_path(dir: &Path) -> PathBuf {
    dir.join(".runtime-integrity.json")
}

fn runtime_transaction_path(dir: &Path) -> PathBuf {
    dir.parent().unwrap_or_else(|| Path::new(".")).join(format!(
        ".{}.transaction",
        dir.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("runtime")
    ))
}

fn write_runtime_integrity(dir: &Path, vulkan: bool) -> Result<(), LocalModelsError> {
    let mut files = std::collections::BTreeMap::new();
    for name in runtime_file_names(vulkan) {
        let path = dir.join(name);
        files.insert(name.to_owned(), crate::file_hash::file_sha256(&path)?);
    }
    let bytes = serde_json::to_vec(&RuntimeIntegrityRecord {
        version: WHISPER_RUNTIME_VERSION.to_owned(),
        files,
    })
    .map_err(|error| LocalModelsError::Download(format!("runtime integrity: {error}")))?;
    fs::write(runtime_integrity_path(dir), bytes)?;
    Ok(())
}

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
                && record.files.get(*name).is_some_and(|hash| {
                    crate::file_hash::file_sha256(&path).is_ok_and(|actual| actual == *hash)
                })
        })
}

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

pub(super) fn recover_runtime_transaction(dir: &Path) -> Result<(), LocalModelsError> {
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
pub fn vulkan_runtime_needs_server_repair(data_dir: &Path) -> bool {
    vulkan_tools_enabled()
        && vulkan_command_path(data_dir).is_file()
        && !vulkan_server_path(data_dir).is_file()
}

pub fn refresh_managed_command_path(data_dir: &Path, cfg: &mut config::DictationConfig) -> bool {
    let Some(command_path) = super::command_path(data_dir) else {
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

pub async fn ensure_whisper_cpp(
    client: &Client,
    data_dir: &Path,
) -> Result<PathBuf, LocalModelsError> {
    ensure_whisper_cpp_with_progress(client, data_dir, &mut |_| {}).await
}

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

#[cfg(test)]
mod tests;
