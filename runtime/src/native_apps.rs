//! Installed native GPUI apps live in `<product-local>/Apps/<id>/<version>` —
//! outside the package store because the artifacts are plain release zips
//! (an executable at the archive root), not `.kspkg` manifests. The app list
//! is hardcoded (`NATIVE_APPS`); the Engine asks each app's own GitHub
//! Releases for the latest tag and verifies downloaded bytes against the
//! release's `SHA256SUMS.txt` (`native_apps::releases`).
//!
//! `install.json` inside each `<id>` dir is the `current` pointer (a small
//! JSON record, not a symlink): a new version extracts and verifies in
//! staging, then the pointer flips atomically and the previous version is
//! dropped. Any failure before the flip leaves the old install untouched.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zip::ZipArchive;

use crate::lock_file::{retry_io, write_owner_only_json};
use crate::package_store::{
    eq_hash, hex_hash, is_reserved_name, normalize_path, MAX_ARCHIVE, MAX_ENTRIES, MAX_EXPANDED,
};

const STATE_FORMAT_VERSION: u32 = 1;
const INSTALL_FILE: &str = "install.json";
static STAGING_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Error)]
pub enum NativeAppError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("archive: {0}")]
    Archive(&'static str),
    #[error("hash mismatch")]
    HashMismatch,
    #[error("size mismatch")]
    SizeMismatch,
    #[error("state: {0}")]
    State(String),
    #[error("invalid: {0}")]
    Invalid(&'static str),
    #[error("app is running")]
    Running,
}

pub type Result<T> = std::result::Result<T, NativeAppError>;

include!("native_apps/descriptor.rs");

/// Everything the install flow needs to place a version on disk. The sha256
/// comes from the release's `SHA256SUMS.txt` — never from the archive itself.
pub struct NativeInstallSpec {
    pub id: String,
    pub version: String,
    pub executable: String,
    pub sha256: String,
    pub size: u64,
    pub repository: String,
    pub release_tag: String,
}

/// The `current` pointer + audit record: `<root>/<id>/install.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativeAppInstall {
    pub schema_version: u32,
    pub id: String,
    /// Installed (and current) version.
    pub version: String,
    /// Executable path inside the version dir, copied from the descriptor.
    pub executable: String,
    /// sha256/size of the archive that produced this install.
    pub sha256: String,
    pub size: u64,
    pub repository: String,
    pub release_tag: String,
    pub installed_at: u64,
}

impl NativeAppInstall {
    fn valid(&self) -> bool {
        self.schema_version == STATE_FORMAT_VERSION
            && valid_app_id(&self.id)
            && semver::Version::parse(&self.version).is_ok()
            && valid_record_executable(&self.executable)
            && self.sha256.len() == 64
            && self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            && self.size > 0
    }
}

fn valid_app_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn valid_record_executable(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.contains('\0')
        && !value.contains('%')
        && value.to_ascii_lowercase().ends_with(".exe")
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.contains(':')
                && !part.ends_with('.')
                && !part.ends_with(' ')
                && !is_reserved_name(part)
        })
}

/// Dev override env var *suffix* for launching an app outside the store
/// layout (read via `brand::env` — `MUNDUS_AGENDA_EXECUTABLE` etc., with
/// legacy-name fallback), shared by the tray and `apps.open`.
pub fn executable_env_override(id: &str) -> Option<&'static str> {
    app_descriptor(id).map(|app| app.env_override)
}

/// `%LOCALAPPDATA%\Mundus\Apps` — the single product-local root every native
/// app installs under.
pub fn native_apps_root() -> Result<PathBuf> {
    crate::data_dir::mundus_local_data_dir()
        .map(|dir| dir.join("Apps"))
        .ok_or(NativeAppError::Invalid("no local data root"))
}

/// Open the store at the product-local root.
pub fn default_store() -> Result<NativeAppStore> {
    NativeAppStore::new(native_apps_root()?)
}

/// A running executable cannot be opened for write on Windows — the loader
/// maps the image with sharing that denies it. Anything else (including a
/// missing file) is not treated as "in use".
#[cfg(windows)]
fn exe_in_use(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    const SHARE_ALL: u32 = 0x0000_0001 | 0x0000_0002 | 0x0000_0004;
    match fs::OpenOptions::new()
        .write(true)
        .share_mode(SHARE_ALL)
        .open(path)
    {
        Ok(_) => false,
        Err(error) => matches!(error.raw_os_error(), Some(32 | 5)),
    }
}

#[cfg(not(windows))]
fn exe_in_use(_path: &Path) -> bool {
    // Unix semantics unlink an in-use inode safely; no probe needed.
    false
}

pub struct NativeAppStore {
    root: PathBuf,
    mutation: Mutex<()>,
}

impl NativeAppStore {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            mutation: Mutex::new(()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn app_dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    fn state_path(&self, id: &str) -> PathBuf {
        self.app_dir(id).join(INSTALL_FILE)
    }

    fn read_record(&self, id: &str) -> Result<Option<NativeAppInstall>> {
        let path = self.state_path(id);
        match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<NativeAppInstall>(&bytes) {
                Ok(record) if record.valid() && record.id == id => Ok(Some(record)),
                // Corrupt or tampered records fail closed: the install is
                // treated as absent (the dir is reclaimable via uninstall).
                _ => Ok(None),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Current record for `id` — the `current` pointer.
    pub fn current(&self, id: &str) -> Option<NativeAppInstall> {
        self.read_record(id).ok().flatten()
    }

    /// All records with a well-formed install.json, sorted by id.
    pub fn list(&self) -> Vec<NativeAppInstall> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut records = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(id) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if let Ok(Some(record)) = self.read_record(id) {
                records.push(record);
            }
        }
        records.sort_by(|a, b| a.id.cmp(&b.id));
        records
    }

    /// Absolute path of the current executable, if the record and file agree.
    pub fn executable_path(&self, id: &str) -> Option<PathBuf> {
        let record = self.current(id)?;
        let exe = self
            .app_dir(id)
            .join(&record.version)
            .join(&record.executable);
        if exe.is_file() {
            Some(exe)
        } else {
            None
        }
    }

    /// The install spec's expected exe, for the running-app probe.
    fn record_executable_path(&self, record: &NativeAppInstall) -> PathBuf {
        self.app_dir(&record.id)
            .join(&record.version)
            .join(&record.executable)
    }
}

pub mod releases;

include!("native_apps/install.rs");

impl NativeAppStore {
    /// Remove the whole `<root>/<id>` tree: record + every version dir. User
    /// data lives elsewhere and is never touched. Refuses while the recorded
    /// executable is running.
    pub fn uninstall(&self, id: &str) -> Result<()> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        if !valid_app_id(id) {
            return Err(NativeAppError::Invalid("package id"));
        }
        let app_dir = self.app_dir(id);
        if let Some(record) = self.current(id) {
            let exe = self.record_executable_path(&record);
            if exe.is_file() && exe_in_use(&exe) {
                return Err(NativeAppError::Running);
            }
        }
        if !app_dir.exists() {
            return Ok(());
        }
        match fs::remove_dir_all(&app_dir) {
            Ok(()) => Ok(()),
            // A live exe deep in the tree surfaces as PermissionDenied —
            // report it as a running-app refusal, not a bare io error.
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                Err(NativeAppError::Running)
            }
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
#[path = "native_apps/tests.rs"]
mod tests;
