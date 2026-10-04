//! Install-location policy for the privileged service binary.
//!
//! The service is a copy of the Engine exe placed under
//! `%ProgramFiles%\<Brand>\Service` — an admin-only-writable location.
//! A binary under `%LOCALAPPDATA%`/other user-writable roots would be a
//! privilege-escalation vector (any user-mode process could swap the binary
//! SYSTEM runs), so the policy below rejects them outright.

#![cfg(windows)]

use std::path::{Path, PathBuf};

use crate::privileged::brand;

/// Directory the service binary copy lives in. Pure for tests — `getenv`
/// resolves `ProgramFiles`, `ProgramFiles(x86)`, and the user-writable roots
/// checked by `is_user_writable_location`.
pub fn install_dir(getenv: &dyn Fn(&str) -> Option<String>) -> Result<PathBuf, String> {
    let program_files = getenv("ProgramFiles")
        .or_else(|| getenv("ProgramFiles(x86)"))
        .ok_or_else(|| "ProgramFiles is not defined".to_string())?;
    let dir = Path::new(&program_files)
        .join(brand::PRODUCT_NAME)
        .join("Service");
    if is_user_writable_location(&dir, getenv) {
        return Err(format!(
            "refusing a service binary path under a user-writable root: {}",
            dir.display()
        ));
    }
    Ok(dir)
}

/// True when `path` sits under a root any unprivileged process can write to:
/// user profile dirs, temp dirs. Environment roots that are empty or unset
/// are skipped.
pub fn is_user_writable_location(path: &Path, getenv: &dyn Fn(&str) -> Option<String>) -> bool {
    let normalized = normalize_path(path);
    [
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "TEMP",
        "TMP",
        "PUBLIC",
    ]
    .iter()
    .filter_map(|key| getenv(key))
    .filter(|root| !root.trim().is_empty())
    .any(|root| {
        let root = normalize_path(Path::new(&root));
        // Component-boundary compare: `C:\Users\kirill` must not match
        // `C:\Users\kirill2\...`.
        normalized == root || normalized.starts_with(&format!("{root}\\"))
    })
}

fn normalize_path(path: &Path) -> String {
    let mut s = path.to_string_lossy().replace('/', "\\");
    while s.ends_with('\\') && s.len() > 3 {
        s.pop();
    }
    s.to_lowercase()
}

pub fn env_get(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Mark `path` for deletion on next boot (MOVEFILE_DELAY_UNTIL_REBOOT) —
/// used when the uninstaller itself is the binary being removed.
pub fn delete_on_reboot(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let _ = MoveFileExW(
            windows::core::PCWSTR(wide.as_ptr()),
            windows::core::PCWSTR::null(),
            MOVEFILE_DELAY_UNTIL_REBOOT,
        );
    }
}

#[cfg(test)]
#[path = "install_path_tests.rs"]
mod tests;
