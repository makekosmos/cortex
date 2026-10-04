//! Shared Win32 API helpers used by more than one module.

use std::path::{Path, PathBuf};

/// `SHGetKnownFolderPath` for a current-user known folder
/// (`FOLDERID_Programs` for `native_apps::shortcuts` and
/// `installer::legacy`). The returned `PWSTR` is freed unconditionally —
/// `CoTaskMemFree` runs before the `to_string` result is inspected, so the
/// error path cannot leak the allocation.
pub fn known_folder(id: &windows::core::GUID) -> Result<PathBuf, String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KNOWN_FOLDER_FLAG};

    unsafe {
        let path = SHGetKnownFolderPath(id, KNOWN_FOLDER_FLAG(0), None)
            .map_err(|e| format!("SHGetKnownFolderPath({id:?}): {e}"))?;
        let text = path.to_string();
        CoTaskMemFree(Some(path.0.cast()));
        text.map(PathBuf::from)
            .map_err(|e| format!("known folder {id:?} path: {e}"))
    }
}

/// One comparable spelling of a Windows path.
///
/// `canonicalize` and `IShellLink` return the long path, often with a
/// `\\?\` prefix. `tempfile` on a GitHub-hosted runner returns the 8.3
/// short path (`C:\Users\RUNNER~1\...`). A raw string compare then says a
/// file is outside the directory it was created in, and a shortcut does
/// not point at the executable it was just given. `GetLongPathNameW`
/// resolves 8.3 names of an existing path. If the path does not exist, the
/// original spelling is kept so non-existent fixture paths still compare.
/// A reparse point that this call resolves to somewhere else falls outside
/// the caller's root and is rejected — fail closed.
pub fn windows_path_key(path: &Path) -> String {
    let path = long_path_if_present(path);
    let text = path.to_string_lossy().replace('/', "\\");
    let text = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        text
    };
    text.trim_end_matches('\\').to_ascii_lowercase()
}

fn long_path_if_present(path: &Path) -> PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    extern "system" {
        fn GetLongPathNameW(
            lpszShortPath: *const u16,
            lpszLongPath: *mut u16,
            cchBuffer: u32,
        ) -> u32;
    }

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut buffer = vec![0u16; 512];
    loop {
        let length =
            unsafe { GetLongPathNameW(wide.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) };
        if length == 0 {
            return path.to_path_buf();
        }
        let length = length as usize;
        if length < buffer.len() {
            return PathBuf::from(std::ffi::OsString::from_wide(&buffer[..length]));
        }
        buffer.resize(length, 0);
    }
}
