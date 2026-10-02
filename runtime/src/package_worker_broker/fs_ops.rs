//! Broker filesystem operations: create/read/write/delete/list under
//! the configured roots.

use super::fs_atomic::{atomic_replace, create_temp_file, delete_temp_file, is_reparse_point};
use super::fs_safety::{
    open_existing_target, open_parent_dir, path_is_under, reject_path, relative_components,
    validate_open_file,
};
use super::*;
#[cfg(windows)]
use std::io::Read;
use std::{fs, io, path::Path};

pub fn create_directory(config: &BrokerConfig, path: &Path) -> Result<(), BrokerError> {
    reject_path(path)?;
    let root = config
        .filesystem_roots
        .iter()
        .find(|root| path_is_under(root, path))
        .ok_or_else(|| BrokerError::Invalid("path escapes configured roots".into()))?;
    let relative = relative_components(root, path)
        .ok_or_else(|| BrokerError::Invalid("path escapes configured roots".into()))?;
    let mut current = root.clone();
    for component in relative {
        let next = current.join(component);
        match fs::create_dir(&next) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        current = fs::canonicalize(next)?;
        if !current.is_dir() || !path_is_under(root, &current) {
            return Err(BrokerError::Invalid("path escapes configured roots".into()));
        }
    }
    Ok(())
}

pub fn read_file(config: &BrokerConfig, path: &Path) -> Result<Vec<u8>, BrokerError> {
    reject_path(path)?;
    #[cfg(windows)]
    {
        let file = open_existing_target(path)?;
        let metadata = file.metadata()?;
        validate_open_file(config, path, &file, &metadata)?;
        if !metadata.is_file() {
            return Err(BrokerError::Invalid("path is not a regular file".into()));
        }
        let mut bytes = Vec::new();
        file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
        if bytes.len() > MAX_BYTES {
            return Err(BrokerError::Invalid("file exceeds 1 MiB".into()));
        }
        return Ok(bytes);
    }
    #[cfg(not(windows))]
    let path = path.to_path_buf();
    #[cfg(not(windows))]
    let configured = config
        .filesystem_roots
        .iter()
        .find(|candidate| path_is_under(candidate, &path))
        .ok_or_else(|| BrokerError::Invalid("path escapes configured roots".into()))?;
    #[cfg(not(windows))]
    let relative = path
        .strip_prefix(configured)
        .map_err(|_| BrokerError::Invalid("path escapes configured roots".into()))?;
    #[cfg(not(windows))]
    let components: Vec<&str> = relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .ok_or_else(|| BrokerError::Invalid("path is not UTF-8".into()))
        })
        .collect::<Result<_, _>>()?;
    #[cfg(not(windows))]
    crate::handle_relative_fs::read_relative(
        &crate::handle_relative_fs::open_root(configured)?,
        &components,
        MAX_BYTES,
    )
    .map_err(BrokerError::Io)
}

pub fn write_file(config: &BrokerConfig, path: &Path, data: &[u8]) -> Result<(), BrokerError> {
    if data.len() > MAX_BYTES {
        return Err(BrokerError::Invalid("file exceeds 1 MiB".into()));
    }
    reject_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| BrokerError::Invalid("missing parent".into()))?;
    let private_state = config
        .private_state_roots
        .iter()
        .any(|root| path_is_under(root, path));
    let (_parent_guard, parent) = open_parent_dir(config, parent)?;
    let target = parent.join(
        path.file_name()
            .ok_or_else(|| BrokerError::Invalid("missing filename".into()))?,
    );
    let target_guard = match open_existing_target(&target) {
        Ok(file) => {
            let metadata = file.metadata()?;
            validate_open_file(config, &target, &file, &metadata)?;
            if !metadata.is_file() {
                return Err(BrokerError::Invalid("path is not a regular file".into()));
            }
            Some(file)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };

    let (temp_path, mut temp) = create_temp_file(&parent, _parent_guard.as_ref())?;
    let result = (|| -> Result<(), BrokerError> {
        std::io::Write::write_all(&mut temp, data)?;
        temp.sync_all()?;
        if private_state {
            crate::lock_file::apply_owner_only_file_permissions(&temp_path)?;
        }
        if let Some(ref target_handle) = target_guard {
            let metadata = target_handle.metadata()?;
            validate_open_file(config, &target, target_handle, &metadata)?;
        }
        drop(target_guard);
        #[cfg(windows)]
        atomic_replace(
            &temp,
            _parent_guard
                .as_ref()
                .ok_or_else(|| io::Error::other("missing parent handle"))?,
            path.file_name()
                .ok_or_else(|| io::Error::other("missing filename"))?,
        )?;
        #[cfg(not(windows))]
        {
            drop(temp);
            atomic_replace(&temp_path, &target)?;
        }
        Ok(())
    })();
    if result.is_err() {
        #[cfg(windows)]
        {
            // The parent directory may have been renamed after validation.  Delete
            // through the already-open temporary-file handle instead of resolving
            // the canonical path again.
            let _ = delete_temp_file(&temp);
        }
        #[cfg(not(windows))]
        {
            let _ = fs::remove_file(temp_path);
        }
    }
    result
}

pub fn delete_file(config: &BrokerConfig, path: &Path) -> Result<(), BrokerError> {
    reject_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| BrokerError::Invalid("missing parent".into()))?;
    let (_parent_guard, parent) = open_parent_dir(config, parent)?;
    let target = parent.join(
        path.file_name()
            .ok_or_else(|| BrokerError::Invalid("missing filename".into()))?,
    );
    let file = open_existing_target(&target)?;
    let metadata = file.metadata()?;
    validate_open_file(config, &target, &file, &metadata)?;
    if !metadata.is_file() {
        return Err(BrokerError::Invalid("path is not a regular file".into()));
    }
    drop(file);
    fs::remove_file(target)?;
    Ok(())
}

pub fn list_directory(
    config: &BrokerConfig,
    path: &Path,
) -> Result<Vec<DirectoryEntry>, BrokerError> {
    reject_path(path)?;
    let (_guard, canonical) = open_parent_dir(config, path)?;
    let mut entries = Vec::new();
    for item in fs::read_dir(canonical)? {
        if entries.len() >= MAX_DIRECTORY_ENTRIES {
            return Err(BrokerError::Invalid("directory exceeds entry bound".into()));
        }
        let item = item?;
        let meta = item.metadata()?;
        if is_reparse_point(&meta) {
            return Err(BrokerError::Invalid("reparse point is not allowed".into()));
        }
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis())
            .unwrap_or(0);
        entries.push(DirectoryEntry {
            name: item.file_name().to_string_lossy().into_owned(),
            kind: if meta.is_dir() {
                "directory".into()
            } else if meta.is_file() {
                "file".into()
            } else {
                "other".into()
            },
            size: meta.len(),
            modified_ms,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub fn poll_metadata(
    config: &BrokerConfig,
    path: &Path,
) -> Result<Vec<DirectoryEntry>, BrokerError> {
    list_directory(config, path)
}
