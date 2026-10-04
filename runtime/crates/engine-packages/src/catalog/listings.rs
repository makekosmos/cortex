//! Store-facing listing vocabulary: what the Manager renders. These types
//! carry no install authority — installs go through `CatalogEntry` hashes.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub use crate::package_manifest::{DataCompatibility, Fidelity, Role};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ListingKind {
    MundusPackage,
    Integration,
    ExternalApp,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum PublisherTier {
    Mundus,
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
impl Platform {
    /// Host OS token in the catalog availability vocabulary. The Engine owns
    /// the OS→token mapping so shells filter listings without OS branches of
    /// their own. Non-desktop targets fold into Linux, matching
    /// `ManifestTarget::supports_current`.
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }
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

/// Public projection returned to Manager. It intentionally has no paths, hashes,
/// or object bodies.
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
    /// Host platform token (`Platform`) so clients can filter listing
    /// `availability.platforms` without their own OS detection.
    pub platform: Platform,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub listings: Vec<StoreListing>,
    pub installed: Vec<InstalledListing>,
}
