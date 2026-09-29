//! Fail-closed, first-party Package v1 trust verification.
//!
//! This module intentionally keeps only verified documents in memory. Callers
//! may persist the returned signed bytes, but no unsigned local trust state is
//! ever read or written here.

use std::collections::{BTreeMap, BTreeSet};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::package_manifest::VersionedManifest;

const SCHEMA_VERSION: u32 = 1;
const MAX_SIGNATURES: usize = 8;
const MAX_RELEASE_KEYS: usize = 16;
const MAX_CATALOG_ENTRIES: usize = 2_000;
const MAX_REVOCATION_KEYS: usize = 256;
const MAX_REVOCATION_PACKAGES: usize = 10_000;

#[derive(Debug, Error)]
pub enum TrustError {
    #[error("missing pinned root or release key")]
    MissingTrust,
    #[error("invalid {0}")]
    Invalid(&'static str),
    #[error("document JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("document signature is invalid")]
    InvalidSignature,
    #[error("document is unsigned by a trusted active key")]
    UntrustedSignature,
    #[error("document is expired")]
    Expired,
    #[error("document sequence is not monotonic")]
    Replay,
    #[error("release key is revoked")]
    RevokedKey,
    #[error("package is revoked")]
    RevokedPackage,
    #[error("manifest: {0}")]
    Manifest(#[from] crate::package_manifest::ManifestError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SignatureSet {
    pub schema_version: u32,
    pub signatures: Vec<DetachedSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DetachedSignature {
    pub key_id: String,
    pub algorithm: String,
    pub signature: String,
}

impl SignatureSet {
    pub(crate) fn validate(&self) -> Result<(), TrustError> {
        if self.schema_version != SCHEMA_VERSION
            || self.signatures.is_empty()
            || self.signatures.len() > MAX_SIGNATURES
        {
            return Err(TrustError::Invalid("signature set"));
        }
        let mut seen = BTreeSet::new();
        for signature in &self.signatures {
            if !valid_id(&signature.key_id)
                || signature.algorithm != "ed25519"
                || !seen.insert(&signature.key_id)
                || STANDARD
                    .decode(&signature.signature)
                    .ok()
                    .and_then(|raw| Signature::from_slice(&raw).ok())
                    .is_none()
            {
                return Err(TrustError::Invalid("signature set"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrustedKey {
    pub key_id: String,
    /// Base64-encoded 32-byte Ed25519 verifying key.
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogDocument {
    pub schema_version: u32,
    pub sequence: u64,
    pub issued_at: String,
    pub expires_at: String,
    pub packages: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    pub manifest: VersionedManifest,
    pub archive_url: String,
    pub sha256: String,
    pub size: u64,
    /// Present exactly when this entry is a native GPUI app: `archive_url`
    /// points at the app's own repository release zip (not a `.kspkg`), and
    /// the executable inside the zip is what the store launches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<NativeArtifact>,
}

/// Provenance + layout descriptor for a native app catalog entry. The signed
/// catalog is the only authority for the archive sha256/size — the release's
/// own `SHA256SUMS.txt` is never consulted at install time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativeArtifact {
    /// Source repository in `org/name` form, e.g. `makekosmos/agenda-gpui`.
    pub repository: String,
    /// Immutable release tag, e.g. `v0.1.1`; must equal `v{manifest.version}`.
    pub release_tag: String,
    /// Target triple the archive was built for, e.g. `x86_64-pc-windows-msvc`.
    pub target: String,
    /// Entry executable path inside the zip, e.g. `agenda-gpui.exe`. Must
    /// equal `manifest.entrypoint`.
    pub executable: String,
}

/// Target triples the Engine can install a native app for.
pub const NATIVE_TARGETS: &[&str] = &["x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"];

/// The host's native-app target triple, or `None` when this platform cannot
/// run store-native apps at all.
pub fn host_native_target() -> Option<&'static str> {
    if cfg!(windows) && cfg!(target_arch = "x86_64") {
        Some("x86_64-pc-windows-msvc")
    } else if cfg!(windows) && cfg!(target_arch = "aarch64") {
        Some("aarch64-pc-windows-msvc")
    } else {
        None
    }
}

fn valid_native_target(value: &str) -> bool {
    NATIVE_TARGETS.contains(&value)
}

fn valid_repository(value: &str) -> bool {
    let mut parts = value.split('/');
    let (Some(org), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let segment = |part: &str| {
        !part.is_empty()
            && part.len() <= 64
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    };
    segment(org) && segment(name)
}

fn valid_executable(value: &str) -> bool {
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
                && !crate::package_store::is_reserved_name(part)
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct KeyTransitionDocument {
    pub schema_version: u32,
    pub sequence: u64,
    pub issued_at: String,
    pub expires_at: String,
    pub old_key_id: String,
    pub new_key: TrustedKey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RevocationDocument {
    pub schema_version: u32,
    pub sequence: u64,
    pub issued_at: String,
    #[serde(default)]
    pub revoked_release_keys: Vec<String>,
    #[serde(default)]
    pub revoked_packages: Vec<PackageRevocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct PackageRevocation {
    pub id: String,
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedDocument<T> {
    pub document: T,
    pub signed_bytes: Vec<u8>,
    pub signatures: SignatureSet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustSummary {
    pub trusted_release_key_ids: Vec<String>,
    pub revoked_release_key_ids: Vec<String>,
    pub revoked_package_count: usize,
    pub catalog_sequence: u64,
    pub transition_sequence: u64,
    pub revocation_sequence: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reconciliation {
    pub revoked: BTreeSet<PackageRevocation>,
    pub disabled: BTreeSet<PackageRevocation>,
}

#[derive(Default)]
struct TrustState {
    release_keys: BTreeMap<String, VerifyingKey>,
    revoked_keys: BTreeSet<String>,
    revoked_packages: BTreeSet<PackageRevocation>,
    catalog_sequence: u64,
    transition_sequence: u64,
    revocation_sequence: u64,
}

/// A pinned root and release-key set. There is deliberately no `open(path)`:
/// accepting writable local trust configuration would make this boundary unsafe.
pub struct TrustStore {
    root_key_id: String,
    root_key: VerifyingKey,
    state: std::sync::Mutex<TrustState>,
}

impl TrustStore {
    pub fn new(
        root: TrustedKey,
        initial_release_keys: Vec<TrustedKey>,
    ) -> Result<Self, TrustError> {
        let root_key_id = checked_key_id(&root.key_id)?;
        let root_key = decode_key(&root.public_key)?;
        if initial_release_keys.is_empty() {
            return Err(TrustError::MissingTrust);
        }
        if initial_release_keys.len() > MAX_RELEASE_KEYS {
            return Err(TrustError::Invalid("release keys"));
        }
        let mut release_keys = BTreeMap::new();
        for key in initial_release_keys {
            let id = checked_key_id(&key.key_id)?;
            if id == root_key_id
                || release_keys
                    .insert(id, decode_key(&key.public_key)?)
                    .is_some()
            {
                return Err(TrustError::Invalid("release keys"));
            }
        }
        Ok(Self {
            root_key_id,
            root_key,
            state: std::sync::Mutex::new(TrustState {
                release_keys,
                ..TrustState::default()
            }),
        })
    }

    pub fn verify_catalog(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<VerifiedDocument<CatalogDocument>, TrustError> {
        self.verify_catalog_at(bytes, signatures, Utc::now())
    }

    pub fn verify_catalog_at(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
        now: DateTime<Utc>,
    ) -> Result<VerifiedDocument<CatalogDocument>, TrustError> {
        signatures.validate()?;
        let document: CatalogDocument = serde_json::from_slice(bytes)?;
        let issued_at = parse_utc(&document.issued_at, "catalog issued_at")?;
        let expiry = parse_utc(&document.expires_at, "catalog expiry")?;
        if document.schema_version != SCHEMA_VERSION
            || document.sequence == 0
            || expiry <= issued_at
        {
            return Err(TrustError::Invalid("catalog"));
        }
        if expiry <= now {
            return Err(TrustError::Expired);
        }
        validate_catalog(&document)?;
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if document.sequence <= state.catalog_sequence {
            return Err(TrustError::Replay);
        }
        let valid_signature = signatures.signatures.iter().any(|signature| {
            state
                .release_keys
                .get(&signature.key_id)
                .filter(|_| !state.revoked_keys.contains(&signature.key_id))
                .is_some_and(|key| verify_signature(bytes, signature, key).is_ok())
        });
        if !valid_signature {
            return Err(TrustError::UntrustedSignature);
        }
        Ok(VerifiedDocument {
            document,
            signed_bytes: bytes.to_vec(),
            signatures,
        })
    }

    pub fn apply_catalog(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<VerifiedDocument<CatalogDocument>, TrustError> {
        let verified = self.verify_catalog(bytes, signatures)?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if verified.document.sequence <= state.catalog_sequence {
            return Err(TrustError::Replay);
        }
        let valid_signature = verified.signatures.signatures.iter().any(|signature| {
            state
                .release_keys
                .get(&signature.key_id)
                .filter(|_| !state.revoked_keys.contains(&signature.key_id))
                .is_some_and(|key| verify_signature(bytes, signature, key).is_ok())
        });
        if !valid_signature {
            return Err(TrustError::UntrustedSignature);
        }
        state.catalog_sequence = verified.document.sequence;
        Ok(verified)
    }

    pub fn verify_key_transition(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<VerifiedDocument<KeyTransitionDocument>, TrustError> {
        self.verify_key_transition_at(bytes, signatures, Utc::now())
    }

    pub fn verify_key_transition_at(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
        now: DateTime<Utc>,
    ) -> Result<VerifiedDocument<KeyTransitionDocument>, TrustError> {
        signatures.validate()?;
        let document: KeyTransitionDocument = serde_json::from_slice(bytes)?;
        let issued_at = parse_utc(&document.issued_at, "transition issued_at")?;
        let expiry = parse_utc(&document.expires_at, "transition expiry")?;
        if document.schema_version != SCHEMA_VERSION
            || document.sequence == 0
            || !valid_id(&document.old_key_id)
            || expiry <= issued_at
        {
            return Err(TrustError::Invalid("key transition"));
        }
        if expiry <= now {
            return Err(TrustError::Expired);
        }
        let new_id = checked_key_id(&document.new_key.key_id)?;
        if new_id == document.old_key_id {
            return Err(TrustError::Invalid("transition key"));
        }
        let new_key = decode_key(&document.new_key.public_key)?;
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if document.sequence <= state.transition_sequence
            || state.revoked_keys.contains(&document.old_key_id)
            || state.revoked_keys.contains(&new_id)
        {
            return Err(if document.sequence <= state.transition_sequence {
                TrustError::Replay
            } else {
                TrustError::RevokedKey
            });
        }
        let old_key = state
            .release_keys
            .get(&document.old_key_id)
            .ok_or(TrustError::UntrustedSignature)?;
        verify_by_id(bytes, &signatures, &document.old_key_id, old_key)?;
        verify_by_id(bytes, &signatures, &new_id, &new_key)?;
        if let Some(existing) = state.release_keys.get(&new_id) {
            if existing != &new_key {
                return Err(TrustError::Invalid("transition key"));
            }
        } else if state.release_keys.len() >= MAX_RELEASE_KEYS {
            return Err(TrustError::Invalid("release keys"));
        }
        Ok(VerifiedDocument {
            document,
            signed_bytes: bytes.to_vec(),
            signatures,
        })
    }

    pub fn apply_key_transition(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<VerifiedDocument<KeyTransitionDocument>, TrustError> {
        let verified = self.verify_key_transition(bytes, signatures)?;
        let new_id = verified.document.new_key.key_id.clone();
        let new_key = decode_key(&verified.document.new_key.public_key)?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if verified.document.sequence <= state.transition_sequence {
            return Err(TrustError::Replay);
        }
        if state.revoked_keys.contains(&verified.document.old_key_id)
            || state.revoked_keys.contains(&new_id)
        {
            return Err(TrustError::RevokedKey);
        }
        if let Some(existing) = state.release_keys.get(&new_id) {
            if existing != &new_key {
                return Err(TrustError::Invalid("transition key"));
            }
        } else {
            state.release_keys.insert(new_id, new_key);
        }
        state.transition_sequence = verified.document.sequence;
        Ok(verified)
    }

    pub fn verify_revocations(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<VerifiedDocument<RevocationDocument>, TrustError> {
        signatures.validate()?;
        let document: RevocationDocument = serde_json::from_slice(bytes)?;
        if document.schema_version != SCHEMA_VERSION || document.sequence == 0 {
            return Err(TrustError::Invalid("revocation"));
        }
        parse_utc(&document.issued_at, "revocation issued_at")?;
        if document.revoked_release_keys.is_empty() && document.revoked_packages.is_empty() {
            return Err(TrustError::Invalid("revocation"));
        }
        if document.revoked_release_keys.len() > MAX_REVOCATION_KEYS
            || document.revoked_packages.len() > MAX_REVOCATION_PACKAGES
        {
            return Err(TrustError::Invalid("revocation"));
        }
        let mut keys = BTreeSet::new();
        for key in &document.revoked_release_keys {
            if !valid_id(key) || !keys.insert(key.clone()) {
                return Err(TrustError::Invalid("revocation keys"));
            }
        }
        let mut packages = BTreeSet::new();
        for package in &document.revoked_packages {
            if !valid_id(&package.id)
                || semver::Version::parse(&package.version).is_err()
                || !valid_sha256(&package.sha256)
                || !packages.insert(package.clone())
            {
                return Err(TrustError::Invalid("revocation packages"));
            }
        }
        verify_by_id(bytes, &signatures, &self.root_key_id, &self.root_key)?;
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if document.sequence <= state.revocation_sequence {
            return Err(TrustError::Replay);
        }
        Ok(VerifiedDocument {
            document,
            signed_bytes: bytes.to_vec(),
            signatures,
        })
    }

    pub fn apply_revocations(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<VerifiedDocument<RevocationDocument>, TrustError> {
        let verified = self.verify_revocations(bytes, signatures)?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if verified.document.sequence <= state.revocation_sequence {
            return Err(TrustError::Replay);
        }
        state
            .revoked_keys
            .extend(verified.document.revoked_release_keys.iter().cloned());
        state
            .revoked_packages
            .extend(verified.document.revoked_packages.iter().cloned());
        state.revocation_sequence = verified.document.sequence;
        Ok(verified)
    }

    pub fn catalog_entry<'a>(
        &self,
        catalog: &'a CatalogDocument,
        id: &str,
        version: &str,
    ) -> Option<&'a CatalogEntry> {
        catalog
            .packages
            .iter()
            .find(|entry| entry.manifest.id() == id && entry.manifest.version() == version)
    }

    pub fn is_release_key_trusted(&self, key_id: &str) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.release_keys.contains_key(key_id) && !state.revoked_keys.contains(key_id)
    }

    pub fn is_package_revoked(&self, id: &str, version: &str, sha256: &str) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .revoked_packages
            .contains(&PackageRevocation {
                id: id.into(),
                version: version.into(),
                sha256: sha256.into(),
            })
    }

    pub fn ensure_package_allowed(&self, entry: &CatalogEntry) -> Result<(), TrustError> {
        if self.is_package_revoked(entry.manifest.id(), entry.manifest.version(), &entry.sha256) {
            Err(TrustError::RevokedPackage)
        } else {
            Ok(())
        }
    }

    pub fn reconcile<I>(&self, installed: I) -> Reconciliation
    where
        I: IntoIterator<Item = PackageRevocation>,
    {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let revoked: BTreeSet<PackageRevocation> = installed
            .into_iter()
            .filter(|package| state.revoked_packages.contains(package))
            .collect();
        Reconciliation {
            disabled: revoked.clone(),
            revoked,
        }
    }

    pub fn summary(&self) -> TrustSummary {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        TrustSummary {
            trusted_release_key_ids: state
                .release_keys
                .keys()
                .filter(|id| !state.revoked_keys.contains(*id))
                .cloned()
                .collect(),
            revoked_release_key_ids: state.revoked_keys.iter().cloned().collect(),
            revoked_package_count: state.revoked_packages.len(),
            catalog_sequence: state.catalog_sequence,
            transition_sequence: state.transition_sequence,
            revocation_sequence: state.revocation_sequence,
        }
    }
}

fn validate_catalog(catalog: &CatalogDocument) -> Result<(), TrustError> {
    if catalog.packages.len() > MAX_CATALOG_ENTRIES {
        return Err(TrustError::Invalid("catalog entries"));
    }
    let mut packages = BTreeSet::new();
    for entry in &catalog.packages {
        entry.manifest.validate()?;
        let archive_url = reqwest::Url::parse(&entry.archive_url).ok();
        // Debug/test builds additionally accept `file://` archive URLs so
        // fixture catalogs can point at a zip on disk; the signed sha256/size
        // still gates the bytes. Release builds require HTTPS.
        if !archive_url.is_some_and(|url| {
            (url.scheme() == "https" && url.host_str().is_some())
                || (cfg!(debug_assertions) && url.scheme() == "file")
        }) || !valid_sha256(&entry.sha256)
            || entry.size == 0
            || !packages.insert((
                entry.manifest.id().to_owned(),
                entry.manifest.version().to_owned(),
            ))
        {
            return Err(TrustError::Invalid("catalog entry"));
        }
        if let Some(native) = &entry.native {
            validate_native_entry(entry, native)?;
        }
    }
    Ok(())
}

/// A native catalog entry is still a normal `kind: "app"` package entry — the
/// existing trust/revocation plumbing applies unchanged — plus a `native`
/// descriptor that must agree with the signed manifest on every field that
/// would otherwise be ambiguous at install time.
fn validate_native_entry(entry: &CatalogEntry, native: &NativeArtifact) -> Result<(), TrustError> {
    let valid = matches!(&entry.manifest, VersionedManifest::V2(manifest) if
    manifest.targets.iter().any(|target| {
        target.runtime == crate::package_manifest::TargetRuntime::Standalone
            && target.os.contains(&crate::package_manifest::TargetOs::Windows)
    })) && entry.manifest.kind() == &crate::package_manifest::PackageKind::App
        && native.executable == entry.manifest.entrypoint()
        && valid_executable(&native.executable)
        && valid_native_target(&native.target)
        && valid_repository(&native.repository)
        && native.release_tag == format!("v{}", entry.manifest.version());
    if !valid {
        return Err(TrustError::Invalid("native catalog entry"));
    }
    Ok(())
}

fn verify_by_id(
    bytes: &[u8],
    signatures: &SignatureSet,
    key_id: &str,
    key: &VerifyingKey,
) -> Result<(), TrustError> {
    let signature = signatures
        .signatures
        .iter()
        .find(|signature| signature.key_id == key_id)
        .ok_or(TrustError::InvalidSignature)?;
    verify_signature(bytes, signature, key)
}

pub(crate) fn verify_signature(
    bytes: &[u8],
    signature: &DetachedSignature,
    key: &VerifyingKey,
) -> Result<(), TrustError> {
    let raw = STANDARD
        .decode(&signature.signature)
        .map_err(|_| TrustError::InvalidSignature)?;
    let signature = Signature::from_slice(&raw).map_err(|_| TrustError::InvalidSignature)?;
    key.verify(bytes, &signature)
        .map_err(|_| TrustError::InvalidSignature)
}

fn checked_key_id(value: &str) -> Result<String, TrustError> {
    if valid_id(value) {
        Ok(value.into())
    } else {
        Err(TrustError::Invalid("key id"))
    }
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}
fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn parse_utc(value: &str, field: &'static str) -> Result<DateTime<Utc>, TrustError> {
    if !value.ends_with('Z') {
        return Err(TrustError::Invalid(field));
    }
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|_| TrustError::Invalid(field))
}
fn decode_key(value: &str) -> Result<VerifyingKey, TrustError> {
    let raw = STANDARD
        .decode(value)
        .map_err(|_| TrustError::Invalid("public key"))?;
    VerifyingKey::from_bytes(
        &raw.try_into()
            .map_err(|_| TrustError::Invalid("public key"))?,
    )
    .map_err(|_| TrustError::Invalid("public key"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_manifest::PackageManifest;
    use ed25519_dalek::{Signer, SigningKey};

    fn key(seed: u8, id: &str) -> (SigningKey, TrustedKey) {
        let signing = SigningKey::from_bytes(&[seed; 32]);
        let public = TrustedKey {
            key_id: id.into(),
            public_key: STANDARD.encode(signing.verifying_key().as_bytes()),
        };
        (signing, public)
    }
    fn store() -> (TrustStore, SigningKey, SigningKey) {
        let (root_signing, root) = key(1, "root");
        let (release_signing, release) = key(2, "release-1");
        (
            TrustStore::new(root, vec![release]).unwrap(),
            root_signing,
            release_signing,
        )
    }
    fn manifest() -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: "com.kosmos.demo".into(),
            name: "Demo".into(),
            version: "1.0.0".into(),
            kind: crate::package_manifest::PackageKind::App,
            engine_api: ">=1.0.0".into(),
            entrypoint: "index.html".into(),
            publisher: "kosmos".into(),
            permissions: vec![],
        }
    }
    fn catalog(sequence: u64) -> CatalogDocument {
        CatalogDocument {
            schema_version: 1,
            sequence,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V1(manifest()),
                archive_url: "https://packages.kosmos.dev/demo.kspkg".into(),
                sha256: "a".repeat(64),
                size: 1,
                native: None,
            }],
        }
    }
    fn signed<T: Serialize>(
        document: &T,
        key_id: &str,
        signing: &SigningKey,
    ) -> (Vec<u8>, SignatureSet) {
        let bytes = serde_json::to_vec(document).unwrap();
        (
            bytes.clone(),
            SignatureSet {
                schema_version: 1,
                signatures: vec![DetachedSignature {
                    key_id: key_id.into(),
                    algorithm: "ed25519".into(),
                    signature: STANDARD.encode(signing.sign(&bytes).to_bytes()),
                }],
            },
        )
    }

    #[test]
    fn fails_closed_without_release_trust() {
        let (_, root) = key(1, "root");
        assert!(matches!(
            TrustStore::new(root, vec![]),
            Err(TrustError::MissingTrust)
        ));
    }
    #[test]
    fn rejects_tamper_expiry_and_replay() {
        let (store, _, release) = store();
        let doc = catalog(1);
        let (bytes, signatures) = signed(&doc, "release-1", &release);
        assert!(matches!(
            store.verify_catalog_at(
                &bytes,
                signatures.clone(),
                DateTime::parse_from_rfc3339("2031-01-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc)
            ),
            Err(TrustError::Expired)
        ));
        let mut tampered = bytes.clone();
        tampered[0] ^= 1;
        assert!(store.verify_catalog(&tampered, signatures.clone()).is_err());
        store.apply_catalog(&bytes, signatures).unwrap();
        let (_, replay_signatures) = signed(&doc, "release-1", &release);
        assert!(matches!(
            store.apply_catalog(&bytes, replay_signatures),
            Err(TrustError::Replay)
        ));
    }

    #[test]
    fn accepts_mixed_v1_and_v2_catalog_manifests() {
        let (store, _, release) = store();
        let mut document = serde_json::to_value(catalog(1)).unwrap();
        document["packages"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "manifest": {
                    "schema_version": 2,
                    "id": "com.kosmos.v2-demo",
                    "name": "V2 Demo",
                    "version": "2.0.0",
                    "kind": "app",
                    "engine_api": ">=1.0.0",
                    "entrypoint": "dist/index.html",
                    "publisher": "kosmos",
                    "permissions": [],
                    "targets": [{"runtime": "standalone", "os": ["windows"]}],
                    "data": {"access": [], "defines": [], "mappings": []}
                },
                "archive_url": "https://packages.kosmos.dev/v2-demo.kspkg",
                "sha256": "b".repeat(64),
                "size": 2
            }));
        let (bytes, signatures) = signed(&document, "release-1", &release);

        let verified = store
            .verify_catalog_at(
                &bytes,
                signatures,
                DateTime::parse_from_rfc3339("2029-02-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            )
            .unwrap();
        assert_eq!(verified.document.packages.len(), 2);
        assert_eq!(
            verified.document.packages[1].manifest.id(),
            "com.kosmos.v2-demo"
        );
        assert_eq!(verified.document.packages[1].manifest.version(), "2.0.0");
        assert!(matches!(
            verified.document.packages[1].manifest,
            VersionedManifest::V2(_)
        ));
    }

    /// A signed catalog carrying a `native` descriptor round-trips through
    /// verify → apply → parse with the field intact.
    fn signed_native_document() -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "sequence": 2,
            "issued_at": "2029-01-01T00:00:00Z",
            "expires_at": "2030-01-01T00:00:00Z",
            "packages": [{
                "manifest": {
                    "schema_version": 2,
                    "id": "com.kosmos.agenda",
                    "name": "Agenda",
                    "version": "0.1.1",
                    "kind": "app",
                    "engine_api": ">=1.0.0",
                    "entrypoint": "agenda-gpui.exe",
                    "publisher": "kosmos",
                    "permissions": [],
                    "targets": [{"runtime": "standalone", "os": ["windows"], "arch": ["x86_64"]}],
                    "data": {"access": [], "defines": [], "mappings": []}
                },
                "archive_url": "https://github.com/makekosmos/agenda-gpui/releases/download/v0.1.1/agenda-gpui-0.1.1-x86_64-pc-windows-msvc.zip",
                "sha256": "c".repeat(64),
                "size": 9549390,
                "native": {
                    "repository": "makekosmos/agenda-gpui",
                    "release_tag": "v0.1.1",
                    "target": "x86_64-pc-windows-msvc",
                    "executable": "agenda-gpui.exe"
                }
            }]
        })
    }

    #[test]
    fn accepts_native_app_catalog_entries() {
        let (store, _, release) = store();
        let document = signed_native_document();
        let (bytes, signatures) = signed(&document, "release-1", &release);
        let verified = store
            .verify_catalog_at(
                &bytes,
                signatures,
                DateTime::parse_from_rfc3339("2029-02-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            )
            .unwrap();
        let entry = &verified.document.packages[0];
        let native = entry.native.as_ref().expect("native descriptor");
        assert_eq!(native.repository, "makekosmos/agenda-gpui");
        assert_eq!(native.release_tag, "v0.1.1");
        assert_eq!(native.target, "x86_64-pc-windows-msvc");
        assert_eq!(native.executable, "agenda-gpui.exe");
        assert_eq!(entry.manifest.id(), "com.kosmos.agenda");
        // Round-trip: serialize the parsed entry, parse back, identical.
        let reserialized = serde_json::to_value(entry).unwrap();
        let reparsed: CatalogEntry = serde_json::from_value(reserialized).unwrap();
        assert_eq!(&reparsed, entry);
    }

    #[test]
    fn rejects_malformed_native_catalog_entries() {
        let (store, _, release) = store();
        let cases: [(&str, fn(&mut serde_json::Value)); 8] = [
            ("traversal executable", |doc| {
                doc["packages"][0]["native"]["executable"] =
                    serde_json::json!("../agenda-gpui.exe");
            }),
            ("non-exe executable", |doc| {
                doc["packages"][0]["native"]["executable"] = serde_json::json!("agenda-gpui.dll");
            }),
            ("unknown target", |doc| {
                doc["packages"][0]["native"]["target"] =
                    serde_json::json!("x86_64-unknown-linux-gnu");
            }),
            ("tag mismatch", |doc| {
                doc["packages"][0]["native"]["release_tag"] = serde_json::json!("v9.9.9");
            }),
            ("bad repository", |doc| {
                doc["packages"][0]["native"]["repository"] = serde_json::json!("not-a-repo");
            }),
            ("not an app", |doc| {
                doc["packages"][0]["manifest"]["kind"] = serde_json::json!("source");
            }),
            ("entrypoint mismatch", |doc| {
                doc["packages"][0]["manifest"]["entrypoint"] = serde_json::json!("dist/index.html");
            }),
            ("no standalone target", |doc| {
                doc["packages"][0]["manifest"]["targets"] = serde_json::json!([
                    {"runtime": "worker", "os": ["windows"], "entrypoint": "w.exe"}
                ]);
            }),
        ];
        for (label, mutate) in cases {
            let mut document = signed_native_document();
            mutate(&mut document);
            let (bytes, signatures) = signed(&document, "release-1", &release);
            assert!(
                store
                    .verify_catalog_at(
                        &bytes,
                        signatures,
                        DateTime::parse_from_rfc3339("2029-02-01T00:00:00Z")
                            .unwrap()
                            .with_timezone(&Utc),
                    )
                    .is_err(),
                "{label}"
            );
        }
    }

    #[test]
    fn requires_old_and_new_signature_for_rotation() {
        let (store, _, old) = store();
        let (new, new_public) = key(3, "release-2");
        let transition = KeyTransitionDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            old_key_id: "release-1".into(),
            new_key: new_public,
        };
        let (bytes, mut signatures) = signed(&transition, "release-1", &old);
        assert!(store
            .apply_key_transition(&bytes, signatures.clone())
            .is_err());
        signatures.signatures.push(DetachedSignature {
            key_id: "release-2".into(),
            algorithm: "ed25519".into(),
            signature: STANDARD.encode(new.sign(&bytes).to_bytes()),
        });
        store.apply_key_transition(&bytes, signatures).unwrap();
        assert!(store.is_release_key_trusted("release-2"));
    }
    #[test]
    fn root_revocation_disables_exact_package_and_key() {
        let (store, root, _) = store();
        let doc = RevocationDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            revoked_release_keys: vec!["release-1".into()],
            revoked_packages: vec![PackageRevocation {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                sha256: "a".repeat(64),
            }],
        };
        let (bytes, signatures) = signed(&doc, "root", &root);
        store.apply_revocations(&bytes, signatures).unwrap();
        assert!(!store.is_release_key_trusted("release-1"));
        assert!(store.is_package_revoked("com.kosmos.demo", "1.0.0", &"a".repeat(64)));
        assert_eq!(
            store
                .reconcile(vec![doc.revoked_packages[0].clone()])
                .revoked
                .len(),
            1
        );
    }
}
