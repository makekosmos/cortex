use crate::runtime_grants::RegisteredType;
use crate::{lock_file::write_owner_only_json, package_manifest::ManifestV2};
use ark_core::type_registry::TypeRegistration;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistrationError {
    #[error("namespace is not owned by package")]
    Namespace,
    #[error("definition conflict")]
    Conflict,
    #[error("definition already owned by system")]
    SystemOwned,
    #[error("persistence failed")]
    Persistence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredDefinition {
    pub type_id: String,
    pub version: String,
    pub owner_id: String,
    pub schema_hash: String,
    #[serde(default)]
    pub content_contract_hash: String,
    #[serde(default)]
    pub relations_hash: String,
    /// The grant compiler needs the immutable field/relation surface after a
    /// restart.  These are derived from the verified archive documents and
    /// persisted alongside their digests; package code never supplies them
    /// at request time.
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub relations: Vec<String>,
    /// Canonical documents retained by Engine after verification.  Keeping
    /// these bytes lets ARK rebuild its authoritative type registry after a
    /// restart without trusting a package or renderer again.
    #[serde(default)]
    pub schema_json: String,
    #[serde(default)]
    pub content_contract_json: String,
    #[serde(default)]
    pub relations_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RegistryState {
    definitions: BTreeMap<String, RegisteredDefinition>,
}

pub struct PackageRegistrationRegistry {
    root: PathBuf,
}

impl PackageRegistrationRegistry {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, RegistrationError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|_| RegistrationError::Persistence)?;
        Ok(Self { root })
    }

    pub fn register_manifest(
        &self,
        manifest: &ManifestV2,
        documents: &BTreeMap<String, Vec<u8>>,
    ) -> Result<(), RegistrationError> {
        let mut state = self.read()?;
        let additions = self.candidates(manifest, documents, &state)?;
        for (key, value) in additions {
            state.definitions.insert(key, value);
        }
        self.write(&state)
    }

    pub fn validate_manifest(
        &self,
        manifest: &ManifestV2,
        documents: &BTreeMap<String, Vec<u8>>,
    ) -> Result<(), RegistrationError> {
        let state = self.read()?;
        self.candidates(manifest, documents, &state).map(|_| ())
    }

    pub fn snapshot(&self) -> Result<Vec<u8>, RegistrationError> {
        serde_json::to_vec(&self.read()?).map_err(|_| RegistrationError::Persistence)
    }

    pub fn restore(&self, snapshot: &[u8]) -> Result<(), RegistrationError> {
        let state = serde_json::from_slice(snapshot).map_err(|_| RegistrationError::Persistence)?;
        self.write(&state)
    }

    fn candidates(
        &self,
        manifest: &ManifestV2,
        documents: &BTreeMap<String, Vec<u8>>,
        state: &RegistryState,
    ) -> Result<Vec<(String, RegisteredDefinition)>, RegistrationError> {
        let mut additions = Vec::new();
        for definition in &manifest.data.defines {
            let prefix = format!("{}.", manifest.id);
            if !definition.type_id.starts_with(&prefix)
                || definition.type_id[prefix.len()..]
                    .split('.')
                    .any(|part| part.is_empty())
            {
                return Err(RegistrationError::Namespace);
            }
            let canonical = ark_core::canonical_types::definitions::canonical_type_registrations()
                .map_err(|_| RegistrationError::SystemOwned)?;
            if canonical.iter().any(|registration| {
                registration.type_id == definition.type_id
                    || registration
                        .aliases
                        .iter()
                        .any(|alias| alias.alias == definition.type_id)
            }) {
                return Err(RegistrationError::SystemOwned);
            }
            let schema = documents
                .get(&definition.schema)
                .ok_or(RegistrationError::Conflict)?;
            let content = definition
                .content_contract
                .as_ref()
                .map(|path| documents.get(path).ok_or(RegistrationError::Conflict))
                .transpose()?
                .map(|v| v.as_slice());
            let relations = definition
                .relations
                .as_ref()
                .map(|path| documents.get(path).ok_or(RegistrationError::Conflict))
                .transpose()?
                .map(|v| v.as_slice());
            validate_json(schema)?;
            if let Some(content) = content {
                validate_json(content)?;
            }
            if let Some(relations) = relations {
                validate_json(relations)?;
            }
            let fields = schema_fields(schema)?;
            let relation_names = relation_names(relations.unwrap_or(b"[]"))?;
            let content_bytes = content.unwrap_or(b"{}");
            let relations_bytes = relations.unwrap_or(b"[]");
            let key = format!("{}@{}", definition.type_id, definition.version);
            let candidate = RegisteredDefinition {
                type_id: definition.type_id.clone(),
                version: definition.version.clone(),
                owner_id: manifest.id.clone(),
                schema_hash: hex_hash_tuple(&[schema, content_bytes, relations_bytes]),
                content_contract_hash: hex_hash(content_bytes),
                relations_hash: hex_hash(relations_bytes),
                fields,
                relations: relation_names,
                schema_json: String::from_utf8_lossy(schema).into_owned(),
                content_contract_json: String::from_utf8_lossy(content_bytes).into_owned(),
                relations_json: String::from_utf8_lossy(relations_bytes).into_owned(),
            };
            if let Some(existing) = state.definitions.get(&key) {
                if existing.owner_id != candidate.owner_id {
                    return Err(RegistrationError::Conflict);
                }
                if existing.schema_json.is_empty()
                    || existing.content_contract_json.is_empty()
                    || existing.relations_json.is_empty()
                {
                    additions.push((key, candidate));
                    continue;
                }
                // A type/version is an immutable contract. Legacy rows that
                // lack the complete digest are deliberately not upgraded in
                // place; they must be migrated to a new additive version.
                if existing != &candidate {
                    return Err(RegistrationError::Conflict);
                }
            } else {
                additions.push((key, candidate));
            }
        }
        Ok(additions)
    }

    pub fn uninstall(&self, _package_id: &str) -> Result<(), RegistrationError> {
        Ok(())
    }

    pub fn definitions(&self) -> Result<Vec<RegisteredDefinition>, RegistrationError> {
        Ok(self.read()?.definitions.into_values().collect())
    }

    pub fn registered_types(&self) -> Result<Vec<RegisteredType>, RegistrationError> {
        Ok(self
            .read()?
            .definitions
            .into_values()
            .map(|definition| RegisteredType {
                type_id: definition.type_id,
                version: definition.version,
                fields: definition.fields,
                relations: definition.relations,
            })
            .collect())
    }

    /// Convert only previously verified, persisted definitions into the
    /// trusted ARK registration DTO.  The renderer/package never supplies
    /// this value at request time.
    pub fn type_registrations(&self) -> Result<Vec<TypeRegistration>, RegistrationError> {
        Ok(self
            .read()?
            .definitions
            .into_values()
            .filter(|definition| {
                !definition.schema_json.is_empty()
                    && !definition.content_contract_json.is_empty()
                    && !definition.relations_json.is_empty()
            })
            .map(|definition| TypeRegistration {
                name: definition.type_id.clone(),
                type_id: definition.type_id,
                schema_json: definition.schema_json,
                ui_schema_json: "{}".into(),
                content_contract_json: if definition.content_contract_json.is_empty() {
                    "{}".into()
                } else {
                    definition.content_contract_json
                },
                relations_json: if definition.relations_json.is_empty() {
                    "[]".into()
                } else {
                    definition.relations_json
                },
                sync_policy_json: "{}".into(),
                version: definition.version,
                // ARK computes and verifies the canonical hash itself.  The
                // package registry keeps a separate archive digest.
                schema_hash: String::new(),
                owner_kind: "package".into(),
                owner_id: Some(definition.owner_id),
                status: "active".into(),
                base_type_id: None,
                aliases: Vec::new(),
                created_at: "package-registration".into(),
            })
            .collect())
    }

    fn path(&self) -> PathBuf {
        self.root.join("definitions.json")
    }
    fn read(&self) -> Result<RegistryState, RegistrationError> {
        if !self.path().exists() {
            return Ok(RegistryState::default());
        }
        serde_json::from_slice(&fs::read(self.path()).map_err(|_| RegistrationError::Persistence)?)
            .map_err(|_| RegistrationError::Persistence)
    }
    fn write(&self, state: &RegistryState) -> Result<(), RegistrationError> {
        write_owner_only_json(&self.path(), state).map_err(|_| RegistrationError::Persistence)
    }
}

fn hex_hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn validate_json(bytes: &[u8]) -> Result<(), RegistrationError> {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .map(|_| ())
        .map_err(|_| RegistrationError::Conflict)
}

fn schema_fields(bytes: &[u8]) -> Result<Vec<String>, RegistrationError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| RegistrationError::Conflict)?;
    let properties = value
        .get("properties")
        .and_then(serde_json::Value::as_object);
    let mut fields = ["title", "content"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if let Some(properties) = properties {
        fields.extend(properties.keys().map(|name| format!("props.{name}")));
    }
    Ok(fields)
}

fn relation_names(bytes: &[u8]) -> Result<Vec<String>, RegistrationError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| RegistrationError::Conflict)?;
    let values = value
        .as_array()
        .cloned()
        .or_else(|| {
            value
                .get("relations")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .ok_or(RegistrationError::Conflict)?;
    values
        .into_iter()
        .map(|relation| {
            relation
                .as_str()
                .or_else(|| relation.get("type").and_then(serde_json::Value::as_str))
                .map(str::to_owned)
                .ok_or(RegistrationError::Conflict)
        })
        .collect()
}

fn hex_hash_tuple(parts: &[&[u8]]) -> String {
    let mut h = Sha256::new();
    for part in parts {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::package_manifest::PackageManifest;
    use tempfile::tempdir;

    fn manifest() -> ManifestV2 {
        let raw = r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.0.0","kind":"app","engine_api":"*","entrypoint":"index.html","publisher":"kosmos","targets":[{"runtime":"kosmos-host","os":["linux"]}],"data":{"access":[],"defines":[{"type":"com.kosmos.demo.note","version":"1.0.0","schema":"schema.json","content_contract":"content.json","relations":"relations.json"}],"mappings":[]}}"#;
        let crate::package_manifest::VersionedManifest::V2(m) =
            PackageManifest::parse(raw).unwrap()
        else {
            panic!()
        };
        m
    }

    #[test]
    fn same_version_contract_changes_conflict_without_mutating_persisted_definition() {
        let dir = tempdir().unwrap();
        let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
        let mut docs = BTreeMap::new();
        docs.insert("schema.json".into(), br#"{}"#.to_vec());
        docs.insert("content.json".into(), br#"{"body":"text"}"#.to_vec());
        docs.insert("relations.json".into(), br#"["project"]"#.to_vec());
        registry.register_manifest(&manifest(), &docs).unwrap();
        let before = registry.definitions().unwrap()[0].schema_hash.clone();
        let mut changed_contract = docs.clone();
        changed_contract.insert("content.json".into(), br#"{"body":"rich_text"}"#.to_vec());
        assert!(matches!(
            registry.register_manifest(&manifest(), &changed_contract),
            Err(RegistrationError::Conflict)
        ));
        assert_eq!(registry.definitions().unwrap()[0].schema_hash, before);
        registry.register_manifest(&manifest(), &docs).unwrap();
        registry.uninstall("com.kosmos.demo").unwrap();
        assert_eq!(registry.definitions().unwrap().len(), 1);
    }

    #[test]
    fn conflicting_reinstall_is_atomic_and_foreign_namespace_rejected() {
        let dir = tempdir().unwrap();
        let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
        let mut docs = BTreeMap::new();
        docs.insert("schema.json".into(), br#"{}"#.to_vec());
        docs.insert("content.json".into(), br#"{"body":"text"}"#.to_vec());
        docs.insert("relations.json".into(), br#"["project"]"#.to_vec());
        registry.register_manifest(&manifest(), &docs).unwrap();
        let mut changed = docs.clone();
        changed.insert("schema.json".into(), br#"{"x":1}"#.to_vec());
        assert!(matches!(
            registry.register_manifest(&manifest(), &changed),
            Err(RegistrationError::Conflict)
        ));
        let foreign = RegisteredDefinition {
            type_id: "com.kosmos.demo.note".into(),
            version: "1.0.0".into(),
            owner_id: "com.kosmos.foreign".into(),
            schema_hash: "foreign".into(),
            content_contract_hash: "foreign".into(),
            relations_hash: "foreign".into(),
            fields: vec![],
            relations: vec![],
            schema_json: "{}".into(),
            content_contract_json: "{}".into(),
            relations_json: "[]".into(),
        };
        fs::write(
            dir.path().join("definitions.json"),
            serde_json::to_vec(&RegistryState {
                definitions: [("com.kosmos.demo.note@1.0.0".into(), foreign)]
                    .into_iter()
                    .collect(),
            })
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            registry.register_manifest(&manifest(), &docs),
            Err(RegistrationError::Conflict)
        ));
        assert_eq!(registry.definitions().unwrap().len(), 1);
    }

    #[test]
    fn content_and_relations_changes_change_digest_independently() {
        let dir = tempdir().unwrap();
        let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
        let mut docs = BTreeMap::from([
            ("schema.json".into(), br#"{}"#.to_vec()),
            ("content.json".into(), br#"{"kind":"text"}"#.to_vec()),
            ("relations.json".into(), br#"["project"]"#.to_vec()),
        ]);
        registry.register_manifest(&manifest(), &docs).unwrap();
        let first = registry.definitions().unwrap().pop().unwrap();
        docs.insert("relations.json".into(), br#"["area"]"#.to_vec());
        assert!(matches!(
            registry.register_manifest(&manifest(), &docs),
            Err(RegistrationError::Conflict)
        ));
        let second = registry.definitions().unwrap().pop().unwrap();
        assert_eq!(first, second);
        docs.insert("content.json".into(), br#"{"kind":"rich_text"}"#.to_vec());
        assert!(matches!(
            registry.register_manifest(&manifest(), &docs),
            Err(RegistrationError::Conflict)
        ));
        assert_eq!(second, registry.definitions().unwrap().pop().unwrap());
    }

    #[test]
    fn missing_or_malformed_referenced_contract_fails_without_persisting() {
        let dir = tempdir().unwrap();
        let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
        let docs = BTreeMap::from([(String::from("schema.json"), br#"{}"#.to_vec())]);
        assert!(matches!(
            registry.register_manifest(&manifest(), &docs),
            Err(RegistrationError::Conflict)
        ));
        assert!(registry.definitions().unwrap().is_empty());
        let mut malformed = docs;
        malformed.insert("content.json".into(), b"{".to_vec());
        malformed.insert("relations.json".into(), br#"[]"#.to_vec());
        assert!(matches!(
            registry.register_manifest(&manifest(), &malformed),
            Err(RegistrationError::Conflict)
        ));
        assert!(registry.definitions().unwrap().is_empty());
    }

    #[test]
    fn absent_optional_contracts_remain_valid_and_legacy_records_deserialize() {
        let dir = tempdir().unwrap();
        let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
        let raw = r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.0.0","kind":"app","engine_api":"*","entrypoint":"index.html","publisher":"kosmos","targets":[{"runtime":"kosmos-host","os":["linux"]}],"data":{"access":[],"defines":[{"type":"com.kosmos.demo.note","version":"1.0.0","schema":"schema.json"}],"mappings":[]}}"#;
        let crate::package_manifest::VersionedManifest::V2(manifest) =
            PackageManifest::parse(raw).unwrap()
        else {
            panic!()
        };
        registry
            .register_manifest(
                &manifest,
                &BTreeMap::from([(String::from("schema.json"), br#"{}"#.to_vec())]),
            )
            .unwrap();
        assert_eq!(registry.definitions().unwrap().len(), 1);
        let legacy = br#"{"definitions":{"com.kosmos.demo.note@1.0.0":{"type_id":"com.kosmos.demo.note","version":"1.0.0","owner_id":"com.kosmos.demo","schema_hash":"old"}}}"#;
        let parsed: RegistryState = serde_json::from_slice(legacy).unwrap();
        assert_eq!(
            parsed.definitions["com.kosmos.demo.note@1.0.0"].content_contract_hash,
            ""
        );
    }

    #[test]
    fn legacy_definition_is_replaced_only_by_verified_archive_documents() {
        let dir = tempdir().unwrap();
        let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
        let legacy = br#"{"definitions":{"com.kosmos.demo.note@1.0.0":{"type_id":"com.kosmos.demo.note","version":"1.0.0","owner_id":"com.kosmos.demo","schema_hash":"old"}}}"#;
        fs::write(dir.path().join("definitions.json"), legacy).unwrap();
        let mut docs = BTreeMap::new();
        docs.insert("schema.json".into(), br#"{"type":"object"}"#.to_vec());
        docs.insert("content.json".into(), br#"{}"#.to_vec());
        docs.insert("relations.json".into(), br#"[]"#.to_vec());
        registry.register_manifest(&manifest(), &docs).unwrap();
        assert_eq!(
            registry.type_registrations().unwrap()[0].schema_json,
            r#"{"type":"object"}"#
        );
    }

    #[test]
    fn incomplete_legacy_definition_is_not_replayed_without_archive() {
        let dir = tempdir().unwrap();
        let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
        let legacy = br#"{"definitions":{"com.kosmos.demo.note@1.0.0":{"type_id":"com.kosmos.demo.note","version":"1.0.0","owner_id":"com.kosmos.demo","schema_hash":"old"}}}"#;
        fs::write(dir.path().join("definitions.json"), legacy).unwrap();
        assert!(registry.type_registrations().unwrap().is_empty());
    }
}
