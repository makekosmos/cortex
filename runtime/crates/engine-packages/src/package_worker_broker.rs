//! Brokered Engine API operations for package workers.
//!
//! This brokers the Engine API; it is not hostile native-code containment.

use crate::package_manifest::SecretInjection;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::AtomicU64,
};

use thiserror::Error;

mod fs_atomic;
mod fs_ops;
mod fs_safety;
mod net;
mod process;
mod secrets;
mod snapshots;
#[cfg(test)]
mod tests;

use net::is_loopback_host;

pub use fs_ops::{
    create_directory, delete_file, list_directory, poll_metadata, read_file, write_file,
};
pub(crate) use net::fetch_with_secret_json_limit;
pub use net::{fetch, fetch_with_secret, fetch_with_secret_json};
pub use process::spawn_process;
pub use snapshots::{
    read_snapshot_tree, PackageSnapshotFile, SnapshotRegistry, MAX_SNAPSHOT_CHUNK,
};

const MAX_BYTES: usize = 1024 * 1024;
const MAX_JSON_BODY: usize = 64 * 1024;
pub(crate) const MAX_NETWORK_RESPONSE: usize = 32 * 1024 * 1024;
pub const MAX_DIRECTORY_ENTRIES: usize = 4096;
const MAX_TEMPFILE_ATTEMPTS: usize = 128;
static TEMPFILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub modified_ms: u128,
}

#[derive(Debug, Error)]
pub enum BrokerError {
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Clone, Debug)]
pub struct BrokerConfig {
    pub allowed_origins: HashSet<String>,
    pub filesystem_roots: Vec<PathBuf>,
    pub private_state_roots: Vec<PathBuf>,
    #[cfg(feature = "package-worker-fixture")]
    pub(crate) allow_local_test_origin: bool,
}

impl BrokerConfig {
    pub fn new<I, S>(origins: I, roots: Vec<PathBuf>) -> Result<Self, BrokerError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut allowed_origins = HashSet::new();
        for origin in origins {
            let url = reqwest::Url::parse(origin.as_ref())
                .map_err(|e| BrokerError::Invalid(e.to_string()))?;
            if (url.scheme() != "https"
                && !(cfg!(feature = "package-worker-fixture")
                    && url.scheme() == "http"
                    && url.host_str().is_some_and(is_loopback_host)))
                || url.username() != ""
                || url.password().is_some()
                || !(url.path().is_empty() || url.path() == "/")
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(BrokerError::Invalid("origins must be HTTPS origins".into()));
            }
            allowed_origins.insert(url.origin().ascii_serialization());
        }
        let filesystem_roots = roots
            .into_iter()
            .map(|p| std::fs::canonicalize(p))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            allowed_origins,
            filesystem_roots,
            private_state_roots: Vec::new(),
            #[cfg(feature = "package-worker-fixture")]
            allow_local_test_origin: false,
        })
    }

    #[cfg(feature = "package-worker-fixture")]
    pub(crate) fn enable_local_test_origin(mut self) -> Self {
        self.allow_local_test_origin = true;
        self
    }

    pub fn with_private_state_root(mut self, root: &Path) -> Result<Self, BrokerError> {
        self.private_state_roots.push(std::fs::canonicalize(root)?);
        Ok(self)
    }
}

pub struct SecretRequest<'a> {
    pub injection: &'a SecretInjection,
    pub secret: &'a str,
    pub allowed_cookie_names: &'a [String],
}
