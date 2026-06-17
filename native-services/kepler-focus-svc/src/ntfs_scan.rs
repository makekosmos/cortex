#![cfg(windows)]

use ntfs_reader::file_info::{FileInfo, VecCache};
use ntfs_reader::mft::Mft;
use ntfs_reader::volume::Volume;
use serde::Serialize;
use std::path::{Path, PathBuf};

const NOISY_FOLDER_NAMES: &[&str] = &[
    "$recycle.bin",
    ".cache",
    ".bun_cache",
    ".e2e",
    ".git",
    ".gradle",
    ".next",
    ".nuxt",
    ".parcel-cache",
    ".pnpm-store",
    ".tmp",
    ".turbo",
    ".venv",
    ".vite",
    "__pycache__",
    "build",
    "cache",
    "coverage",
    "dist",
    "node_modules",
    "out",
    "target",
    "tmp",
    "venv",
];

#[derive(Debug, Serialize)]
pub struct NtfsScanEntry {
    pub path: String,
    pub name: String,
    pub mtime: i64,
}

pub fn scan_drive_root(root: &str, exclude_noisy: bool) -> Result<Vec<NtfsScanEntry>, String> {
    let drive = drive_letter(root)?;
    let volume =
        Volume::new(volume_path(drive)).map_err(|e| format!("open NTFS volume failed: {e}"))?;
    let mft = Mft::new(volume).map_err(|e| format!("read NTFS MFT failed: {e}"))?;
    Ok(mft_to_files(&mft, drive, exclude_noisy))
}

fn mft_to_files(mft: &Mft, drive: char, exclude_noisy: bool) -> Vec<NtfsScanEntry> {
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

        out.push(NtfsScanEntry {
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

fn drive_letter(root: &str) -> Result<char, String> {
    root.chars()
        .next()
        .filter(|letter| letter.is_ascii_alphabetic())
        .map(|letter| letter.to_ascii_uppercase())
        .ok_or_else(|| format!("invalid drive root: {root}"))
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

fn path_contains_noisy_folder(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy().to_lowercase();
        NOISY_FOLDER_NAMES
            .iter()
            .any(|candidate| name == *candidate)
    })
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
            user_path(Path::new(r"\\?\D:\Projects\kosmos\README.md"), 'D'),
            PathBuf::from(r"D:\Projects\kosmos\README.md")
        );
    }
}
