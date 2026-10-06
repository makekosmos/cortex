use crate::handle_relative_fs::{self, RootHandle, RootIdentity};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::cell::Cell;
use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[cfg(test)]
thread_local! {
    static FAIL_PERSIST_AFTER_FSYNC: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrantProvenance {
    NativeDialog,
    PersistedUserData,
}
impl GrantProvenance {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "native-dialog" => Some(Self::NativeDialog),
            "persisted" | "persisted-userdata" => Some(Self::PersistedUserData),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeDialog => "native-dialog",
            Self::PersistedUserData => "persisted",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantOwner {
    pub session_id: String,
    pub generation: u64,
    pub connection_id: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct PersistedRecord {
    version: u32,
    persistent_grant_id: String,
    extension_id: String,
    provenance: GrantProvenance,
    exact_file: bool,
    selected_path: String,
    root_identity: RootIdentity,
    exact_file_identity: Option<RootIdentity>,
    revoked: bool,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}
struct Grant {
    owner: GrantOwner,
    extension_id: String,
    exact_file: bool,
    selected_name: Option<String>,
    exact_file_identity: Option<RootIdentity>,
    root: RootHandle,
    identity: RootIdentity,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantError {
    Invalid,
    NotFound,
    OwnerMismatch,
    ExtensionMismatch,
    ScopeMismatch,
    IdentityChanged,
    Persistence,
}
const MAX_GRANT_FILE_BYTES: usize = 1024 * 1024;
const MAX_GRANT_DIRECTORY_ENTRIES: usize = 4096;
const MAX_GRANT_TRANSACTION_BYTES: usize = 16 * 1024 * 1024;
const LEGACY_EXTENSION_IDS: &[&str] = &["arcadia", "arrancador", "eden", "delphi"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyGrantRevocation {
    pub transaction_token: Option<String>,
    pub revoked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationGrantRestoration {
    pub snapshot_found: bool,
    pub restored: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum LegacyGrantTransactionState {
    Active,
    Restored,
    Committed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyGrantTransaction {
    version: u32,
    token: String,
    source_ids: Vec<String>,
    records: Vec<PersistedRecord>,
    state: LegacyGrantTransactionState,
}

pub struct GrantAuthorityRegistry {
    grants: Mutex<HashMap<String, Grant>>,
    legacy_transactions: Mutex<()>,
    data_dir: Option<PathBuf>,
}
impl Default for GrantAuthorityRegistry {
    fn default() -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
            legacy_transactions: Mutex::new(()),
            data_dir: None,
        }
    }
}
impl GrantAuthorityRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
            legacy_transactions: Mutex::new(()),
            data_dir: Some(data_dir),
        }
    }
    pub fn len(&self) -> usize {
        self.grants.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

mod grants;
mod legacy;
mod persistence;

#[cfg(test)]
mod tests;
