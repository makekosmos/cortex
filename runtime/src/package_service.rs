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

impl PackageService {
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, PackageError> {
        let root = data_dir.as_ref().join("packages");
        fs::create_dir_all(&root).map_err(|_| PackageError::Persistence)?;
        let trust = match (
            option_env!("KOSMOS_PACKAGE_ROOT_KEY_JSON"),
            option_env!("KOSMOS_PACKAGE_RELEASE_KEYS_JSON"),
        ) {
            (Some(root_json), Some(releases)) => serde_json::from_str::<TrustedKey>(root_json)
                .ok()
                .and_then(|root_key| {
                    serde_json::from_str::<Vec<TrustedKey>>(releases)
                        .ok()
                        .and_then(|keys| TrustStore::new(root_key, keys).ok())
                }),
            _ => production_trust(),
        };
        Self::from_parts(root, trust)
    }

    #[cfg(test)]
    pub fn open_with_trust(
        data_dir: impl AsRef<Path>,
        trust: TrustStore,
    ) -> Result<Self, PackageError> {
        Self::from_parts(data_dir.as_ref().join("packages"), Some(trust))
    }

    fn from_parts(root: PathBuf, trust: Option<TrustStore>) -> Result<Self, PackageError> {
        let unavailable = trust.is_none();
        let bridge_configs = read_bridge_configs(&root);
        let store = std::sync::Arc::new(PackageStore::new(&root)?);
        let package_registrations = PackageRegistrationRegistry::open(root.join("definitions"))
            .map_err(|_| PackageError::Persistence)?;
        // Re-validate every installed package against its immutable blob on
        // startup. Invalid/tampered package definitions fail closed, while
        // definitions from an uninstall remain in the registry file.
        for package in store.list()? {
            let VersionedManifest::V2(manifest) = &package.manifest else {
                let _ = store.disable(&package.id, &package.version);
                continue;
            };
            if manifest.data.defines.is_empty() {
                continue;
            }
            let Ok(documents) = definition_documents_from_store(&store, &package) else {
                let _ = store.disable(&package.id, &package.version);
                continue;
            };
            if package_registrations
                .register_manifest(manifest, &documents)
                .is_err()
            {
                let _ = store.disable(&package.id, &package.version);
            }
        }
        let mut typed_registry = canonical_registry_snapshot()?;
        typed_registry.types.extend(
            package_registrations
                .registered_types()
                .map_err(|_| PackageError::Persistence)?,
        );
        let service = Self {
            store,
            root,
            state: Mutex::new(State {
                trust,
                fault: unavailable.then(|| "package_trust_unavailable".into()),
                catalog: None,
            }),
            mutation: Mutex::new(()),
            worker: None,
            bridge_configs: Mutex::new(bridge_configs),
            typed_grants: Mutex::new(HashMap::new()),
            package_registrations,
            package_definition_dispatcher: Mutex::new(None),
            typed_registry: Mutex::new(typed_registry),
        };
        // A stale installed v2 contract must not brick Engine startup. Invalid
        // packages stay installed but disabled and cannot be enabled/launched.
        let _ = service.rebuild_typed_grants();
        service.replay();
        Ok(service)
    }

    pub fn build_engine_snapshot(
        &self,
        package_id: &str,
        source: &str,
    ) -> Result<Vec<PackageSnapshotFile>, PackageError> {
        if package_id.is_empty()
            || !package_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            || source != "installed" && source != "auto"
        {
            return Err(PackageError::Invalid);
        }
        let package = self
            .store
            .list()?
            .into_iter()
            .find(|item| item.id == package_id && !item.revoked && item.enabled)
            .ok_or(PackageError::Invalid)?;
        let root = self
            .root
            .join("unpacked")
            .join(&package.id)
            .join(&package.version)
            .join(&package.hash);
        let config =
            BrokerConfig::new(std::iter::empty::<&str>(), vec![self.root.join("unpacked")])
                .map_err(|_| PackageError::Invalid)?;
        read_snapshot_tree(&config, &root).map_err(|_| PackageError::Invalid)
    }
    pub fn engine_snapshot_identity(
        &self,
        package_id: &str,
        source: &str,
    ) -> Result<(String, u64, u64), PackageError> {
        let package = self
            .store
            .list()?
            .into_iter()
            .find(|item| item.id == package_id && !item.revoked && item.enabled)
            .ok_or(PackageError::Invalid)?;
        let root = fs::canonicalize(
            self.root
                .join("unpacked")
                .join(&package.id)
                .join(&package.version)
                .join(&package.hash),
        )
        .map_err(|_| PackageError::Invalid)?;
        let metadata = fs::metadata(&root).map_err(|_| PackageError::Invalid)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            return Ok((
                root.to_string_lossy().into_owned(),
                metadata.dev(),
                metadata.ino(),
            ));
        }
        #[cfg(not(unix))]
        {
            let _ = (source, metadata);
            Err(PackageError::Invalid)
        }
    }
    pub fn build_engine_grant_snapshot(
        &self,
        extension_id: &str,
        root: &Path,
        identity_dev: u64,
        identity_ino: u64,
        exact_file: bool,
    ) -> Result<Vec<PackageSnapshotFile>, PackageError> {
        if extension_id.is_empty() || !root.is_absolute() {
            return Err(PackageError::Invalid);
        }
        let configured_parent = self.root.parent().ok_or(PackageError::Invalid)?;
        let canonical = fs::canonicalize(root).map_err(|_| PackageError::Invalid)?;
        let configured_parent =
            fs::canonicalize(configured_parent).map_err(|_| PackageError::Invalid)?;
        if !(canonical == configured_parent || canonical.starts_with(&configured_parent)) {
            return Err(PackageError::Invalid);
        }
        let metadata = fs::metadata(&canonical).map_err(|_| PackageError::Invalid)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != identity_dev || metadata.ino() != identity_ino {
                return Err(PackageError::Invalid);
            }
        }
        #[cfg(not(unix))]
        let _ = (identity_dev, identity_ino, metadata);
        let snapshot_root = if exact_file {
            canonical.parent().ok_or(PackageError::Invalid)?
        } else {
            &canonical
        };
        let config = BrokerConfig::new(std::iter::empty::<&str>(), vec![configured_parent])
            .map_err(|_| PackageError::Invalid)?;
        let mut files =
            read_snapshot_tree(&config, snapshot_root).map_err(|_| PackageError::Invalid)?;
        if exact_file {
            let filename = canonical
                .file_name()
                .ok_or(PackageError::Invalid)?
                .to_string_lossy();
            files.retain(|file| file.path == filename);
        }
        if files.is_empty() {
            return Err(PackageError::Invalid);
        }
        Ok(files)
    }
    fn bridge_config_path(&self) -> PathBuf {
        self.root.join("bridge-config.json")
    }

    fn bridge_key(id: &str, version: &str) -> String {
        format!("{id}@{version}")
    }

    fn bridge_roots(&self, id: &str, version: &str) -> Result<Vec<PathBuf>, PackageError> {
        let config = Self::lock(&self.bridge_configs)
            .configs
            .get(&Self::bridge_key(id, version))
            .cloned()
            .ok_or(PackageError::Invalid)?;
        Ok(vec![
            PathBuf::from(config.vault_root),
            self.root.join("bridge-state").join(id).join(version),
        ])
    }

    pub fn bridge_config(
        &self,
        id: &str,
        version: &str,
    ) -> Result<BridgeConfigStatus, PackageError> {
        let package = self.store.installed(id, version)?;
        if !matches!(package.manifest.kind(), PackageKind::Bridge) {
            return Err(PackageError::Invalid);
        }
        let config = Self::lock(&self.bridge_configs)
            .configs
            .get(&Self::bridge_key(id, version))
            .cloned();
        Ok(BridgeConfigStatus {
            configured: config.is_some(),
            config,
            worker_state: self
                .worker
                .as_ref()
                .map(|worker| worker.supervisor.health(id, version).state)
                .unwrap_or(WorkerState::Stopped),
            worker_status: self.worker.as_ref().and_then(|worker| {
                worker
                    .supervisor
                    .diagnostics()
                    .into_iter()
                    .find(|item| item.id == id && item.version == version)
                    .and_then(|item| item.bridge_status)
            }),
        })
    }

    /// Stop before changing the exact filesystem root, then persist the new grant source.
    pub async fn set_bridge_config(
        &self,
        id: &str,
        version: &str,
        config: BridgeConfig,
    ) -> Result<BridgeConfigStatus, PackageError> {
        validate_bridge_config(&config)?;
        let package = self.store.installed(id, version)?;
        if !matches!(package.manifest.kind(), PackageKind::Bridge) {
            return Err(PackageError::Invalid);
        }
        let was_enabled = package.enabled;
        if let Some(worker) = self.worker.as_ref() {
            let _worker_mutation = worker.mutation.lock().await;
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
        }
        {
            let mut configs = Self::lock(&self.bridge_configs);
            ensure_owner_only_directory(&self.root.join("bridge-state").join(id).join(version))
                .map_err(|_| PackageError::Persistence)?;
            configs
                .configs
                .insert(Self::bridge_key(id, version), config);
            write_owner_only_json(&self.bridge_config_path(), &*configs)
                .map_err(|_| PackageError::Persistence)?;
        }
        if was_enabled {
            // The old grant was revoked by stop(); this starts with only the newly persisted roots.
            self.set_enabled(id, version, true).await?;
        }
        self.bridge_config(id, version)
    }

    pub fn configure_workers(
        &mut self,
        supervisor: PackageWorkerSupervisor,
        roots: Vec<PathBuf>,
        correlation_id: String,
    ) {
        self.worker = Some(WorkerRuntime {
            supervisor,
            roots,
            correlation_id,
            mutation: tokio::sync::Mutex::new(()),
        });
        if let Some(worker) = self.worker.as_ref() {
            worker.supervisor.bind_store(self.store.clone());
        }
    }

    /// Install the host-compiled v2 grant that will be bound to the next
    /// authenticated worker launch. This is intentionally private to the
    /// Engine: the registry snapshot is canonical ARK data, never renderer
    /// input.
    fn register_typed_manifest(
        &self,
        manifest: ManifestV2,
        registry: &RegistrySnapshot,
        manifest_digest: &str,
    ) -> Result<(), PackageError> {
        manifest.validate().map_err(|_| PackageError::Invalid)?;
        let grant = compile_manifest_v2(&manifest, registry, manifest_digest)
            .map_err(|_| PackageError::Invalid)?;
        Self::lock(&self.typed_grants).insert((manifest.id, manifest.version), grant);
        Ok(())
    }

    fn rebuild_typed_grants(&self) -> Result<(), PackageError> {
        let mut grants = HashMap::new();
        let registry = Self::lock(&self.typed_registry);
        for package in self.store.list()? {
            let VersionedManifest::V2(manifest) = package.manifest else {
                let _ = self.store.disable(&package.id, &package.version);
                continue;
            };
            let Ok(digest) = manifest_digest(&manifest) else {
                let _ = self.store.disable(&package.id, &package.version);
                continue;
            };
            let Ok(grant) = compile_manifest_v2(&manifest, &registry, &digest) else {
                let _ = self.store.disable(&package.id, &package.version);
                continue;
            };
            grants.insert((manifest.id, manifest.version), grant);
        }
        *Self::lock(&self.typed_grants) = grants;
        Ok(())
    }

    fn refresh_typed_registry(&self) -> Result<(), PackageError> {
        let mut registry = canonical_registry_snapshot()?;
        registry.types.extend(
            self.package_registrations
                .registered_types()
                .map_err(|_| PackageError::Persistence)?,
        );
        *Self::lock(&self.typed_registry) = registry;
        Ok(())
    }

    fn restore_install_after_failure(
        &self,
        definition_snapshot: Option<&[u8]>,
        previous: Option<&InstalledPackage>,
        id: &str,
        version: &str,
    ) -> Result<(), PackageError> {
        if let Some(snapshot) = definition_snapshot {
            self.package_registrations
                .restore(snapshot)
                .map_err(|_| PackageError::Persistence)?;
        }
        self.store.uninstall(id, version)?;
        let Some(previous) = previous else {
            self.refresh_typed_registry()?;
            return self.rebuild_typed_grants();
        };
        let archive = self
            .root
            .join("blobs")
            .join(format!("{}.kspkg", previous.hash));
        let size = fs::metadata(&archive)
            .map_err(|_| PackageError::Persistence)?
            .len();
        self.store.install_versioned(
            &archive,
            size,
            &previous.hash,
            &previous.manifest,
            previous.catalog_sequence,
        )?;
        self.store.restore_record(previous)?;
        self.refresh_typed_registry()?;
        self.rebuild_typed_grants()
    }

    fn ensure_typed_grant(&self, package: &InstalledPackage) -> Result<LaunchGrant, PackageError> {
        let VersionedManifest::V2(manifest) = &package.manifest else {
            return Err(PackageError::Invalid);
        };
        let digest = manifest_digest(manifest)?;
        let registry = Self::lock(&self.typed_registry);
        self.register_typed_manifest(manifest.clone(), &registry, &digest)?;
        Self::lock(&self.typed_grants)
            .get(&(package.id.clone(), package.version.clone()))
            .cloned()
            .ok_or(PackageError::Invalid)
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn disable_trust(state: &mut State) {
        state.trust = None;
        state.catalog = None;
        state.fault = Some("package_trust_unavailable".into());
    }

    fn disable_catalog(state: &mut State) {
        state.catalog = None;
        state.fault = Some("catalog_unavailable".into());
    }

    fn replay(&self) {
        let _mutation = Self::lock(&self.mutation);
        let mut state = Self::lock(&self.state);
        if state.trust.is_none() {
            return;
        }
        for path in self.document_paths("transition-") {
            if self.replay_transition(&mut state, &path).is_err() {
                Self::disable_trust(&mut state);
                return;
            }
        }
        for path in self.document_paths("revocation-") {
            if self.replay_revocation(&mut state, &path).is_err() {
                Self::disable_trust(&mut state);
                return;
            }
        }
        let path = self.root.join("catalog.json");
        if path.exists() && self.replay_catalog(&mut state, &path).is_err() {
            // A stale/revoked/corrupt catalog must never erase configured trust.
            Self::disable_catalog(&mut state);
        }
    }

    fn document_paths(&self, prefix: &str) -> Vec<PathBuf> {
        let mut paths = fs::read_dir(&self.root)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".json"))
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    fn read_env(path: &Path) -> Result<(Vec<u8>, SignatureSet), PackageError> {
        let metadata = fs::metadata(path).map_err(|_| PackageError::Persistence)?;
        if !metadata.is_file() || metadata.len() > MAX_ENVELOPE {
            return Err(PackageError::Invalid);
        }
        let env: Envelope =
            serde_json::from_slice(&fs::read(path).map_err(|_| PackageError::Persistence)?)
                .map_err(|_| PackageError::Persistence)?;
        let bytes = STANDARD
            .decode(env.bytes)
            .map_err(|_| PackageError::Persistence)?;
        if bytes.len() > MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        Ok((bytes, env.signatures))
    }

    fn replay_transition(&self, state: &mut State, path: &Path) -> Result<(), PackageError> {
        let (bytes, signatures) = Self::read_env(path)?;
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        trust.verify_key_transition(&bytes, signatures.clone())?;
        trust.apply_key_transition(&bytes, signatures)?;
        Ok(())
    }

    fn replay_revocation(&self, state: &mut State, path: &Path) -> Result<(), PackageError> {
        let (bytes, signatures) = Self::read_env(path)?;
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        let verified = trust.verify_revocations(&bytes, signatures.clone())?;
        trust.apply_revocations(&bytes, signatures)?;
        self.store
            .reconcile_revocations(verified.document.revoked_packages.iter().map(|package| {
                (
                    package.id.as_str(),
                    package.version.as_str(),
                    package.sha256.as_str(),
                )
            }))?;
        Ok(())
    }

    fn replay_catalog(&self, state: &mut State, path: &Path) -> Result<(), PackageError> {
        let (bytes, signatures) = Self::read_env(path)?;
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        let verified = trust.verify_catalog(&bytes, signatures.clone())?;
        trust.apply_catalog(&bytes, signatures.clone())?;
        state.catalog = Some(CatalogState {
            document: verified.document,
            signatures,
        });
        state.fault = None;
        Ok(())
    }

    fn persist(
        &self,
        name: &str,
        bytes: &[u8],
        signatures: &SignatureSet,
    ) -> Result<(), PackageError> {
        if bytes.len() > MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        let envelope = Envelope {
            bytes: STANDARD.encode(bytes),
            signatures: signatures.clone(),
        };
        if serde_json::to_vec(&envelope)
            .map_err(|_| PackageError::Persistence)?
            .len() as u64
            > MAX_ENVELOPE
        {
            return Err(PackageError::Invalid);
        }
        write_owner_only_json(&self.root.join(name), &envelope)
            .map_err(|_| PackageError::Persistence)
    }

    fn current_entry(
        state: &mut State,
        id: &str,
        version: &str,
    ) -> Result<CatalogEntry, PackageError> {
        let Some(trust) = state.trust.as_ref() else {
            return Err(PackageError::TrustUnavailable);
        };
        let Some(catalog) = state.catalog.as_ref() else {
            return Err(PackageError::TrustUnavailable);
        };
        let expiry = DateTime::parse_from_rfc3339(&catalog.document.expires_at)
            .map_err(|_| PackageError::Invalid)?
            .with_timezone(&Utc);
        if expiry <= Utc::now()
            || !catalog
                .signatures
                .signatures
                .iter()
                .any(|signature| trust.is_release_key_trusted(&signature.key_id))
        {
            Self::disable_catalog(state);
            return Err(PackageError::TrustUnavailable);
        }
        let entry = trust
            .catalog_entry(&catalog.document, id, version)
            .cloned()
            .ok_or(PackageError::Invalid)?;
        trust.ensure_package_allowed(&entry)?;
        VersionReq::parse(entry.manifest.engine_api())
            .map_err(|_| PackageError::Invalid)?
            .matches(&Version::new(
                API_VERSION_CURRENT.major.into(),
                API_VERSION_CURRENT.minor.into(),
                API_VERSION_CURRENT.patch.into(),
            ))
            .then_some(entry)
            .ok_or(PackageError::Invalid)
    }

    pub fn list(&self) -> Result<PackageListSummary, PackageError> {
        self.list_filtered(None)
    }

    /// Manager-safe installed package view for Store. It is sourced from the
    /// Phase 5 compiled grant, never from Store catalog claims or manifests.
    pub fn store_installed_listings(&self) -> Result<Vec<InstalledListing>, PackageError> {
        let mut packages = self.store.list()?;
        packages.sort_by(|left, right| (&left.id, &left.version).cmp(&(&right.id, &right.version)));
        let grants = Self::lock(&self.typed_grants);
        Ok(packages
            .into_iter()
            .map(|package| {
                let effective_grants = match &package.manifest {
                    VersionedManifest::V1(_) => Vec::new(),
                    VersionedManifest::V2(_) => grants
                        .get(&(package.id.clone(), package.version.clone()))
                        .map(effective_grant_projections)
                        .unwrap_or_default(),
                };
                Ok(InstalledListing {
                    effective_grants,
                    id: package.id,
                    version: package.version,
                    kind: match package.manifest.kind() {
                        PackageKind::App => "app".into(),
                        PackageKind::Source => "source".into(),
                        PackageKind::Bridge => "bridge".into(),
                    },
                    enabled: package.enabled,
                    revoked: package.revoked,
                    publisher: package.manifest.publisher().to_owned(),
                })
            })
            .collect::<Result<Vec<_>, PackageError>>()?)
    }

    /// Store listings may only reference a currently trusted Package Index
    /// release. Catalog metadata never becomes package authority.
    pub fn has_catalog_release(&self, id: &str, version: &str, is_bridge: bool) -> bool {
        let state = Self::lock(&self.state);
        let (Some(trust), Some(catalog)) = (state.trust.as_ref(), state.catalog.as_ref()) else {
            return false;
        };
        let Some(entry) = trust.catalog_entry(&catalog.document, id, version) else {
            return false;
        };
        matches!(entry.manifest.kind(), PackageKind::Bridge) == is_bridge
            && trust.ensure_package_allowed(entry).is_ok()
    }

    pub fn list_filtered(
        &self,
        kind: Option<PackageKind>,
    ) -> Result<PackageListSummary, PackageError> {
        let packages = self.store.list()?;
        let state = Self::lock(&self.state);
        let packages = packages
            .into_iter()
            .filter(|package| {
                kind.as_ref()
                    .is_none_or(|kind| package.manifest.kind() == kind)
            })
            .collect::<Vec<_>>();
        let total = packages.len();
        Ok(PackageListSummary {
            packages: packages
                .into_iter()
                .take(MAX_LISTED_PACKAGES)
                .map(|package| {
                    let update_version = latest_update_version(&state, &package);
                    let mut summary = summary(package, self.worker.as_ref(), &self.store);
                    summary.update_version = update_version;
                    summary
                })
                .collect(),
            total,
            truncated: total > MAX_LISTED_PACKAGES,
        })
    }

    pub fn worker_diagnostics(&self) -> Vec<WorkerDiagnostics> {
        self.worker
            .as_ref()
            .map(|runtime| runtime.supervisor.diagnostics())
            .unwrap_or_default()
    }

    pub fn trust_summary(&self) -> TrustSummary {
        let state = Self::lock(&self.state);
        let catalog_seq = state
            .catalog
            .as_ref()
            .map_or(0, |catalog| catalog.document.sequence);
        let Some(trust) = state.trust.as_ref() else {
            return TrustSummary {
                trusted_release_keys: 0,
                revoked_release_keys: 0,
                revoked_packages: 0,
                sequence: 0,
                configured: false,
                fault_code: state.fault.clone(),
                catalog_sequence: catalog_seq,
                transition_sequence: 0,
                revocation_sequence: 0,
            };
        };
        let summary = trust.summary();
        TrustSummary {
            trusted_release_keys: summary.trusted_release_key_ids.len(),
            revoked_release_keys: summary.revoked_release_key_ids.len(),
            revoked_packages: summary.revoked_package_count,
            sequence: summary
                .catalog_sequence
                .max(summary.transition_sequence)
                .max(summary.revocation_sequence),
            configured: true,
            fault_code: state.fault.clone(),
            catalog_sequence: summary.catalog_sequence,
            transition_sequence: summary.transition_sequence,
            revocation_sequence: summary.revocation_sequence,
        }
    }

    pub fn catalog_summary(&self) -> Option<CatalogSummary> {
        Self::lock(&self.state)
            .catalog
            .as_ref()
            .map(|catalog| CatalogSummary {
                sequence: catalog.document.sequence,
                expires_at: catalog.document.expires_at.clone(),
                package_count: catalog.document.packages.len(),
            })
    }

    pub fn storage_root(&self) -> &Path {
        &self.root
    }

    /// Returns the immutable, Engine-verified package type definitions for
    /// registration in ARK.  This is intentionally a snapshot: ARK owns the
    /// database transaction and package code never receives a DB handle.
    pub fn package_type_registrations(
        &self,
    ) -> Result<Vec<ark_core::type_registry::TypeRegistration>, PackageError> {
        self.package_registrations
            .type_registrations()
            .map_err(|_| PackageError::Persistence)
    }

    pub fn configure_package_definition_dispatcher(
        &self,
        dispatcher: std::sync::Arc<crate::engine_dispatch::EngineDispatcher>,
    ) {
        *Self::lock(&self.package_definition_dispatcher) = Some(dispatcher);
    }

    pub(crate) async fn register_package_definitions(
        &self,
        dispatcher: &crate::engine_dispatch::EngineDispatcher,
    ) -> Result<(), PackageError> {
        let registrations = self.package_type_registrations()?;
        if registrations.is_empty() {
            return Ok(());
        }
        let response = dispatcher
            .dispatch(crate::engine_dispatch::DispatchRequest {
                request_id: Some("package-definition-registration".into()),
                operation: crate::engine_dispatch::Operation::Named(
                    "types.registerPackageDefinitions".into(),
                ),
                params: serde_json::json!({ "registrations": registrations }),
                client: crate::engine_dispatch::DispatchClient::default(),
            })
            .await
            .map_err(|_| PackageError::Invalid)?;
        if response.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
            Ok(())
        } else {
            Err(PackageError::Invalid)
        }
    }

    fn register_configured_package_definitions(&self) -> Result<(), PackageError> {
        let Some(dispatcher) = Self::lock(&self.package_definition_dispatcher).clone() else {
            return Ok(());
        };
        let handle =
            tokio::runtime::Handle::try_current().map_err(|_| PackageError::Persistence)?;
        tokio::task::block_in_place(|| {
            handle.block_on(self.register_package_definitions(&dispatcher))
        })
    }

    /// Resolve only an enabled, non-revoked App package for the authenticated
    /// Engine launch boundary. An omitted version selects the highest
    /// installed semver for the id.
    pub fn resolve_app(&self, id: &str, version: Option<&str>) -> Result<AppLaunch, PackageError> {
        if id.is_empty() || id.len() > 64 || id.chars().any(char::is_control) {
            return Err(PackageError::Invalid);
        }
        let mut packages = self
            .store
            .list()?
            .into_iter()
            .filter(|package| {
                package.id == id
                    && matches!(package.manifest.kind(), PackageKind::App)
                    && package.enabled
                    && !package.revoked
                    && version.is_none_or(|wanted| package.version == wanted)
            })
            .collect::<Vec<_>>();
        packages.sort_by(|left, right| {
            Version::parse(&right.version)
                .unwrap_or_else(|_| Version::new(0, 0, 0))
                .cmp(&Version::parse(&left.version).unwrap_or_else(|_| Version::new(0, 0, 0)))
        });
        let package = packages.into_iter().next().ok_or(PackageError::Invalid)?;
        let grant = self.ensure_typed_grant(&package)?;
        // Verify the immutable blob and entrypoint before minting a token.
        self.store
            .read_blob_entry(&package, package.manifest.entrypoint())?;
        Ok(AppLaunch { package, grant })
    }

    /// Compatibility alias for existing internal callers; HTTP launch lease
    /// creation is owned exclusively by the Engine API boundary.
    pub fn launch_app(&self, id: &str, version: Option<&str>) -> Result<AppLaunch, PackageError> {
        self.resolve_app(id, version)
    }

    /// Re-check package lifecycle and hash state, then read an asset from the
    /// immutable `.kspkg` blob. This is the only path used by the HTTP asset
    /// route; mutable unpacked files are never exposed.
    pub fn read_app_asset(
        &self,
        id: &str,
        version: &str,
        hash: &str,
        path: &str,
    ) -> Result<Vec<u8>, PackageError> {
        let package = self.store.installed(id, version)?;
        if !matches!(package.manifest.kind(), PackageKind::App)
            || !package.enabled
            || package.revoked
            || !package.hash.eq_ignore_ascii_case(hash)
        {
            return Err(PackageError::Invalid);
        }
        Ok(self.store.read_blob_entry(&package, path)?)
    }

    pub fn catalog_packages(
        &self,
        kind: Option<&PackageKind>,
    ) -> Result<Vec<CatalogPackageSummary>, PackageError> {
        let state = Self::lock(&self.state);
        let catalog = state
            .catalog
            .as_ref()
            .ok_or(PackageError::TrustUnavailable)?;
        Ok(catalog
            .document
            .packages
            .iter()
            .filter(|entry| kind.is_none_or(|kind| entry.manifest.kind() == kind))
            .map(|entry| CatalogPackageSummary {
                id: entry.manifest.id().to_owned(),
                version: entry.manifest.version().to_owned(),
                name: entry.manifest.name().to_owned(),
                kind: entry.manifest.kind().clone(),
                publisher: entry.manifest.publisher().to_owned(),
                revoked: state
                    .trust
                    .as_ref()
                    .is_none_or(|trust| trust.ensure_package_allowed(entry).is_err()),
            })
            .collect())
    }

    pub async fn refresh_catalog(&self) -> Result<CatalogSummary, PackageError> {
        let url = std::env::var("KOSMOS_PACKAGE_CATALOG_URL")
            .unwrap_or_else(|_| PRODUCTION_CATALOG_URL.to_string());
        let parsed = reqwest::Url::parse(&url).map_err(|_| PackageError::Invalid)?;
        if parsed.scheme() != "https"
            || parsed.username() != ""
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err(PackageError::Invalid);
        }
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| PackageError::Invalid)?
            .get(parsed)
            .send()
            .await
            .map_err(|_| PackageError::Invalid)?;
        if !response.status().is_success() {
            return Err(PackageError::Invalid);
        }
        let body = response.bytes().await.map_err(|_| PackageError::Invalid)?;
        if body.len() > MAX_ENVELOPE as usize {
            return Err(PackageError::Invalid);
        }
        let envelope: Envelope =
            serde_json::from_slice(&body).map_err(|_| PackageError::Invalid)?;
        let bytes = STANDARD
            .decode(envelope.bytes)
            .map_err(|_| PackageError::Invalid)?;
        self.apply_catalog(bytes, envelope.signatures)
    }

    pub fn apply_catalog(
        &self,
        bytes: impl AsRef<[u8]>,
        signatures: SignatureSet,
    ) -> Result<CatalogSummary, PackageError> {
        let _mutation = Self::lock(&self.mutation);
        let bytes = bytes.as_ref();
        if bytes.len() > MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        let mut state = Self::lock(&self.state);
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        let verified = trust.verify_catalog(bytes, signatures.clone())?;
        trust.apply_catalog(bytes, signatures.clone())?;
        if self.persist("catalog.json", bytes, &signatures).is_err() {
            Self::disable_trust(&mut state);
            return Err(PackageError::Persistence);
        }
        state.catalog = Some(CatalogState {
            document: verified.document.clone(),
            signatures,
        });
        state.fault = None;
        Ok(CatalogSummary {
            sequence: verified.document.sequence,
            expires_at: verified.document.expires_at,
            package_count: verified.document.packages.len(),
        })
    }

    pub fn apply_transition(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        if bytes.len() > MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        let mut state = Self::lock(&self.state);
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        let verified = trust.verify_key_transition(bytes, signatures.clone())?;
        if trust
            .apply_key_transition(bytes, signatures.clone())
            .is_err()
        {
            Self::disable_trust(&mut state);
            return Err(PackageError::TrustUnavailable);
        }
        if self
            .persist(
                &format!("transition-{:020}.json", verified.document.sequence),
                bytes,
                &signatures,
            )
            .is_err()
        {
            Self::disable_trust(&mut state);
            return Err(PackageError::Persistence);
        }
        Ok(())
    }

    pub fn apply_revocations(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        if bytes.len() > MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        let mut state = Self::lock(&self.state);
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        let verified = trust.verify_revocations(bytes, signatures.clone())?;
        if trust.apply_revocations(bytes, signatures.clone()).is_err() {
            Self::disable_trust(&mut state);
            return Err(PackageError::TrustUnavailable);
        }
        let revoked = verified.document.revoked_packages.iter().map(|package| {
            (
                package.id.as_str(),
                package.version.as_str(),
                package.sha256.as_str(),
            )
        });
        if self.store.reconcile_revocations(revoked).is_err() {
            Self::disable_trust(&mut state);
            return Err(PackageError::TrustUnavailable);
        }
        if self
            .persist(
                &format!("revocation-{:020}.json", verified.document.sequence),
                bytes,
                &signatures,
            )
            .is_err()
        {
            Self::disable_trust(&mut state);
            return Err(PackageError::Persistence);
        }
        let catalog_untrusted = state.catalog.as_ref().is_some_and(|catalog| {
            catalog
                .signatures
                .signatures
                .iter()
                .all(|signature| !trust.is_release_key_trusted(&signature.key_id))
        });
        if catalog_untrusted {
            Self::disable_catalog(&mut state);
        }
        Ok(())
    }

    pub async fn apply_revocations_with_worker_stop(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<(), PackageError> {
        let targets = {
            let state = Self::lock(&self.state);
            let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
            trust
                .verify_revocations(bytes, signatures.clone())?
                .document
                .revoked_packages
                .into_iter()
                .map(|package| (package.id, package.version))
                .collect::<Vec<_>>()
        };
        if let Some(worker) = self.worker.as_ref() {
            let _worker_mutation = worker.mutation.lock().await;
            for (id, version) in &targets {
                worker
                    .supervisor
                    .stop(id, version)
                    .await
                    .map_err(|_| PackageError::Persistence)?;
            }
            return self.apply_revocations(bytes, signatures);
        }
        self.apply_revocations(bytes, signatures)
    }

    pub fn install_from_path(
        &self,
        id: &str,
        version: &str,
        path: impl AsRef<Path>,
    ) -> Result<PackageSummary, PackageError> {
        let _mutation = Self::lock(&self.mutation);
        let path = path.as_ref();
        if id.len() > 64
            || version.len() > 64
            || !path.is_absolute()
            || path.as_os_str().len() > 4096
        {
            return Err(PackageError::Invalid);
        }
        let mut state = Self::lock(&self.state);
        let entry = Self::current_entry(&mut state, id, version)?;
        let sequence = state
            .catalog
            .as_ref()
            .map(|catalog| catalog.document.sequence)
            .ok_or(PackageError::TrustUnavailable)?;
        let definition_documents = match &entry.manifest {
            VersionedManifest::V2(manifest) if !manifest.data.defines.is_empty() => {
                let documents = definition_documents_from_archive(path, manifest)?;
                self.package_registrations
                    .validate_manifest(manifest, &documents)
                    .map_err(|_| PackageError::Invalid)?;
                Some((manifest.clone(), documents))
            }
            _ => None,
        };
        let definition_snapshot = definition_documents
            .as_ref()
            .map(|_| self.package_registrations.snapshot())
            .transpose()
            .map_err(|_| PackageError::Persistence)?;
        let previous = self.store.installed(id, version).ok();
        if self.worker.as_ref().is_some_and(|worker| {
            !matches!(
                worker.supervisor.health(id, version).state,
                WorkerState::Stopped
            )
        }) {
            // Synchronous callers cannot safely stop a worker. The async
            // wrapper below owns that lifecycle transition; fail closed for
            // any direct caller that bypasses it.
            return Err(PackageError::Persistence);
        }
        let package = self.store.install_versioned(
            path,
            entry.size,
            &entry.sha256,
            &entry.manifest,
            sequence,
        )?;
        if let Some((manifest, documents)) = definition_documents {
            if let Err(error) = self
                .package_registrations
                .register_manifest(&manifest, &documents)
            {
                let rollback = self.restore_install_after_failure(
                    definition_snapshot.as_deref(),
                    previous.as_ref(),
                    &package.id,
                    &package.version,
                );
                if let Err(rollback) = rollback {
                    return Err(rollback);
                }
                return Err(match error {
                    crate::package_registration::RegistrationError::Persistence => {
                        PackageError::Persistence
                    }
                    _ => PackageError::Invalid,
                });
            }
            if let Err(error) = self.refresh_typed_registry() {
                self.restore_install_after_failure(
                    definition_snapshot.as_deref(),
                    previous.as_ref(),
                    &package.id,
                    &package.version,
                )?;
                return Err(error);
            }
            if let Err(error) = self.ensure_typed_grant(&package) {
                self.restore_install_after_failure(
                    definition_snapshot.as_deref(),
                    previous.as_ref(),
                    &package.id,
                    &package.version,
                )?;
                return Err(error);
            }
            if let Err(error) = self.register_configured_package_definitions() {
                self.restore_install_after_failure(
                    definition_snapshot.as_deref(),
                    previous.as_ref(),
                    &package.id,
                    &package.version,
                )?;
                return Err(error);
            }
        } else if let Err(error) = self.ensure_typed_grant(&package) {
            self.restore_install_after_failure(
                None,
                previous.as_ref(),
                &package.id,
                &package.version,
            )?;
            return Err(error);
        }
        Ok(summary(package, self.worker.as_ref(), &self.store))
    }

    /// Replace a package only after its worker has been stopped and its grant
    /// revoked. If installation fails, the exact prior record is restored by
    /// `install_from_path`; an enabled worker is restarted only after that
    /// record is verified byte-for-byte in the store.
    pub async fn install_from_path_with_worker_stop(
        &self,
        id: &str,
        version: &str,
        path: impl AsRef<Path>,
    ) -> Result<PackageSummary, PackageError> {
        let previous = self.store.installed(id, version).ok();
        let worker_guard = if let Some(worker) = self.worker.as_ref() {
            Some(worker.mutation.lock().await)
        } else {
            None
        };
        let worker_was_live = self.worker.as_ref().is_some_and(|worker| {
            !matches!(
                worker.supervisor.health(id, version).state,
                WorkerState::Stopped
            )
        });
        let should_restart = worker_was_live
            && previous
                .as_ref()
                .is_some_and(|package| package.enabled && !package.revoked);
        let definition_snapshot = should_restart
            .then(|| self.package_registrations.snapshot())
            .transpose()
            .map_err(|_| PackageError::Persistence)?;
        if worker_was_live {
            let worker = self.worker.as_ref().ok_or(PackageError::Persistence)?;
            if worker.supervisor.stop(id, version).await.is_err() {
                return Err(PackageError::Persistence);
            }
        }

        let result = self.install_from_path(id, version, path);
        drop(worker_guard);
        match result {
            Ok(summary) if !should_restart => Ok(summary),
            Ok(_) => match self.set_enabled(id, version, true).await {
                Ok(summary) => Ok(summary),
                Err(error) => {
                    if let Some(worker) = self.worker.as_ref() {
                        let _ = worker.supervisor.stop(id, version).await;
                    }
                    let Some(previous) = previous.as_ref() else {
                        return Err(PackageError::Persistence);
                    };
                    let restored = self.restore_install_after_failure(
                        definition_snapshot.as_deref(),
                        Some(previous),
                        id,
                        version,
                    );
                    if restored.is_err()
                        || self.store.installed(id, version).ok().as_ref() != Some(previous)
                    {
                        return Err(PackageError::Persistence);
                    }
                    Err(error)
                }
            },
            Err(error) => {
                if !should_restart {
                    return Err(error);
                }
                let Some(previous) = previous.as_ref() else {
                    return Err(PackageError::Persistence);
                };
                if !restored_package_record_matches(&self.store, previous, id, version) {
                    if let Some(worker) = self.worker.as_ref() {
                        let _ = worker.supervisor.stop(id, version).await;
                    }
                    let _ = self.disable(id, version);
                    return Err(PackageError::Persistence);
                }
                if self.set_enabled(id, version, true).await.is_err() {
                    if let Some(worker) = self.worker.as_ref() {
                        let _ = worker.supervisor.stop(id, version).await;
                    }
                    return Err(PackageError::Persistence);
                }
                Err(error)
            }
        }
    }

    pub async fn install_from_catalog(
        &self,
        id: &str,
        version: &str,
    ) -> Result<PackageSummary, PackageError> {
        let (url, expected_size) = {
            let mut state = Self::lock(&self.state);
            let entry = Self::current_entry(&mut state, id, version)?;
            (entry.archive_url, entry.size)
        };
        let parsed = reqwest::Url::parse(&url).map_err(|_| PackageError::Invalid)?;
        if parsed.scheme() != "https"
            || parsed.username() != ""
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err(PackageError::Invalid);
        }
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|_| PackageError::Invalid)?
            .get(parsed)
            .send()
            .await
            .map_err(|_| PackageError::Invalid)?;
        if !response.status().is_success() {
            return Err(PackageError::Invalid);
        }
        if response
            .content_length()
            .is_some_and(|size| size != expected_size)
        {
            return Err(PackageError::Invalid);
        }
        let bytes = response.bytes().await.map_err(|_| PackageError::Invalid)?;
        if bytes.len() as u64 != expected_size {
            return Err(PackageError::Invalid);
        }
        let path = self.root.join(format!(".download-{id}-{version}.kspkg"));
        fs::write(&path, &bytes).map_err(|_| PackageError::Persistence)?;
        let result = self
            .install_from_path_with_worker_stop(id, version, &path)
            .await;
        let _ = fs::remove_file(path);
        result
    }

    pub fn enable(&self, id: &str, version: &str) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        let mut state = Self::lock(&self.state);
        let entry = Self::current_entry(&mut state, id, version)?;
        let installed = self
            .store
            .list()?
            .into_iter()
            .find(|package| package.id == id && package.version == version)
            .ok_or(PackageError::Invalid)?;
        if !installed.hash.eq_ignore_ascii_case(&entry.sha256) {
            return Err(PackageError::Invalid);
        }
        self.ensure_typed_grant(&installed)?;
        self.store.enable(id, version)?;
        Ok(())
    }

    pub async fn set_enabled(
        &self,
        id: &str,
        version: &str,
        enabled: bool,
    ) -> Result<PackageSummary, PackageError> {
        let installed = self.store.installed(id, version)?;
        let typed_bound = self.ensure_typed_grant(&installed)?;
        if matches!(installed.manifest.kind(), PackageKind::App) {
            if enabled {
                self.enable(id, version)?;
            } else {
                self.disable(id, version)?;
            }
            return Ok(summary(
                self.store.installed(id, version)?,
                self.worker.as_ref(),
                &self.store,
            ));
        }
        let worker = self.worker.as_ref().ok_or(PackageError::Invalid)?;
        let _worker_mutation = worker.mutation.lock().await;
        if !enabled {
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
            self.disable(id, version)?;
            return Ok(summary(
                self.store.installed(id, version)?,
                self.worker.as_ref(),
                &self.store,
            ));
        }
        let launch = {
            let _mutation = Self::lock(&self.mutation);
            let mut state = Self::lock(&self.state);
            let entry = Self::current_entry(&mut state, id, version)?;
            let installed = self.store.installed(id, version)?;
            if installed.revoked || !installed.hash.eq_ignore_ascii_case(&entry.sha256) {
                return Err(PackageError::Invalid);
            }
            let executable = self.store.immutable_entrypoint(&installed)?;
            let bridge_config = if matches!(installed.manifest.kind(), PackageKind::Bridge) {
                let config = Self::lock(&self.bridge_configs)
                    .configs
                    .get(&Self::bridge_key(id, version))
                    .cloned()
                    .ok_or(PackageError::Invalid)?;
                let state_root = self.root.join("bridge-state").join(id).join(version);
                Some(BridgeWorkerConfig {
                    vault_root: config.vault_root,
                    state_root: state_root.to_string_lossy().into_owned(),
                    selected_types: config.selected_types,
                    editable_fields: config.editable_fields,
                    readonly_fields: config.readonly_fields,
                })
            } else {
                None
            };
            let roots = if matches!(installed.manifest.kind(), PackageKind::Bridge) {
                self.bridge_roots(id, version)?
            } else {
                worker.roots.clone()
            };
            (
                installed.manifest.common_manifest(),
                installed.hash,
                executable,
                roots,
                bridge_config,
            )
        };
        worker
            .supervisor
            .bind_typed_launch(id, version, &worker.correlation_id, 1, typed_bound)
            .map_err(|_| {
                worker.supervisor.revoke_typed_launch(id, version);
                PackageError::Invalid
            })?;
        worker
            .supervisor
            .start(
                &launch.0,
                launch.2,
                launch.1,
                &launch.3,
                worker.correlation_id.clone(),
                launch.4,
            )
            .await
            .map_err(|_| {
                worker.supervisor.revoke_typed_launch(id, version);
                PackageError::Invalid
            })?;
        if self.store.enable_worker(id, version).is_err() {
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
            worker.supervisor.revoke_typed_launch(id, version);
            return Err(PackageError::Persistence);
        }
        if !worker.supervisor.activate(id, version) {
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
            worker.supervisor.revoke_typed_launch(id, version);
            let _ = self.store.disable(id, version);
            return Err(PackageError::Invalid);
        }
        Ok(summary(
            self.store.installed(id, version)?,
            self.worker.as_ref(),
            &self.store,
        ))
    }

    pub fn disable(&self, id: &str, version: &str) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        if let Some(worker) = self.worker.as_ref() {
            worker.supervisor.revoke_typed_launch(id, version);
        }
        self.store.disable(id, version)?;
        Ok(())
    }

    pub fn uninstall(&self, id: &str, version: &str) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        if let Some(worker) = self.worker.as_ref() {
            worker.supervisor.revoke_typed_launch(id, version);
        }
        self.store.uninstall(id, version)?;
        Self::lock(&self.typed_grants).remove(&(id.to_owned(), version.to_owned()));
        Ok(())
    }

    pub async fn uninstall_with_worker_stop(
        &self,
        id: &str,
        version: &str,
    ) -> Result<(), PackageError> {
        if let Some(worker) = self.worker.as_ref() {
            let _worker_mutation = worker.mutation.lock().await;
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
        }
        self.uninstall(id, version)
    }
}

fn restored_package_record_matches(
    store: &PackageStore,
    previous: &InstalledPackage,
    id: &str,
    version: &str,
) -> bool {
    store
        .installed(id, version)
        .ok()
        .is_some_and(|restored| restored == *previous)
}

fn summary(
    package: InstalledPackage,
    worker: Option<&WorkerRuntime>,
    store: &PackageStore,
) -> PackageSummary {
    let health = worker.map(|runtime| runtime.supervisor.health(&package.id, &package.version));
    let icon_path = package
        .manifest
        .icon()
        .and_then(|icon| store.immutable_asset_path(&package, icon).ok())
        .map(|path| {
            let path = path.to_string_lossy();
            path.strip_prefix(r"\\?\")
                .unwrap_or(path.as_ref())
                .to_owned()
        });
    PackageSummary {
        id: package.id,
        name: package.manifest.name().to_owned(),
        version: package.version,
        kind: package.manifest.kind().clone(),
        enabled: package.enabled,
        revoked: package.revoked,
        worker_state: health
            .as_ref()
            .map(|health| health.state)
            .unwrap_or(WorkerState::Stopped),
        worker_restart_count: health
            .map(|health| health.restart_count)
            .unwrap_or_default(),
        publisher: package.manifest.publisher().to_owned(),
        icon_path,
        update_version: None,
    }
}

fn definition_documents_from_archive(
    archive_path: &Path,
    manifest: &ManifestV2,
) -> Result<BTreeMap<String, Vec<u8>>, PackageError> {
    let mut archive =
        ZipArchive::new(fs::File::open(archive_path).map_err(|_| PackageError::Invalid)?)
            .map_err(|_| PackageError::Invalid)?;
    let paths = manifest
        .data
        .defines
        .iter()
        .flat_map(|definition| {
            std::iter::once(&definition.schema)
                .chain(definition.content_contract.iter())
                .chain(definition.relations.iter())
        })
        .collect::<Vec<_>>();
    let mut documents = BTreeMap::new();
    let mut total = 0usize;
    for path in paths {
        if documents.contains_key(path) {
            continue;
        }
        let mut file = archive.by_name(path).map_err(|_| PackageError::Invalid)?;
        if file.is_dir() || file.size() > 512 * 1024 {
            return Err(PackageError::Invalid);
        }
        total = total.saturating_add(file.size() as usize);
        if total > 8 * 1024 * 1024 {
            return Err(PackageError::Invalid);
        }
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes)
            .map_err(|_| PackageError::Invalid)?;
        documents.insert(path.clone(), bytes);
    }
    Ok(documents)
}

fn definition_documents_from_store(
    store: &PackageStore,
    package: &InstalledPackage,
) -> Result<BTreeMap<String, Vec<u8>>, PackageError> {
    let VersionedManifest::V2(manifest) = &package.manifest else {
        return Ok(BTreeMap::new());
    };
    let mut documents = BTreeMap::new();
    for path in manifest.data.defines.iter().flat_map(|definition| {
        std::iter::once(&definition.schema)
            .chain(definition.content_contract.iter())
            .chain(definition.relations.iter())
    }) {
        if documents.contains_key(path) {
            continue;
        }
        let bytes = store.read_blob_entry(package, path)?;
        documents.insert(path.clone(), bytes);
    }
    Ok(documents)
}

fn canonical_registry_snapshot() -> Result<RegistrySnapshot, PackageError> {
    let registrations = ark_core::canonical_types::definitions::canonical_type_registrations()
        .map_err(|_| PackageError::Invalid)?;
    let types = registrations
        .into_iter()
        .map(|registration| {
            let schema: Value = serde_json::from_str(&registration.schema_json)
                .map_err(|_| PackageError::Invalid)?;
            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .ok_or(PackageError::Invalid)?;
            let mut fields = ["title", "content"]
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            fields.extend(properties.keys().map(|name| format!("props.{name}")));

            let relations: Value = serde_json::from_str(&registration.relations_json)
                .map_err(|_| PackageError::Invalid)?;
            let relations = relations
                .as_array()
                .ok_or(PackageError::Invalid)?
                .iter()
                .map(|relation| {
                    relation
                        .get("type")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or(PackageError::Invalid)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(RegisteredType {
                type_id: registration.type_id,
                version: registration.version,
                fields,
                relations,
            })
        })
        .collect::<Result<Vec<_>, PackageError>>()?;
    Ok(RegistrySnapshot::new(types))
}

fn manifest_digest(manifest: &ManifestV2) -> Result<String, PackageError> {
    let bytes = serde_json::to_vec(manifest).map_err(|_| PackageError::Invalid)?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn effective_grant_projections(grant: &LaunchGrant) -> Vec<EffectiveGrantProjection> {
    grant
        .rules
        .iter()
        .map(|rule| {
            let mut roles = std::collections::BTreeSet::new();
            if rule.actions.contains("read") {
                roles.insert(Role::Read);
            }
            if rule.actions.contains("create") || rule.actions.contains("update") {
                roles.insert(Role::Edit);
            }
            EffectiveGrantProjection {
                type_id: rule.type_id.clone(),
                version: rule.versions.join(","),
                roles,
                fields_read: rule.fields_read.clone(),
                fields_write: rule.fields_write.clone(),
                relations_read: rule.relations_read.clone(),
                relations_write: rule.relations_write.clone(),
            }
        })
        .collect()
}

fn read_bridge_configs(root: &Path) -> BridgeConfigState {
    let path = root.join("bridge-config.json");
    fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn validate_bridge_config(config: &BridgeConfig) -> Result<(), PackageError> {
    const MAX_ITEMS: usize = 64;
    let root = PathBuf::from(&config.vault_root);
    if config.vault_root.is_empty()
        || config.vault_root.len() > 4096
        || !root.is_absolute()
        || config.vault_root.starts_with(r"\\")
        || root.components().any(|part| {
            matches!(
                part,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || !fs::canonicalize(&root)
            .ok()
            .is_some_and(|path| path.is_dir())
        || config.selected_types.is_empty()
        || config.selected_types.len() > MAX_ITEMS
        || config.editable_fields.len() > MAX_ITEMS
        || config.readonly_fields.len() > MAX_ITEMS
        || !config
            .selected_types
            .iter()
            .all(|value| valid_bridge_name(value))
        || !config
            .editable_fields
            .iter()
            .all(|value| valid_bridge_name(value))
        || !config
            .readonly_fields
            .iter()
            .all(|value| valid_bridge_name(value))
    {
        return Err(PackageError::Invalid);
    }
    Ok(())
}

fn valid_bridge_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' || byte == b'.'
        })
}

fn latest_update_version(state: &State, package: &InstalledPackage) -> Option<String> {
    let trust = state.trust.as_ref()?;
    let catalog = state.catalog.as_ref()?;
    let installed = Version::parse(&package.version).ok()?;
    catalog
        .document
        .packages
        .iter()
        .filter(|entry| entry.manifest.id() == package.id)
        .filter(|entry| trust.ensure_package_allowed(entry).is_ok())
        .filter_map(|entry| {
            let version = Version::parse(entry.manifest.version()).ok()?;
            (version > installed).then_some((version, entry.manifest.version().to_owned()))
        })
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, version)| version)
}

#[cfg(test)]
#[allow(clippy::panic)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        package_manifest::PermissionRequest,
        package_trust::{DetachedSignature, KeyTransitionDocument, PackageRevocation},
    };
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    use std::{fs::File, io::Write};
    use tempfile::tempdir;
    use zip::write::FileOptions;

    fn key(seed: u8, id: &str) -> (SigningKey, TrustedKey) {
        let signing = SigningKey::from_bytes(&[seed; 32]);
        let public_key = STANDARD.encode(signing.verifying_key().as_bytes());
        (
            signing,
            TrustedKey {
                key_id: id.into(),
                public_key,
            },
        )
    }

    fn trust() -> (TrustStore, SigningKey, SigningKey) {
        let (root_signing, root) = key(1, "root");
        let (release_signing, release) = key(2, "release-1");
        (
            TrustStore::new(root, vec![release]).expect("test trust"),
            root_signing,
            release_signing,
        )
    }

    fn manifest() -> ManifestV2 {
        ManifestV2 {
            schema_version: 2,
            id: "com.kosmos.demo".into(),
            name: "Demo".into(),
            version: "1.0.0".into(),
            kind: PackageKind::App,
            engine_api: ">=1.0.0".into(),
            entrypoint: "index.html".into(),
            icon: None,
            publisher: "kosmos".into(),
            permissions: vec![],
            targets: vec![crate::package_manifest::ManifestTarget {
                runtime: crate::package_manifest::TargetRuntime::KosmosHost,
                os: vec![crate::package_manifest::TargetOs::Windows],
                arch: None,
            }],
            data: crate::package_manifest::ManifestData {
                access: vec![],
                defines: vec![],
                mappings: vec![],
            },
        }
    }

    fn legacy_manifest() -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: "com.kosmos.demo".into(),
            name: "Demo".into(),
            version: "1.0.0".into(),
            kind: PackageKind::App,
            engine_api: ">=1.0.0".into(),
            entrypoint: "index.html".into(),
            publisher: "kosmos".into(),
            permissions: vec![],
        }
    }

    fn manifest_v2_with_canonical_access() -> ManifestV2 {
        ManifestV2 {
            schema_version: 2,
            id: "com.kosmos.demo".into(),
            name: "Demo v2".into(),
            version: "2.0.0".into(),
            kind: PackageKind::App,
            engine_api: ">=1.0.0".into(),
            entrypoint: "index.html".into(),
            icon: None,
            publisher: "kosmos".into(),
            permissions: vec![],
            targets: vec![crate::package_manifest::ManifestTarget {
                runtime: crate::package_manifest::TargetRuntime::KosmosHost,
                os: vec![crate::package_manifest::TargetOs::Windows],
                arch: None,
            }],
            data: crate::package_manifest::ManifestData {
                access: vec![crate::package_manifest::DataAccessRule {
                    type_id: "com.kosmos.note".into(),
                    versions: "^1.0.0".into(),
                    actions: vec![crate::package_manifest::DataAction::Read],
                    fields: crate::package_manifest::FieldAccess {
                        read: vec!["props.description".into()],
                        write: vec![],
                    },
                    relations: Some(crate::package_manifest::RelationAccess {
                        read: vec!["related".into()],
                        write: vec![],
                    }),
                }],
                defines: vec![],
                mappings: vec![],
            },
        }
    }

    fn catalog(sequence: u64, hash: String, size: u64, expires_at: &str) -> CatalogDocument {
        CatalogDocument {
            schema_version: 1,
            sequence,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: expires_at.into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V2(manifest()),
                archive_url: "https://packages.kosmos.dev/demo.kspkg".into(),
                sha256: hash,
                size,
            }],
        }
    }

    fn signed<T: Serialize>(
        document: &T,
        key_id: &str,
        key: &SigningKey,
    ) -> (Vec<u8>, SignatureSet) {
        let bytes = serde_json::to_vec(document).expect("test json");
        let signature = DetachedSignature {
            key_id: key_id.into(),
            algorithm: "ed25519".into(),
            signature: STANDARD.encode(key.sign(&bytes).to_bytes()),
        };
        (
            bytes,
            SignatureSet {
                schema_version: 1,
                signatures: vec![signature],
            },
        )
    }

    fn archive(root: &Path) -> (PathBuf, String, u64) {
        archive_with_versioned_manifest(root, &VersionedManifest::V2(manifest()))
    }

    fn archive_with_manifest(
        root: &Path,
        package_manifest: &PackageManifest,
    ) -> (PathBuf, String, u64) {
        let path = root.join("demo.kspkg");
        let file = File::create(&path).expect("archive file");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default())
            .expect("manifest entry");
        zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
            .expect("manifest write");
        zip.start_file("index.html", FileOptions::default())
            .expect("entrypoint entry");
        zip.write_all(b"ok").expect("entrypoint write");
        zip.finish().expect("archive finish");
        let bytes = fs::read(&path).expect("archive bytes");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        (path, hash, bytes.len() as u64)
    }

    fn archive_with_versioned_manifest(
        root: &Path,
        package_manifest: &VersionedManifest,
    ) -> (PathBuf, String, u64) {
        let path = root.join("demo-v2.kspkg");
        let file = File::create(&path).expect("archive file");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default())
            .expect("manifest entry");
        zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
            .expect("manifest write");
        zip.start_file("index.html", FileOptions::default())
            .expect("entrypoint entry");
        zip.write_all(b"ok").expect("entrypoint write");
        if let Some(icon) = package_manifest.icon() {
            zip.start_file(icon, FileOptions::default())
                .expect("icon entry");
            zip.write_all(b"icon").expect("icon write");
        }
        zip.finish().expect("archive finish");
        let bytes = fs::read(&path).expect("archive bytes");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        (path, hash, bytes.len() as u64)
    }

    #[test]
    fn package_summary_exposes_a_verified_icon_path() {
        let dir = tempdir().expect("temp dir");
        let mut manifest = manifest();
        manifest.icon = Some("icon.ico".into());
        let expected = VersionedManifest::V2(manifest);
        let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &expected);
        let store = PackageStore::new(dir.path().join("packages")).expect("store");
        let package = store
            .install_versioned(&archive, size, &hash, &expected, 1)
            .expect("install");

        let listed = summary(package, None, &store);
        assert_eq!(listed.name, "Demo");
        let icon = listed.icon_path.expect("icon path");
        assert!(icon.ends_with("icon.ico"));
        assert!(!icon.starts_with(r"\\?\"));
    }

    fn archive_with_versioned_manifest_and_documents(
        root: &Path,
        package_manifest: &VersionedManifest,
        documents: &[(&str, &[u8])],
    ) -> (PathBuf, String, u64) {
        let path = root.join("demo-defined.kspkg");
        let file = File::create(&path).expect("archive file");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default())
            .expect("manifest entry");
        zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
            .expect("manifest write");
        for (name, bytes) in documents {
            zip.start_file(*name, FileOptions::default())
                .expect("definition entry");
            zip.write_all(bytes).expect("definition write");
        }
        zip.start_file("index.html", FileOptions::default())
            .expect("entrypoint entry");
        zip.write_all(b"ok").expect("entrypoint write");
        zip.finish().expect("archive finish");
        let bytes = fs::read(&path).expect("archive bytes");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        (path, hash, bytes.len() as u64)
    }

    #[cfg(windows)]
    fn archive_bridge_binary(
        root: &Path,
        package_manifest: &VersionedManifest,
    ) -> (PathBuf, String, u64) {
        let path = root.join("bridge.kspkg");
        let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/ark-markdown-bridge.exe");
        assert!(
            binary.is_file(),
            "build ark-markdown-bridge before this test: {binary:?}"
        );
        let file = File::create(&path).expect("archive file");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default())
            .expect("manifest entry");
        zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
            .expect("manifest write");
        zip.start_file(package_manifest.entrypoint(), FileOptions::default())
            .expect("entrypoint entry");
        zip.write_all(&fs::read(binary).expect("bridge binary"))
            .expect("entrypoint write");
        zip.finish().expect("archive finish");
        let bytes = fs::read(&path).expect("archive bytes");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        (path, hash, bytes.len() as u64)
    }

    pub(crate) fn enabled_app_service(dir: &Path) -> (PackageService, PathBuf, String) {
        let (archive, hash, size) = archive(dir);
        let (trust, _, release) = trust();
        let service = PackageService::open_with_trust(dir, trust).expect("service");
        let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
        let (bytes, signatures) = signed(&doc, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        service
            .install_from_path("com.kosmos.demo", "1.0.0", &archive)
            .expect("install");
        service.enable("com.kosmos.demo", "1.0.0").expect("enable");
        (service, archive, hash)
    }

    #[test]
    fn missing_compile_time_trust_fails_closed_without_blocking_engine() {
        let dir = tempdir().expect("tempdir");
        let service =
            PackageService::from_parts(dir.path().join("packages"), None).expect("service");
        assert!(!service.trust_summary().configured);
        assert_eq!(
            service.trust_summary().fault_code.as_deref(),
            Some("package_trust_unavailable")
        );
    }

    #[test]
    fn production_open_uses_pinned_trust() {
        let dir = tempdir().expect("tempdir");
        let summary = PackageService::open(dir.path())
            .expect("service")
            .trust_summary();
        assert!(summary.configured);
        assert_eq!(summary.trusted_release_keys, 1);
        assert_eq!(summary.revoked_release_keys, 0);
    }

    #[test]
    fn signed_catalog_applies_and_replay_or_tamper_fails() {
        let dir = tempdir().expect("tempdir");
        let (trust_store, _, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let doc = catalog(1, "a".repeat(64), 1, "2030-01-01T00:00:00Z");
        let (bytes, signatures) = signed(&doc, "release-1", &release);
        assert_eq!(
            service
                .apply_catalog(&bytes, signatures.clone())
                .expect("catalog")
                .sequence,
            1
        );
        assert_eq!(
            service
                .catalog_packages(Some(&PackageKind::App))
                .expect("app catalog")
                .len(),
            1
        );
        assert!(service
            .catalog_packages(Some(&PackageKind::Source))
            .expect("source catalog")
            .is_empty());
        assert!(matches!(
            service.apply_catalog(&bytes, signatures),
            Err(PackageError::Trust(TrustError::Replay))
        ));
        let mut tampered = bytes;
        tampered[0] ^= 1;
        let (_, signatures) = signed(&doc, "release-1", &release);
        assert!(service.apply_catalog(tampered, signatures).is_err());
    }

    #[test]
    fn expired_or_revoked_catalog_clears_cache_but_accepts_fresh_catalog() {
        let dir = tempdir().expect("tempdir");
        let (trust_store, root, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let mut expired = catalog(1, "a".repeat(64), 1, "2025-01-01T00:00:00Z");
        expired.issued_at = "2024-01-01T00:00:00Z".into();
        let (expired_bytes, expired_signatures) = signed(&expired, "release-1", &release);
        assert!(service
            .apply_catalog(&expired_bytes, expired_signatures)
            .is_err());
        let (release_two, release_two_key) = key(3, "release-2");
        let transition = KeyTransitionDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            old_key_id: "release-1".into(),
            new_key: release_two_key,
        };
        let (transition_bytes, mut transition_signatures) =
            signed(&transition, "release-1", &release);
        transition_signatures.signatures.push(DetachedSignature {
            key_id: "release-2".into(),
            algorithm: "ed25519".into(),
            signature: STANDARD.encode(release_two.sign(&transition_bytes).to_bytes()),
        });
        service
            .apply_transition(&transition_bytes, transition_signatures)
            .expect("transition");
        let valid = catalog(2, "b".repeat(64), 1, "2030-01-01T00:00:00Z");
        let (valid_bytes, valid_signatures) = signed(&valid, "release-1", &release);
        service
            .apply_catalog(&valid_bytes, valid_signatures)
            .expect("valid catalog");
        let revoke = crate::package_trust::RevocationDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            revoked_release_keys: vec!["release-1".into()],
            revoked_packages: vec![],
        };
        let (revoke_bytes, revoke_signatures) = signed(&revoke, "root", &root);
        service
            .apply_revocations(&revoke_bytes, revoke_signatures)
            .expect("revocation");
        assert!(service.catalog_summary().is_none());
        assert_eq!(
            service.trust_summary().fault_code.as_deref(),
            Some("catalog_unavailable")
        );
        let fresh = catalog(3, "c".repeat(64), 1, "2030-01-01T00:00:00Z");
        let (fresh_bytes, fresh_signatures) = signed(&fresh, "release-2", &release_two);
        assert_eq!(
            service
                .apply_catalog(fresh_bytes, fresh_signatures)
                .expect("fresh catalog")
                .sequence,
            3
        );
    }

    #[test]
    fn transition_and_revocation_replay_are_rejected() {
        let dir = tempdir().expect("tempdir");
        let (trust_store, root, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let (release_two, release_two_key) = key(3, "release-2");
        let transition = KeyTransitionDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            old_key_id: "release-1".into(),
            new_key: release_two_key,
        };
        let (bytes, mut signatures) = signed(&transition, "release-1", &release);
        signatures.signatures.push(DetachedSignature {
            key_id: "release-2".into(),
            algorithm: "ed25519".into(),
            signature: STANDARD.encode(release_two.sign(&bytes).to_bytes()),
        });
        service
            .apply_transition(&bytes, signatures.clone())
            .expect("transition");
        assert!(service.apply_transition(&bytes, signatures).is_err());
        let revocation = crate::package_trust::RevocationDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            revoked_release_keys: vec!["release-1".into()],
            revoked_packages: vec![],
        };
        let (bytes, signatures) = signed(&revocation, "root", &root);
        service
            .apply_revocations(&bytes, signatures.clone())
            .expect("revocation");
        assert!(service.apply_revocations(&bytes, signatures).is_err());
    }

    #[test]
    fn catalog_bound_archive_installs_and_enable_refuses_hash_mismatch() {
        let dir = tempdir().expect("tempdir");
        let (archive, hash, size) = archive(dir.path());
        let (trust_store, _, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
        let (bytes, signatures) = signed(&doc, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        assert_eq!(
            service
                .install_from_path("com.kosmos.demo", "1.0.0", &archive)
                .expect("install")
                .id,
            "com.kosmos.demo"
        );
        service.enable("com.kosmos.demo", "1.0.0").expect("enable");

        let replacement = catalog(2, "c".repeat(64), size, "2030-01-01T00:00:00Z");
        let (bytes, signatures) = signed(&replacement, "release-1", &release);
        service
            .apply_catalog(bytes, signatures)
            .expect("replacement catalog");
        assert!(service.enable("com.kosmos.demo", "1.0.0").is_err());
    }

    #[test]
    fn summaries_contain_no_sensitive_package_fields() {
        let dir = tempdir().expect("tempdir");
        let (trust, _, _) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust).expect("service");
        let json = serde_json::to_string(&service.list().expect("list")).expect("json");
        for forbidden in [
            "sha256",
            "hash",
            "entrypoint",
            "manifest",
            "signature",
            "public_key",
            "path",
        ] {
            assert!(!json.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn root_signed_exact_revocation_marks_installed_package() {
        let dir = tempdir().expect("tempdir");
        let (archive, hash, size) = archive(dir.path());
        let (trust_store, root, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
        let (bytes, signatures) = signed(&doc, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        service
            .install_from_path("com.kosmos.demo", "1.0.0", archive)
            .expect("install");
        let revoke = crate::package_trust::RevocationDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            revoked_release_keys: vec![],
            revoked_packages: vec![PackageRevocation {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                sha256: hash,
            }],
        };
        let (bytes, signatures) = signed(&revoke, "root", &root);
        service
            .apply_revocations(&bytes, signatures)
            .expect("revoke");
        assert!(service.list().expect("list").packages[0].revoked);
        let (fresh_trust, _, _) = trust();
        let restarted = PackageService::open_with_trust(dir.path(), fresh_trust).expect("restart");
        assert!(restarted.list().expect("restarted list").packages[0].revoked);
    }

    #[test]
    fn app_launch_asset_lifecycle_and_immutable_tamper_boundary() {
        let dir = tempdir().expect("tempdir");
        let (service, _archive, hash) = enabled_app_service(dir.path());
        let launch = service.launch_app("com.kosmos.demo", None).expect("launch");
        assert_eq!(launch.package.hash, hash);
        assert_eq!(
            service
                .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "index.html")
                .expect("asset"),
            b"ok"
        );
        assert!(service
            .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "../index.html")
            .is_err());
        service
            .disable("com.kosmos.demo", "1.0.0")
            .expect("disable");
        assert!(service.launch_app("com.kosmos.demo", None).is_err());
        assert!(service
            .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "index.html")
            .is_err());

        let tamper_dir = tempdir().expect("tamper tempdir");
        let (service, _archive, hash) = enabled_app_service(tamper_dir.path());
        let blob = tamper_dir
            .path()
            .join("packages/blobs")
            .join(format!("{hash}.kspkg"));
        let mut bytes = fs::read(&blob).expect("blob");
        bytes[0] ^= 1;
        fs::write(blob, bytes).expect("tamper");
        assert!(service
            .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "index.html")
            .is_err());
    }

    #[test]
    fn v2_grants_compile_from_canonical_registry_and_survive_restart() {
        let dir = tempdir().expect("tempdir");
        let manifest = manifest_v2_with_canonical_access();
        let versioned = VersionedManifest::V2(manifest.clone());
        let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &versioned);
        let expected_hash = hash.clone();
        let (trust_store, _, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let catalog = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: versioned,
                archive_url: "https://packages.kosmos.dev/demo-v2.kspkg".into(),
                sha256: hash,
                size,
            }],
        };
        let (bytes, signatures) = signed(&catalog, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        service
            .install_from_path(&manifest.id, &manifest.version, &archive)
            .expect("install");
        let listing = service
            .store_installed_listings()
            .expect("installed listing");
        assert_eq!(listing.len(), 1);
        assert_eq!(listing[0].effective_grants.len(), 1);
        assert_eq!(listing[0].effective_grants[0].type_id, "com.kosmos.note");
        assert_eq!(
            listing[0].effective_grants[0].fields_read,
            ["props.description"]
        );
        assert_eq!(listing[0].effective_grants[0].relations_read, ["related"]);
        service
            .enable(&manifest.id, &manifest.version)
            .expect("enable");
        assert_eq!(
            service
                .launch_app(&manifest.id, Some(&manifest.version))
                .expect("launch")
                .package
                .hash,
            expected_hash
        );

        drop(service);
        let (trust_store, _, _) = trust();
        let restarted = PackageService::open_with_trust(dir.path(), trust_store).expect("restart");
        let listing = restarted
            .store_installed_listings()
            .expect("restarted listing");
        assert_eq!(listing[0].effective_grants[0].type_id, "com.kosmos.note");
        assert!(restarted
            .launch_app(&manifest.id, Some(&manifest.version))
            .is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn package_definitions_are_archive_bound_registered_in_ark_and_survive_uninstall() {
        let dir = tempdir().expect("tempdir");
        let mut manifest = manifest_v2_with_canonical_access();
        manifest.id = "com.kosmos.demo.definitions".into();
        manifest.name = "Defined v2".into();
        manifest.version = "1.0.0".into();
        manifest.data.access = vec![crate::package_manifest::DataAccessRule {
            type_id: "com.kosmos.demo.definitions.journal".into(),
            versions: "^1.0.0".into(),
            actions: vec![crate::package_manifest::DataAction::Read],
            fields: crate::package_manifest::FieldAccess {
                read: vec!["props.body".into()],
                write: vec![],
            },
            relations: Some(crate::package_manifest::RelationAccess {
                read: vec!["related".into()],
                write: vec![],
            }),
        }];
        manifest.data.defines = vec![crate::package_manifest::DefinitionReference {
            type_id: "com.kosmos.demo.definitions.journal".into(),
            version: "1.0.0".into(),
            schema: "schemas/journal.schema.json".into(),
            content_contract: Some("schemas/journal.content.json".into()),
            relations: Some("schemas/journal.relations.json".into()),
        }];
        manifest.validate().expect("definition manifest");
        let versioned = VersionedManifest::V2(manifest.clone());
        let documents = [
            (
                "schemas/journal.schema.json",
                br#"{"type":"object","properties":{"body":{"type":"string"}}}"# as &[u8],
            ),
            ("schemas/journal.content.json", br#"{"kind":"text"}"#),
            ("schemas/journal.relations.json", br#"[{"type":"related"}]"#),
        ];
        let (archive, hash, size) =
            archive_with_versioned_manifest_and_documents(dir.path(), &versioned, &documents);
        let docs = definition_documents_from_archive(&archive, &manifest).expect("documents");
        let probe = crate::package_registration::PackageRegistrationRegistry::open(
            dir.path().join("probe-definitions"),
        )
        .expect("probe registry");
        probe
            .validate_manifest(&manifest, &docs)
            .expect("definition contract");
        let (trust_store, _, release) = trust();
        let service = std::sync::Arc::new(
            PackageService::open_with_trust(dir.path(), trust_store).expect("service"),
        );
        let ark = std::sync::Arc::new(
            crate::ark_host::ArkHost::spawn(
                &crate::ark_host::resolve_ark_core_rpc_path().expect("ark-core-rpc binary"),
                dir.path().join("ark.db").to_str().expect("db path"),
            )
            .await
            .expect("ark host"),
        );
        let dispatcher = package_definition_dispatcher(ark.clone());
        service.configure_package_definition_dispatcher(dispatcher.clone());
        let catalog = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: versioned,
                archive_url: "https://packages.kosmos.dev/defined.kspkg".into(),
                sha256: hash,
                size,
            }],
        };
        let (bytes, signatures) = signed(&catalog, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        {
            let service = service.clone();
            let id = manifest.id.clone();
            let version = manifest.version.clone();
            tokio::task::spawn_blocking(move || service.install_from_path(&id, &version, &archive))
                .await
                .expect("install task")
                .expect("install");
        }
        assert_eq!(
            ark.request(
                "types.get",
                serde_json::json!({ "typeId": manifest.data.defines[0].type_id }),
            )
            .await
            .expect("ARK response")
            .data
            .pointer("/summary/ownerKind")
            .and_then(serde_json::Value::as_str),
            Some("package")
        );
        let listing = service.store_installed_listings().expect("listing");
        assert_eq!(
            listing[0].effective_grants[0].type_id,
            manifest.data.defines[0].type_id
        );
        assert_eq!(listing[0].effective_grants[0].fields_read, ["props.body"]);
        let persisted = dir.path().join("packages/definitions/definitions.json");
        assert!(persisted.is_file());
        service
            .uninstall(&manifest.id, &manifest.version)
            .expect("uninstall");
        assert!(
            persisted.is_file(),
            "definitions are retained after uninstall"
        );
        drop(service);
        drop(dispatcher);
        drop(ark);
        let (trust_store, _, _) = trust();
        let restarted = PackageService::open_with_trust(dir.path(), trust_store).expect("restart");
        let ark = std::sync::Arc::new(
            crate::ark_host::ArkHost::spawn(
                &crate::ark_host::resolve_ark_core_rpc_path().expect("ark-core-rpc binary"),
                dir.path().join("ark.db").to_str().expect("db path"),
            )
            .await
            .expect("ARK restart"),
        );
        let dispatcher = package_definition_dispatcher(ark.clone());
        let definitions = fs::read_to_string(&persisted).expect("definitions");
        assert!(definitions.contains(&manifest.data.defines[0].type_id));
        restarted.configure_package_definition_dispatcher(dispatcher.clone());
        crate::engine_api::register_package_definitions(&restarted, &dispatcher)
            .await
            .expect("idempotent replay after restart");
        assert_eq!(
            ark.request(
                "types.listVersions",
                serde_json::json!({ "typeId": manifest.data.defines[0].type_id }),
            )
            .await
            .expect("ARK response")
            .data
            .as_array()
            .expect("version list")
            .len(),
            1
        );
        assert!(restarted
            .store_installed_listings()
            .expect("listing")
            .is_empty());
    }

    fn package_definition_dispatcher(
        ark: std::sync::Arc<crate::ark_host::ArkHost>,
    ) -> std::sync::Arc<crate::engine_dispatch::EngineDispatcher> {
        std::sync::Arc::new(crate::engine_dispatch::EngineDispatcher::new(
            std::sync::Arc::new(move |request| {
                let ark = ark.clone();
                Box::pin(async move {
                    let response = ark
                        .request(request.operation.as_str(), request.params)
                        .await
                        .map_err(|error| {
                            crate::engine_dispatch::DispatchError::Failed(error.to_string())
                        })?;
                    Ok(serde_json::json!({
                        "ok": response.ok,
                        "data": response.data,
                        "error": response.error,
                    }))
                })
            }),
        ))
    }

    #[tokio::test]
    async fn ark_conflict_rolls_back_to_the_enabled_package() {
        let dir = tempdir().unwrap();
        let prior = manifest_v2_with_canonical_access();
        let prior_versioned = VersionedManifest::V2(prior.clone());
        let (prior_archive, prior_hash, prior_size) =
            archive_with_versioned_manifest(dir.path(), &prior_versioned);
        let (trust_store, _, release) = trust();
        let service =
            std::sync::Arc::new(PackageService::open_with_trust(dir.path(), trust_store).unwrap());
        let initial = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: prior_versioned,
                archive_url: "https://packages.kosmos.dev/prior.kspkg".into(),
                sha256: prior_hash.clone(),
                size: prior_size,
            }],
        };
        let (bytes, signatures) = signed(&initial, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        service
            .install_from_path(&prior.id, &prior.version, &prior_archive)
            .unwrap();
        service.enable(&prior.id, &prior.version).unwrap();
        let ark = std::sync::Arc::new(
            crate::ark_host::ArkHost::spawn(
                &crate::ark_host::resolve_ark_core_rpc_path().unwrap(),
                dir.path().join("ark.db").to_str().unwrap(),
            )
            .await
            .unwrap(),
        );
        service.configure_package_definition_dispatcher(package_definition_dispatcher(ark.clone()));
        let type_id = "com.kosmos.demo.journal";
        assert!(ark.request("types.registerPackageDefinitions", serde_json::json!({"registrations":[{
            "type_id":type_id,"name":"Foreign","schema_json":"{}","ui_schema_json":"{}","content_contract_json":"{}","relations_json":"[]","sync_policy_json":"{}","version":"1.0.0","schema_hash":"","owner_kind":"package","owner_id":"com.example.foreign","status":"active","base_type_id":null,"aliases":[],"created_at":"now"
        }]})).await.unwrap().ok);
        let mut replacement = manifest_v2_with_canonical_access();
        replacement.id = prior.id.clone();
        replacement.version = prior.version.clone();
        replacement.data.access[0].fields.read = vec!["props.title".into()];
        replacement.data.defines = vec![crate::package_manifest::DefinitionReference {
            type_id: type_id.into(),
            version: "1.0.0".into(),
            schema: "schema.json".into(),
            content_contract: None,
            relations: None,
        }];
        replacement.validate().unwrap();
        let (archive, hash, size) = archive_with_versioned_manifest_and_documents(
            dir.path(),
            &VersionedManifest::V2(replacement.clone()),
            &[("schema.json", br#"{}"#)],
        );
        let update = CatalogDocument {
            schema_version: 1,
            sequence: 2,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V2(replacement),
                archive_url: "https://packages.kosmos.dev/update.kspkg".into(),
                sha256: hash,
                size,
            }],
        };
        let (bytes, signatures) = signed(&update, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        let install = {
            let service = service.clone();
            let id = prior.id.clone();
            let version = prior.version.clone();
            tokio::task::spawn_blocking(move || service.install_from_path(&id, &version, archive))
                .await
                .unwrap()
        };
        assert!(install.is_err());
        assert_eq!(
            service
                .resolve_app(&prior.id, Some(&prior.version))
                .unwrap()
                .package
                .hash,
            prior_hash
        );
        assert!(service.list().unwrap().packages[0].enabled);
        let listing = service.store_installed_listings().unwrap();
        assert_eq!(listing[0].effective_grants.len(), 1);
        assert_eq!(
            listing[0].effective_grants[0].fields_read,
            ["props.description"]
        );
        assert!(
            service
                .package_type_registrations()
                .unwrap()
                .iter()
                .all(|registration| registration.type_id != type_id),
            "a failed install must not persist the rejected definition"
        );
    }

    #[test]
    fn failed_update_restores_revoked_record_without_reenabling_it() {
        let dir = tempdir().unwrap();
        let prior = manifest_v2_with_canonical_access();
        let prior_versioned = VersionedManifest::V2(prior.clone());
        let (prior_archive, prior_hash, prior_size) =
            archive_with_versioned_manifest(dir.path(), &prior_versioned);
        let (trust_store, root, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).unwrap();
        let initial = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: prior_versioned,
                archive_url: "https://packages.kosmos.dev/prior.kspkg".into(),
                sha256: prior_hash.clone(),
                size: prior_size,
            }],
        };
        let (bytes, signatures) = signed(&initial, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        service
            .install_from_path(&prior.id, &prior.version, &prior_archive)
            .unwrap();
        let revocation = crate::package_trust::RevocationDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            revoked_release_keys: vec![],
            revoked_packages: vec![PackageRevocation {
                id: prior.id.clone(),
                version: prior.version.clone(),
                sha256: prior_hash,
            }],
        };
        let (bytes, signatures) = signed(&revocation, "root", &root);
        service.apply_revocations(&bytes, signatures).unwrap();
        let before = service.store.installed(&prior.id, &prior.version).unwrap();
        assert!(before.revoked);
        assert!(!before.enabled);

        let mut replacement = prior.clone();
        replacement.data.access[0].type_id = "com.kosmos.unknown".into();
        let replacement_versioned = VersionedManifest::V2(replacement.clone());
        let (replacement_archive, replacement_hash, replacement_size) =
            archive_with_versioned_manifest(dir.path(), &replacement_versioned);
        let update = CatalogDocument {
            schema_version: 1,
            sequence: 2,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: replacement_versioned,
                archive_url: "https://packages.kosmos.dev/replacement.kspkg".into(),
                sha256: replacement_hash,
                size: replacement_size,
            }],
        };
        let (bytes, signatures) = signed(&update, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();

        assert!(service
            .install_from_path(&replacement.id, &replacement.version, replacement_archive)
            .is_err());
        assert_eq!(
            service.store.installed(&prior.id, &prior.version).unwrap(),
            before,
            "rollback must preserve revoked/enabled/timestamp/catalog metadata"
        );
    }

    #[test]
    fn restart_gate_requires_the_exact_prior_package_record() {
        let dir = tempdir().unwrap();
        let (archive, hash, size) = archive(dir.path());
        let store = PackageStore::new(dir.path().join("store")).unwrap();
        store
            .install_versioned(&archive, size, &hash, &VersionedManifest::V2(manifest()), 1)
            .unwrap();
        store.enable("com.kosmos.demo", "1.0.0").unwrap();
        let previous = store.installed("com.kosmos.demo", "1.0.0").unwrap();

        assert!(restored_package_record_matches(
            &store,
            &previous,
            &previous.id,
            &previous.version
        ));
        store.disable(&previous.id, &previous.version).unwrap();
        assert!(!restored_package_record_matches(
            &store,
            &previous,
            &previous.id,
            &previous.version
        ));
    }

    #[test]
    fn live_v2_manifests_compile_with_wildcards() {
        let registry = canonical_registry_snapshot().expect("canonical registry");
        for package in ["delphi", "cosmos-graph"] {
            let raw = fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../../products/{package}/manifest.json")),
            )
            .unwrap_or_else(|_| panic!("{package} manifest"));
            let VersionedManifest::V2(manifest) =
                PackageManifest::parse(&raw).unwrap_or_else(|_| panic!("{package} parse"))
            else {
                panic!("{package} manifest must be v2");
            };
            let grant = compile_manifest_v2(&manifest, &registry, "test-digest")
                .unwrap_or_else(|_| panic!("{package} grant"));
            assert!(!grant.rules.is_empty(), "{package} grant has no rules");
        }
    }

    #[test]
    fn invalid_installed_v2_contract_does_not_brick_restart() {
        let dir = tempdir().expect("tempdir");
        let mut manifest = manifest_v2_with_canonical_access();
        manifest.data.access[0].type_id = "com.kosmos.unknown".into();
        let versioned = VersionedManifest::V2(manifest.clone());
        let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &versioned);
        let (trust_store, _, _) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        service
            .store
            .install_versioned(&archive, size, &hash, &versioned, 1)
            .expect("install fixture");
        service
            .store
            .enable(&manifest.id, &manifest.version)
            .expect("enable fixture");
        drop(service);

        let (trust_store, _, _) = trust();
        let restarted = PackageService::open_with_trust(dir.path(), trust_store).expect("restart");
        assert!(!restarted.list().expect("list").packages[0].enabled);
        assert!(restarted.store_installed_listings().expect("listing")[0]
            .effective_grants
            .is_empty());
        assert!(restarted.enable(&manifest.id, &manifest.version).is_err());
    }

    #[test]
    fn legacy_v1_package_cannot_launch_or_survive_restart_enabled() {
        let dir = tempdir().expect("tempdir");
        let legacy = legacy_manifest();
        let (archive, hash, size) = archive_with_manifest(dir.path(), &legacy);
        let (trust_store, _, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let legacy_catalog = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V1(legacy.clone()),
                archive_url: "https://packages.kosmos.dev/demo.kspkg".into(),
                sha256: hash.clone(),
                size,
            }],
        };
        let (bytes, signatures) = signed(&legacy_catalog, "release-1", &release);
        service
            .apply_catalog(bytes, signatures)
            .expect("legacy catalog");
        assert!(service
            .install_from_path("com.kosmos.demo", "1.0.0", &archive)
            .is_err());
        assert!(service
            .store
            .install(&archive, size, &hash, &legacy, 1)
            .is_err());
        assert!(service.list().expect("list").packages.is_empty());
    }

    #[test]
    fn source_package_never_resolves_as_app() {
        let dir = tempdir().expect("tempdir");
        let mut source = manifest();
        source.kind = PackageKind::Source;
        let versioned = VersionedManifest::V2(source.clone());
        let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &versioned);
        let service = PackageService::open(dir.path()).expect("service");
        service
            .store
            .install_versioned(&archive, size, &hash, &versioned, 1)
            .expect("install source");
        assert!(service.launch_app("com.kosmos.demo", None).is_err());
    }

    #[test]
    fn bridge_config_requires_one_real_vault_and_persists_owner_state() {
        let dir = tempdir().expect("tempdir");
        let vault = dir.path().join("vault");
        fs::create_dir(&vault).expect("vault");
        let config = BridgeConfig {
            vault_root: vault.to_string_lossy().into_owned(),
            selected_types: vec!["com.kosmos.note".into()],
            editable_fields: vec!["title".into(), "body".into()],
            readonly_fields: vec!["machine_output".into()],
        };
        assert!(validate_bridge_config(&config).is_ok());
        let state = BridgeConfigState {
            configs: HashMap::from([("ark-markdown-bridge@1.0.0".into(), config)]),
        };
        let packages = dir.path().join("packages");
        fs::create_dir(&packages).expect("packages");
        write_owner_only_json(&packages.join("bridge-config.json"), &state).expect("write");
        assert_eq!(read_bridge_configs(&packages).configs, state.configs);
        assert!(validate_bridge_config(&BridgeConfig {
            vault_root: r"\\server\vault".into(),
            selected_types: vec!["note".into()],
            editable_fields: vec![],
            readonly_fields: vec![]
        })
        .is_err());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn signed_catalog_bridge_runs_through_service() {
        use crate::{
            ark_host::{resolve_ark_core_rpc_path, ArkHost},
            package_worker_supervisor::PackageWorkerSupervisor,
        };
        use std::sync::Arc;
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir(&vault).unwrap();
        let manifest = ManifestV2 {
            schema_version: 2,
            id: "ark-markdown-bridge".into(),
            name: "ARK Markdown Bridge".into(),
            version: "1.0.0".into(),
            kind: PackageKind::Bridge,
            engine_api: ">=1".into(),
            entrypoint: "ark-markdown-bridge.exe".into(),
            icon: None,
            publisher: "kosmos".into(),
            permissions: vec![
                PermissionRequest {
                    capability: "ark.read".into(),
                    scopes: vec!["list_objects".into(), "get_object".into()],
                },
                PermissionRequest {
                    capability: "ark.write".into(),
                    scopes: vec!["upsert_object".into(), "external_refs.upsert".into()],
                },
                PermissionRequest {
                    capability: "filesystem.read".into(),
                    scopes: vec![],
                },
                PermissionRequest {
                    capability: "filesystem.write".into(),
                    scopes: vec![],
                },
            ],
            targets: vec![crate::package_manifest::ManifestTarget {
                runtime: crate::package_manifest::TargetRuntime::Worker,
                os: vec![crate::package_manifest::TargetOs::Windows],
                arch: None,
            }],
            data: crate::package_manifest::ManifestData {
                access: vec![],
                defines: vec![],
                mappings: vec![],
            },
        };
        let versioned = VersionedManifest::V2(manifest.clone());
        let (archive, hash, size) = archive_bridge_binary(dir.path(), &versioned);
        let (trust, _, release) = trust();
        let mut service = PackageService::open_with_trust(dir.path(), trust).unwrap();
        let catalog = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V2(manifest.clone()),
                archive_url: "https://packages.kosmos.dev/bridge.kspkg".into(),
                sha256: hash,
                size,
            }],
        };
        let (bytes, signatures) = signed(&catalog, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        service
            .install_from_path(&manifest.id, &manifest.version, archive)
            .unwrap();
        let ark = Arc::new(
            ArkHost::spawn(
                &resolve_ark_core_rpc_path().unwrap(),
                dir.path().join("ark.db").to_str().unwrap(),
            )
            .await
            .unwrap(),
        );
        assert!(ark.request("upsert_object_type", serde_json::json!({"object_type":{"id":"note","name":"Note","schemaJson":"{}","uiSchemaJson":"{}","createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","systemLocked":false},"device_id":"bridge-service"})).await.unwrap().ok);
        assert!(ark.request("upsert_object", serde_json::json!({"object":{"id":"service-note","typeId":"note","title":"Service note","contentJson":{},"propsJson":{"body":"from ark"},"createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","deletedAt":null},"device_id":"bridge-service"})).await.unwrap().ok);
        let supervisor = PackageWorkerSupervisor::with_ark(1, ark.clone());
        service.configure_workers(supervisor.clone(), vec![], "bridge-service".into());
        service
            .set_bridge_config(
                &manifest.id,
                &manifest.version,
                BridgeConfig {
                    vault_root: vault.to_string_lossy().into_owned(),
                    selected_types: vec!["note".into()],
                    editable_fields: vec!["title".into(), "body".into()],
                    readonly_fields: vec![],
                },
            )
            .await
            .unwrap();
        if let Err(error) = service
            .set_enabled(&manifest.id, &manifest.version, true)
            .await
        {
            panic!(
                "bridge enable failed: {error:?}; diagnostics: {:?}",
                supervisor.diagnostics()
            );
        }
        let markdown = vault.join("Service note-service-note.md");
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            while !markdown.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        let content = fs::read_to_string(&markdown).unwrap();
        fs::write(&markdown, content.replace("from ark", "from vault")).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            loop {
                let current = ark
                    .request("get_object", serde_json::json!({"id":"service-note"}))
                    .await
                    .unwrap();
                if current.data["propsJson"]["body"] == "from vault" {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();

        let mut replacement = manifest.clone();
        replacement.name = "ARK Markdown Bridge Updated".into();
        let (replacement_archive, replacement_hash, replacement_size) =
            archive_bridge_binary(dir.path(), &VersionedManifest::V2(replacement.clone()));
        let update = CatalogDocument {
            schema_version: 1,
            sequence: 2,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V2(replacement.clone()),
                archive_url: "https://packages.kosmos.dev/bridge-update.kspkg".into(),
                sha256: replacement_hash,
                size: replacement_size,
            }],
        };
        let (bytes, signatures) = signed(&update, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        let updated = service
            .install_from_path_with_worker_stop(
                &replacement.id,
                &replacement.version,
                replacement_archive,
            )
            .await
            .unwrap();
        assert!(updated.enabled);
        assert_eq!(updated.worker_state, WorkerState::Running);
        assert_eq!(
            service
                .store
                .installed(&replacement.id, &replacement.version)
                .unwrap()
                .manifest
                .common_manifest()
                .name,
            replacement.name
        );

        service
            .set_enabled(&manifest.id, &manifest.version, false)
            .await
            .unwrap();
        assert_eq!(
            supervisor.health(&manifest.id, &manifest.version).state,
            WorkerState::Stopped
        );
    }
}
