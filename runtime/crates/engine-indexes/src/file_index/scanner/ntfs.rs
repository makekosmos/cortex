use super::IndexedFile;
use ntfs_reader::mft::Mft;
use ntfs_reader::volume::Volume;
use std::path::Path;

use crate::ntfs_common::{mft_to_entries, volume_path};
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
    Ok(mft_to_entries(&mft, drive, exclude_noisy)
        .into_iter()
        .map(|entry| IndexedFile {
            path: entry.path,
            name: entry.name,
            mtime: entry.mtime,
        })
        .collect())
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

fn drive_letter(root: &Path) -> Result<char, String> {
    let raw = root.to_string_lossy();
    raw.chars()
        .next()
        .filter(|letter| letter.is_ascii_alphabetic())
        .map(|letter| letter.to_ascii_uppercase())
        .ok_or_else(|| format!("invalid drive root: {}", root.to_string_lossy()))
}
