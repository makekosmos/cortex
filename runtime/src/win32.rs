//! Shared Win32 API helpers used by more than one module.

use std::path::PathBuf;

/// `SHGetKnownFolderPath` for a current-user known folder
/// (`FOLDERID_Programs` for `native_apps::shortcuts` and
/// `installer::legacy`). The returned `PWSTR` is freed unconditionally —
/// `CoTaskMemFree` runs before the `to_string` result is inspected, so the
/// error path cannot leak the allocation.
pub(crate) fn known_folder(id: &windows::core::GUID) -> Result<PathBuf, String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KNOWN_FOLDER_FLAG};

    unsafe {
        let path = SHGetKnownFolderPath(id, KNOWN_FOLDER_FLAG(0), HANDLE::default())
            .map_err(|e| format!("SHGetKnownFolderPath({id:?}): {e}"))?;
        let text = path.to_string();
        CoTaskMemFree(Some(path.0.cast()));
        text.map(PathBuf::from)
            .map_err(|e| format!("known folder {id:?} path: {e}"))
    }
}
