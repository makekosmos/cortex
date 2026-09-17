//! App-scoped binary user data on the handle-relative native boundary.
//!
//! The desktop Host registers its userData base directory once; every
//! subsequent operation resolves `extension-data/<app_id>/<key>` relative to
//! that pinned directory handle through `handle_relative_fs`, so a junction,
//! symlink, or replaced parent directory between calls can never redirect
//! I/O outside the pinned root.
use crate::handle_relative_fs::{self, RootHandle, RootIdentity};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const USER_DATA_MAX_BYTES: usize = 25 * 1024 * 1024;
const MAX_USER_DATA_ROOTS: usize = 64;
const MAX_USER_DATA_KEY_BYTES: usize = 512;
const APP_DIRECTORY: &str = "extension-data";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserDataError {
    NotFound,
    InvalidKey,
    TooLarge,
    UnknownRoot,
    InvalidRequest,
    Unavailable,
    Io,
}

impl UserDataError {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotFound => "not-found",
            Self::InvalidKey => "invalid-key",
            Self::TooLarge => "too-large",
            Self::UnknownRoot => "unknown-root",
            Self::InvalidRequest => "invalid-request",
            Self::Unavailable => "unavailable",
            Self::Io => "io-error",
        }
    }
}

struct UserDataRoot {
    path: PathBuf,
    handle: Arc<RootHandle>,
    identity: RootIdentity,
}

pub struct UserDataRoots {
    roots: Mutex<HashMap<String, UserDataRoot>>,
    max_file_bytes: usize,
}

impl Default for UserDataRoots {
    fn default() -> Self {
        Self::new()
    }
}

impl UserDataRoots {
    pub fn new() -> Self {
        Self {
            roots: Mutex::new(HashMap::new()),
            max_file_bytes: USER_DATA_MAX_BYTES,
        }
    }

    #[cfg(test)]
    fn with_max_file_bytes(max_file_bytes: usize) -> Self {
        Self {
            roots: Mutex::new(HashMap::new()),
            max_file_bytes,
        }
    }

    /// Pin `root` — the Host's userData base directory — to a verified
    /// directory handle. Re-registering the same path reuses the handle while
    /// the directory identity is unchanged and re-pins it after replacement.
    pub fn open(&self, root: &Path) -> Result<String, UserDataError> {
        if !root.is_absolute() {
            return Err(UserDataError::InvalidRequest);
        }
        let handle = handle_relative_fs::open_root(root).map_err(classify_io)?;
        let identity = handle_relative_fs::root_identity(&handle).map_err(classify_io)?;
        let mut roots = self.roots.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((id, existing)) = roots
            .iter_mut()
            .find(|(_, registered)| registered.path == root)
        {
            if existing.identity != identity {
                existing.handle = Arc::new(handle);
                existing.identity = identity;
            }
            return Ok(id.clone());
        }
        if roots.len() >= MAX_USER_DATA_ROOTS {
            return Err(UserDataError::Unavailable);
        }
        let id = uuid::Uuid::new_v4().to_string();
        roots.insert(
            id.clone(),
            UserDataRoot {
                path: root.to_path_buf(),
                handle: Arc::new(handle),
                identity,
            },
        );
        Ok(id)
    }

    pub fn close(&self, root_id: &str) -> Result<(), UserDataError> {
        self.roots
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(root_id)
            .map(|_| ())
            .ok_or(UserDataError::UnknownRoot)
    }

    pub fn read(&self, root_id: &str, app_id: &str, key: &str) -> Result<Vec<u8>, UserDataError> {
        let components = components(app_id, key)?;
        let root = self.handle(root_id)?;
        let refs = component_refs(&components);
        handle_relative_fs::read_relative(&root, &refs, self.max_file_bytes).map_err(classify_io)
    }

    pub fn stat(&self, root_id: &str, app_id: &str, key: &str) -> Result<u64, UserDataError> {
        let components = components(app_id, key)?;
        let root = self.handle(root_id)?;
        let refs = component_refs(&components);
        handle_relative_fs::stat_relative(&root, &refs).map_err(classify_io)
    }

    /// Atomically replace `key` under the app directory, creating missing
    /// parent directories relative to the pinned root handle.
    pub fn write(
        &self,
        root_id: &str,
        app_id: &str,
        key: &str,
        bytes: &[u8],
    ) -> Result<u64, UserDataError> {
        if bytes.len() > self.max_file_bytes {
            return Err(UserDataError::TooLarge);
        }
        let components = components(app_id, key)?;
        let root = self.handle(root_id)?;
        let refs = component_refs(&components);
        handle_relative_fs::mkdir_relative(&root, &refs[..refs.len() - 1]).map_err(classify_io)?;
        handle_relative_fs::write_relative(&root, &refs, bytes, self.max_file_bytes)
            .map_err(classify_io)?;
        Ok(bytes.len() as u64)
    }

    pub fn delete(&self, root_id: &str, app_id: &str, key: &str) -> Result<(), UserDataError> {
        let components = components(app_id, key)?;
        let root = self.handle(root_id)?;
        let refs = component_refs(&components);
        handle_relative_fs::delete_relative(&root, &refs).map_err(classify_io)
    }

    fn handle(&self, root_id: &str) -> Result<Arc<RootHandle>, UserDataError> {
        self.roots
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(root_id)
            .map(|root| root.handle.clone())
            .ok_or(UserDataError::UnknownRoot)
    }
}

/// A user data component mirrors the Host key contract: `[\w][\w.-]{0,255}`
/// where `\w` is ASCII-only. This rejects `.`/`..`, separators, drive
/// prefixes, control characters, and empty segments outright.
fn valid_component(component: &str) -> bool {
    let bytes = component.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 256
        && matches!(bytes[0], b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_')
        && bytes[1..]
            .iter()
            .all(|b| matches!(*b, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'.' | b'-'))
}

fn key_components(key: &str) -> Result<Vec<String>, UserDataError> {
    if key.is_empty() || key.len() > MAX_USER_DATA_KEY_BYTES {
        return Err(UserDataError::InvalidKey);
    }
    let normalized = key.replace('\\', "/");
    let parts: Vec<String> = normalized.split('/').map(str::to_owned).collect();
    if parts.iter().any(|part| !valid_component(part)) {
        return Err(UserDataError::InvalidKey);
    }
    Ok(parts)
}

fn components(app_id: &str, key: &str) -> Result<Vec<String>, UserDataError> {
    if !valid_component(app_id) {
        return Err(UserDataError::InvalidRequest);
    }
    let mut components = Vec::with_capacity(10);
    components.push(APP_DIRECTORY.to_owned());
    components.push(app_id.to_owned());
    components.extend(key_components(key)?);
    Ok(components)
}

fn component_refs(components: &[String]) -> Vec<&str> {
    components.iter().map(String::as_str).collect()
}

fn classify_io(error: io::Error) -> UserDataError {
    match error.kind() {
        io::ErrorKind::NotFound => UserDataError::NotFound,
        io::ErrorKind::FileTooLarge => UserDataError::TooLarge,
        _ => UserDataError::Io,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    include!("user_data/tests.rs");
    include!("user_data/tests_race.rs");
}
