//! Verified install of a pinned whisper.cpp runtime zip: download with a
//! hard size limit, verify size + sha256 via the shared `file_hash` helper,
//! safe-extract into staging, smoke-test, then atomically swap into place.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use std::process::Command;

use reqwest::Client;

use super::super::download::{download_file, DownloadProgress, ProgressCallback};
use super::super::{verify_error, LocalModelsError};
use super::{
    runtime_transaction_path, write_runtime_integrity, write_runtime_transaction,
    WHISPER_RUNTIME_VERSION,
};

pub(super) async fn install_whisper_cpp_zip(
    client: &Client,
    dir: &Path,
    archive_name: &str,
    url: &str,
    expected_sha256: &str,
    expected_size: u64,
    vulkan: bool,
    progress: &mut ProgressCallback<'_>,
) -> Result<(), LocalModelsError> {
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
    let integrity =
        crate::file_hash::verify_size_and_sha256(&archive_path, expected_size, expected_sha256)
            .map_err(|error| verify_error(&archive_path, error));
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

pub(super) fn extract_whisper_runtime(
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
