//! Immutable, fail-closed `.kspkg` archive store. This module never executes package code.

use crate::lock_file::{retry_io, write_owner_only_json};
use crate::package_manifest::{ManifestError, PackageKind, PackageManifest, VersionedManifest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use zip::ZipArchive;

mod identity;
mod install;
mod lifecycle;
mod paths;
mod sweep;
mod verify;

#[cfg(test)]
mod tests;

pub(crate) use paths::{is_reserved_name, normalize_path};

pub(crate) const MAX_ARCHIVE: u64 = 128 * 1024 * 1024;
pub(crate) const MAX_EXPANDED: u64 = 512 * 1024 * 1024;
pub(crate) const MAX_ENTRIES: usize = 512;
pub(crate) const MAX_ASSET_BYTES: u64 = 16 * 1024 * 1024;
const STATE_FORMAT_VERSION: u32 = 1;
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("manifest: {0}")]
    Manifest(#[from] ManifestError),
    #[error("archive {0}")]
    Archive(String),
    #[error("hash mismatch")]
    HashMismatch,
    #[error("archive size mismatch")]
    SizeMismatch,
    #[error("manifest mismatch")]
    ManifestMismatch,
    #[error("package is not enabled until worker support exists")]
    WorkerRequired,
    #[error("package is revoked")]
    Revoked,
    #[error("state: {0}")]
    State(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstalledPackage {
    pub id: String,
    pub version: String,
    pub hash: String,
    pub manifest: VersionedManifest,
    pub enabled: bool,
    pub revoked: bool,
    pub installed_at: u64,
    pub catalog_sequence: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreState {
    format_version: u32,
    packages: Vec<InstalledPackage>,
}

impl Default for StoreState {
    fn default() -> Self {
        Self {
            format_version: STATE_FORMAT_VERSION,
            packages: Vec::new(),
        }
    }
}

pub struct PackageStore {
    root: PathBuf,
    mutation: Mutex<()>,
    #[cfg(test)]
    fail_next_state_write: std::sync::atomic::AtomicBool,
}

impl PackageStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        retry_io(|| fs::create_dir_all(root.join("blobs")))?;
        retry_io(|| fs::create_dir_all(root.join("unpacked")))?;
        sweep::sweep_stale_leftovers(&root);
        Ok(Self {
            root,
            mutation: Mutex::new(()),
            #[cfg(test)]
            fail_next_state_write: std::sync::atomic::AtomicBool::new(false),
        })
    }
    pub fn state_path(&self) -> PathBuf {
        self.root.join("state.json")
    }

    pub fn installed(&self, id: &str, version: &str) -> Result<InstalledPackage, StoreError> {
        self.list()?
            .into_iter()
            .find(|p| p.id == id && p.version == version)
            .ok_or_else(|| StoreError::State("package not found".into()))
    }

    fn read_state(&self) -> Result<StoreState, StoreError> {
        if !self.state_path().exists() {
            return Ok(StoreState::default());
        }
        let state: StoreState = serde_json::from_slice(&retry_io(|| fs::read(self.state_path()))?)
            .map_err(|e| StoreError::State(e.to_string()))?;
        if state.format_version != STATE_FORMAT_VERSION {
            return Err(StoreError::State("unsupported state format".into()));
        }
        for package in &state.packages {
            package.manifest.validate()?;
            if package.id != package.manifest.id()
                || package.version != package.manifest.version()
                || !is_hash(&package.hash)
                || (package.revoked && package.enabled)
            {
                return Err(StoreError::State("invalid package record".into()));
            }
            self.verify_installed_manifest(package)?;
        }
        Ok(state)
    }

    fn verify_installed_manifest(&self, package: &InstalledPackage) -> Result<(), StoreError> {
        let raw = self.read_blob_entry(package, "manifest.json")?;
        let actual = PackageManifest::parse(
            std::str::from_utf8(&raw)
                .map_err(|_| StoreError::Archive("manifest is not UTF-8".into()))?,
        )?;
        if actual != package.manifest {
            return Err(StoreError::ManifestMismatch);
        }
        Ok(())
    }
    fn write_state(&self, state: &StoreState) -> Result<(), StoreError> {
        #[cfg(test)]
        if self.fail_next_state_write.swap(false, Ordering::Relaxed) {
            return Err(StoreError::State("injected atomic write failure".into()));
        }
        write_owner_only_json(&self.state_path(), state)
            .map_err(|error| StoreError::State(error.to_string()))
    }
}

pub(crate) fn hex_hash(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
fn hash_reader(reader: impl Read) -> Result<String, StoreError> {
    Ok(crate::file_hash::sha256_reader(reader)?)
}

pub(crate) use crate::file_hash::eq_hash;
fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
