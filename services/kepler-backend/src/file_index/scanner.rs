use super::{FileIndexError, IndexedFile, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::{DirEntry, WalkDir};

#[cfg(windows)]
mod ntfs;

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

pub fn scan_roots(roots: &[PathBuf], exclude_noisy: bool) -> Vec<IndexedFile> {
    let mut files = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        files.extend(scan_root(root, exclude_noisy));
    }
    files
}

fn scan_root(root: &Path, exclude_noisy: bool) -> Vec<IndexedFile> {
    #[cfg(windows)]
    if is_drive_root(root) {
        match ntfs::scan_drive_root(root, exclude_noisy) {
            Ok(files) => {
                tracing::info!(
                    target: "file_index",
                    root = %root.to_string_lossy(),
                    total = files.len(),
                    "ntfs fast scan finished"
                );
                return files;
            }
            Err(error) => {
                tracing::warn!(
                    target: "file_index",
                    root = %root.to_string_lossy(),
                    error,
                    "ntfs fast scan unavailable; using walk fallback"
                );
            }
        }
    }
    scan_walk_root(root, exclude_noisy)
}

fn scan_walk_root(root: &Path, exclude_noisy: bool) -> Vec<IndexedFile> {
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| should_enter(entry, exclude_noisy));
    let mut files = Vec::new();
    for entry in walker.filter_map(|entry| entry.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.is_empty() {
            continue;
        }
        let mtime = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|mtime| mtime.as_secs() as i64)
            .unwrap_or_default();
        files.push(IndexedFile {
            path: entry.path().to_string_lossy().into_owned(),
            name,
            mtime,
        });
    }
    files
}

#[cfg(windows)]
fn is_drive_root(root: &Path) -> bool {
    let raw = root.to_string_lossy();
    raw.len() == 3
        && raw.as_bytes()[1] == b':'
        && matches!(raw.as_bytes()[2], b'\\' | b'/')
        && raw.as_bytes()[0].is_ascii_alphabetic()
}

fn should_enter(entry: &DirEntry, exclude_noisy: bool) -> bool {
    if entry.depth() == 0 || !exclude_noisy || !entry.file_type().is_dir() {
        return true;
    }
    !is_noisy_folder(entry.path())
}

fn is_noisy_folder(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    NOISY_FOLDER_NAMES
        .iter()
        .any(|candidate| name == *candidate)
}

pub fn path_contains_noisy_folder(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy().to_lowercase();
        NOISY_FOLDER_NAMES
            .iter()
            .any(|candidate| name == *candidate)
    })
}

pub fn default_roots() -> Vec<PathBuf> {
    if let Ok(roots) = std::env::var("KEPLER_FILE_INDEX_ROOTS") {
        return roots
            .split(';')
            .map(str::trim)
            .filter(|root| !root.is_empty())
            .map(PathBuf::from)
            .collect();
    }
    if std::env::var("KOSMOS_TEST_MODE").as_deref() == Ok("1") {
        return Vec::new();
    }
    platform_fixed_drive_roots()
}

#[cfg(windows)]
fn platform_fixed_drive_roots() -> Vec<PathBuf> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    use windows::Win32::System::WindowsProgramming::DRIVE_FIXED;

    let mut out = Vec::new();
    let drives = unsafe { GetLogicalDrives() };
    for offset in 0..26u32 {
        if drives & (1 << offset) == 0 {
            continue;
        }
        let letter = char::from_u32('A' as u32 + offset).unwrap_or('C');
        let root = format!("{letter}:\\");
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        if unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) } == DRIVE_FIXED {
            out.push(PathBuf::from(root));
        }
    }
    out
}

#[cfg(not(windows))]
fn platform_fixed_drive_roots() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(windows)]
pub fn open_file(path: &str) -> Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "", path])
        .spawn()
        .map(|_| ())
        .map_err(|e| FileIndexError::Open(e.to_string()))
}

#[cfg(not(windows))]
pub fn open_file(path: &str) -> Result<()> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| FileIndexError::Open(e.to_string()))
}
