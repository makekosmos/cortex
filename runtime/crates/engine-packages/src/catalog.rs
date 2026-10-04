//! The single integrations catalog — one unsigned `catalog.json` served from
//! `makekosmos/integrations` releases. Trust model: HTTPS + GitHub Releases +
//! the per-package `sha256`/`size` pins in this document, the same model the
//! Engine updater and native apps already use. There are no signing keys,
//! envelopes or trusted-key state anywhere in this path.
//!
//! The document doubles as install authority (`packages[]` hashes gate the
//! `.kspkg` bytes) and storefront source (`external_apps[]` plus the `store`
//! block inside each package manifest become `StoreListing`s for Manager).

mod listings;

pub use listings::{
    Availability, CatalogDto, DataCompatibility, Distribution, EffectiveGrantProjection, Fidelity,
    InstalledListing, ListingKind, Platform, PublisherTier, Role, StoreListing,
};

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::package_manifest::{PackageKind, TargetArch, TargetOs, VersionedManifest};

pub const MAX_DOCUMENT: usize = 1024 * 1024;
const MAX_PACKAGES: usize = 2_000;
const MAX_EXTERNAL_APPS: usize = 256;
const MAX_REVOCATIONS: usize = 10_000;
const SCHEMA_VERSION: u32 = 1;

/// The one catalog URL. `releases/latest` resolves to the newest `catalog-N`
/// release; every `.kspkg` and `icon-<id>.png` lives on the same release.
pub const PRODUCTION_CATALOG_URL: &str =
    "https://github.com/makekosmos/integrations/releases/latest/download/catalog.json";
/// Asset base for derived URLs (per-package Store icons).
const RELEASE_ASSET_BASE: &str =
    "https://github.com/makekosmos/integrations/releases/latest/download";

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("invalid catalog: {0}")]
    Invalid(&'static str),
    #[error("catalog JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("catalog expired")]
    Expired,
    #[error("catalog sequence is not monotonic")]
    Replay,
    #[error("catalog unavailable")]
    Unavailable,
    #[error("catalog persistence failed")]
    Persistence,
    #[error("manifest: {0}")]
    Manifest(#[from] crate::package_manifest::ManifestError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogDocument {
    pub schema_version: u32,
    pub sequence: u64,
    pub issued_at: String,
    pub expires_at: String,
    pub packages: Vec<CatalogEntry>,
    #[serde(default)]
    pub external_apps: Vec<ExternalAppListing>,
    #[serde(default)]
    pub revoked: Vec<PackageRevocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    pub manifest: VersionedManifest,
    /// One artifact per published platform — the same manifest, different
    /// bytes. Install selects the archive matching the current platform.
    pub archives: Vec<CatalogArchive>,
}

/// A platform-specific `.kspkg` artifact of one package release.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogArchive {
    pub os: TargetOs,
    pub arch: TargetArch,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

impl CatalogEntry {
    /// The artifact for the platform this Engine runs on, if published.
    pub fn archive(&self) -> Option<&CatalogArchive> {
        let os = TargetOs::current();
        let arch = TargetArch::current();
        self.archives
            .iter()
            .find(|archive| archive.os == os && archive.arch == arch)
    }
}

/// Storefront-only third-party app an integration connects to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExternalAppListing {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub publisher_tier: PublisherTier,
    pub description: String,
    pub categories: Vec<String>,
    pub platforms: Vec<Platform>,
    pub official_url: String,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub data_compatibility: Vec<DataCompatibility>,
}

/// A revocation trusted exactly like the rest of the catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct PackageRevocation {
    pub id: String,
    pub version: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl CatalogDocument {
    /// Parse and validate a downloaded/persisted catalog. `min_sequence` is
    /// the last-applied sequence: anything at or below it is a replay.
    pub fn parse(
        bytes: &[u8],
        now: DateTime<Utc>,
        min_sequence: u64,
    ) -> Result<Self, CatalogError> {
        if bytes.len() > MAX_DOCUMENT {
            return Err(CatalogError::Invalid("size"));
        }
        let document: CatalogDocument = serde_json::from_slice(bytes)?;
        let issued = parse_utc(&document.issued_at)?;
        let expires = parse_utc(&document.expires_at)?;
        if document.schema_version != SCHEMA_VERSION || document.sequence == 0 || expires <= issued
        {
            return Err(CatalogError::Invalid("envelope"));
        }
        if expires <= now {
            return Err(CatalogError::Expired);
        }
        if document.sequence <= min_sequence {
            return Err(CatalogError::Replay);
        }
        document.validate()?;
        Ok(document)
    }

    /// Same validation minus freshness checks — used for a persisted document
    /// that was already accepted once (an expired catalog stays displayable
    /// as stale but never installs).
    pub fn parse_persisted(bytes: &[u8]) -> Result<Self, CatalogError> {
        if bytes.len() > MAX_DOCUMENT {
            return Err(CatalogError::Invalid("size"));
        }
        let document: CatalogDocument = serde_json::from_slice(bytes)?;
        let issued = parse_utc(&document.issued_at)?;
        let expires = parse_utc(&document.expires_at)?;
        if document.schema_version != SCHEMA_VERSION || document.sequence == 0 || expires <= issued
        {
            return Err(CatalogError::Invalid("envelope"));
        }
        document.validate()?;
        Ok(document)
    }

    fn validate(&self) -> Result<(), CatalogError> {
        if self.packages.is_empty()
            || self.packages.len() > MAX_PACKAGES
            || self.external_apps.len() > MAX_EXTERNAL_APPS
            || self.revoked.len() > MAX_REVOCATIONS
        {
            return Err(CatalogError::Invalid("entry count"));
        }
        let external_ids: BTreeSet<&str> = self
            .external_apps
            .iter()
            .map(|app| app.id.as_str())
            .collect();
        if external_ids.len() != self.external_apps.len() {
            return Err(CatalogError::Invalid("external app ids"));
        }
        let mut seen = BTreeSet::new();
        for entry in &self.packages {
            entry.manifest.validate()?;
            if entry.archives.is_empty() || entry.archives.len() > 8 {
                return Err(CatalogError::Invalid("catalog entry"));
            }
            let mut platforms = BTreeSet::new();
            for archive in &entry.archives {
                let url = reqwest::Url::parse(&archive.url).ok();
                // Debug/test builds additionally accept `file://` archive URLs
                // so fixture catalogs can point at a zip on disk; the pinned
                // sha256/size still gates the bytes. Release builds require
                // HTTPS. An archive may only serve a platform the manifest
                // declares — v1 manifests predate targets and are unchecked.
                let declared = entry.manifest.targets().is_empty()
                    || entry.manifest.targets().iter().any(|target| {
                        target.os.contains(&archive.os)
                            && target
                                .arch
                                .as_ref()
                                .is_none_or(|arches| arches.contains(&archive.arch))
                    });
                if !url.is_some_and(|url| {
                    (url.scheme() == "https" && url.host_str().is_some())
                        || (cfg!(debug_assertions) && url.scheme() == "file")
                }) || !valid_sha256(&archive.sha256)
                    || archive.size == 0
                    || !declared
                    || !platforms.insert((archive.os.clone(), archive.arch.clone()))
                {
                    return Err(CatalogError::Invalid("catalog entry"));
                }
            }
            if !seen.insert((
                entry.manifest.id().to_owned(),
                entry.manifest.version().to_owned(),
            )) {
                return Err(CatalogError::Invalid("catalog entry"));
            }
            // A `connects_to` reference must resolve to a listed external app.
            if let Some(store) = entry.manifest.store() {
                if let Some(connects_to) = &store.connects_to {
                    if !external_ids.contains(connects_to.as_str()) {
                        return Err(CatalogError::Invalid("connects_to"));
                    }
                }
            }
        }
        for app in &self.external_apps {
            if !app.id.starts_with("external.")
                || app.id.len() > 128
                || app.name.is_empty()
                || app.name.len() > 256
                || app.publisher.len() > 256
                || app.description.len() > 4096
                || !https(&app.official_url)
                || app.icon_url.as_ref().is_some_and(|url| !https(url))
                || has_duplicates(&app.platforms)
                || has_duplicates(&app.categories)
            {
                return Err(CatalogError::Invalid("external app"));
            }
        }
        let mut revoked = BTreeSet::new();
        for item in &self.revoked {
            if item.id.is_empty()
                || semver::Version::parse(&item.version).is_err()
                || !valid_sha256(&item.sha256)
                || item
                    .reason
                    .as_ref()
                    .is_some_and(|reason| reason.len() > 1024)
                || !revoked.insert(item.clone())
            {
                return Err(CatalogError::Invalid("revoked entry"));
            }
        }
        Ok(())
    }

    pub fn entry(&self, id: &str, version: &str) -> Option<&CatalogEntry> {
        self.packages
            .iter()
            .find(|entry| entry.manifest.id() == id && entry.manifest.version() == version)
    }

    /// Whether this entry's release is revoked on the current platform. With
    /// no platform archive, a revoked row for any artifact still counts, so a
    /// foreign-platform package cannot hide behind its own platform.
    pub fn entry_revoked(&self, entry: &CatalogEntry) -> bool {
        let (id, version) = (entry.manifest.id(), entry.manifest.version());
        match entry.archive() {
            Some(archive) => self.is_revoked(id, version, &archive.sha256),
            None => entry
                .archives
                .iter()
                .any(|archive| self.is_revoked(id, version, &archive.sha256)),
        }
    }

    pub fn is_revoked(&self, id: &str, version: &str, sha256: &str) -> bool {
        self.revoked.iter().any(|item| {
            item.id == id && item.version == version && item.sha256.eq_ignore_ascii_case(sha256)
        })
    }

    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        parse_utc(&self.expires_at).ok()
    }

    /// Store listings derived from this one document: package entries first
    /// (each with a deterministic icon asset URL), then external apps.
    pub fn listings(&self) -> Vec<StoreListing> {
        let mut listings: Vec<StoreListing> = self
            .packages
            .iter()
            .map(|entry| package_listing(entry))
            .chain(self.external_apps.iter().map(external_listing))
            .collect();
        listings.sort_by(|a, b| a.id.as_bytes().cmp(b.id.as_bytes()));
        listings
    }

    /// Resolve a Store-provided external-app URL. Grants no launch authority.
    pub fn external_url(&self, listing_id: &str) -> Option<String> {
        self.external_apps
            .iter()
            .find(|app| app.id == listing_id && https(&app.official_url))
            .map(|app| app.official_url.clone())
    }
}

fn package_listing(entry: &CatalogEntry) -> StoreListing {
    let manifest = &entry.manifest;
    let id = manifest.id().to_owned();
    let store = manifest.store();
    let connects_to = store.and_then(|store| store.connects_to.clone());
    let integration = matches!(manifest.kind(), PackageKind::Source | PackageKind::Bridge);
    StoreListing {
        kind: if integration {
            ListingKind::Integration
        } else {
            ListingKind::MundusPackage
        },
        name: manifest.name().to_owned(),
        publisher: manifest.publisher().to_owned(),
        publisher_tier: PublisherTier::Mundus,
        description: manifest.store_description().unwrap_or_default().to_owned(),
        categories: store
            .map(|store| store.categories.clone())
            .unwrap_or_default(),
        availability: Availability {
            platforms: manifest
                .target_platforms()
                .into_iter()
                .map(platform_of)
                .collect(),
        },
        data_compatibility: store
            .map(|store| store.data_compatibility.clone())
            .unwrap_or_default(),
        distribution: match connects_to.clone() {
            Some(connects_to) => Distribution::Integration {
                package_id: id.clone(),
                version: manifest.version().to_owned(),
                connects_to,
            },
            None => Distribution::Package {
                package_id: id.clone(),
                version: manifest.version().to_owned(),
            },
        },
        connects_to,
        icon_url: manifest
            .icon()
            .map(|_| format!("{RELEASE_ASSET_BASE}/icon-{id}.png")),
        screenshots: vec![],
        id,
    }
}

fn external_listing(app: &ExternalAppListing) -> StoreListing {
    StoreListing {
        id: app.id.clone(),
        kind: ListingKind::ExternalApp,
        name: app.name.clone(),
        publisher: app.publisher.clone(),
        publisher_tier: app.publisher_tier.clone(),
        description: app.description.clone(),
        categories: app.categories.clone(),
        availability: Availability {
            platforms: app.platforms.clone(),
        },
        data_compatibility: app.data_compatibility.clone(),
        distribution: Distribution::External {
            official_url: app.official_url.clone(),
        },
        connects_to: None,
        icon_url: app.icon_url.clone(),
        screenshots: vec![],
    }
}

fn platform_of(os: TargetOs) -> Platform {
    match os {
        TargetOs::Windows => Platform::Windows,
        TargetOs::Macos => Platform::Macos,
        TargetOs::Linux => Platform::Linux,
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
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

fn parse_utc(value: &str) -> Result<DateTime<Utc>, CatalogError> {
    if !value.ends_with('Z') {
        return Err(CatalogError::Invalid("timestamp"));
    }
    DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| CatalogError::Invalid("timestamp"))
}

#[cfg(test)]
mod tests;
