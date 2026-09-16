//! Immutable, fail-closed `.kspkg` archive store. This module never executes package code.

use crate::lock_file::write_owner_only_json;
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

const MAX_ARCHIVE: u64 = 128 * 1024 * 1024;
const MAX_EXPANDED: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 512;
pub(crate) const MAX_ASSET_BYTES: u64 = 16 * 1024 * 1024;
const STATE_FORMAT_VERSION: u32 = 1;
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

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
        fs::create_dir_all(root.join("blobs"))?;
        fs::create_dir_all(root.join("unpacked"))?;
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

    pub fn immutable_entrypoint(&self, package: &InstalledPackage) -> Result<PathBuf, StoreError> {
        let entrypoint = package
            .manifest
            .worker_entrypoint()
            .ok_or(StoreError::WorkerRequired)?;
        let canonical = self.immutable_asset_path(package, entrypoint)?;
        if !canonical
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        {
            return Err(StoreError::Archive("invalid worker entrypoint".into()));
        }
        Ok(canonical)
    }

    /// Return a verified, unpacked static asset path for an installed package.
    pub fn immutable_asset_path(
        &self,
        package: &InstalledPackage,
        entry: &str,
    ) -> Result<PathBuf, StoreError> {
        if !safe_asset_path(entry) {
            return Err(StoreError::Archive("invalid asset path".into()));
        }
        let root = self
            .root
            .join("unpacked")
            .join(&package.id)
            .join(&package.version)
            .join(&package.hash);
        let path = root.join(entry);
        let canonical_root = fs::canonicalize(&root).map_err(StoreError::Io)?;
        let canonical = fs::canonicalize(&path).map_err(StoreError::Io)?;
        if !canonical.starts_with(&canonical_root) || !canonical.is_file() {
            return Err(StoreError::Archive("invalid asset path".into()));
        }
        let blob = self
            .root
            .join("blobs")
            .join(format!("{}.kspkg", package.hash));
        if !eq_hash(&hash_reader(fs::File::open(&blob)?)?, &package.hash) {
            return Err(StoreError::HashMismatch);
        }
        let mut archive = ZipArchive::new(fs::File::open(blob)?)?;
        let archived_hash = hash_reader(archive.by_name(entry)?)?;
        if archived_hash != hash_reader(fs::File::open(&canonical)?)? {
            return Err(StoreError::HashMismatch);
        }
        Ok(canonical)
    }

    /// Read one static asset directly from the verified immutable archive.
    /// Unpacked package files are deliberately never consulted here.
    pub fn read_blob_entry(
        &self,
        package: &InstalledPackage,
        entry: &str,
    ) -> Result<Vec<u8>, StoreError> {
        if !safe_asset_path(entry) {
            return Err(StoreError::Archive("invalid asset path".into()));
        }
        let blob = self
            .root
            .join("blobs")
            .join(format!("{}.kspkg", package.hash));
        let mut blob_file = open_immutable_read(&blob)?;
        if !eq_hash(&hash_reader(&mut blob_file)?, &package.hash) {
            return Err(StoreError::HashMismatch);
        }
        blob_file.seek(SeekFrom::Start(0))?;
        let mut archive = ZipArchive::new(blob_file)?;
        let file = archive
            .by_name(entry)
            .map_err(|_| StoreError::Archive("asset not found".into()))?;
        if file.is_dir() {
            return Err(StoreError::Archive("asset is a directory".into()));
        }
        if file.size() > MAX_ASSET_BYTES {
            return Err(StoreError::Archive("asset too large".into()));
        }
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.take(MAX_ASSET_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_ASSET_BYTES {
            return Err(StoreError::Archive("asset too large".into()));
        }
        Ok(bytes)
    }

    /// Re-check an unpacked entrypoint against its immutable archive bytes.
    /// The launcher calls this immediately before spawning a worker.
    pub(crate) fn verify_immutable_entrypoint_path(path: &Path) -> Result<(), StoreError> {
        let canonical = fs::canonicalize(path)?;
        let Some(hash_dir) = canonical.ancestors().find(|candidate| {
            let Some(name) = candidate.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            is_hash(name)
                && candidate
                    .parent()
                    .and_then(Path::parent)
                    .and_then(Path::parent)
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "unpacked")
        }) else {
            return Err(StoreError::Archive("invalid worker entrypoint".into()));
        };
        let hash = hash_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| StoreError::Archive("invalid worker entrypoint".into()))?;
        let relative = canonical
            .strip_prefix(hash_dir)
            .map_err(|_| StoreError::Archive("invalid worker entrypoint".into()))?;
        let entrypoint = relative.to_string_lossy().replace('\\', "/");
        if entrypoint.is_empty() || entrypoint.contains("..") {
            return Err(StoreError::Archive("invalid worker entrypoint".into()));
        }
        let store_root = hash_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or_else(|| StoreError::Archive("invalid worker entrypoint".into()))?;
        let blob = store_root.join("blobs").join(format!("{hash}.kspkg"));
        let mut blob_file = open_immutable_read(&blob)?;
        if !eq_hash(&hash_reader(&mut blob_file)?, hash) {
            return Err(StoreError::HashMismatch);
        }
        blob_file.seek(SeekFrom::Start(0))?;
        let mut archive = ZipArchive::new(blob_file)?;
        let archived_hash = hash_reader(archive.by_name(&entrypoint)?)?;
        let unpacked_hash = hash_reader(fs::File::open(canonical)?)?;
        if archived_hash != unpacked_hash {
            return Err(StoreError::HashMismatch);
        }
        Ok(())
    }

    pub fn enable_worker(&self, id: &str, version: &str) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read_state()?;
        let package = state
            .packages
            .iter()
            .find(|p| p.id == id && p.version == version)
            .ok_or_else(|| StoreError::State("package not found".into()))?;
        if package.revoked {
            return Err(StoreError::Revoked);
        }
        if package.manifest.worker_entrypoint().is_none() {
            return Err(StoreError::WorkerRequired);
        }
        for package in &mut state.packages {
            if package.id == id {
                package.enabled = package.version == version;
            }
        }
        self.write_state(&state)
    }

    /// Atomically clear the enabled bit for a set of worker versions.
    /// Callers stop those workers before invoking this method.
    pub fn disable_worker_versions(&self, id: &str, versions: &[String]) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read_state()?;
        for version in versions {
            let package = state
                .packages
                .iter()
                .find(|package| package.id == id && package.version == *version)
                .ok_or_else(|| StoreError::State("package not found".into()))?;
            if package.manifest.worker_entrypoint().is_none() {
                return Err(StoreError::WorkerRequired);
            }
        }
        for package in &mut state.packages {
            if package.id == id && versions.iter().any(|version| version == &package.version) {
                package.enabled = false;
            }
        }
        self.write_state(&state)
    }
    pub fn list(&self) -> Result<Vec<InstalledPackage>, StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read_state()?.packages)
    }

    pub fn install(
        &self,
        archive: impl AsRef<Path>,
        expected_size: u64,
        expected_hash: &str,
        expected_manifest: &PackageManifest,
        catalog_sequence: u64,
    ) -> Result<InstalledPackage, StoreError> {
        let _ = (
            archive,
            expected_size,
            expected_hash,
            expected_manifest,
            catalog_sequence,
        );
        Err(StoreError::Manifest(ManifestError::InvalidField(
            "schema_version",
        )))
    }

    pub fn install_versioned(
        &self,
        archive: impl AsRef<Path>,
        expected_size: u64,
        expected_hash: &str,
        expected_manifest: &VersionedManifest,
        catalog_sequence: u64,
    ) -> Result<InstalledPackage, StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(expected_manifest, VersionedManifest::V1(_)) {
            return Err(StoreError::Manifest(ManifestError::InvalidField(
                "schema_version",
            )));
        }
        expected_manifest.validate()?;
        let archive = archive.as_ref();
        let metadata = fs::metadata(archive)?;
        if metadata.len() != expected_size || metadata.len() > MAX_ARCHIVE {
            return Err(if metadata.len() != expected_size {
                StoreError::SizeMismatch
            } else {
                StoreError::Archive("archive too large".into())
            });
        }
        let bytes = fs::read(archive)?;
        let hash = hex_hash(&bytes);
        if !eq_hash(&hash, expected_hash) {
            return Err(StoreError::HashMismatch);
        }
        let file = fs::File::open(archive)?;
        let mut zip = ZipArchive::new(file)?;
        if zip.len() > MAX_ENTRIES {
            return Err(StoreError::Archive("too many entries".into()));
        }
        let staging = self.root.join(format!(
            ".staging-{hash}-{}",
            STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;
        let result = self.extract_verify(&mut zip, &staging, expected_manifest, &hash);
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        let blob = self.root.join("blobs").join(format!("{hash}.kspkg"));
        if !blob.exists() {
            let blob_tmp = self
                .root
                .join("blobs")
                .join(format!(".{hash}.{}.tmp", std::process::id()));
            fs::copy(archive, &blob_tmp)?;
            fs::rename(blob_tmp, &blob)?;
        }
        let unpacked = self
            .root
            .join("unpacked")
            .join(expected_manifest.id())
            .join(expected_manifest.version())
            .join(&hash);
        if let Some(parent) = unpacked.parent() {
            fs::create_dir_all(parent)?;
        }
        if !unpacked.exists() {
            fs::rename(&staging, &unpacked)?;
        } else {
            fs::remove_dir_all(&staging)?;
        }
        let package = InstalledPackage {
            id: expected_manifest.id().to_owned(),
            version: expected_manifest.version().to_owned(),
            hash,
            manifest: expected_manifest.clone(),
            enabled: false,
            revoked: false,
            installed_at: now(),
            catalog_sequence,
        };
        let mut state = self.read_state()?;
        state
            .packages
            .retain(|p| p.id != package.id || p.version != package.version);
        state.packages.push(package.clone());
        self.write_state(&state)?;
        Ok(package)
    }

    fn extract_verify(
        &self,
        zip: &mut ZipArchive<fs::File>,
        staging: &Path,
        expected: &VersionedManifest,
        _hash: &str,
    ) -> Result<(), StoreError> {
        let mut total = 0u64;
        let mut names = std::collections::HashSet::new();
        let mut found_manifest = None;
        let mut found_entry = false;
        let mut missing_workers = match expected {
            VersionedManifest::V2(manifest) => manifest
                .declared_worker_entrypoints()
                .into_iter()
                .map(str::to_owned)
                .collect::<std::collections::HashSet<_>>(),
            VersionedManifest::V1(_) => std::collections::HashSet::new(),
        };
        let mut found_icon = expected.icon().is_none();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let name = entry
                .enclosed_name()
                .ok_or_else(|| StoreError::Archive("unsafe path".into()))?
                .to_path_buf();
            let normalized =
                normalize_path(&name).ok_or_else(|| StoreError::Archive("unsafe path".into()))?;
            if !names.insert(normalized.to_ascii_lowercase()) {
                return Err(StoreError::Archive("duplicate path".into()));
            }
            let file_type = entry.unix_mode().map(|mode| mode & 0o170000);
            if file_type == Some(0o120000) {
                return Err(StoreError::Archive("symlink rejected".into()));
            }
            if entry.is_dir() {
                if let Some(file_type) = file_type {
                    if file_type != 0 && file_type != 0o040000 {
                        return Err(StoreError::Archive("unsupported file type".into()));
                    }
                }
                fs::create_dir_all(staging.join(&name))?;
                continue;
            }
            if let Some(file_type) = file_type {
                if file_type != 0 && file_type != 0o100000 {
                    return Err(StoreError::Archive("unsupported file type".into()));
                }
            }
            total = total.saturating_add(entry.size());
            if total > MAX_EXPANDED {
                return Err(StoreError::Archive("expanded archive too large".into()));
            }
            let out = staging.join(&name);
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = fs::File::create(&out)?;
            let declared_size = entry.size();
            let copied = io::copy(
                &mut (&mut entry).take(declared_size.saturating_add(1)),
                &mut file,
            )?;
            if copied != declared_size {
                return Err(StoreError::Archive("entry size mismatch".into()));
            }
            if normalized.eq_ignore_ascii_case("manifest.json") {
                found_manifest = Some(fs::read(&out)?);
            }
            if normalized == expected.entrypoint() {
                found_entry = true;
            }
            missing_workers.remove(&normalized);
            if expected.icon().is_some_and(|icon| normalized == icon) {
                found_icon = true;
            }
        }
        let raw =
            found_manifest.ok_or_else(|| StoreError::Archive("manifest.json missing".into()))?;
        let actual = PackageManifest::parse(
            std::str::from_utf8(&raw)
                .map_err(|_| StoreError::Archive("manifest is not UTF-8".into()))?,
        )
        .map_err(StoreError::Manifest)?;
        if &actual != expected {
            return Err(StoreError::ManifestMismatch);
        }
        if !found_entry {
            return Err(StoreError::Archive("entrypoint missing".into()));
        }
        if !missing_workers.is_empty() {
            return Err(StoreError::Archive("worker entrypoint missing".into()));
        }
        if !found_icon {
            return Err(StoreError::Archive("icon missing".into()));
        }
        Ok(())
    }
    pub fn enable(&self, id: &str, version: &str) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read_state()?;
        let package = state
            .packages
            .iter()
            .find(|p| p.id == id && p.version == version)
            .ok_or_else(|| StoreError::State("package not found".into()))?;
        if package.revoked {
            return Err(StoreError::Revoked);
        }
        if !matches!(package.manifest.kind(), PackageKind::App) {
            return Err(StoreError::WorkerRequired);
        }
        for package in &mut state.packages {
            if package.id == id {
                package.enabled = package.version == version;
            }
        }
        self.write_state(&state)
    }
    pub fn disable(&self, id: &str, version: &str) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read_state()?;
        let package = state
            .packages
            .iter_mut()
            .find(|p| p.id == id && p.version == version)
            .ok_or_else(|| StoreError::State("package not found".into()))?;
        package.enabled = false;
        self.write_state(&state)
    }
    pub fn uninstall(&self, id: &str, version: &str) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read_state()?;
        state
            .packages
            .retain(|p| !(p.id == id && p.version == version));
        self.write_state(&state)
    }

    /// Restore an already verified package record without changing its
    /// lifecycle metadata. The immutable blob/unpacked tree is restored by
    /// `install_versioned`; this method only puts the exact prior record back.
    pub fn restore_record(&self, package: &InstalledPackage) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        package.manifest.validate()?;
        if package.id != package.manifest.id()
            || package.version != package.manifest.version()
            || !is_hash(&package.hash)
            || (package.revoked && package.enabled)
        {
            return Err(StoreError::State("invalid package record".into()));
        }
        let mut state = self.read_state()?;
        state
            .packages
            .retain(|item| item.id != package.id || item.version != package.version);
        state.packages.push(package.clone());
        self.write_state(&state)
    }
    pub fn reconcile_revocations<I, S>(&self, revoked: I) -> Result<(), StoreError>
    where
        I: IntoIterator<Item = (S, S, S)>,
        S: AsRef<str>,
    {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let revoked: Vec<(String, String, String)> = revoked
            .into_iter()
            .map(|(id, version, hash)| {
                (
                    id.as_ref().to_owned(),
                    version.as_ref().to_owned(),
                    hash.as_ref().to_ascii_lowercase(),
                )
            })
            .collect();
        let mut state = self.read_state()?;
        for package in &mut state.packages {
            if revoked.iter().any(|(id, version, hash)| {
                id == &package.id && version == &package.version && hash == &package.hash
            }) {
                package.revoked = true;
                package.enabled = false;
            }
        }
        self.write_state(&state)
    }
    fn read_state(&self) -> Result<StoreState, StoreError> {
        if !self.state_path().exists() {
            return Ok(StoreState::default());
        }
        let state: StoreState = serde_json::from_slice(&fs::read(self.state_path())?)
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

fn normalize_path(path: &Path) -> Option<String> {
    let value = path.to_string_lossy().replace('\\', "/");
    let value = value.trim_end_matches('/');
    let mut components = Vec::new();
    for component in value.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains(':')
            || component.ends_with('.')
            || component.ends_with(' ')
            || is_reserved_name(component)
        {
            return None;
        }
        components.push(component);
    }
    (!components.is_empty()).then(|| components.join("/"))
}

fn safe_asset_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.contains('\0')
        && !value.contains('%')
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

fn is_reserved_name(component: &str) -> bool {
    let component = component.split('.').next().unwrap_or(component);
    matches!(
        component
            .trim_end_matches(['.', ' '])
            .to_ascii_uppercase()
            .as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn hex_hash(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
fn hash_reader(mut reader: impl Read) -> Result<String, StoreError> {
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn open_immutable_read(path: &Path) -> io::Result<fs::File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        return fs::OpenOptions::new().read(true).share_mode(1).open(path);
    }
    #[cfg(not(windows))]
    {
        fs::File::open(path)
    }
}
fn eq_hash(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b) && b.len() == 64 && b.bytes().all(|c| c.is_ascii_hexdigit())
}
fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use zip::{write::FileOptions, ZipWriter};
    fn manifest() -> PackageManifest {
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
    fn archive<M: Serialize>(path: &Path, manifest: &M, entry: &str) {
        let f = fs::File::create(path).unwrap();
        let mut z = ZipWriter::new(f);
        let o = FileOptions::default();
        z.start_file("manifest.json", o).unwrap();
        z.write_all(&serde_json::to_vec(manifest).unwrap()).unwrap();
        z.start_file(entry, o).unwrap();
        z.write_all(b"ok").unwrap();
        z.finish().unwrap();
    }
    fn manifest_v2() -> VersionedManifest {
        PackageManifest::parse(
            r#"{"schema_version":2,"id":"com.kosmos.v2-demo","name":"V2 Demo","version":"2.0.0","kind":"app","engine_api":">=1.0.0","entrypoint":"index.html","publisher":"kosmos","permissions":[],"targets":[{"runtime":"kosmos-host","os":["windows"]}],"data":{"access":[],"defines":[],"mappings":[]}}"#,
        )
        .unwrap()
    }
    fn archive_versioned(path: &Path, manifest: &VersionedManifest, entry: &str) {
        let f = fs::File::create(path).unwrap();
        let mut z = ZipWriter::new(f);
        let o = FileOptions::default();
        z.start_file("manifest.json", o).unwrap();
        z.write_all(&serde_json::to_vec(manifest).unwrap()).unwrap();
        z.start_file(entry, o).unwrap();
        z.write_all(b"ok").unwrap();
        z.finish().unwrap();
    }
    fn central_entry(bytes: &[u8], expected_name: &[u8]) -> usize {
        bytes
            .windows(4)
            .enumerate()
            .find_map(|(offset, signature)| {
                if signature != b"PK\x01\x02" {
                    return None;
                }
                let name_len =
                    u16::from_le_bytes([bytes[offset + 28], bytes[offset + 29]]) as usize;
                (&bytes[offset + 46..offset + 46 + name_len] == expected_name).then_some(offset)
            })
            .unwrap()
    }
    use std::io::Write;
    #[test]
    fn installs_and_preserves_previous_on_bad_update() {
        let d = tempdir().unwrap();
        let p = d.path().join("a.kspkg");
        let m = manifest_v2();
        archive(&p, &m, "index.html");
        let bytes = fs::read(&p).unwrap();
        let s = PackageStore::new(d.path().join("store")).unwrap();
        assert!(s
            .install_versioned(&p, bytes.len() as u64, &hex_hash(&bytes), &m, 1)
            .is_ok());
        assert!(s
            .install_versioned(&p, bytes.len() as u64, "00", &m, 2)
            .is_err());
        assert_eq!(s.list().unwrap().len(), 1);
    }

    #[test]
    fn installs_and_reloads_v2_manifest_without_downgrade() {
        let d = tempdir().unwrap();
        let p = d.path().join("v2.kspkg");
        let expected = manifest_v2();
        archive_versioned(&p, &expected, "index.html");
        let bytes = fs::read(&p).unwrap();
        let hash = hex_hash(&bytes);
        let store_root = d.path().join("store");
        let store = PackageStore::new(&store_root).unwrap();
        store
            .install_versioned(&p, bytes.len() as u64, &hash, &expected, 1)
            .unwrap();
        assert_eq!(
            store
                .installed("com.kosmos.v2-demo", "2.0.0")
                .unwrap()
                .manifest,
            expected
        );
        drop(store);
        let reopened = PackageStore::new(&store_root).unwrap();
        assert_eq!(
            reopened
                .installed("com.kosmos.v2-demo", "2.0.0")
                .unwrap()
                .manifest,
            expected
        );
    }

    #[test]
    fn rejects_state_manifest_tampering_against_immutable_archive() {
        let d = tempdir().unwrap();
        let p = d.path().join("v2.kspkg");
        let expected = manifest_v2();
        archive_versioned(&p, &expected, "index.html");
        let bytes = fs::read(&p).unwrap();
        let store = PackageStore::new(d.path().join("store")).unwrap();
        store
            .install_versioned(&p, bytes.len() as u64, &hex_hash(&bytes), &expected, 1)
            .unwrap();
        let state_path = store.state_path();
        let tampered = fs::read_to_string(&state_path)
            .unwrap()
            .replace("V2 Demo", "Tampered");
        fs::write(state_path, tampered).unwrap();
        assert!(matches!(store.list(), Err(StoreError::ManifestMismatch)));
    }

    #[test]
    fn enabling_an_app_replaces_its_active_version() {
        let dir = tempdir().unwrap();
        let store = PackageStore::new(dir.path().join("store")).unwrap();
        for version in ["2.0.0", "2.1.0"] {
            let VersionedManifest::V2(mut manifest) = manifest_v2() else {
                unreachable!();
            };
            manifest.version = version.into();
            let expected = VersionedManifest::V2(manifest);
            let archive_path = dir.path().join(format!("{version}.kspkg"));
            archive_versioned(&archive_path, &expected, "index.html");
            let bytes = fs::read(&archive_path).unwrap();
            store
                .install_versioned(
                    &archive_path,
                    bytes.len() as u64,
                    &hex_hash(&bytes),
                    &expected,
                    1,
                )
                .unwrap();
            store.enable(expected.id(), version).unwrap();
        }
        let enabled: Vec<_> = store
            .list()
            .unwrap()
            .into_iter()
            .filter(|package| package.enabled)
            .map(|package| package.version)
            .collect();
        assert_eq!(enabled, ["2.1.0"]);
    }

    #[test]
    fn verifies_declared_icon_path_against_the_immutable_archive() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("icon.kspkg");
        let VersionedManifest::V2(mut manifest) = manifest_v2() else {
            unreachable!();
        };
        manifest.icon = Some("icon.ico".into());
        let expected = VersionedManifest::V2(manifest);
        let file = fs::File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default();
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(&serde_json::to_vec(&expected).unwrap())
            .unwrap();
        zip.start_file("index.html", options).unwrap();
        zip.write_all(b"ok").unwrap();
        zip.start_file("icon.ico", options).unwrap();
        zip.write_all(b"icon").unwrap();
        zip.finish().unwrap();
        let bytes = fs::read(&archive_path).unwrap();
        let store = PackageStore::new(dir.path().join("store")).unwrap();
        let package = store
            .install_versioned(
                &archive_path,
                bytes.len() as u64,
                &hex_hash(&bytes),
                &expected,
                1,
            )
            .unwrap();

        let icon = store.immutable_asset_path(&package, "icon.ico").unwrap();
        assert!(icon.is_file());
        assert!(store.immutable_asset_path(&package, "../icon.ico").is_err());
        fs::write(&icon, b"tampered").unwrap();
        assert!(matches!(
            store.immutable_asset_path(&package, "icon.ico"),
            Err(StoreError::HashMismatch)
        ));
    }

    #[test]
    fn rejects_v1_at_both_install_boundaries() {
        let d = tempdir().unwrap();
        let p = d.path().join("v1.kspkg");
        let legacy = manifest();
        archive(&p, &legacy, "index.html");
        let bytes = fs::read(&p).unwrap();
        let store = PackageStore::new(d.path().join("store")).unwrap();
        assert!(store
            .install(&p, bytes.len() as u64, &hex_hash(&bytes), &legacy, 1)
            .is_err());
        assert!(store
            .install_versioned(
                &p,
                bytes.len() as u64,
                &hex_hash(&bytes),
                &VersionedManifest::V1(legacy),
                1,
            )
            .is_err());
        assert!(!store.state_path().exists());
    }
    #[test]
    fn rejects_traversal() {
        let d = tempdir().unwrap();
        let p = d.path().join("a.kspkg");
        let f = fs::File::create(&p).unwrap();
        let mut z = ZipWriter::new(f);
        z.start_file("../evil", FileOptions::default()).unwrap();
        z.write_all(b"x").unwrap();
        z.finish().unwrap();
        let s = PackageStore::new(d.path().join("store")).unwrap();
        assert!(s
            .install_versioned(
                &p,
                fs::metadata(&p).unwrap().len(),
                &hex_hash(&fs::read(&p).unwrap()),
                &manifest_v2(),
                1
            )
            .is_err());
    }

    #[test]
    fn source_stays_disabled_and_revocation_is_state_only() {
        let d = tempdir().unwrap();
        let p = d.path().join("source.kspkg");
        let VersionedManifest::V2(mut m) = manifest_v2() else {
            unreachable!()
        };
        m.kind = PackageKind::Source;
        m.entrypoint = "worker.exe".into();
        m.targets[0].runtime = crate::package_manifest::TargetRuntime::Worker;
        m.targets[0].os = vec![
            crate::package_manifest::TargetOs::Windows,
            crate::package_manifest::TargetOs::Macos,
            crate::package_manifest::TargetOs::Linux,
        ];
        m.targets[0].entrypoint = Some("worker.exe".into());
        archive(&p, &m, "worker.exe");
        let bytes = fs::read(&p).unwrap();
        let hash = hex_hash(&bytes);
        let s = PackageStore::new(d.path().join("store")).unwrap();
        let expected = VersionedManifest::V2(m.clone());
        s.install_versioned(&p, bytes.len() as u64, &hash, &expected, 1)
            .unwrap();
        assert!(matches!(
            s.enable(&m.id, &m.version),
            Err(StoreError::WorkerRequired)
        ));
        s.enable_worker(&m.id, &m.version).unwrap();
        assert!(s.installed(&m.id, &m.version).unwrap().enabled);
        s.reconcile_revocations(vec![(m.id.clone(), m.version.clone(), hash.clone())])
            .unwrap();
        let item = &s.list().unwrap()[0];
        assert!(item.revoked && !item.enabled);
        assert!(d
            .path()
            .join("store/blobs")
            .join(format!("{hash}.kspkg"))
            .exists());
    }

    #[test]
    fn enabling_a_worker_replaces_its_active_version_atomically() {
        let d = tempdir().unwrap();
        let store = PackageStore::new(d.path().join("store")).unwrap();
        for version in ["1.0.0", "2.0.0"] {
            let VersionedManifest::V2(mut manifest) = manifest_v2() else {
                unreachable!();
            };
            manifest.kind = PackageKind::Source;
            manifest.version = version.into();
            manifest.entrypoint = "worker.exe".into();
            manifest.targets[0].runtime = crate::package_manifest::TargetRuntime::Worker;
            manifest.targets[0].os = vec![
                crate::package_manifest::TargetOs::Windows,
                crate::package_manifest::TargetOs::Macos,
                crate::package_manifest::TargetOs::Linux,
            ];
            manifest.targets[0].entrypoint = Some("worker.exe".into());
            let expected = VersionedManifest::V2(manifest);
            let archive_path = d.path().join(format!("{version}.kspkg"));
            archive_versioned(&archive_path, &expected, "worker.exe");
            let bytes = fs::read(&archive_path).unwrap();
            store
                .install_versioned(
                    &archive_path,
                    bytes.len() as u64,
                    &hex_hash(&bytes),
                    &expected,
                    1,
                )
                .unwrap();
            if version == "2.0.0" {
                store.fail_next_state_write.store(true, Ordering::Relaxed);
                assert!(store.enable_worker(expected.id(), version).is_err());
                assert!(store.installed(expected.id(), "1.0.0").unwrap().enabled);
                assert!(!store.installed(expected.id(), version).unwrap().enabled);
            } else {
                store.enable_worker(expected.id(), version).unwrap();
            }
        }
        let store_root = d.path().join("store");
        drop(store);
        let store = PackageStore::new(store_root).unwrap();
        store.enable_worker("com.kosmos.v2-demo", "2.0.0").unwrap();
        let enabled = store
            .list()
            .unwrap()
            .into_iter()
            .filter(|package| package.id == "com.kosmos.v2-demo" && package.enabled)
            .map(|package| package.version)
            .collect::<Vec<_>>();
        assert_eq!(enabled, ["2.0.0"]);
    }

    #[test]
    fn immutable_worker_entrypoint_rejects_store_tampering() {
        let d = tempdir().unwrap();
        let p = d.path().join("source.kspkg");
        let VersionedManifest::V2(mut m) = manifest_v2() else {
            unreachable!()
        };
        m.kind = PackageKind::Source;
        m.entrypoint = "worker.exe".into();
        m.targets[0].runtime = crate::package_manifest::TargetRuntime::Worker;
        m.targets[0].os = vec![
            crate::package_manifest::TargetOs::Windows,
            crate::package_manifest::TargetOs::Macos,
            crate::package_manifest::TargetOs::Linux,
        ];
        m.targets[0].entrypoint = Some("worker.exe".into());
        archive(&p, &m, "worker.exe");
        let bytes = fs::read(&p).unwrap();
        let hash = hex_hash(&bytes);
        let store = PackageStore::new(d.path().join("store")).unwrap();
        let expected = VersionedManifest::V2(m.clone());
        let installed = store
            .install_versioned(&p, bytes.len() as u64, &hash, &expected, 1)
            .unwrap();
        assert!(store.immutable_entrypoint(&installed).is_ok());
        fs::write(
            d.path()
                .join("store/unpacked")
                .join(&m.id)
                .join(&m.version)
                .join(&hash)
                .join("worker.exe"),
            b"tampered",
        )
        .unwrap();
        assert!(matches!(
            store.immutable_entrypoint(&installed),
            Err(StoreError::HashMismatch)
        ));
    }

    #[test]
    fn rejects_windows_unsafe_and_case_collision_paths_without_state() {
        for (label, names) in [
            ("case", vec!["Foo", "foo"]),
            ("file-directory-case", vec!["index.html", "INDEX.HTML/"]),
            ("ads", vec!["name:stream"]),
            ("device", vec!["CON.txt"]),
        ] {
            let d = tempdir().unwrap();
            let p = d.path().join(format!("{label}.kspkg"));
            let f = fs::File::create(&p).unwrap();
            let mut z = ZipWriter::new(f);
            for name in names {
                z.start_file(name, FileOptions::default()).unwrap();
                z.write_all(b"x").unwrap();
            }
            z.finish().unwrap();
            let bytes = fs::read(&p).unwrap();
            let store = PackageStore::new(d.path().join("store")).unwrap();
            assert!(store
                .install_versioned(&p, bytes.len() as u64, &hex_hash(&bytes), &manifest_v2(), 1)
                .is_err());
            assert!(!store.state_path().exists());
            assert!(fs::read_dir(d.path().join("store")).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".staging-")
            }));
        }
    }

    #[test]
    fn atomic_state_write_failure_preserves_previous_active_package() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first.kspkg");
        let second = dir.path().join("second.kspkg");
        let manifest = manifest_v2();
        archive(&first, &manifest, "index.html");
        archive(&second, &manifest, "index.html");
        fs::OpenOptions::new()
            .append(true)
            .open(&second)
            .unwrap()
            .write_all(b"different immutable bytes")
            .unwrap();

        let store = PackageStore::new(dir.path().join("store")).unwrap();
        let first_bytes = fs::read(&first).unwrap();
        let original = store
            .install_versioned(
                &first,
                first_bytes.len() as u64,
                &hex_hash(&first_bytes),
                &manifest_v2(),
                1,
            )
            .unwrap();
        store.fail_next_state_write.store(true, Ordering::Relaxed);
        let second_bytes = fs::read(&second).unwrap();
        assert!(store
            .install_versioned(
                &second,
                second_bytes.len() as u64,
                &hex_hash(&second_bytes),
                &manifest,
                2,
            )
            .is_err());
        assert_eq!(store.list().unwrap()[0].hash, original.hash);
    }

    #[test]
    fn rejects_declared_expanded_size_bomb() {
        let dir = tempdir().unwrap();
        let archive = dir.path().join("bomb.kspkg");
        let file = fs::File::create(&archive).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default())
            .unwrap();
        zip.write_all(&serde_json::to_vec(&manifest_v2()).unwrap())
            .unwrap();
        zip.start_file("index.html", FileOptions::default())
            .unwrap();
        zip.write_all(b"ok").unwrap();
        zip.start_file("bomb.bin", FileOptions::default()).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();

        let mut bytes = fs::read(&archive).unwrap();
        let central = central_entry(&bytes, b"bomb.bin");
        let local =
            u32::from_le_bytes(bytes[central + 42..central + 46].try_into().unwrap()) as usize;
        let declared = (MAX_EXPANDED + 1) as u32;
        bytes[central + 24..central + 28].copy_from_slice(&declared.to_le_bytes());
        bytes[local + 22..local + 26].copy_from_slice(&declared.to_le_bytes());
        fs::write(&archive, &bytes).unwrap();

        let store = PackageStore::new(dir.path().join("store")).unwrap();
        assert!(matches!(
            store.install_versioned(
                &archive,
                bytes.len() as u64,
                &hex_hash(&bytes),
                &manifest_v2(),
                1
            ),
            Err(StoreError::Archive(message)) if message == "expanded archive too large"
        ));
        assert!(!store.state_path().exists());
    }

    #[test]
    fn rejects_actual_symlink_metadata_without_state() {
        let dir = tempdir().unwrap();
        let archive = dir.path().join("symlink.kspkg");
        let file = fs::File::create(&archive).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default())
            .unwrap();
        zip.write_all(&serde_json::to_vec(&manifest_v2()).unwrap())
            .unwrap();
        zip.start_file("index.html", FileOptions::default())
            .unwrap();
        zip.write_all(b"ok").unwrap();
        zip.start_file("link", FileOptions::default()).unwrap();
        zip.write_all(b"index.html").unwrap();
        zip.finish().unwrap();

        let mut bytes = fs::read(&archive).unwrap();
        let central = central_entry(&bytes, b"link");
        bytes[central + 5] = 3;
        bytes[central + 38..central + 42].copy_from_slice(&((0o120777u32) << 16).to_le_bytes());
        fs::write(&archive, &bytes).unwrap();
        let store = PackageStore::new(dir.path().join("store")).unwrap();
        assert!(matches!(
            store.install_versioned(
                &archive,
                bytes.len() as u64,
                &hex_hash(&bytes),
                &manifest_v2(),
                1
            ),
            Err(StoreError::Archive(message)) if message == "symlink rejected"
        ));
        assert!(!store.state_path().exists());
    }

    #[test]
    fn rejects_non_regular_file_metadata_without_state() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("device.kspkg");
        let manifest = manifest();
        archive(&archive_path, &manifest, "index.html");

        let mut bytes = fs::read(&archive_path).unwrap();
        let central = central_entry(&bytes, b"index.html");
        bytes[central + 5] = 3;
        bytes[central + 38..central + 42].copy_from_slice(&((0o020644u32) << 16).to_le_bytes());
        fs::write(&archive_path, &bytes).unwrap();

        let store = PackageStore::new(dir.path().join("store")).unwrap();
        assert!(matches!(
            store.install_versioned(
                &archive_path,
                bytes.len() as u64,
                &hex_hash(&bytes),
                &manifest_v2(),
                1
            ),
            Err(StoreError::Archive(message)) if message == "unsupported file type"
        ));
        assert!(!store.state_path().exists());
    }

    #[test]
    fn rejects_entry_limit_and_manifest_mismatch_without_state() {
        for (label, build) in [
            (
                "entry-limit",
                Box::new(|zip: &mut ZipWriter<fs::File>| {
                    for index in 0..=MAX_ENTRIES {
                        zip.start_file(format!("entry-{index}"), FileOptions::default())
                            .unwrap();
                    }
                }) as Box<dyn Fn(&mut ZipWriter<fs::File>)>,
            ),
            (
                "manifest-mismatch",
                Box::new(|zip: &mut ZipWriter<fs::File>| {
                    let mut wrong = manifest();
                    wrong.version = "2.0.0".into();
                    zip.start_file("manifest.json", FileOptions::default())
                        .unwrap();
                    zip.write_all(&serde_json::to_vec(&wrong).unwrap()).unwrap();
                    zip.start_file("index.html", FileOptions::default())
                        .unwrap();
                }),
            ),
        ] {
            let dir = tempdir().unwrap();
            let archive = dir.path().join(format!("{label}.kspkg"));
            let mut zip = ZipWriter::new(fs::File::create(&archive).unwrap());
            build(&mut zip);
            zip.finish().unwrap();
            let bytes = fs::read(&archive).unwrap();
            let store = PackageStore::new(dir.path().join("store")).unwrap();
            assert!(store
                .install_versioned(
                    &archive,
                    bytes.len() as u64,
                    &hex_hash(&bytes),
                    &manifest_v2(),
                    1
                )
                .is_err());
            assert!(!store.state_path().exists());
        }
    }
}
