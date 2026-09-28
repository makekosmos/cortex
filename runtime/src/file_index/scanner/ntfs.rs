use super::{path_contains_noisy_folder, IndexedFile};
use ntfs_reader::file_info::{FileInfo, VecCache};
use ntfs_reader::mft::Mft;
use ntfs_reader::volume::Volume;
use serde::Deserialize;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const SYSTEM_SERVICE_PIPE: &str = crate::brand::SYSTEM_SERVICE_PIPE;
// MIGRATION(KOS-267): remove after 2026-11-01. Pipes listened on by system
// services installed by older product generations.
const LEGACY_PIPES: [&str; 2] = [
    r"\\.\pipe\kosmos-system-service", // MIGRATION(KOS-267)
    r"\\.\pipe\kepler-focus-svc",      // MIGRATION(KOS-267)
];

pub fn scan_drive_root(root: &Path, exclude_noisy: bool) -> Result<Vec<IndexedFile>, String> {
    let drive = drive_letter(root)?;
    match scan_via_service(root, exclude_noisy) {
        Ok(files) => return Ok(files),
        Err(error) => {
            tracing::warn!(
                target: "file_index",
                root = %root.to_string_lossy(),
                error,
                "ntfs service scan unavailable; trying local fast scan"
            );
        }
    }
    let volume =
        Volume::new(volume_path(drive)).map_err(|e| format!("open NTFS volume failed: {e}"))?;
    let mft = Mft::new(volume).map_err(|e| format!("read NTFS MFT failed: {e}"))?;
    Ok(mft_to_files(&mft, drive, exclude_noisy))
}

#[derive(Debug, Deserialize)]
struct ServiceResponse {
    ok: bool,
    error: Option<String>,
    files: Option<Vec<IndexedFileWire>>,
}

#[derive(Debug, Deserialize)]
struct IndexedFileWire {
    path: String,
    name: String,
    mtime: i64,
}

fn scan_via_service(root: &Path, exclude_noisy: bool) -> Result<Vec<IndexedFile>, String> {
    match scan_via_service_pipe(SYSTEM_SERVICE_PIPE, root, exclude_noisy) {
        Ok(files) => return Ok(files),
        Err(error) => tracing::debug!(
            target: "file_index",
            error,
            "primary service pipe unavailable; trying legacy pipes"
        ),
    }
    let mut last_error = String::from("no legacy pipes configured");
    for pipe in LEGACY_PIPES {
        match scan_via_service_pipe(pipe, root, exclude_noisy) {
            Ok(files) => return Ok(files),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

fn scan_via_service_pipe(
    pipe_path: &str,
    root: &Path,
    exclude_noisy: bool,
) -> Result<Vec<IndexedFile>, String> {
    let mut pipe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_path)
        .map_err(|e| format!("connect service pipe failed: {e}"))?;

    let request = serde_json::json!({
        "op": "ntfs_scan",
        "root": root.to_string_lossy(),
        "exclude_noisy": exclude_noisy,
    });
    writeln!(pipe, "{request}").map_err(|e| format!("write service request failed: {e}"))?;
    pipe.flush()
        .map_err(|e| format!("flush service request failed: {e}"))?;

    // Regression L4 (2026-05-24): single-line read_line was brittle — service
    // could legitimately return multi-line JSON or a payload bigger than the
    // BufReader's line buffer. Slurp the whole pipe until EOF — but bound it:
    // the pipe name is well known and the server is not authenticated, so a
    // squatter (anything holding the name while the real service is down)
    // could stream a never-ending response and grow Engine memory without
    // limit. 1 GiB exceeds a legitimate whole-drive MFT listing by a wide
    // margin (millions of entries at ~150 B each).
    const MAX_SERVICE_RESPONSE_BYTES: u64 = 1 << 30;
    let mut raw = String::new();
    std::io::Read::take(&pipe, MAX_SERVICE_RESPONSE_BYTES + 1)
        .read_to_string(&mut raw)
        .map_err(|e| format!("read service response failed: {e}"))?;
    if raw.len() as u64 > MAX_SERVICE_RESPONSE_BYTES {
        return Err("service response exceeds 1 GiB".to_string());
    }
    let response: ServiceResponse = serde_json::from_str(raw.trim())
        .map_err(|e| format!("parse service response failed: {e}"))?;
    if !response.ok {
        return Err(response
            .error
            .unwrap_or_else(|| "service returned error".to_string()));
    }
    let files = response
        .files
        .ok_or_else(|| "service response missing files".to_string())?;
    Ok(files
        .into_iter()
        .map(|file| IndexedFile {
            path: file.path,
            name: file.name,
            mtime: file.mtime,
        })
        .collect())
}

fn mft_to_files(mft: &Mft, drive: char, exclude_noisy: bool) -> Vec<IndexedFile> {
    let mut cache = VecCache::default();
    let mut out = Vec::new();

    for file in mft.files() {
        let info = FileInfo::with_cache(mft, &file, &mut cache);
        if info.is_directory || info.name.is_empty() || info.path.as_os_str().is_empty() {
            continue;
        }

        let path = user_path(&info.path, drive);
        if exclude_noisy && path_contains_noisy_folder(&path) {
            continue;
        }

        out.push(IndexedFile {
            path: path.to_string_lossy().into_owned(),
            name: info.name,
            mtime: info
                .modified
                .map(|modified| modified.unix_timestamp())
                .unwrap_or_default(),
        });
    }

    out
}

fn drive_letter(root: &Path) -> Result<char, String> {
    let raw = root.to_string_lossy();
    raw.chars()
        .next()
        .filter(|letter| letter.is_ascii_alphabetic())
        .map(|letter| letter.to_ascii_uppercase())
        .ok_or_else(|| format!("invalid drive root: {}", root.to_string_lossy()))
}

fn volume_path(drive: char) -> String {
    format!(r"\\.\{}:", drive)
}

fn user_path(path: &Path, drive: char) -> PathBuf {
    let raw = path.to_string_lossy();
    let device_prefix = volume_path(drive);
    let verbatim_prefix = format!(r"\\?\{}:", drive);
    if let Some(user) = strip_volume_prefix(&raw, &device_prefix, drive) {
        return PathBuf::from(user);
    }
    if let Some(user) = strip_volume_prefix(&raw, &verbatim_prefix, drive) {
        return PathBuf::from(user);
    }
    path.to_path_buf()
}

fn strip_volume_prefix(raw: &str, prefix: &str, drive: char) -> Option<String> {
    if raw.len() < prefix.len() || !raw[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }
    let suffix = &raw[prefix.len()..];
    if suffix.is_empty() {
        return Some(format!("{drive}:\\"));
    }
    if suffix.starts_with(['\\', '/']) {
        return Some(format!("{drive}:{suffix}"));
    }
    Some(format!(r"{drive}:\{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_nt_device_paths_to_user_paths() {
        assert_eq!(
            user_path(Path::new(r"\\.\C:\Users\Kirill\note.md"), 'C'),
            PathBuf::from(r"C:\Users\Kirill\note.md")
        );
        assert_eq!(
            user_path(Path::new(r"\\?\D:\Projects\mundus\README.md"), 'D'),
            PathBuf::from(r"D:\Projects\mundus\README.md")
        );
    }
}
