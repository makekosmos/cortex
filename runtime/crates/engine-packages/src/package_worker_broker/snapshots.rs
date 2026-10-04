//! Broker snapshot registry: owner-scoped, bounded chunked reads of
//! package trees and network responses.

use super::fs_safety::{path_is_under, reject_path};
use super::*;
use std::{collections::HashMap, path::Path, sync::Mutex};

pub const MAX_SNAPSHOT_CHUNK: usize = 256 * 1024;

struct SnapshotEntry {
    owner: String,
    bytes: Vec<u8>,
}

pub struct SnapshotRegistry {
    entries: Mutex<HashMap<String, SnapshotEntry>>,
    network: bool,
}

impl SnapshotRegistry {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            network: false,
        }
    }

    pub(crate) fn network_responses() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            network: true,
        }
    }

    pub fn reserve(
        &self,
        owner: &str,
        _package_id: &str,
        _source: &str,
        bytes: Vec<u8>,
    ) -> Result<String, BrokerError> {
        let limit = if self.network {
            MAX_NETWORK_RESPONSE
        } else {
            MAX_BYTES * 16
        };
        if bytes.len() > limit {
            return Err(BrokerError::Invalid("snapshot is too large".into()));
        }
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if self.network
            && (entries.values().any(|entry| entry.owner == owner) || entries.len() >= 4)
        {
            return Err(BrokerError::Invalid(
                "network response capacity exhausted".into(),
            ));
        }
        let handle = uuid::Uuid::new_v4().to_string();
        entries.insert(
            handle.clone(),
            SnapshotEntry {
                owner: owner.to_owned(),
                bytes,
            },
        );
        Ok(handle)
    }

    pub fn chunk(
        &self,
        handle: &str,
        owner: &str,
        offset: usize,
        requested: usize,
    ) -> Result<Vec<u8>, BrokerError> {
        if requested > MAX_SNAPSHOT_CHUNK {
            return Err(BrokerError::Invalid("snapshot chunk too large".into()));
        }
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get(handle)
            .ok_or_else(|| BrokerError::Invalid("snapshot unavailable".into()))?;
        if entry.owner != owner {
            return Err(BrokerError::Invalid("snapshot owner mismatch".into()));
        }
        if offset > entry.bytes.len() {
            return Err(BrokerError::Invalid("snapshot offset out of range".into()));
        }
        let end = offset.saturating_add(requested).min(entry.bytes.len());
        Ok(entry.bytes[offset..end].to_vec())
    }

    pub fn size(&self, handle: &str, owner: &str) -> Result<usize, BrokerError> {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get(handle)
            .ok_or_else(|| BrokerError::Invalid("snapshot unavailable".into()))?;
        if entry.owner != owner {
            return Err(BrokerError::Invalid("snapshot owner mismatch".into()));
        }
        Ok(entry.bytes.len())
    }

    pub fn close(&self, handle: &str, owner: &str) -> Result<(), BrokerError> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get(handle)
            .ok_or_else(|| BrokerError::Invalid("snapshot unavailable".into()))?;
        if entry.owner != owner {
            return Err(BrokerError::Invalid("snapshot owner mismatch".into()));
        }
        entries.remove(handle);
        Ok(())
    }

    pub fn close_owner(&self, owner: &str) {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|_, entry| entry.owner != owner);
    }

    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PackageSnapshotFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

pub fn read_snapshot_tree(
    config: &BrokerConfig,
    root: &Path,
) -> Result<Vec<PackageSnapshotFile>, BrokerError> {
    reject_path(root)?;
    if !config
        .filesystem_roots
        .iter()
        .any(|candidate| path_is_under(candidate, root))
    {
        return Err(BrokerError::Invalid("path escapes configured roots".into()));
    }
    let handle = crate::handle_relative_fs::open_root(root)?;
    let limits = crate::handle_relative_fs::Limits {
        max_bytes_per_file: MAX_BYTES,
        max_total_bytes: MAX_BYTES.saturating_mul(16),
        max_files: MAX_DIRECTORY_ENTRIES,
        max_depth: 64,
    };
    let files = crate::handle_relative_fs::walk_files(&handle, limits).map_err(BrokerError::Io)?;
    Ok(files
        .into_iter()
        .map(|file| PackageSnapshotFile {
            path: file.components.join("/"),
            bytes: file.bytes,
        })
        .collect())
}
