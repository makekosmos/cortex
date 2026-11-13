use super::{FileIndexError, Result};
use std::path::PathBuf;
#[cfg(not(windows))]
use std::process::Command;

pub fn default_roots() -> Vec<PathBuf> {
    if let Ok(roots) = std::env::var("MUNDUS_FILE_INDEX_ROOTS") {
        return roots
            .split(';')
            .map(str::trim)
            .filter(|root| !root.is_empty())
            .map(PathBuf::from)
            .collect();
    }
    if std::env::var("MUNDUS_TEST_MODE").as_deref() == Ok("1") {
        return Vec::new();
    }
    // См. postmortems.md § 2026-06-08: broad profile scans are opt-in only.
    Vec::new()
}

#[cfg(windows)]
pub fn open_file(path: &str) -> Result<()> {
    crate::app_index::platform::windows::shell_execute_open(path)
        .map_err(|e| FileIndexError::Open(e.to_string()))
}

#[cfg(not(windows))]
pub fn open_file(path: &str) -> Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| FileIndexError::Open(e.to_string()))
}
