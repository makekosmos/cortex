//! Fast whole-drive file listing via the NTFS MFT (`ntfs-reader` crate).
//!
//! Runs inside the privileged service as LocalSystem — raw volume access is
//! an admin capability. Two guards make it safe to expose over the pipe:
//!
//! * the request `root` is constrained to a drive root (`C:\`, `D:`...) —
//!   no arbitrary paths;
//! * results are filtered by `retain_visible_to` so file names under *other*
//!   users' profile directories are never returned to the caller.

#![cfg(windows)]

use ntfs_reader::file_info::{FileInfo, VecCache};
use ntfs_reader::mft::Mft;
use ntfs_reader::volume::Volume;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

use crate::file_index::scanner::path_contains_noisy_folder;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NtfsScanEntry {
    pub path: String,
    pub name: String,
    pub mtime: i64,
}

/// The only roots a client may request: a bare drive root (`C:`, `C:\`,
/// `C:/`). Returns the normalized `X:\` form.
pub fn validate_scan_root(root: &str) -> Result<String, String> {
    let trimmed = root.trim();
    let bytes = trimmed.as_bytes();
    let drive = bytes
        .first()
        .copied()
        .filter(|b| b.is_ascii_alphabetic())
        .map(|b| (b as char).to_ascii_uppercase())
        .ok_or_else(|| format!("invalid scan root: {root:?}"))?;
    // The root is exactly `<letter>:` plus an optional single separator.
    let rest = &trimmed[1..];
    if rest == ":" || rest == ":\\" || rest == ":/" {
        return Ok(format!("{drive}:\\"));
    }
    Err(format!(
        "scan root must be a drive root (e.g. C:\\), got {root:?}"
    ))
}

pub fn scan_drive_root(root: &str, exclude_noisy: bool) -> Result<Vec<NtfsScanEntry>, String> {
    let root = validate_scan_root(root)?;
    let drive = root.chars().next().unwrap_or('C');
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

/// Drop entries under `<profiles_root>\<other-user>` — paths beneath the
/// profiles directory (e.g. `C:\Users`) that are not the caller's own
/// `profile_dir`. `Public` stays visible; everything else under the profiles
/// root that is not the caller's home is other users' data and is never
/// returned.
pub fn retain_visible_to(files: Vec<NtfsScanEntry>, profile_dir: &Path) -> Vec<NtfsScanEntry> {
    let Some(profiles_root) = profile_dir.parent() else {
        return files;
    };
    let public = profiles_root.join("Public");
    files
        .into_iter()
        .filter(|entry| {
            let path = Path::new(&entry.path);
            path_starts_with_ci(path, profile_dir)
                || path_starts_with_ci(path, &public)
                || !path_starts_with_ci(path, profiles_root)
        })
        .collect()
}

/// Case-insensitive `Path::starts_with` — NTFS paths and profile dirs may
/// differ in casing.
fn path_starts_with_ci(path: &Path, prefix: &Path) -> bool {
    let mut path_components = path.components();
    for want in prefix.components() {
        match path_components.next() {
            Some(got) => {
                let got = component_text(&got);
                let want = component_text(&want);
                if !got.eq_ignore_ascii_case(&want) {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

fn component_text(component: &Component<'_>) -> String {
    component.as_os_str().to_string_lossy().into_owned()
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

/// `ntfs_scan` pipe-request handler with caller gating (moved out of
/// `pipe_server` — this is request logic, not accept-loop logic):
/// 1. impersonate the client and prove it can enumerate the requested drive
///    root itself;
/// 2. scan as SYSTEM;
/// 3. drop entries under other users' profile directories.
pub fn handle_pipe_request(
    pipe: windows::Win32::Foundation::HANDLE,
    root: &str,
    exclude_noisy: bool,
) -> crate::privileged::protocol::Response {
    use crate::privileged::impersonate;
    use crate::privileged::protocol::Response;

    let root = match validate_scan_root(root) {
        Ok(r) => r,
        Err(e) => return Response::err(e),
    };
    let profile = match impersonate::client_profile_dir(pipe) {
        Ok(p) => p,
        Err(e) => return Response::err(e),
    };

    // Access check under the client's identity: it must be able to enumerate
    // the requested root on its own.
    let readable =
        impersonate::with_client_impersonation(pipe, || std::fs::read_dir(&root).is_ok());
    if !readable {
        return Response::err(format!("scan root {root} is not readable by the caller"));
    }

    match scan_drive_root(&root, exclude_noisy) {
        Ok(files) => Response::files(retain_visible_to(files, &profile)),
        Err(e) => Response::err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> NtfsScanEntry {
        NtfsScanEntry {
            path: path.to_string(),
            name: path.rsplit('\\').next().unwrap_or_default().to_string(),
            mtime: 0,
        }
    }

    #[test]
    fn converts_nt_device_paths_to_user_paths() {
        assert_eq!(
            user_path(Path::new(r"\\.\C:\Users\Kirill\note.md"), 'C'),
            PathBuf::from(r"C:\Users\Kirill\note.md")
        );
        assert_eq!(
            user_path(Path::new(r"\\?\D:\Projects\app\README.md"), 'D'),
            PathBuf::from(r"D:\Projects\app\README.md")
        );
    }

    #[test]
    fn only_drive_roots_are_scannable() {
        assert_eq!(validate_scan_root("C:\\").unwrap(), "C:\\");
        assert_eq!(validate_scan_root("d:").unwrap(), "D:\\");
        assert_eq!(validate_scan_root("e:/").unwrap(), "E:\\");
        for bad in [
            "",
            "C:\\Users",
            "C:\\Windows\\Temp",
            "\\\\nas\\share",
            "/etc",
            "C:x",
        ] {
            assert!(validate_scan_root(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn other_users_files_are_never_returned() {
        let profile = Path::new(r"C:\Users\Kirill");
        let files = vec![
            entry(r"C:\Users\Kirill\Documents\note.md"),
            entry(r"C:\Users\kirill\Downloads\pic.png"), // case-insensitive own profile
            entry(r"C:\Users\Public\shared.txt"),
            entry(r"C:\Users\OtherUser\secret.docx"),
            entry(r"C:\Projects\code\main.rs"),
            entry(r"D:\Data\file.bin"), // other drive untouched by the filter
        ];
        let kept: Vec<String> = retain_visible_to(files, profile)
            .into_iter()
            .map(|e| e.path)
            .collect();
        assert!(kept.contains(&r"C:\Users\Kirill\Documents\note.md".to_string()));
        assert!(kept.contains(&r"C:\Users\kirill\Downloads\pic.png".to_string()));
        assert!(kept.contains(&r"C:\Users\Public\shared.txt".to_string()));
        assert!(kept.contains(&r"C:\Projects\code\main.rs".to_string()));
        assert!(kept.contains(&r"D:\Data\file.bin".to_string()));
        assert!(!kept.iter().any(|p| p.contains("OtherUser")));
    }
}
