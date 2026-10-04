//! Shared NTFS MFT-scan plumbing (KOS-335).
//!
//! `file_index::scanner::ntfs` (user-mode fast path) and
//! `privileged::ntfs_scan` (service-side scan) used to carry ~12 identical
//! helpers; they live here once. Both sides keep their own entry type
//! (`IndexedFile` / `NtfsScanEntry`) and map from [`MftEntry`].

use ntfs_reader::file_info::{FileInfo, VecCache};
use ntfs_reader::mft::Mft;
use std::path::{Path, PathBuf};

use crate::file_index::scanner::path_contains_noisy_folder;

/// One file row read from the MFT — the common shape `IndexedFile` and
/// `NtfsScanEntry` are both mapped from.
pub(crate) struct MftEntry {
    pub path: String,
    pub name: String,
    pub mtime: i64,
}

pub(crate) fn volume_path(drive: char) -> String {
    format!(r"\\.\{}:", drive)
}

/// MFT → flat file list. `exclude_noisy` applies the same
/// `path_contains_noisy_folder` pre-filter on both the user-mode and the
/// service side so results match exactly.
pub(crate) fn mft_to_entries(mft: &Mft, drive: char, exclude_noisy: bool) -> Vec<MftEntry> {
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

        out.push(MftEntry {
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

pub(crate) fn user_path(path: &Path, drive: char) -> PathBuf {
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
