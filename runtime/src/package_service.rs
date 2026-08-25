//! Engine-owned, fail-closed Package v1 service.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};
use thiserror::Error;
use zip::ZipArchive;

pub use crate::package_manifest::{
    DefinitionSnapshotReader, ManifestV2, PackageKind, PackageManifest, VersionedManifest,
};
use crate::{
    lock_file::{ensure_owner_only_directory, write_owner_only_json},
    package_registration::PackageRegistrationRegistry,
    package_store::{InstalledPackage, PackageStore, StoreError},
    package_trust::{
        CatalogDocument, CatalogEntry, SignatureSet, TrustError, TrustStore, TrustedKey,
    },
    package_worker_broker::{read_snapshot_tree, BrokerConfig, PackageSnapshotFile},
    package_worker_protocol::{BridgeStatus, BridgeWorkerConfig},
    package_worker_supervisor::{PackageWorkerSupervisor, WorkerDiagnostics, WorkerState},
    protocol_version::API_VERSION_CURRENT,
    runtime_grants::{compile_manifest_v2, LaunchGrant, RegisteredType, RegistrySnapshot},
    store_catalog::{EffectiveGrantProjection, InstalledListing, Role},
};

const MAX_DOCUMENT: usize = 1024 * 1024;
const MAX_ENVELOPE: u64 = 2 * 1024 * 1024;
const MAX_LISTED_PACKAGES: usize = 1024;
const PRODUCTION_CATALOG_URL: &str =
    "https://github.com/makekosmos/package-index/releases/latest/download/catalog.envelope.json";

fn production_trust() -> Option<TrustStore> {
    TrustStore::new(
        TrustedKey {
            key_id: "kosmos-root-2026".into(),
            public_key: "Si7FgOdf6Xnmpa+0LGZL9pRCeAtLhGpCjga89j4tsaY=".into(),
        },
        vec![TrustedKey {
            key_id: "kosmos-release-2026".into(),
            public_key: "Mr7fRkGegxRpquVQPEeMxFRMPB3tT9pDJ49ydDmomDQ=".into(),
        }],
    )
    .ok()
}

#[derive(Debug, Error)]
pub enum PackageError {
    #[error("package trust is unavailable")]
    TrustUnavailable,
    #[error("package request is invalid")]
    Invalid,
    #[error("package trust: {0}")]
    Trust(#[from] TrustError),
    #[error("package store: {0}")]
    Store(#[from] StoreError),
    #[error("package persistence failed")]
    Persistence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    bytes: String,
    signatures: SignatureSet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: PackageKind,
    pub enabled: bool,
    pub revoked: bool,
    pub worker_state: WorkerState,
    pub worker_restart_count: u32,
    pub publisher: String,
    pub icon_path: Option<String>,
    pub update_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageListSummary {
    pub packages: Vec<PackageSummary>,
    pub total: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppLaunch {
    pub package: InstalledPackage,
    /// The host-compiled v2 data grant bound to this exact package launch.
    pub grant: LaunchGrant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustSummary {
    pub trusted_release_keys: usize,
    pub revoked_release_keys: usize,
    pub revoked_packages: usize,
    pub sequence: u64,
    pub configured: bool,
    pub fault_code: Option<String>,
    pub catalog_sequence: u64,
    pub transition_sequence: u64,
    pub revocation_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSummary {
    pub sequence: u64,
    pub expires_at: String,
    pub package_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogPackageSummary {
    pub id: String,
    pub version: String,
    pub name: String,
    pub kind: PackageKind,
    pub publisher: String,
    pub archive_size: u64,
    pub revoked: bool,
}

/// Configuration owned by Engine, never by the bridge worker or package archive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BridgeConfig {
    pub vault_root: String,
    pub selected_types: Vec<String>,
    pub editable_fields: Vec<String>,
    pub readonly_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BridgeConfigStatus {
    pub configured: bool,
    pub config: Option<BridgeConfig>,
    pub worker_state: WorkerState,
    pub worker_status: Option<BridgeStatus>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeConfigState {
    #[serde(default)]
    configs: HashMap<String, BridgeConfig>,
}

struct CatalogState {
    document: CatalogDocument,
    signatures: SignatureSet,
}

struct State {
    trust: Option<TrustStore>,
    fault: Option<String>,
    catalog: Option<CatalogState>,
}

pub struct PackageService {
    root: PathBuf,
    store: std::sync::Arc<PackageStore>,
    state: Mutex<State>,
    // Serializes all mutations spanning trust, catalog cache and package state.
    mutation: Mutex<()>,
    worker: Option<WorkerRuntime>,
    bridge_configs: Mutex<BridgeConfigState>,
    typed_grants: Mutex<HashMap<(String, String), LaunchGrant>>,
    package_registrations: PackageRegistrationRegistry,
    package_definition_dispatcher:
        Mutex<Option<std::sync::Arc<crate::engine_dispatch::EngineDispatcher>>>,
    typed_registry: Mutex<RegistrySnapshot>,
}

struct WorkerRuntime {
    supervisor: PackageWorkerSupervisor,
    roots: Vec<PathBuf>,
    correlation_id: String,
    mutation: tokio::sync::Mutex<()>,
}

include!("package_service/core.rs");
include!("package_service/operations.rs");
include!("package_service/helpers.rs");
#[cfg(test)]
include!("package_service/tests.rs");
