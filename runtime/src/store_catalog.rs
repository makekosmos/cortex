//! Signed Store Catalog discovery metadata. This module has no install authority.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::package_trust::{self, DetachedSignature, SignatureSet};
const MAX_DOCUMENT: usize = 1024 * 1024;
const MAX_ENVELOPE: usize = 2 * 1024 * 1024;
const PRODUCTION_CATALOG_URL: &str =
    "https://github.com/makekosmos/store/releases/latest/download/catalog.envelope.json";
const PRODUCTION_KEY_ID: &str = "kosmos-store-2026";
const PRODUCTION_PUBLIC_KEY_B64: &str = "it14mzPjoqdgaHXdCDIjCoUgGXf/f5izJrGRUuk3o/A=";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TrustError {
    #[error("invalid catalog: {0}")]
    Invalid(&'static str),
    #[error("catalog JSON: {0}")]
    Json(String),
    #[error("invalid signature")]
    InvalidSignature,
    #[error("untrusted store key")]
    UntrustedKey,
    #[error("catalog expired")]
    Expired,
    #[error("catalog replay")]
    Replay,
    #[error("package release is unavailable")]
    MissingRelease,
    #[error("catalog persistence failed")]
    Persistence,
    #[error("catalog unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StoreCatalogEnvelope {
    pub bytes: String,
    pub signatures: SignatureSet,
}
impl StoreCatalogEnvelope {
    pub fn new(bytes: Vec<u8>, key_id: impl Into<String>, signature: Vec<u8>) -> Self {
        Self {
            bytes: STANDARD.encode(bytes),
            signatures: SignatureSet {
                schema_version: 1,
                signatures: vec![DetachedSignature {
                    key_id: key_id.into(),
                    algorithm: "ed25519".into(),
                    signature: STANDARD.encode(signature),
                }],
            },
        }
    }
    fn size(&self) -> usize {
        self.bytes.len()
            + self
                .signatures
                .signatures
                .iter()
                .map(|signature| signature.key_id.len() + signature.signature.len())
                .sum::<usize>()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogDocument {
    pub schema_version: u32,
    pub sequence: u64,
    pub issued_at: String,
    pub expires_at: String,
    pub listings: Vec<StoreListing>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StoreListing {
    pub id: String,
    pub kind: ListingKind,
    pub name: String,
    pub publisher: String,
    pub publisher_tier: PublisherTier,
    pub description: String,
    pub categories: Vec<String>,
    pub availability: Availability,
    pub data_compatibility: Vec<DataCompatibility>,
    pub distribution: Distribution,
    pub connects_to: Option<String>,
    pub icon_url: Option<String>,
    pub screenshots: Vec<String>,
}
impl StoreListing {
    pub fn external(id: &str, name: &str, official_url: &str) -> Self {
        Self {
            id: id.into(),
            kind: ListingKind::ExternalApp,
            name: name.into(),
            publisher: String::new(),
            publisher_tier: PublisherTier::Community,
            description: String::new(),
            categories: vec![],
            availability: Availability { platforms: vec![] },
            data_compatibility: vec![],
            distribution: Distribution::External {
                official_url: official_url.into(),
            },
            connects_to: None,
            icon_url: None,
            screenshots: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ListingKind {
    KosmosPackage,
    Integration,
    ExternalApp,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum PublisherTier {
    Kosmos,
    Verified,
    Community,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Availability {
    pub platforms: Vec<Platform>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    Macos,
    Linux,
    Ios,
    Android,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DataCompatibility {
    #[serde(rename = "type")]
    pub type_id: String,
    pub versions: String,
    pub roles: BTreeSet<Role>,
    pub via: String,
    pub fidelity: Fidelity,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Read,
    Edit,
    Import,
    Export,
    Sync,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Fidelity {
    Native,
    Lossless,
    Lossy,
    MetadataOnly,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged, deny_unknown_fields)]
pub enum Distribution {
    Package {
        package_id: String,
        version: String,
    },
    External {
        official_url: String,
    },
    Integration {
        package_id: String,
        version: String,
        connects_to: String,
    },
}

pub trait PackageIndexLookup {
    fn package_release(&self, package_id: &str, version: &str, is_bridge: bool) -> bool;
    fn canonical_type_version(&self, type_id: &str, versions: &str) -> bool;
}

#[derive(Debug, Clone)]
pub struct VerifiedCatalog {
    pub document: CatalogDocument,
    pub signed_bytes: Vec<u8>,
    pub expires_at: DateTime<Utc>,
}

pub struct StoreCatalogTrust {
    key_id: String,
    key: VerifyingKey,
    sequence: Mutex<u64>,
}
impl StoreCatalogTrust {
    pub fn new(key_id: impl Into<String>, key: VerifyingKey) -> Result<Self, TrustError> {
        let key_id = key_id.into();
        if key_id.is_empty() {
            return Err(TrustError::Invalid("key id"));
        }
        Ok(Self {
            key_id,
            key,
            sequence: Mutex::new(0),
        })
    }
    fn set_sequence(&self, sequence: u64) {
        let mut current = self.sequence.lock().unwrap_or_else(|p| p.into_inner());
        *current = (*current).max(sequence);
    }
    pub fn verify_at<I: PackageIndexLookup>(
        &self,
        envelope: &StoreCatalogEnvelope,
        now: DateTime<Utc>,
        index: &I,
    ) -> Result<VerifiedCatalog, TrustError> {
        if envelope.size() > MAX_ENVELOPE {
            return Err(TrustError::Invalid("size"));
        }
        envelope
            .signatures
            .validate()
            .map_err(|_| TrustError::Invalid("signature set"))?;
        let signed = STANDARD
            .decode(&envelope.bytes)
            .map_err(|_| TrustError::Invalid("document encoding"))?;
        if signed.len() > MAX_DOCUMENT {
            return Err(TrustError::Invalid("size"));
        }
        let signature = envelope
            .signatures
            .signatures
            .iter()
            .find(|signature| signature.key_id == self.key_id)
            .ok_or(TrustError::UntrustedKey)?;
        package_trust::verify_signature(&signed, signature, &self.key)
            .map_err(|_| TrustError::InvalidSignature)?;
        let document: CatalogDocument =
            serde_json::from_slice(&signed).map_err(|e| TrustError::Json(e.to_string()))?;
        let issued = parse_time(&document.issued_at)?;
        let expires = parse_time(&document.expires_at)?;
        if document.schema_version != 1 || document.sequence == 0 || expires <= issued {
            return Err(TrustError::Invalid("envelope"));
        }
        if expires <= now {
            return Err(TrustError::Expired);
        }
        validate_document(&document, index)?;
        let sequence = *self.sequence.lock().unwrap_or_else(|p| p.into_inner());
        if document.sequence <= sequence {
            return Err(TrustError::Replay);
        }
        Ok(VerifiedCatalog {
            document,
            signed_bytes: signed,
            expires_at: expires,
        })
    }
    pub fn apply<I: PackageIndexLookup>(
        &self,
        envelope: &StoreCatalogEnvelope,
        now: DateTime<Utc>,
        index: &I,
    ) -> Result<VerifiedCatalog, TrustError> {
        let verified = self.verify_at(envelope, now, index)?;
        let mut sequence = self.sequence.lock().unwrap_or_else(|p| p.into_inner());
        if verified.document.sequence <= *sequence {
            return Err(TrustError::Replay);
        }
        *sequence = verified.document.sequence;
        Ok(verified)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheState {
    Empty,
    Fresh { sequence: u64 },
    Expired { sequence: u64 },
}
#[derive(Default)]
pub struct CatalogCache {
    inner: Mutex<Option<VerifiedCatalog>>,
}
impl CatalogCache {
    pub fn apply(&self, verified: &VerifiedCatalog) -> Result<(), TrustError> {
        let mut slot = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if slot
            .as_ref()
            .is_some_and(|old| verified.document.sequence <= old.document.sequence)
        {
            return Err(TrustError::Replay);
        }
        *slot = Some(verified.clone());
        Ok(())
    }
    pub fn sequence(&self) -> Option<u64> {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|v| v.document.sequence)
    }
    pub fn state(&self) -> CacheState {
        self.state_at(Utc::now())
    }
    pub fn state_at(&self, now: DateTime<Utc>) -> CacheState {
        let slot = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        match slot.as_ref() {
            None => CacheState::Empty,
            Some(v) if v.expires_at <= now => CacheState::Expired {
                sequence: v.document.sequence,
            },
            Some(v) => CacheState::Fresh {
                sequence: v.document.sequence,
            },
        }
    }
    pub fn normalized(&self) -> Option<CatalogDocument> {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|v| {
                let mut d = v.document.clone();
                normalize_document(&mut d);
                d
            })
    }

    pub fn dto(&self, now: DateTime<Utc>, installed: Vec<InstalledListing>) -> CatalogDto {
        let slot = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let (state, sequence, issued_at, expires_at, listings) = match slot.as_ref() {
            Some(value) => {
                let state = if value.expires_at <= now {
                    "expired"
                } else {
                    "fresh"
                };
                (
                    state,
                    Some(value.document.sequence),
                    Some(value.document.issued_at.clone()),
                    Some(value.document.expires_at.clone()),
                    normalize_document_value(&value.document),
                )
            }
            None => ("unavailable", None, None, None, Vec::new()),
        };
        CatalogDto {
            state: state.into(),
            sequence,
            issued_at,
            expires_at,
            listings,
            installed,
        }
    }
}

/// Public projection returned to Manager. It intentionally has no paths, hashes,
/// signatures, tokens, or object bodies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledListing {
    pub id: String,
    pub version: String,
    pub kind: String,
    pub enabled: bool,
    pub revoked: bool,
    pub publisher: String,
    #[serde(default)]
    pub effective_grants: Vec<EffectiveGrantProjection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectiveGrantProjection {
    #[serde(rename = "type")]
    pub type_id: String,
    pub version: String,
    pub roles: BTreeSet<Role>,
    #[serde(default)]
    pub fields_read: Vec<String>,
    #[serde(default)]
    pub fields_write: Vec<String>,
    #[serde(default)]
    pub relations_read: Vec<String>,
    #[serde(default)]
    pub relations_write: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogDto {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub listings: Vec<StoreListing>,
    pub installed: Vec<InstalledListing>,
}

fn normalize_document_value(document: &CatalogDocument) -> Vec<StoreListing> {
    let mut document = document.clone();
    normalize_document(&mut document);
    document.listings
}

/// Atomic, replay-resistant Store Catalog persistence and refresh helper.
pub struct StoreCatalogService {
    path: PathBuf,
    trust: StoreCatalogTrust,
    cache: CatalogCache,
}

impl StoreCatalogService {
    /// Store discovery has its own compile-time trust root; it never reuses the
    /// Package Index release-key configuration.
    pub fn open_compiled(data_dir: impl AsRef<Path>) -> Result<Self, TrustError> {
        let key_id = option_env!("KOSMOS_STORE_CATALOG_KEY_ID")
            .filter(|key_id| !key_id.is_empty())
            .unwrap_or(PRODUCTION_KEY_ID);
        let key =
            option_env!("KOSMOS_STORE_CATALOG_PUBLIC_KEY_B64").unwrap_or(PRODUCTION_PUBLIC_KEY_B64);
        let key = STANDARD
            .decode(key)
            .ok()
            .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
            .and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
            .ok_or(TrustError::Unavailable)?;
        Self::open(data_dir, key_id, key)
    }

    pub fn open(
        data_dir: impl AsRef<Path>,
        key_id: impl Into<String>,
        key: VerifyingKey,
    ) -> Result<Self, TrustError> {
        let root = data_dir.as_ref().join("store");
        fs::create_dir_all(&root).map_err(|_| TrustError::Persistence)?;
        let path = root.join("catalog.json");
        let service = Self {
            path,
            trust: StoreCatalogTrust::new(key_id, key)?,
            cache: CatalogCache::default(),
        };
        service.load_persisted()?;
        Ok(service)
    }

    fn load_persisted(&self) -> Result<(), TrustError> {
        if !self.path.exists() {
            return Ok(());
        }
        let bytes = fs::read(&self.path).map_err(|_| TrustError::Persistence)?;
        let envelope: StoreCatalogEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| TrustError::Persistence)?;
        // Expired entries remain displayable as stale; signature and schema are
        // still checked at the document issue time during restart.
        let issued = parse_time(
            &serde_json::from_slice::<CatalogDocument>(
                &STANDARD
                    .decode(&envelope.bytes)
                    .map_err(|_| TrustError::Persistence)?,
            )
            .map_err(|_| TrustError::Persistence)?
            .issued_at,
        )?;
        let verified = self.trust.verify_at(&envelope, issued, &NoopLookup)?;
        self.trust.set_sequence(verified.document.sequence);
        self.cache.apply(&verified)
    }

    pub fn catalog(&self, now: DateTime<Utc>, installed: Vec<InstalledListing>) -> CatalogDto {
        self.cache.dto(now, installed)
    }

    /// Resolves a Store-provided external-app URL without granting any launch
    /// authority. Stale and unavailable catalogs are intentionally unusable.
    pub fn external_url_at(
        &self,
        listing_id: &str,
        now: DateTime<Utc>,
    ) -> Result<String, TrustError> {
        if listing_id.is_empty()
            || listing_id.len() > 128
            || listing_id.chars().any(char::is_control)
        {
            return Err(TrustError::Invalid("listing id"));
        }
        if !matches!(self.cache.state_at(now), CacheState::Fresh { .. }) {
            return Err(TrustError::Unavailable);
        }
        let listing = self
            .cache
            .normalized()
            .and_then(|document| {
                document
                    .listings
                    .into_iter()
                    .find(|listing| listing.id == listing_id)
            })
            .ok_or(TrustError::Unavailable)?;
        match (listing.kind, listing.distribution) {
            (ListingKind::ExternalApp, Distribution::External { official_url })
                if https(&official_url) =>
            {
                Ok(official_url)
            }
            _ => Err(TrustError::Unavailable),
        }
    }

    pub async fn refresh<I: PackageIndexLookup + Sync>(
        &self,
        index: &I,
    ) -> Result<CatalogDto, TrustError> {
        let url = std::env::var("KOSMOS_STORE_CATALOG_URL")
            .unwrap_or_else(|_| PRODUCTION_CATALOG_URL.to_string());
        if !https(&url) {
            return Err(TrustError::Unavailable);
        }
        let response = reqwest::get(url)
            .await
            .map_err(|_| TrustError::Unavailable)?;
        if !response.status().is_success() {
            return Err(TrustError::Unavailable);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| TrustError::Unavailable)?;
        if bytes.len() > MAX_ENVELOPE {
            return Err(TrustError::Invalid("size"));
        }
        let envelope: StoreCatalogEnvelope =
            serde_json::from_slice(&bytes).map_err(|e| TrustError::Json(e.to_string()))?;
        self.apply(&envelope, Utc::now(), index)?;
        Ok(self.catalog(Utc::now(), Vec::new()))
    }

    pub fn apply<I: PackageIndexLookup>(
        &self,
        envelope: &StoreCatalogEnvelope,
        now: DateTime<Utc>,
        index: &I,
    ) -> Result<(), TrustError> {
        let verified = self.trust.apply(envelope, now, index)?;
        let encoded = serde_json::to_vec(envelope).map_err(|_| TrustError::Persistence)?;
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, encoded).map_err(|_| TrustError::Persistence)?;
        fs::rename(&temp, &self.path).map_err(|_| TrustError::Persistence)?;
        self.cache.apply(&verified)
    }
}

struct NoopLookup;
impl PackageIndexLookup for NoopLookup {
    fn package_release(&self, _package_id: &str, _version: &str, _is_bridge: bool) -> bool {
        true
    }
    fn canonical_type_version(&self, _type_id: &str, _versions: &str) -> bool {
        true
    }
}

fn parse_time(value: &str) -> Result<DateTime<Utc>, TrustError> {
    if !value.ends_with('Z') {
        return Err(TrustError::Invalid("timestamp"));
    }
    DateTime::parse_from_rfc3339(value)
        .map(|x| x.with_timezone(&Utc))
        .map_err(|_| TrustError::Invalid("timestamp"))
}
fn validate_document<I: PackageIndexLookup>(
    document: &CatalogDocument,
    index: &I,
) -> Result<(), TrustError> {
    if document.listings.len() > 2000 {
        return Err(TrustError::Invalid("listing count"));
    }
    let mut ids = BTreeSet::new();
    let external_ids: BTreeSet<&str> = document
        .listings
        .iter()
        .filter(|l| l.kind == ListingKind::ExternalApp)
        .map(|l| l.id.as_str())
        .collect();
    for listing in &document.listings {
        if listing.id.is_empty()
            || listing.id.len() > 128
            || !ids.insert(listing.id.clone())
            || listing.name.len() > 256
            || listing.publisher.len() > 256
            || listing.description.len() > 4096
            || listing.categories.len() > 32
            || listing.availability.platforms.len() > 5
            || listing.data_compatibility.len() > 64
            || listing.screenshots.len() > 12
        {
            return Err(TrustError::Invalid("listing bounds"));
        }
        if has_duplicates(&listing.categories)
            || has_duplicates(&listing.availability.platforms)
            || has_duplicates(&listing.screenshots)
        {
            return Err(TrustError::Invalid("duplicate metadata"));
        }
        match (&listing.kind, &listing.distribution, &listing.connects_to) {
            (
                ListingKind::KosmosPackage,
                Distribution::Package {
                    package_id,
                    version,
                },
                None,
            ) if index.package_release(package_id, version, false) => {}
            (ListingKind::ExternalApp, Distribution::External { official_url }, None)
                if https(official_url) => {}
            (
                ListingKind::Integration,
                Distribution::Integration {
                    package_id,
                    version,
                    connects_to,
                },
                Some(link),
            ) if link == connects_to
                && external_ids.contains(connects_to.as_str())
                && index.package_release(package_id, version, false) => {}
            _ => return Err(TrustError::Invalid("kind distribution")),
        }
        for row in &listing.data_compatibility {
            if row.via != listing.id
                && !(matches!(listing.kind, ListingKind::Integration)
                    && listing.connects_to.as_deref() == Some(row.via.as_str()))
            {
                return Err(TrustError::Invalid("compatibility via"));
            }
            if row.versions.parse::<semver::VersionReq>().is_err()
                || row.type_id.is_empty()
                || !index.canonical_type_version(&row.type_id, &row.versions)
                || row.roles.is_empty()
            {
                return Err(TrustError::Invalid("compatibility"));
            }
        }
    }
    Ok(())
}
fn https(value: &str) -> bool {
    value.len() <= 2048
        && reqwest::Url::parse(value)
            .ok()
            .is_some_and(|u| u.scheme() == "https" && u.host_str().is_some())
}

fn has_duplicates<T: Ord>(values: &[T]) -> bool {
    let mut seen = BTreeSet::new();
    values.iter().any(|value| !seen.insert(value))
}

fn normalize_document(document: &mut CatalogDocument) {
    document
        .listings
        .sort_by(|a, b| a.id.as_bytes().cmp(b.id.as_bytes()));
    for listing in &mut document.listings {
        listing.categories.sort();
        listing.availability.platforms.sort();
        listing
            .screenshots
            .sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        listing.data_compatibility.sort_by(|a, b| {
            (&a.type_id, &a.versions, &a.via, &a.fidelity).cmp(&(
                &b.type_id,
                &b.versions,
                &b.via,
                &b.fidelity,
            ))
        });
    }
}
