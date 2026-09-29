use super::{path_contains_noisy_folder, IndexedFile};
use ntfs_reader::file_info::{FileInfo, VecCache};
use ntfs_reader::mft::Mft;
use ntfs_reader::volume::Volume;
use std::path::{Path, PathBuf};

use crate::privileged::brand;
use crate::privileged::ntfs_scan::NtfsScanEntry;
use crate::privileged::pipe;
use crate::privileged::protocol::Request;

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

/// Service fast path: primary pipe first, then the legacy pipe names so a
/// not-yet-migrated service still works. The versioned request wire format
/// parses identically on older services (unknown fields are ignored).
// MIGRATION(KOS-267): drop the legacy pipe names after 2026-11-01.
fn scan_via_service(root: &Path, exclude_noisy: bool) -> Result<Vec<IndexedFile>, String> {
    let req = Request::NtfsScan {
        root: root.to_string_lossy().into_owned(),
        exclude_noisy,
    };
    let mut errors = Vec::new();
    let pipe_names =
        std::iter::once(brand::PIPE_NAME).chain(brand::LEGACY_PIPES.iter().map(|(name, _)| *name));
    for pipe_name in pipe_names {
        match pipe::request_on(pipe_name, &req) {
            Ok(resp) => {
                if !resp.ok {
                    errors.push(format!("{pipe_name}: {}", resp.error.unwrap_or_default()));
                    continue;
                }
                let files = resp
                    .files
                    .ok_or_else(|| "service response missing files".to_string())?;
                return Ok(files.into_iter().map(wire_to_indexed).collect());
            }
            Err(e) => errors.push(format!("{pipe_name}: {e}")),
        }
    }
    Err(errors.join("; "))
}

fn wire_to_indexed(file: NtfsScanEntry) -> IndexedFile {
    IndexedFile {
        path: file.path,
        name: file.name,
        mtime: file.mtime,
    }
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
