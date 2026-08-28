//! Strict, executable-free package manifest models.

pub mod integration;

pub use integration::{
    BrowserLogin, CookieHeaderInjection, IntegrationManifest, IntegrationRequestMethod,
    IntegrationSchedule, IntegrationSetting, IntegrationSettingKind, SecretInjection,
};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

const MAX_ID: usize = 64;
const MAX_NAME: usize = 128;
const MAX_DESCRIPTION: usize = 4096;
const MAX_ENTRYPOINT: usize = 256;
const MAX_ICON: usize = 256;
const MAX_PERMISSIONS: usize = 32;
const MAX_SCOPES: usize = 16;
const MAX_SCOPE: usize = 128;
const MAX_TARGETS: usize = 16;
const MAX_ACCESS: usize = 64;
const MAX_DEFINES: usize = 64;
const MAX_MAPPINGS: usize = 64;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ManifestError {
    #[error("manifest field `{0}` is invalid")]
    InvalidField(&'static str),
    #[error("unknown capability `{0}`")]
    UnknownCapability(String),
    #[error("duplicate permission capability `{0}`")]
    DuplicateCapability(String),
    #[error("package dependencies are not supported")]
    DependenciesNotSupported,
    #[error("manifest JSON is invalid: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    App,
    Source,
    Bridge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PermissionRequest {
    pub capability: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PackageManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: PackageKind,
    pub engine_api: String,
    pub entrypoint: String,
    pub publisher: String,
    #[serde(default)]
    pub permissions: Vec<PermissionRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum TargetRuntime {
    KosmosHost,
    Worker,
    Standalone,
    Web,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TargetOs {
    Windows,
    Macos,
    Linux,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TargetArch {
    X86_64,
    Arm64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct ManifestTarget {
    pub runtime: TargetRuntime,
    pub os: Vec<TargetOs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<Vec<TargetArch>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum DataAction {
    Read,
    Create,
    Update,
    Delete,
    Subscribe,
    Link,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FieldAccess {
    pub read: Vec<String>,
    #[serde(default)]
    pub write: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RelationAccess {
    pub read: Vec<String>,
    #[serde(default)]
    pub write: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DataAccessRule {
    #[serde(rename = "type")]
    pub type_id: String,
    pub versions: String,
    pub actions: Vec<DataAction>,
    pub fields: FieldAccess,
    #[serde(default)]
    pub relations: Option<RelationAccess>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DefinitionReference {
    #[serde(rename = "type")]
    pub type_id: String,
    pub version: String,
    pub schema: String,
    #[serde(default)]
    pub content_contract: Option<String>,
    #[serde(default)]
    pub relations: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MappingDirection {
    Import,
    Export,
    BidirectionalSync,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MappingFidelity {
    Native,
    Lossless,
    Lossy,
    MetadataOnly,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManifestMapping {
    #[serde(rename = "type")]
    pub type_id: String,
    pub versions: String,
    pub direction: MappingDirection,
    pub fidelity: MappingFidelity,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManifestData {
    #[serde(default)]
    pub access: Vec<DataAccessRule>,
    #[serde(default)]
    pub defines: Vec<DefinitionReference>,
    #[serde(default)]
    pub mappings: Vec<ManifestMapping>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManifestV2 {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub version: String,
    pub kind: PackageKind,
    pub engine_api: String,
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub publisher: String,
    #[serde(default)]
    pub permissions: Vec<PermissionRequest>,
    pub targets: Vec<ManifestTarget>,
    pub data: ManifestData,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration: Option<IntegrationManifest>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum VersionedManifest {
    V1(PackageManifest),
    V2(ManifestV2),
}

impl VersionedManifest {
    /// Common package metadata used by the legacy store/launch surfaces.
    /// Version-specific fields stay in the enum variant and are persisted by
    /// callers that need the full v2 data contract.
    pub fn common_manifest(&self) -> PackageManifest {
        match self {
            Self::V1(manifest) => manifest.clone(),
            Self::V2(manifest) => PackageManifest {
                schema_version: 1,
                id: manifest.id.clone(),
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                kind: manifest.kind.clone(),
                engine_api: manifest.engine_api.clone(),
                entrypoint: manifest.entrypoint.clone(),
                publisher: manifest.publisher.clone(),
                permissions: manifest.permissions.clone(),
            },
        }
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        match self {
            Self::V1(manifest) => manifest.validate(),
            Self::V2(manifest) => manifest.validate(),
        }
    }

    pub fn schema_version(&self) -> u32 {
        match self {
            Self::V1(manifest) => manifest.schema_version,
            Self::V2(manifest) => manifest.schema_version,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.id,
            Self::V2(manifest) => &manifest.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.name,
            Self::V2(manifest) => &manifest.name,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.version,
            Self::V2(manifest) => &manifest.version,
        }
    }

    pub fn kind(&self) -> &PackageKind {
        match self {
            Self::V1(manifest) => &manifest.kind,
            Self::V2(manifest) => &manifest.kind,
        }
    }

    pub fn engine_api(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.engine_api,
            Self::V2(manifest) => &manifest.engine_api,
        }
    }

    pub fn entrypoint(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.entrypoint,
            Self::V2(manifest) => &manifest.entrypoint,
        }
    }

    pub fn icon(&self) -> Option<&str> {
        match self {
            Self::V1(_) => None,
            Self::V2(manifest) => manifest.icon.as_deref(),
        }
    }

    pub fn publisher(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.publisher,
            Self::V2(manifest) => &manifest.publisher,
        }
    }

    pub fn permissions(&self) -> &[PermissionRequest] {
        match self {
            Self::V1(manifest) => &manifest.permissions,
            Self::V2(manifest) => &manifest.permissions,
        }
    }
}

/// Trusted-install evidence for legacy v1 compatibility. Parsing itself does
/// not prove signatures or first-party ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V1TrustContext {
    pub signature_verified: bool,
    pub first_party_package: bool,
}

impl V1TrustContext {
    pub fn verified_first_party() -> Self {
        Self {
            signature_verified: true,
            first_party_package: true,
        }
    }
}

pub trait DefinitionSnapshotReader {
    fn read_definition(&self, path: &str) -> Result<Vec<u8>, String>;
}

impl ManifestV2 {
    pub fn validate_definition_documents<R: DefinitionSnapshotReader>(
        &self,
        reader: &R,
    ) -> Result<(), ManifestError> {
        let mut total = 0usize;
        for definition in &self.data.defines {
            for path in std::iter::once(&definition.schema)
                .chain(definition.content_contract.iter())
                .chain(definition.relations.iter())
            {
                let bytes = reader
                    .read_definition(path)
                    .map_err(|_| ManifestError::InvalidField("data.defines"))?;
                if bytes.len() > 512 * 1024 || total + bytes.len() > 8 * 1024 * 1024 {
                    return Err(ManifestError::InvalidField("data.defines"));
                }
                total += bytes.len();
                let value: serde_json::Value = serde_json::from_slice(&bytes)
                    .map_err(|_| ManifestError::InvalidField("data.defines"))?;
                if !value.is_object() {
                    return Err(ManifestError::InvalidField("data.defines"));
                }
            }
        }
        Ok(())
    }
}

impl PackageManifest {
    pub fn parse_with_v1_trust(
        input: &str,
        trust: &V1TrustContext,
    ) -> Result<VersionedManifest, ManifestError> {
        let parsed = Self::parse(input)?;
        if matches!(parsed, VersionedManifest::V1(_))
            && (!trust.signature_verified || !trust.first_party_package)
        {
            return Err(ManifestError::InvalidField("v1_trust"));
        }
        Ok(parsed)
    }

    pub fn parse(input: &str) -> Result<VersionedManifest, ManifestError> {
        let value: serde_json::Value =
            serde_json::from_str(input).map_err(|e| ManifestError::Parse(e.to_string()))?;
        let version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or(ManifestError::InvalidField("schema_version"))?;
        match version {
            1 => {
                let manifest: PackageManifest = serde_json::from_value(value)
                    .map_err(|e| ManifestError::Parse(e.to_string()))?;
                manifest.validate()?;
                Ok(VersionedManifest::V1(manifest))
            }
            2 => {
                let manifest: ManifestV2 = serde_json::from_value(value)
                    .map_err(|e| ManifestError::Parse(e.to_string()))?;
                manifest.validate()?;
                Ok(VersionedManifest::V2(manifest))
            }
            _ => Err(ManifestError::InvalidField("schema_version")),
        }
    }
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != 1 {
            return Err(ManifestError::InvalidField("schema_version"));
        }
        validate_identity(
            self.schema_version,
            &self.id,
            &self.name,
            &self.version,
            &self.engine_api,
            &self.entrypoint,
            &self.publisher,
            &self.permissions,
        )?;
        Ok(())
    }
}
impl ManifestV2 {
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != 2 {
            return Err(ManifestError::InvalidField("schema_version"));
        }
        validate_identity(
            self.schema_version,
            &self.id,
            &self.name,
            &self.version,
            &self.engine_api,
            &self.entrypoint,
            &self.publisher,
            &self.permissions,
        )?;
        validate_v2(self)
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_identity(
    schema_version: u32,
    id: &str,
    name: &str,
    version: &str,
    engine_api: &str,
    entrypoint: &str,
    publisher: &str,
    permissions: &[PermissionRequest],
) -> Result<(), ManifestError> {
    if !matches!(schema_version, 1 | 2) || !safe_package_id(id) {
        return Err(ManifestError::InvalidField("schema_version/id"));
    }
    if name.is_empty() || name.len() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(ManifestError::InvalidField("name"));
    }
    Version::parse(version).map_err(|_| ManifestError::InvalidField("version"))?;
    VersionReq::parse(engine_api).map_err(|_| ManifestError::InvalidField("engine_api"))?;
    if publisher != "kosmos" {
        return Err(ManifestError::InvalidField("publisher"));
    }
    if !safe_relative_path(entrypoint, MAX_ENTRYPOINT) {
        return Err(ManifestError::InvalidField("entrypoint"));
    }
    if permissions.len() > MAX_PERMISSIONS {
        return Err(ManifestError::InvalidField("permissions"));
    }
    let mut seen = HashSet::new();
    for p in permissions {
        if !known_capability(&p.capability) {
            return Err(ManifestError::UnknownCapability(p.capability.clone()));
        }
        if !seen.insert(&p.capability) {
            return Err(ManifestError::DuplicateCapability(p.capability.clone()));
        }
        if p.scopes.len() > MAX_SCOPES
            || p.scopes
                .iter()
                .any(|s| s.is_empty() || s.len() > MAX_SCOPE || s.contains('\0'))
        {
            return Err(ManifestError::InvalidField("permissions.scopes"));
        }
    }
    Ok(())
}
fn validate_v2(m: &ManifestV2) -> Result<(), ManifestError> {
    if m.description
        .as_ref()
        .is_some_and(|description| description.len() > MAX_DESCRIPTION)
    {
        return Err(ManifestError::InvalidField("description"));
    }
    if m.icon.as_ref().is_some_and(|icon| !safe_icon_path(icon)) {
        return Err(ManifestError::InvalidField("icon"));
    }
    if m.targets.is_empty()
        || m.targets.len() > MAX_TARGETS
        || unique_len(&m.targets) != m.targets.len()
    {
        return Err(ManifestError::InvalidField("targets"));
    }
    for t in &m.targets {
        if t.os.is_empty() || t.os.len() > 8 || unique_len(&t.os) != t.os.len() {
            return Err(ManifestError::InvalidField("targets.os"));
        }
        if let Some(a) = &t.arch {
            if a.is_empty() || a.len() > 8 || unique_len(a) != a.len() {
                return Err(ManifestError::InvalidField("targets.arch"));
            }
        }
    }
    if m.data.access.len() > MAX_ACCESS
        || m.data.defines.len() > MAX_DEFINES
        || m.data.mappings.len() > MAX_MAPPINGS
    {
        return Err(ManifestError::InvalidField("data"));
    }
    let mut types = HashSet::new();
    for r in &m.data.access {
        if !valid_type_id(&r.type_id)
            || r.type_id.len() > 128
            || !types.insert(&r.type_id)
            || r.actions.is_empty()
            || r.actions.len() > 6
            || unique_len(&r.actions) != r.actions.len()
        {
            return Err(ManifestError::InvalidField("data.access"));
        }
        if r.versions.len() > 128 {
            return Err(ManifestError::InvalidField("data.access.versions"));
        }
        VersionReq::parse(&r.versions)
            .map_err(|_| ManifestError::InvalidField("data.access.versions"))?;
        validate_lists(&r.fields.read, 128, true, "data.access.fields.read")?;
        validate_lists(&r.fields.write, 128, false, "data.access.fields.write")?;
        if !r.fields.read.is_empty() && !r.actions.contains(&DataAction::Read) {
            return Err(ManifestError::InvalidField("data.access.fields.read"));
        }
        if !r.fields.write.is_empty()
            && !r
                .actions
                .iter()
                .any(|a| matches!(a, DataAction::Create | DataAction::Update))
        {
            return Err(ManifestError::InvalidField("data.access.fields.write"));
        }
        if let Some(rel) = &r.relations {
            validate_lists(&rel.read, 64, true, "data.access.relations.read")?;
            validate_lists(&rel.write, 64, false, "data.access.relations.write")?;
            if !rel.read.is_empty() && !r.actions.contains(&DataAction::Read) {
                return Err(ManifestError::InvalidField("data.access.relations.read"));
            }
            if !rel.write.is_empty() && !r.actions.contains(&DataAction::Link) {
                return Err(ManifestError::InvalidField("data.access.relations.write"));
            }
        }
    }
    let mut defs = HashSet::new();
    for d in &m.data.defines {
        let prefix = format!("{}.", m.id);
        let suffix = d.type_id.strip_prefix(&prefix).unwrap_or("");
        if suffix.is_empty()
            || suffix.len() > 64
            || suffix.split('.').any(|part| part.is_empty())
            || !d.type_id.starts_with(&prefix)
            || !valid_type_id(&d.type_id)
            || Version::parse(&d.version).is_err()
            || !defs.insert((&d.type_id, &d.version))
            || !safe_relative_path(&d.schema, 256)
            || d.content_contract
                .as_ref()
                .is_some_and(|p| !safe_relative_path(p, 256))
            || d.relations
                .as_ref()
                .is_some_and(|p| !safe_relative_path(p, 256))
        {
            return Err(ManifestError::InvalidField("data.defines"));
        }
    }
    let mut mappings = HashSet::new();
    for x in &m.data.mappings {
        if !valid_type_id(&x.type_id)
            || x.versions.len() > 128
            || VersionReq::parse(&x.versions).is_err()
            || !mappings.insert(&x.type_id)
        {
            return Err(ManifestError::InvalidField("data.mappings"));
        }
    }
    if let Some(integration) = &m.integration {
        integration.validate(m)?;
    }
    Ok(())
}
fn validate_lists(
    xs: &[String],
    max: usize,
    read: bool,
    field: &'static str,
) -> Result<(), ManifestError> {
    if xs.len() > max
        || unique_len(xs) != xs.len()
        || xs
            .iter()
            .any(|x| x.is_empty() || x.chars().any(char::is_control) || (!read && x == "*"))
    {
        return Err(ManifestError::InvalidField(field));
    }
    Ok(())
}
fn unique_len<T: Eq + std::hash::Hash>(xs: &[T]) -> usize {
    xs.iter().collect::<HashSet<_>>().len()
}
fn safe_relative_path(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !Path::new(value).is_absolute()
        && !value.contains('\\')
        && !value.contains('\0')
        && value
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
fn safe_icon_path(value: &str) -> bool {
    safe_relative_path(value, MAX_ICON)
        && value.rsplit_once('.').is_some_and(|(_, extension)| {
            extension.eq_ignore_ascii_case("ico") || extension.eq_ignore_ascii_case("png")
        })
}
fn valid_type_id(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|p| {
            !p.is_empty()
                && p.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-')
                })
        })
}
fn known_capability(value: &str) -> bool {
    matches!(
        value,
        "ark.read"
            | "ark.write"
            | "launcher.search"
            | "network"
            | "filesystem.read"
            | "filesystem.write"
            | "clipboard"
            | "notifications"
            | "process.spawn"
    )
}
fn safe_package_id(value: &str) -> bool {
    value.len() <= MAX_ID
        && valid_type_id(value)
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && !matches!(
            value.split('.').next().unwrap_or_default(),
            "con"
                | "prn"
                | "aux"
                | "nul"
                | "com1"
                | "com2"
                | "com3"
                | "com4"
                | "com5"
                | "com6"
                | "com7"
                | "com8"
                | "com9"
                | "lpt1"
                | "lpt2"
                | "lpt3"
                | "lpt4"
                | "lpt5"
                | "lpt6"
                | "lpt7"
                | "lpt8"
                | "lpt9"
        )
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    fn valid() -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: "com.kosmos.demo".into(),
            name: "Demo".into(),
            version: "1.2.3".into(),
            kind: PackageKind::App,
            engine_api: ">=1.0.0".into(),
            entrypoint: "dist/index.html".into(),
            publisher: "kosmos".into(),
            permissions: vec![],
        }
    }
    #[test]
    fn v1_validation_and_unknown_fields() {
        let m = valid();
        assert!(m.validate().is_ok());
        assert!(serde_json::from_str::<PackageManifest>(r#"{"schema_version":1,"id":"x","name":"x","version":"1.0.0","kind":"app","engine_api":"*","entrypoint":"x","publisher":"kosmos","nope":1}"#).is_err());
    }
    #[test]
    fn v2_strict_contract_and_dependencies() {
        let s = r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.2.3","kind":"app","engine_api":"*","entrypoint":"index.html","icon":"icon.ico","publisher":"kosmos","targets":[{"runtime":"kosmos-host","os":["linux"]}],"data":{"access":[],"defines":[],"mappings":[]}}"#;
        let VersionedManifest::V2(m) = PackageManifest::parse(s).unwrap() else {
            panic!()
        };
        assert!(m.validate().is_ok());
        assert_eq!(m.icon.as_deref(), Some("icon.ico"));
        let d = s.replace("\"mappings\":[]", "\"mappings\":[],\"nope\":1");
        assert!(PackageManifest::parse(&d).is_err());
    }

    #[test]
    fn v2_rejects_unsafe_or_non_windows_icon_paths() {
        for icon in ["../icon.ico", "icon.svg", "icon.ico/extra"] {
            let input = format!(
                r#"{{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.2.3","kind":"app","engine_api":"*","entrypoint":"index.html","icon":"{icon}","publisher":"kosmos","targets":[{{"runtime":"kosmos-host","os":["linux"]}}],"data":{{"access":[],"defines":[],"mappings":[]}}}}"#
            );
            assert!(PackageManifest::parse(&input).is_err(), "{icon}");
        }
    }

    #[test]
    fn v2_rejects_duplicate_targets_and_overlong_requirements() {
        let base = r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.2.3","kind":"app","engine_api":"*","entrypoint":"index.html","publisher":"kosmos","targets":[{"runtime":"kosmos-host","os":["linux"]},{"runtime":"kosmos-host","os":["linux"]}],"data":{"access":[],"defines":[],"mappings":[]}}"#;
        assert!(PackageManifest::parse(base).is_err());
        let long = base.replace("\"access\":[]", &format!("\"access\":[{{\"type\":\"com.kosmos.note\",\"versions\":\"{}\",\"actions\":[\"read\"],\"fields\":{{\"read\":[],\"write\":[]}}}}]", "x".repeat(129)));
        assert!(PackageManifest::parse(&long).is_err());
    }

    #[test]
    fn v2_rejects_duplicate_mappings_and_long_namespace_suffix() {
        let s = r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.2.3","kind":"app","engine_api":"*","entrypoint":"index.html","publisher":"kosmos","targets":[{"runtime":"kosmos-host","os":["linux"]}],"data":{"access":[],"defines":[],"mappings":[{"type":"com.kosmos.note","versions":"*","direction":"import","fidelity":"native"},{"type":"com.kosmos.note","versions":"*","direction":"export","fidelity":"native"}]}}"#;
        assert!(PackageManifest::parse(s).is_err());
        let long = s.replace("\"mappings\":[", &format!("\"defines\":[{{\"type\":\"com.kosmos.demo.{}\",\"version\":\"1.0.0\",\"schema\":\"schema.json\"}}],\"mappings\":[", "x".repeat(65)));
        assert!(PackageManifest::parse(&long).is_err());
    }

    #[test]
    fn v1_trust_is_explicit_and_schema_versions_are_not_shared() {
        let input = serde_json::to_string(&valid()).unwrap();
        assert!(PackageManifest::parse_with_v1_trust(
            &input,
            &V1TrustContext::verified_first_party()
        )
        .is_ok());
        assert!(PackageManifest::parse_with_v1_trust(
            &input,
            &V1TrustContext {
                signature_verified: false,
                first_party_package: true,
            }
        )
        .is_err());
        let mut wrong = valid();
        wrong.schema_version = 2;
        assert!(wrong.validate().is_err());
    }
}
