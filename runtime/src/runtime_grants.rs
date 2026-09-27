//! Engine-owned Phase 5 launch authority and closed typed request boundary.
use crate::package_manifest::{DataAction, ManifestV2};
use rand::RngCore;
use semver::{Version, VersionReq};
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

/// Dictation is an Engine-owned privileged service. Package manifests can only
/// request these named operations; microphone capture itself never crosses the
/// package boundary.
pub const DICTATION_READ_OPERATIONS: &[&str] = &[
    "dictation.get_state",
    "dictation.get_config",
    "dictation.list_local_models",
    "dictation.capture.start",
    "dictation.capture.stop",
    "dictation.speech.transcribe",
    "dictation.input.insert_text",
    "dictation.window.foreground",
];
pub const DICTATION_WRITE_OPERATIONS: &[&str] = &[
    "dictation.update_config",
    "dictation.start_recording",
    "dictation.cancel",
    "dictation.lifecycle.set_autostart",
];
const DICTATION_CONTROL_OPERATIONS: &[&str] = &[
    "dictation.capture.start",
    "dictation.capture.stop",
    "dictation.speech.transcribe",
    "dictation.input.insert_text",
    "dictation.window.foreground",
    "dictation.lifecycle.set_autostart",
];

pub fn dictation_operation_capability(operation: &str) -> Option<&'static str> {
    if DICTATION_CONTROL_OPERATIONS.contains(&operation) {
        Some("dictation.control")
    } else if DICTATION_READ_OPERATIONS.contains(&operation) {
        Some("ark.read")
    } else if DICTATION_WRITE_OPERATIONS.contains(&operation) {
        Some("ark.write")
    } else {
        None
    }
}

/// Focus packages can manage only their own persisted block-lists and timer.
/// Native blocking remains a Host concern and is never an ARK permission.
pub const FOCUS_READ_OPERATIONS: &[&str] = &[
    "focus.list_blocklists",
    "focus.get_active_state",
    "focus.resolve_blocklist_domains",
    "pomodoro.get_state",
];
pub const FOCUS_WRITE_OPERATIONS: &[&str] = &[
    "focus.upsert_blocklist",
    "focus.delete_blocklist",
    "focus.set_active_state",
    "pomodoro.start",
    "pomodoro.pause",
    "pomodoro.resume",
    "pomodoro.skip",
    "pomodoro.stop",
];

pub fn focus_operation_capability(operation: &str) -> Option<&'static str> {
    if FOCUS_READ_OPERATIONS.contains(&operation) {
        Some("ark.read")
    } else if FOCUS_WRITE_OPERATIONS.contains(&operation) {
        Some("ark.write")
    } else {
        None
    }
}

/// Daedalus may use only these exact Engine-owned agent operations. The
/// capability split is intentionally explicit; a package cannot turn a read
/// grant into an agent mutation or use an unregistered `agents.*` operation.
pub const AGENTS_READ_OPERATIONS: &[&str] = &[
    "agents.projects.list",
    "agents.sessions.list",
    "agents.sessions.get",
    "agents.sessions.timeline",
    "agents.diff.get",
    "agents.models.list",
    "agents.editors.list",
    "agents.snapshot",
];
pub const AGENTS_WRITE_OPERATIONS: &[&str] = &[
    "agents.projects.add",
    "agents.projects.remove",
    "agents.sessions.create",
    "agents.sessions.send",
    "agents.sessions.interrupt",
    "agents.sessions.archive",
    "agents.sessions.remove_worktree",
    "agents.approvals.respond",
    "agents.editors.open",
];

pub fn agents_operation_capability(operation: &str) -> Option<&'static str> {
    if AGENTS_READ_OPERATIONS.contains(&operation) {
        Some("ark.read")
    } else if AGENTS_WRITE_OPERATIONS.contains(&operation) {
        Some("ark.write")
    } else {
        None
    }
}

/// App-facing Engine network ops (KOS-152): `network` manifest scopes are
/// named groups — `bookMetadata` and `images` — and each name owns a fixed
/// set of operations. Origin-form network scopes (`https://…`) are worker
/// grants, not app ops, and never land here.
pub const APP_NETWORK_SCOPES: &[&str] = &["bookMetadata", "images"];
const APP_NETWORK_OPERATIONS: &[(&str, &str)] = &[
    ("bookMetadata.lookupIsbn", "bookMetadata"),
    ("bookMetadata.fetchPage", "bookMetadata"),
    ("images.fetch", "images"),
    ("images.dominantColor", "images"),
    ("images.storeCover", "images"),
];

/// Maps a registered app network operation to the `network` scope that owns
/// it; unknown operation names return `None` so `parse_app_rpc` rejects them.
pub fn app_network_operation_scope(operation: &str) -> Option<&'static str> {
    APP_NETWORK_OPERATIONS
        .iter()
        .find(|(name, _)| *name == operation)
        .map(|(_, scope)| *scope)
}

/// Deny registered app network ops whose `network` scope is not granted.
pub fn require_app_network_scope(operation: &str, grant: &LaunchGrant) -> Result<(), &'static str> {
    let scope = app_network_operation_scope(operation).ok_or("unsupported app operation")?;
    if !grant.allows_app_network_scope(scope) {
        return Err("network grant denied");
    }
    Ok(())
}

/// Vault filesystem operations (KOS-155). `filesystem.read` scopes open,
/// scan, read, and close a user-selected directory grant; `filesystem.write`
/// additionally allows registering an export target and writing beneath it.
/// Each operation is Engine-owned — an app never sees raw filesystem access.
pub const FILESYSTEM_READ_OPERATIONS: &[&str] = &[
    "filesystem.vault.open",
    "filesystem.vault.read",
    "filesystem.vault.close",
];
pub const FILESYSTEM_WRITE_OPERATIONS: &[&str] =
    &["filesystem.vault.register", "filesystem.vault.export"];

pub fn filesystem_operation_capability(operation: &str) -> Option<&'static str> {
    if FILESYSTEM_READ_OPERATIONS.contains(&operation) {
        Some("filesystem.read")
    } else if FILESYSTEM_WRITE_OPERATIONS.contains(&operation) {
        Some("filesystem.write")
    } else {
        None
    }
}

const MAX_RULES: usize = 64;
const MAX_CAPABILITIES: usize = 64;
const MAX_BATCH: usize = 100;
const MAX_FIELDS: usize = 256;
const MAX_RELATIONS: usize = 128;

fn id(s: &str, max: usize) -> Result<String, GrantError> {
    if s.is_empty() || s.len() > max || !s.is_char_boundary(s.len()) {
        return Err(GrantError::Contract("identifier"));
    }
    Ok(s.to_owned())
}
fn unique(xs: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    xs.iter().all(|x| seen.insert(x))
}
fn bounded_list(xs: &[String], max: usize) -> Result<(), GrantError> {
    if xs.len() > max || !unique(xs) {
        return Err(GrantError::Contract("bounded unique list"));
    }
    xs.iter().try_for_each(|x| id(x, 128).map(|_| ()))
}

fn deserialize_unique_strings<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<String>::deserialize(deserializer)?;
    if unique(&values) {
        Ok(values)
    } else {
        Err(D::Error::custom("duplicate string"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DataRequest {
    ReadObject {
        type_id: String,
        type_version: String,
        object_id: String,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        fields: Vec<String>,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        relations: Vec<String>,
    },
    ListObjects {
        type_id: String,
        type_version: String,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        fields: Vec<String>,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        relations: Vec<String>,
        cursor: Option<String>,
        limit: u16,
    },
    CreateObject {
        type_id: String,
        type_version: String,
        object_id: Option<String>,
        fields: Vec<FieldInput>,
        links: Vec<LinkInput>,
    },
    UpdateObject {
        type_id: String,
        type_version: String,
        object_id: String,
        expected_hlc: Option<String>,
        fields: Vec<FieldInput>,
        links: Vec<LinkInput>,
    },
    DeleteObject {
        type_id: String,
        type_version: String,
        object_id: String,
        expected_hlc: Option<String>,
    },
    Subscribe {
        type_id: String,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        versions: Vec<String>,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        fields: Vec<String>,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        relations: Vec<String>,
        cursor: Option<String>,
    },
    Link {
        source_type_id: String,
        source_version: String,
        source_object_id: String,
        relation: String,
        target_type_id: String,
        target_version: String,
        target_object_id: String,
        expected_hlc: Option<String>,
    },
    Batch {
        requests: Vec<BatchRequest>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FieldInput {
    pub field_id: String,
    pub value: FieldValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LinkInput {
    pub relation_id: String,
    pub target_type_id: String,
    pub target_version: String,
    pub target_object_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FieldValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<FieldValue>),
    Object(BTreeMap<String, FieldValue>),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BatchRequest {
    ReadObject {
        type_id: String,
        type_version: String,
        object_id: String,
        fields: Vec<String>,
        relations: Vec<String>,
    },
    ListObjects {
        type_id: String,
        type_version: String,
        fields: Vec<String>,
        relations: Vec<String>,
        cursor: Option<String>,
        limit: u16,
    },
    CreateObject {
        type_id: String,
        type_version: String,
        object_id: Option<String>,
        fields: Vec<FieldInput>,
        links: Vec<LinkInput>,
    },
    UpdateObject {
        type_id: String,
        type_version: String,
        object_id: String,
        expected_hlc: Option<String>,
        fields: Vec<FieldInput>,
        links: Vec<LinkInput>,
    },
    DeleteObject {
        type_id: String,
        type_version: String,
        object_id: String,
        expected_hlc: Option<String>,
    },
    Subscribe {
        type_id: String,
        versions: Vec<String>,
        fields: Vec<String>,
        relations: Vec<String>,
        cursor: Option<String>,
    },
    Link {
        source_type_id: String,
        source_version: String,
        source_object_id: String,
        relation: String,
        target_type_id: String,
        target_version: String,
        target_object_id: String,
        expected_hlc: Option<String>,
    },
}
impl DataRequest {
    pub fn validate(&self) -> Result<(), GrantError> {
        match self {
            Self::Batch { requests } => {
                if requests.is_empty() || requests.len() > MAX_BATCH {
                    return Err(GrantError::Contract("batch"));
                }
                requests.iter().try_for_each(BatchRequest::validate)
            }
            Self::ReadObject {
                type_id,
                type_version,
                object_id,
                fields,
                relations,
            } => {
                id(type_id, 128)?;
                id(type_version, 64)?;
                id(object_id, 128)?;
                bounded_list(fields, MAX_FIELDS)?;
                bounded_list(relations, MAX_RELATIONS)
            }
            Self::ListObjects {
                type_id,
                type_version,
                fields,
                relations,
                cursor,
                limit,
            } => {
                id(type_id, 128)?;
                id(type_version, 64)?;
                if *limit == 0 || *limit > 500 {
                    return Err(GrantError::Contract("limit"));
                };
                if cursor.as_ref().is_some_and(|x| x.len() > 4096) {
                    return Err(GrantError::Contract("cursor"));
                };
                bounded_list(fields, MAX_FIELDS)?;
                bounded_list(relations, MAX_RELATIONS)
            }
            Self::CreateObject {
                type_id,
                type_version,
                object_id,
                fields,
                links,
            } => {
                id(type_id, 128)?;
                id(type_version, 64)?;
                if let Some(x) = object_id {
                    id(x, 128)?;
                }
                validate_fields(fields)?;
                validate_links(links)
            }
            Self::UpdateObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
                fields,
                links,
            } => {
                id(type_id, 128)?;
                id(type_version, 64)?;
                id(object_id, 128)?;
                if expected_hlc.as_ref().is_some_and(|x| x.len() > 128) {
                    return Err(GrantError::Contract("hlc"));
                };
                validate_fields(fields)?;
                validate_links(links)
            }
            Self::DeleteObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
            } => {
                id(type_id, 128)?;
                id(type_version, 64)?;
                id(object_id, 128)?;
                if expected_hlc.as_ref().is_some_and(|x| x.len() > 128) {
                    return Err(GrantError::Contract("hlc"));
                };
                Ok(())
            }
            Self::Subscribe {
                type_id,
                versions,
                fields,
                relations,
                cursor,
            } => {
                id(type_id, 128)?;
                if versions.is_empty() {
                    return Err(GrantError::Contract("versions"));
                };
                versions.iter().try_for_each(|x| {
                    Version::parse(x)
                        .map(|_| ())
                        .map_err(|_| GrantError::Contract("semver"))
                })?;
                if cursor.as_ref().is_some_and(|x| x.len() > 4096) {
                    return Err(GrantError::Contract("cursor"));
                };
                bounded_list(fields, MAX_FIELDS)?;
                bounded_list(relations, MAX_RELATIONS)
            }
            Self::Link {
                source_type_id,
                source_version,
                source_object_id,
                relation,
                target_type_id,
                target_version,
                target_object_id,
                ..
            } => {
                for x in [source_type_id, target_type_id] {
                    id(x, 128)?;
                }
                for x in [source_version, target_version] {
                    Version::parse(x).map_err(|_| GrantError::Contract("semver"))?;
                }
                for x in [source_object_id, target_object_id, relation] {
                    id(x, 128)?;
                }
                Ok(())
            }
        }
    }
}
fn validate_fields(fields: &[FieldInput]) -> Result<(), GrantError> {
    if fields.len() > MAX_FIELDS
        || !fields
            .iter()
            .map(|x| x.field_id.clone())
            .collect::<BTreeSet<_>>()
            .len()
            .eq(&fields.len())
    {
        return Err(GrantError::Contract("fields"));
    };
    for f in fields {
        id(&f.field_id, 128)?;
        validate_value(&f.value, 0, 0)?;
    }
    Ok(())
}
fn validate_links(links: &[LinkInput]) -> Result<(), GrantError> {
    if links.len() > MAX_RELATIONS {
        return Err(GrantError::Contract("links"));
    };
    let mut s = BTreeSet::new();
    for l in links {
        id(&l.relation_id, 128)?;
        if !s.insert(l.relation_id.clone()) {
            return Err(GrantError::Contract("duplicate link"));
        };
        id(&l.target_type_id, 128)?;
        Version::parse(&l.target_version).map_err(|_| GrantError::Contract("semver"))?;
        id(&l.target_object_id, 128)?;
    }
    Ok(())
}
fn validate_value(v: &FieldValue, depth: usize, nodes: usize) -> Result<(), GrantError> {
    if depth > 32 || nodes > 4096 {
        return Err(GrantError::Contract("value bounds"));
    };
    match v {
        FieldValue::Number(n) if !n.is_f64() && n.as_i64().is_none() && n.as_u64().is_none() => {
            return Err(GrantError::Contract("number"))
        }
        FieldValue::String(s) if s.len() > 65536 => return Err(GrantError::Contract("string")),
        FieldValue::Array(a) => {
            for x in a {
                validate_value(x, depth + 1, nodes + 1)?
            }
        }
        FieldValue::Object(o) => {
            for (k, x) in o {
                id(k, 65536)?;
                validate_value(x, depth + 1, nodes + 1)?
            }
        }
        _ => {}
    }
    Ok(())
}
impl BatchRequest {
    fn validate(&self) -> Result<(), GrantError> {
        DataRequest::from(self.clone()).validate()
    }
}
impl From<BatchRequest> for DataRequest {
    fn from(x: BatchRequest) -> Self {
        match x {
            BatchRequest::ReadObject {
                type_id,
                type_version,
                object_id,
                fields,
                relations,
            } => Self::ReadObject {
                type_id,
                type_version,
                object_id,
                fields,
                relations,
            },
            BatchRequest::ListObjects {
                type_id,
                type_version,
                fields,
                relations,
                cursor,
                limit,
            } => Self::ListObjects {
                type_id,
                type_version,
                fields,
                relations,
                cursor,
                limit,
            },
            BatchRequest::CreateObject {
                type_id,
                type_version,
                object_id,
                fields,
                links,
            } => Self::CreateObject {
                type_id,
                type_version,
                object_id,
                fields,
                links,
            },
            BatchRequest::UpdateObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
                fields,
                links,
            } => Self::UpdateObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
                fields,
                links,
            },
            BatchRequest::DeleteObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
            } => Self::DeleteObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
            },
            BatchRequest::Subscribe {
                type_id,
                versions,
                fields,
                relations,
                cursor,
            } => Self::Subscribe {
                type_id,
                versions,
                fields,
                relations,
                cursor,
            },
            BatchRequest::Link {
                source_type_id,
                source_version,
                source_object_id,
                relation,
                target_type_id,
                target_version,
                target_object_id,
                expected_hlc,
            } => Self::Link {
                source_type_id,
                source_version,
                source_object_id,
                relation,
                target_type_id,
                target_version,
                target_object_id,
                expected_hlc,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredType {
    pub type_id: String,
    pub version: String,
    pub fields: Vec<String>,
    pub relations: Vec<String>,
}
impl RegisteredType {
    pub fn new(id: &str, v: &str, f: &[&str], r: &[&str]) -> Self {
        Self {
            type_id: id.into(),
            version: v.into(),
            fields: f.iter().map(|x| (*x).into()).collect(),
            relations: r.iter().map(|x| (*x).into()).collect(),
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct RegistrySnapshot {
    pub types: Vec<RegisteredType>,
}
impl RegistrySnapshot {
    pub fn new(types: Vec<RegisteredType>) -> Self {
        Self { types }
    }
    fn find(&self, id: &str, v: &str) -> Option<&RegisteredType> {
        self.types
            .iter()
            .find(|t| t.type_id == id && t.version == v)
    }
    fn has(&self, id: &str, v: &str) -> bool {
        self.find(id, v).is_some()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrantRule {
    pub type_id: String,
    pub versions: Vec<String>,
    pub actions: BTreeSet<String>,
    pub fields_read: Vec<String>,
    pub fields_write: Vec<String>,
    pub relations_read: Vec<String>,
    pub relations_write: Vec<String>,
}
impl GrantRule {
    pub fn read(id: &str, v: &str, f: &[&str], r: &[&str]) -> Self {
        Self {
            type_id: id.into(),
            versions: vec![v.into()],
            actions: ["read".into()].into_iter().collect(),
            fields_read: f.iter().map(|x| (*x).into()).collect(),
            fields_write: vec![],
            relations_read: r.iter().map(|x| (*x).into()).collect(),
            relations_write: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompileInput {
    pub package_id: String,
    pub package_version: String,
    pub manifest_digest: String,
    pub rules: Vec<GrantRule>,
}
impl CompileInput {
    pub fn new(p: &str, v: &str, d: &str, rules: Vec<GrantRule>) -> Self {
        Self {
            package_id: p.into(),
            package_version: v.into(),
            manifest_digest: d.into(),
            rules,
        }
    }
}

/// Compiles the typed capability directly from the validated v2 manifest's
/// canonical data access section. The registry remains authoritative for the
/// definition/version/field/relation surface; manifest payloads never supply
/// a grant at request time.
pub fn compile_manifest_v2(
    manifest: &ManifestV2,
    registry: &RegistrySnapshot,
    manifest_digest: &str,
) -> Result<LaunchGrant, GrantError> {
    let rules = manifest
        .data
        .access
        .iter()
        .map(|access| {
            let matching = registry
                .types
                .iter()
                .filter(|registered| {
                    registered.type_id == access.type_id
                        && Version::parse(&registered.version)
                            .ok()
                            .zip(VersionReq::parse(&access.versions).ok())
                            .is_some_and(|(version, requirement)| requirement.matches(&version))
                })
                .collect::<Vec<_>>();
            let fields_read = expand_scope(&access.fields.read, &matching, |registered| {
                &registered.fields
            })?;
            let fields_write = expand_scope(&access.fields.write, &matching, |registered| {
                &registered.fields
            })?;
            let relations_read = access
                .relations
                .as_ref()
                .map(|relations| {
                    expand_scope(&relations.read, &matching, |registered| {
                        &registered.relations
                    })
                })
                .transpose()?
                .unwrap_or_default();
            let relations_write = access
                .relations
                .as_ref()
                .map(|relations| {
                    expand_scope(&relations.write, &matching, |registered| {
                        &registered.relations
                    })
                })
                .transpose()?
                .unwrap_or_default();
            Ok(GrantRule {
                type_id: access.type_id.clone(),
                versions: vec![access.versions.clone()],
                actions: access
                    .actions
                    .iter()
                    .map(|action| match action {
                        DataAction::Read => "read",
                        DataAction::Create => "create",
                        DataAction::Update => "update",
                        DataAction::Delete => "delete",
                        DataAction::Subscribe => "subscribe",
                        DataAction::Link => "link",
                    })
                    .map(str::to_owned)
                    .collect(),
                fields_read,
                fields_write,
                relations_read,
                relations_write,
            })
        })
        .collect::<Result<Vec<_>, GrantError>>()?;
    let mut grant = GrantCompiler::compile(
        CompileInput::new(&manifest.id, &manifest.version, manifest_digest, rules),
        registry,
    )?;
    let dictation_operations: Vec<String> = manifest
        .permissions
        .iter()
        .flat_map(|permission| {
            permission.scopes.iter().filter_map(move |scope| {
                if dictation_operation_capability(scope.as_str())
                    == Some(permission.capability.as_str())
                {
                    Some(scope.clone())
                } else {
                    None
                }
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !dictation_operations.is_empty() {
        grant.capabilities.push(ScopedCapability::Dictation {
            operations: dictation_operations,
        });
    }
    let focus_operations: Vec<String> = manifest
        .permissions
        .iter()
        .flat_map(|permission| {
            permission.scopes.iter().filter_map(move |scope| {
                if focus_operation_capability(scope.as_str())
                    == Some(permission.capability.as_str())
                {
                    Some(scope.clone())
                } else {
                    None
                }
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !focus_operations.is_empty() {
        grant.capabilities.push(ScopedCapability::Focus {
            operations: focus_operations,
        });
    }
    let agents_operations: Vec<String> = manifest
        .permissions
        .iter()
        .flat_map(|permission| {
            permission.scopes.iter().filter_map(move |scope| {
                if agents_operation_capability(scope.as_str())
                    == Some(permission.capability.as_str())
                {
                    Some(scope.clone())
                } else {
                    None
                }
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !agents_operations.is_empty() {
        grant.capabilities.push(ScopedCapability::Agents {
            operations: agents_operations,
        });
    }
    let worker_operations: Vec<String> = manifest
        .permissions
        .iter()
        .filter(|permission| permission.capability == "worker.invoke")
        .flat_map(|permission| permission.scopes.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !worker_operations.is_empty() {
        grant.capabilities.push(ScopedCapability::WorkerInvoke {
            operations: worker_operations,
        });
    }
    let app_network_scopes: Vec<String> = manifest
        .permissions
        .iter()
        .filter(|permission| permission.capability == "network")
        .flat_map(|permission| permission.scopes.iter().cloned())
        .filter(|scope| APP_NETWORK_SCOPES.contains(&scope.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !app_network_scopes.is_empty() {
        grant.capabilities.push(ScopedCapability::AppNetwork {
            scopes: app_network_scopes,
        });
    }
    let filesystem_operations: Vec<String> = manifest
        .permissions
        .iter()
        .flat_map(|permission| {
            permission.scopes.iter().filter_map(move |scope| {
                if filesystem_operation_capability(scope.as_str())
                    == Some(permission.capability.as_str())
                {
                    Some(scope.clone())
                } else {
                    None
                }
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !filesystem_operations.is_empty() {
        grant.capabilities.push(ScopedCapability::Filesystem {
            operations: filesystem_operations,
        });
    }
    Ok(grant)
}

fn expand_scope<'a, F>(
    patterns: &[String],
    matching: &[&'a RegisteredType],
    values: F,
) -> Result<Vec<String>, GrantError>
where
    F: Fn(&'a RegisteredType) -> &'a [String],
{
    let available = matching
        .iter()
        .flat_map(|registered| values(registered).iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut expanded = BTreeSet::new();
    for pattern in patterns {
        if pattern == "*" || pattern.ends_with(".*") {
            let prefix = pattern.strip_suffix(".*").unwrap_or_default();
            let matches = available
                .iter()
                .filter(|value| prefix.is_empty() || value.starts_with(&format!("{prefix}.")))
                .cloned()
                .collect::<Vec<_>>();
            if matches.is_empty() && pattern != "*" {
                return Err(GrantError::Contract("wildcard registry"));
            }
            expanded.extend(matches);
        } else {
            expanded.insert(pattern.clone());
        }
    }
    Ok(expanded.into_iter().collect())
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchGrant {
    pub package_id: String,
    pub package_version: String,
    pub manifest_digest: String,
    pub rules: Vec<GrantRule>,
    pub capabilities: Vec<ScopedCapability>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScopedCapability {
    Storage {
        root_capability_id: String,
        actions: Vec<String>,
        path_patterns: Vec<String>,
        max_bytes: u64,
    },
    Network {
        origins: Vec<String>,
        methods: Vec<String>,
        max_request_bytes: u64,
        max_response_bytes: u64,
    },
    Dialog {
        kinds: Vec<String>,
    },
    Window {
        actions: Vec<String>,
    },
    Dictation {
        operations: Vec<String>,
    },
    Focus {
        operations: Vec<String>,
    },
    Agents {
        operations: Vec<String>,
    },
    WorkerInvoke {
        operations: Vec<String>,
    },
    /// Named `network` scopes (`bookMetadata`, `images`) — Engine-owned app
    /// network ops, distinct from origin-form worker network grants.
    AppNetwork {
        scopes: Vec<String>,
    },
    Filesystem {
        operations: Vec<String>,
    },
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GrantError {
    #[error("contract violation: {0}")]
    Contract(&'static str),
    #[error("grant capacity")]
    Capacity,
    #[error("grant expired")]
    Expired,
    #[error("stale generation")]
    StaleGeneration,
    #[error("grant missing")]
    Missing,
    #[error("grant revoked")]
    Revoked,
}
pub struct GrantCompiler;
impl GrantCompiler {
    pub fn compile(i: CompileInput, r: &RegistrySnapshot) -> Result<LaunchGrant, GrantError> {
        if i.rules.len() > MAX_RULES {
            return Err(GrantError::Contract("rules"));
        };
        Version::parse(&i.package_version).map_err(|_| GrantError::Contract("package semver"))?;
        let mut map: BTreeMap<String, GrantRule> = BTreeMap::new();
        for mut x in i.rules {
            id(&x.type_id, 128)?;
            if x.versions.is_empty() {
                return Err(GrantError::Contract("versions"));
            };
            let mut matching = Vec::new();
            for v in &x.versions {
                let req = VersionReq::parse(v)
                    .map_err(|_| GrantError::Contract("version requirement"))?;
                let versions = r
                    .types
                    .iter()
                    .filter(|t| {
                        t.type_id == x.type_id
                            && Version::parse(&t.version)
                                .ok()
                                .is_some_and(|registered| req.matches(&registered))
                    })
                    .collect::<Vec<_>>();
                if versions.is_empty() {
                    return Err(GrantError::Contract("registry version"));
                }
                matching.extend(versions);
            }
            bounded_list(&x.fields_read, MAX_FIELDS)?;
            bounded_list(&x.fields_write, MAX_FIELDS)?;
            bounded_list(&x.relations_read, MAX_RELATIONS)?;
            bounded_list(&x.relations_write, MAX_RELATIONS)?;
            for reg in matching {
                if x.fields_read
                    .iter()
                    .chain(x.fields_write.iter())
                    .any(|f| !reg.fields.contains(f))
                {
                    return Err(GrantError::Contract("field registry"));
                };
                if x.relations_read
                    .iter()
                    .chain(x.relations_write.iter())
                    .any(|f| !reg.relations.contains(f))
                {
                    return Err(GrantError::Contract("relation registry"));
                };
            }
            if let Some(old) = map.get_mut(&x.type_id) {
                old.versions.retain(|v| x.versions.contains(v));
                old.actions = old.actions.intersection(&x.actions).cloned().collect();
                old.fields_read.retain(|v| x.fields_read.contains(v));
                old.fields_write.retain(|v| x.fields_write.contains(v));
                old.relations_read.retain(|v| x.relations_read.contains(v));
                old.relations_write
                    .retain(|v| x.relations_write.contains(v));
            } else {
                x.versions.sort();
                x.fields_read.sort();
                x.fields_write.sort();
                x.relations_read.sort();
                x.relations_write.sort();
                map.insert(x.type_id.clone(), x);
            }
        }
        if map
            .values()
            .any(|x| x.versions.is_empty() || x.actions.is_empty())
        {
            return Err(GrantError::Contract("empty intersection"));
        };
        Ok(LaunchGrant {
            package_id: i.package_id,
            package_version: i.package_version,
            manifest_digest: i.manifest_digest,
            rules: map.into_values().collect(),
            capabilities: vec![],
        })
    }
}
impl LaunchGrant {
    pub fn allows_dictation_operation(&self, operation: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(capability, ScopedCapability::Dictation { operations } if operations.iter().any(|allowed| allowed == operation))
        })
    }

    pub fn allows_focus_operation(&self, operation: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(capability, ScopedCapability::Focus { operations } if operations.iter().any(|allowed| allowed == operation))
        })
    }

    pub fn allows_agents_operation(&self, operation: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(capability, ScopedCapability::Agents { operations } if operations.iter().any(|allowed| allowed == operation))
        })
    }

    pub fn allows_worker_operation(&self, operation: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(capability, ScopedCapability::WorkerInvoke { operations } if operations.iter().any(|allowed| {
                allowed == operation || allowed.strip_suffix(".*").is_some_and(|prefix| operation.starts_with(&format!("{prefix}.")))
            }))
        })
    }

    pub fn allows_app_network_scope(&self, scope: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(capability, ScopedCapability::AppNetwork { scopes } if scopes.iter().any(|allowed| allowed == scope))
        })
    }

    pub fn allows_filesystem_operation(&self, operation: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(capability, ScopedCapability::Filesystem { operations } if operations.iter().any(|allowed| allowed == operation))
        })
    }

    /// Authorize the canonical typed request against the grant compiled for
    /// this launch. Callers must invoke this before translating to legacy RPC.
    pub fn authorize_request(&self, request: &DataRequest) -> Result<(), GrantError> {
        request.validate()?;
        let (type_id, version, action, fields, relations): (
            &str,
            &str,
            &str,
            Vec<String>,
            Vec<String>,
        ) = match request {
            DataRequest::ReadObject {
                type_id,
                type_version,
                fields,
                relations,
                ..
            }
            | DataRequest::ListObjects {
                type_id,
                type_version,
                fields,
                relations,
                ..
            } => (
                type_id,
                type_version,
                "read",
                fields.clone(),
                relations.clone(),
            ),
            DataRequest::CreateObject {
                type_id,
                type_version,
                fields,
                links,
                ..
            } => (
                type_id,
                type_version,
                "create",
                fields.iter().map(|f| f.field_id.clone()).collect(),
                links.iter().map(|l| l.relation_id.clone()).collect(),
            ),
            DataRequest::UpdateObject {
                type_id,
                type_version,
                fields,
                links,
                ..
            } => (
                type_id,
                type_version,
                "update",
                fields.iter().map(|f| f.field_id.clone()).collect(),
                links.iter().map(|l| l.relation_id.clone()).collect(),
            ),
            DataRequest::DeleteObject {
                type_id,
                type_version,
                ..
            } => (type_id, type_version, "delete", vec![], vec![]),
            DataRequest::Subscribe {
                type_id,
                versions,
                fields,
                relations,
                ..
            } => (
                type_id,
                versions.first().ok_or(GrantError::Missing)?,
                "subscribe",
                fields.clone(),
                relations.clone(),
            ),
            DataRequest::Link {
                source_type_id,
                source_version,
                relation,
                target_type_id,
                target_version,
                ..
            } => {
                let source = self
                    .rules
                    .iter()
                    .find(|r| {
                        r.type_id == *source_type_id
                            && r.versions.iter().any(|v| {
                                VersionReq::parse(v).ok().is_some_and(|req| {
                                    Version::parse(source_version)
                                        .ok()
                                        .is_some_and(|version| req.matches(&version))
                                })
                            })
                    })
                    .ok_or(GrantError::Missing)?;
                if !source.actions.contains("link")
                    || !source.relations_write.iter().any(|x| x == relation)
                {
                    return Err(GrantError::Missing);
                }
                let target = self
                    .rules
                    .iter()
                    .find(|r| {
                        r.type_id == *target_type_id
                            && r.versions.iter().any(|v| {
                                VersionReq::parse(v).ok().is_some_and(|req| {
                                    Version::parse(target_version)
                                        .ok()
                                        .is_some_and(|version| req.matches(&version))
                                })
                            })
                    })
                    .ok_or(GrantError::Missing)?;
                if !target.actions.contains("read") {
                    return Err(GrantError::Missing);
                }
                return Ok(());
            }
            DataRequest::Batch { requests } => {
                for item in requests {
                    self.authorize_request(&item.clone().into())?;
                }
                return Ok(());
            }
        };
        let version = Version::parse(version).map_err(|_| GrantError::Contract("semver"))?;
        let rule = self
            .rules
            .iter()
            .find(|r| {
                r.type_id == *type_id
                    && r.versions.iter().any(|v| {
                        VersionReq::parse(v)
                            .ok()
                            .is_some_and(|req| req.matches(&version))
                    })
            })
            .ok_or(GrantError::Missing)?;
        if !rule.actions.contains(action) {
            return Err(GrantError::Missing);
        }
        let allowed_fields = if matches!(action, "read" | "subscribe") {
            &rule.fields_read
        } else {
            &rule.fields_write
        };
        if fields
            .iter()
            .any(|field| !allowed_fields.iter().any(|allowed| allowed == field))
        {
            return Err(GrantError::Missing);
        }
        let allowed_relations = if matches!(action, "read" | "subscribe") {
            &rule.relations_read
        } else {
            &rule.relations_write
        };
        if relations
            .iter()
            .any(|relation| !allowed_relations.iter().any(|allowed| allowed == relation))
        {
            return Err(GrantError::Missing);
        }
        Ok(())
    }

    pub fn projection_json(&self) -> Result<String, GrantError> {
        serde_json::to_string(self).map_err(|_| GrantError::Contract("serialize"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GrantIdentity {
    pub grant_id: String,
    pub launch_id: String,
    pub package_id: String,
    pub package_version: String,
    pub manifest_digest: String,
    pub generation: u64,
    pub boot_epoch: [u8; 32],
}
impl GrantIdentity {
    pub fn new(g: &str, l: &str, p: &str, v: &str, d: &str, n: u64) -> Self {
        Self {
            grant_id: g.into(),
            launch_id: l.into(),
            package_id: p.into(),
            package_version: v.into(),
            manifest_digest: d.into(),
            generation: n,
            boot_epoch: [0; 32],
        }
    }
    pub fn with_epoch(mut self, e: [u8; 32]) -> Self {
        self.boot_epoch = e;
        self
    }
}
#[derive(Clone)]
pub struct TestClock(Arc<Mutex<u64>>);
impl TestClock {
    pub fn new(v: u64) -> Self {
        Self(Arc::new(Mutex::new(v)))
    }
    pub fn set(&self, v: u64) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = v
    }
    fn now(&self) -> u64 {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
#[derive(Clone)]
struct Stored {
    identity: GrantIdentity,
    expires: u64,
    revoked: bool,
}
pub struct GrantStore {
    clock: TestClock,
    max_global: usize,
    max_package: usize,
    epoch: [u8; 32],
    grants: Mutex<HashMap<String, Stored>>,
}
impl GrantStore {
    pub fn with_limits(clock: TestClock, g: usize, p: usize) -> Self {
        Self {
            clock,
            max_global: g,
            max_package: p,
            epoch: [0; 32],
            grants: Mutex::new(HashMap::new()),
        }
    }
    pub fn with_epoch(mut self, e: [u8; 32]) -> Self {
        self.epoch = e;
        self
    }
    pub fn mint(&self, mut id: GrantIdentity, expires: u64) -> Result<GrantIdentity, GrantError> {
        let mut m = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        let now = self.clock.now();
        m.retain(|_, x| x.expires > now && !x.revoked);
        if m.len() >= self.max_global
            || m.values()
                .filter(|x| x.identity.package_id == id.package_id)
                .count()
                >= self.max_package
        {
            return Err(GrantError::Capacity);
        };
        id.boot_epoch = self.epoch;
        m.insert(
            id.launch_id.clone(),
            Stored {
                identity: id.clone(),
                expires,
                revoked: false,
            },
        );
        Ok(id)
    }
    pub fn renew(
        &self,
        launch: &str,
        generation: u64,
        expires: u64,
    ) -> Result<GrantIdentity, GrantError> {
        let mut m = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        let x = m.get_mut(launch).ok_or(GrantError::Missing)?;
        if x.revoked {
            return Err(GrantError::Revoked);
        }
        if x.identity.boot_epoch != self.epoch {
            return Err(GrantError::Revoked);
        }
        if x.expires <= self.clock.now() {
            return Err(GrantError::Expired);
        }
        if x.identity.generation != generation {
            return Err(GrantError::StaleGeneration);
        }
        x.identity.generation += 1;
        x.expires = expires;
        Ok(x.identity.clone())
    }
    pub fn revoke(&self, l: &str) -> bool {
        self.grants
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(l)
            .is_some()
    }
    pub fn active_count(&self) -> usize {
        self.grants.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeProjection {
    package_id: String,
    package_version: String,
    manifest_digest: String,
    capabilities: Vec<ScopedCapability>,
}
impl SafeProjection {
    pub fn from_parts(p: &str, v: &str, d: &str, c: Vec<ScopedCapability>) -> Self {
        Self {
            package_id: p.into(),
            package_version: v.into(),
            manifest_digest: d.into(),
            capabilities: c,
        }
    }
    pub fn to_value(&self) -> Value {
        serde_json::json!({"package_id":self.package_id,"package_version":self.package_version,"manifest_digest":self.manifest_digest,"capabilities":self.capabilities.iter().map(|c|match c{ScopedCapability::Storage{root_capability_id,actions,max_bytes,..}=>serde_json::json!({"kind":"storage","root_capability_id":root_capability_id,"actions":actions,"max_bytes":max_bytes}),_=>serde_json::to_value(c).unwrap_or(Value::Null)}).collect::<Vec<_>>()})
    }
}
pub fn boot_epoch() -> Result<[u8; 32], GrantError> {
    let mut x = [0; 32];
    rand::thread_rng().fill_bytes(&mut x);
    Ok(x)
}
pub fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|x| x.as_secs())
        .unwrap_or(0)
}

/// Capability lookup covering every scoped app-RPC family (dictation, focus,
/// agents, filesystem). Returns the capability id when the operation belongs
/// to a scoped family.
pub fn scoped_capability(operation: &str) -> Option<&'static str> {
    dictation_operation_capability(operation)
        .or_else(|| focus_operation_capability(operation))
        .or_else(|| agents_operation_capability(operation))
        .or_else(|| filesystem_operation_capability(operation))
}

/// `Some(denied)` when `operation` is capability-scoped and the launch grant
/// does not allow it. `filesystem.*` additionally accepts a worker grant
/// since every vault operation runs Engine-side (no raw FS in the app).
pub fn scoped_capability_denied(operation: &str, grant: &LaunchGrant) -> Option<&'static str> {
    if dictation_operation_capability(operation).is_some() {
        return (!grant.allows_dictation_operation(operation)).then_some("dictation grant denied");
    }
    if focus_operation_capability(operation).is_some() {
        return (!grant.allows_focus_operation(operation)).then_some("focus grant denied");
    }
    if agents_operation_capability(operation).is_some() {
        return (!grant.allows_agents_operation(operation)).then_some("agents grant denied");
    }
    if filesystem_operation_capability(operation).is_some() {
        return (!grant.allows_filesystem_operation(operation)
            && !grant.allows_worker_operation(operation))
        .then_some("filesystem grant denied");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_v2_compiles_only_matching_dictation_permissions() {
        let raw = r#"{
            "schema_version": 2,
            "id": "com.kosmos.demo",
            "name": "Demo",
            "version": "1.0.0",
            "kind": "app",
            "engine_api": "*",
            "entrypoint": "index.html",
            "publisher": "kosmos",
            "permissions": [
                {"capability": "ark.read", "scopes": ["dictation.get_state", "dictation.start_recording"]},
                {"capability": "ark.write", "scopes": ["dictation.cancel", "dictation.get_config"]}
            ],
            "targets": [{"runtime": "kosmos-host", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
        let crate::package_manifest::VersionedManifest::V2(manifest) =
            crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
        else {
            unreachable!("expected v2 manifest");
        };

        let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
            .expect("compiled grant");
        assert!(grant.allows_dictation_operation("dictation.get_state"));
        assert!(grant.allows_dictation_operation("dictation.cancel"));
        assert!(!grant.allows_dictation_operation("dictation.get_config"));
        assert!(!grant.allows_dictation_operation("dictation.start_recording"));
    }

    #[test]
    fn manifest_v2_grants_dictation_config_updates_only_with_write_scope() {
        assert_eq!(
            dictation_operation_capability("dictation.update_config"),
            Some("ark.write")
        );
        assert_eq!(
            dictation_operation_capability("dictation.submit_audio"),
            None
        );
        assert_eq!(
            dictation_operation_capability("dictation.capture.future"),
            None
        );
    }

    #[test]
    fn dictation_worker_contract_uses_only_control_scopes() {
        assert_eq!(
            dictation_operation_capability("dictation.capture.start"),
            Some("dictation.control")
        );
        assert_eq!(
            dictation_operation_capability("dictation.lifecycle.set_autostart"),
            Some("dictation.control")
        );
        assert_eq!(
            dictation_operation_capability("dictation.submit_audio"),
            None
        );
    }

    #[test]
    fn manifest_v2_grants_named_network_scopes_only() {
        let raw = r#"{
            "schema_version": 2, "id": "com.kosmos.memoria", "name": "Memoria",
            "version": "0.6.9", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [{"capability": "network", "scopes": ["bookMetadata", "images"]}],
            "targets": [{"runtime": "kosmos-host", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
        let crate::package_manifest::VersionedManifest::V2(manifest) =
            crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
        else {
            unreachable!("expected v2 manifest");
        };

        let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
            .expect("compiled grant");
        for operation in [
            "bookMetadata.lookupIsbn",
            "bookMetadata.fetchPage",
            "images.fetch",
            "images.dominantColor",
            "images.storeCover",
        ] {
            let scope = app_network_operation_scope(operation).unwrap();
            assert!(grant.allows_app_network_scope(scope), "{operation}");
        }
        assert_eq!(app_network_operation_scope("bookMetadata.evil"), None);
        assert_eq!(app_network_operation_scope("images.deleteAll"), None);
    }

    #[test]
    fn manifest_v2_without_network_scope_denies_app_network_ops() {
        let raw = r#"{
            "schema_version": 2, "id": "com.kosmos.demo", "name": "Demo",
            "version": "1.0.0", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [{"capability": "network", "scopes": ["https://api.example.com"]}],
            "targets": [{"runtime": "kosmos-host", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
        let crate::package_manifest::VersionedManifest::V2(manifest) =
            crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
        else {
            unreachable!("expected v2 manifest");
        };
        let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
            .expect("compiled grant");
        assert!(!grant.allows_app_network_scope("bookMetadata"));
        assert!(!grant.allows_app_network_scope("images"));
    }

    #[test]
    fn manifest_v2_grants_only_requested_worker_operations() {
        let raw = r#"{
            "schema_version": 2, "id": "com.kosmos.arcadia", "name": "Arcadia",
            "version": "0.1.0", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [{"capability": "worker.invoke", "scopes": ["games.*"]}],
            "targets": [{"runtime": "kosmos-host", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
        let crate::package_manifest::VersionedManifest::V2(manifest) =
            crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
        else {
            unreachable!("expected v2 manifest");
        };

        let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
            .expect("compiled grant");
        assert!(grant.allows_worker_operation("games.list"));
        assert!(grant.allows_worker_operation("games.rawg.search"));
        assert!(!grant.allows_worker_operation("dictation.get_config"));
    }

    #[test]
    fn manifest_v2_grants_only_requested_focus_operations() {
        let raw = r#"{
            "schema_version": 2, "id": "com.kosmos.focus", "name": "Focus",
            "version": "0.1.0", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [
                {"capability": "ark.read", "scopes": ["pomodoro.get_state"]},
                {"capability": "ark.write", "scopes": ["pomodoro.start", "focus.set_active_state"]}
            ],
            "targets": [{"runtime": "kosmos-host", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
        let crate::package_manifest::VersionedManifest::V2(manifest) =
            crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
        else {
            unreachable!("expected v2 manifest");
        };

        let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
            .expect("compiled grant");
        assert!(grant.allows_focus_operation("pomodoro.get_state"));
        assert!(grant.allows_focus_operation("pomodoro.start"));
        assert!(grant.allows_focus_operation("focus.set_active_state"));
        assert!(!grant.allows_focus_operation("pomodoro.stop"));
        assert!(!grant.allows_focus_operation("focus.delete_blocklist"));
    }

    #[test]
    fn manifest_v2_grants_only_exact_agents_operations_by_capability() {
        let raw = r#"{
            "schema_version": 2, "id": "com.kosmos.daedalus", "name": "Daedalus",
            "version": "0.1.0", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [
                {"capability": "ark.read", "scopes": ["agents.projects.list", "agents.models.list", "agents.sessions.create", "agents.unknown"]},
                {"capability": "ark.write", "scopes": ["agents.sessions.create", "agents.editors.open", "agents.projects.list"]}
            ],
            "targets": [{"runtime": "kosmos-host", "os": ["windows"]}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
        let crate::package_manifest::VersionedManifest::V2(manifest) =
            crate::package_manifest::PackageManifest::parse(raw).expect("valid manifest")
        else {
            unreachable!("expected v2 manifest");
        };

        let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "digest")
            .expect("compiled grant");
        assert!(grant.allows_agents_operation("agents.projects.list"));
        assert!(grant.allows_agents_operation("agents.sessions.create"));
        assert!(grant.allows_agents_operation("agents.editors.open"));
        assert!(!grant.allows_agents_operation("agents.unknown"));
        assert_eq!(
            agents_operation_capability("agents.projects.list"),
            Some("ark.read")
        );
        assert_eq!(
            agents_operation_capability("agents.sessions.create"),
            Some("ark.write")
        );
        assert_eq!(agents_operation_capability("agents.unknown"), None);
    }
}
