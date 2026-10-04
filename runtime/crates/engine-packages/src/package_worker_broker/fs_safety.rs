//! Broker filesystem safety: path rejection, root confinement and
//! handle-identity validation.

use super::fs_atomic::is_reparse_point;
use super::*;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

pub(super) fn relative_components(root: &Path, path: &Path) -> Option<Vec<std::ffi::OsString>> {
    let normalize = |component: std::path::Component<'_>| {
        component
            .as_os_str()
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_ascii_lowercase()
    };
    let root_components = root.components().map(normalize).collect::<Vec<_>>();
    let path_components = path.components().collect::<Vec<_>>();
    if path_components.len() < root_components.len()
        || path_components
            .iter()
            .take(root_components.len())
            .copied()
            .map(normalize)
            .ne(root_components.iter().cloned())
    {
        return None;
    }
    path_components[root_components.len()..]
        .iter()
        .map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect()
}

pub(super) fn reject_path(path: &Path) -> Result<(), BrokerError> {
    if !path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
        || path.to_string_lossy().starts_with("\\\\")
    {
        return Err(BrokerError::Invalid(
            "path must be absolute and traversal-free".into(),
        ));
    }
    Ok(())
}

pub(super) fn open_existing_target(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options
            .share_mode(0x00000001 | 0x00000002 | 0x00000004)
            .custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}

pub(super) fn open_parent_dir(
    config: &BrokerConfig,
    path: &Path,
) -> Result<(Option<File>, PathBuf), BrokerError> {
    reject_path(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = OpenOptions::new()
            .read(true)
            .write(true)
            // Minimal directory access for the handle-relative operations:
            // GENERIC_READ lets us query the dir identity and list entries,
            // GENERIC_WRITE covers FILE_ADD_FILE/FILE_ADD_SUBDIRECTORY for
            // creating the temp file and for the atomic handle-relative
            // rename. DELETE and FILE_DELETE_CHILD are NOT requested: nothing
            // here deletes the directory itself or one of its children —
            // delete permission for the replace target is carried by the
            // temp/target file handles. A user's Modify grant (no
            // FILE_DELETE_CHILD) must suffice — Full Control is not required
            // (KOS-270).
            .share_mode(0x00000001 | 0x00000002 | 0x00000004)
            .access_mode(0x80000000 | 0x40000000)
            .custom_flags(0x00200000 | 0x02000000) // OPEN_REPARSE_POINT | BACKUP_SEMANTICS
            .open(path)?;
        let metadata = dir.metadata()?;
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return Err(BrokerError::Invalid("path must be a real directory".into()));
        }
        let canonical = final_path_by_handle(&dir)?;
        if !is_under_configured_root(config, &canonical) {
            return Err(BrokerError::Invalid("path escapes configured roots".into()));
        }
        return Ok((Some(dir), canonical));
    }
    #[cfg(not(windows))]
    {
        Ok((None, canonical_under_root(config, path)?))
    }
}

pub(super) fn validate_open_file(
    config: &BrokerConfig,
    _requested: &Path,
    _file: &File,
    metadata: &fs::Metadata,
) -> Result<(), BrokerError> {
    if is_reparse_point(metadata) {
        return Err(BrokerError::Invalid(
            "path must not be a reparse point".into(),
        ));
    }
    #[cfg(windows)]
    {
        let actual = final_path_by_handle(_file)?;
        if !is_under_configured_root(config, &actual) {
            return Err(BrokerError::Invalid("path escapes configured roots".into()));
        }
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::MetadataExt;
        let actual = fs::canonicalize(_requested)?;
        if !is_under_configured_root(config, &actual) {
            return Err(BrokerError::Invalid("path escapes configured roots".into()));
        }
        let current = fs::metadata(&actual)?;
        if current.dev() != metadata.dev() || current.ino() != metadata.ino() {
            return Err(BrokerError::Invalid(
                "path changed during validation".into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn is_under_configured_root(config: &BrokerConfig, path: &Path) -> bool {
    config
        .filesystem_roots
        .iter()
        .any(|root| path_is_under(root, path))
}

#[cfg(windows)]
pub(super) fn path_is_under(root: &Path, path: &Path) -> bool {
    // Both sides go through the long-path key. The configured root is stored
    // canonical (`\\?\C:\Users\runneradmin\...`) while the caller's path is
    // often the 8.3 form tempfile produced (`C:\Users\RUNNER~1\...`).
    // `GetLongPathNameW` only resolves existing paths, so a not-yet-created
    // path first resolves its longest existing ancestor.
    let root = crate::win32::windows_path_key(root);
    let path = crate::win32::windows_path_key(&resolve_lexical(path));
    path == root || path.starts_with(&(root + "\\"))
}

/// Canonicalize the deepest existing ancestor of `path` and re-append the
/// missing tail. `filesystem_roots` are stored canonical, but callers pass
/// raw paths — on macOS `tempfile` lives under `/var`, a symlink to
/// `/private/var`, and on Windows it can carry 8.3 short names — so a
/// textual compare rejects every file under a configured root.
/// Non-existent paths (a `create_directory` target) canonicalize their
/// longest existing prefix instead.
pub(super) fn resolve_lexical(path: &Path) -> PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => resolve_lexical(parent).join(name),
        _ => path.to_path_buf(),
    }
}

#[cfg(not(windows))]
pub(super) fn path_is_under(root: &Path, path: &Path) -> bool {
    let path = resolve_lexical(path);
    path == root || path.starts_with(root)
}

#[cfg(windows)]
fn final_path_by_handle(file: &File) -> io::Result<PathBuf> {
    use std::{os::windows::ffi::OsStringExt, os::windows::io::AsRawHandle};
    extern "system" {
        fn GetFinalPathNameByHandleW(
            hFile: *mut std::ffi::c_void,
            lpszFilePath: *mut u16,
            cchFilePath: u32,
            dwFlags: u32,
        ) -> u32;
    }
    let handle = file.as_raw_handle();
    let mut buffer = vec![0u16; 512];
    loop {
        let length = unsafe {
            GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, 0)
        };
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        if (length as usize) < buffer.len() {
            return Ok(PathBuf::from(std::ffi::OsString::from_wide(
                &buffer[..length as usize],
            )));
        }
        buffer.resize(buffer.len() * 2, 0);
    }
}

#[cfg(not(windows))]
fn canonical_under_root(config: &BrokerConfig, path: &Path) -> Result<PathBuf, BrokerError> {
    reject_path(path)?;
    let canonical = std::fs::canonicalize(path)?;
    if config
        .filesystem_roots
        .iter()
        .any(|r| path_is_under(r, &canonical))
    {
        Ok(canonical)
    } else {
        Err(BrokerError::Invalid("path escapes configured roots".into()))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::path_is_under;

    #[test]
    fn canonical_root_contains_the_path_tempfile_returned() {
        // Regression: GitHub runners canonicalize to the long name and
        // tempfile keeps the 8.3 name. A spelling compare rejected every
        // file under the configured root.
        let td = tempfile::tempdir().unwrap();
        let child = td.path().join("pkg");
        std::fs::create_dir(&child).unwrap();
        let root = std::fs::canonicalize(td.path()).unwrap();
        assert!(
            path_is_under(&root, &child),
            "root {root:?} does not contain {child:?}"
        );
    }
}
