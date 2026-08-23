use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegacyRecord {
    pub id: String,
    pub legacy_type_id: String,
    pub title: String,
    pub content: Value,
    pub props: Value,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalState {
    pub data_json: Value,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quarantine {
    pub fields_json: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedRecord {
    pub object: crate::types::ArkObject,
    pub links: Vec<crate::types::ObjectLink>,
    pub local_state: Vec<LocalState>,
    pub quarantine: Vec<Quarantine>,
    pub raw_source: Option<Vec<u8>>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompatFailure {
    MalformedJson { pointer: String },
    InvalidField { pointer: String },
    ConflictingFields { pointer: String },
    UnsupportedLegacyValue { pointer: String },
    SecretField { pointer: String },
    DataLossRisk { pointer: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityError {
    MalformedJson {
        source_kind: String,
        source_id: String,
        pointer: String,
        raw_source: Vec<u8>,
    },
    InvalidField {
        source_kind: String,
        source_id: String,
        pointer: String,
        raw_source: Vec<u8>,
    },
    ConflictingFields {
        source_kind: String,
        source_id: String,
        pointer: String,
        raw_source: Vec<u8>,
    },
    UnsupportedLegacyValue {
        source_kind: String,
        source_id: String,
        pointer: String,
        raw_source: Vec<u8>,
    },
    SecretField {
        source_kind: String,
        source_id: String,
        pointer: String,
        raw_source: Vec<u8>,
    },
    DataLossRisk {
        source_kind: String,
        source_id: String,
        pointer: String,
        raw_source: Vec<u8>,
    },
}
impl CompatibilityError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MalformedJson { .. } => "MALFORMED_JSON",
            Self::InvalidField { .. } => "INVALID_FIELD",
            Self::ConflictingFields { .. } => "CONFLICTING_FIELDS",
            Self::UnsupportedLegacyValue { .. } => "UNSUPPORTED_LEGACY_VALUE",
            Self::SecretField { .. } => "SECRET_FIELD",
            Self::DataLossRisk { .. } => "DATA_LOSS_RISK",
        }
    }
    pub fn source_kind(&self) -> &str {
        match self {
            Self::MalformedJson { source_kind, .. }
            | Self::InvalidField { source_kind, .. }
            | Self::ConflictingFields { source_kind, .. }
            | Self::UnsupportedLegacyValue { source_kind, .. }
            | Self::SecretField { source_kind, .. }
            | Self::DataLossRisk { source_kind, .. } => source_kind,
        }
    }
    pub fn source_id(&self) -> &str {
        match self {
            Self::MalformedJson { source_id, .. }
            | Self::InvalidField { source_id, .. }
            | Self::ConflictingFields { source_id, .. }
            | Self::UnsupportedLegacyValue { source_id, .. }
            | Self::SecretField { source_id, .. }
            | Self::DataLossRisk { source_id, .. } => source_id,
        }
    }
    pub fn pointer(&self) -> &str {
        match self {
            Self::MalformedJson { pointer, .. }
            | Self::InvalidField { pointer, .. }
            | Self::ConflictingFields { pointer, .. }
            | Self::UnsupportedLegacyValue { pointer, .. }
            | Self::SecretField { pointer, .. }
            | Self::DataLossRisk { pointer, .. } => pointer,
        }
    }
    pub fn raw_source(&self) -> &[u8] {
        match self {
            Self::MalformedJson { raw_source, .. }
            | Self::InvalidField { raw_source, .. }
            | Self::ConflictingFields { raw_source, .. }
            | Self::UnsupportedLegacyValue { raw_source, .. }
            | Self::SecretField { raw_source, .. }
            | Self::DataLossRisk { raw_source, .. } => raw_source,
        }
    }
}
pub fn canonical_id(a: &str) -> Option<&'static str> {
    Some(match a {
        "note_obj" => "com.kosmos.note",
        "task_obj" => "com.kosmos.task",
        "project_obj" => "com.kosmos.project",
        "tag_obj" => "com.kosmos.tag",
        "person_obj" => "com.kosmos.person",
        "image_obj" => "com.kosmos.image",
        "time_entry_obj" => "com.kosmos.time-entry",
        "game_obj" => "com.kosmos.game",
        "book_obj" => "com.kosmos.book",
        _ => return None,
    })
}
pub fn malformed(p: &str) -> CompatFailure {
    CompatFailure::MalformedJson { pointer: p.into() }
}
pub fn invalid(p: &str) -> CompatFailure {
    CompatFailure::InvalidField { pointer: p.into() }
}
pub fn conflict(p: &str) -> CompatFailure {
    CompatFailure::ConflictingFields { pointer: p.into() }
}
pub fn val<'a>(
    m: &'a Map<String, Value>,
    c: &str,
    aliases: &[&str],
    used: &mut std::collections::BTreeSet<String>,
) -> Result<Option<&'a Value>, CompatFailure> {
    let mut found = Vec::new();
    if let Some(v) = m.get(c) {
        found.push((c, v));
    }
    for a in aliases {
        if let Some(v) = m.get(*a) {
            found.push((*a, v));
        }
    }
    if found.len() > 1 && found.windows(2).any(|w| w[0].1 != w[1].1) {
        return Err(conflict(&format!("/props/{c}")));
    }
    for (k, _) in &found {
        used.insert((*k).into());
    }
    Ok(found.first().map(|(_, v)| *v))
}
pub fn string(
    m: &Map<String, Value>,
    c: &str,
    a: &[&str],
    u: &mut std::collections::BTreeSet<String>,
    nullable: bool,
) -> Result<Value, CompatFailure> {
    match val(m, c, a, u)? {
        None if nullable => Ok(Value::Null),
        None => Err(invalid(&format!("/props/{c}"))),
        Some(v) if v.is_string() || (nullable && v.is_null()) => Ok(v.clone()),
        Some(_) => Err(invalid(&format!("/props/{c}"))),
    }
}
pub fn boolv(
    m: &Map<String, Value>,
    c: &str,
    a: &[&str],
    u: &mut std::collections::BTreeSet<String>,
) -> Result<Value, CompatFailure> {
    match val(m, c, a, u)? {
        None => Ok(Value::Bool(false)),
        Some(v) if v.is_boolean() => Ok(v.clone()),
        Some(_) => Err(invalid(&format!("/props/{c}"))),
    }
}
pub fn array_or_null(
    m: &Map<String, Value>,
    c: &str,
    a: &[&str],
    u: &mut std::collections::BTreeSet<String>,
    default: Value,
) -> Result<Value, CompatFailure> {
    match val(m, c, a, u)? {
        None => Ok(default),
        Some(v) if v.is_array() => Ok(v.clone()),
        Some(_) => Err(invalid(&format!("/props/{c}"))),
    }
}
pub fn put_extensions(o: &mut Map<String, Value>) {
    o.entry("extensions")
        .or_insert_with(|| Value::Object(Map::new()));
}
fn relation_val<'a>(
    m: &'a Map<String, Value>,
    c: &str,
    aliases: &[&str],
    used: &mut std::collections::BTreeSet<String>,
) -> Result<Option<&'a Value>, CompatFailure> {
    let mut found = Vec::new();
    if let Some(v) = m.get(c) {
        found.push((c, v));
    }
    for a in aliases {
        if let Some(v) = m.get(*a) {
            found.push((*a, v));
        }
    }
    if found.len() > 1 {
        let normalized = found
            .iter()
            .map(|(_, v)| relation_set(v))
            .collect::<Result<Vec<_>, _>>()?;
        if normalized.windows(2).any(|w| w[0] != w[1]) {
            return Err(conflict(&format!("/props/{c}")));
        }
    }
    for (k, _) in &found {
        used.insert((*k).into());
    }
    Ok(found.first().map(|(_, v)| *v))
}

fn relation_set(v: &Value) -> Result<std::collections::BTreeSet<String>, CompatFailure> {
    let values = match v {
        Value::String(s) => vec![s.as_str()],
        Value::Array(a) => a
            .iter()
            .map(|v| v.as_str().ok_or_else(|| invalid("/props/relation")))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(invalid("/props/relation")),
    };
    Ok(values.into_iter().map(str::to_owned).collect())
}

pub fn relation_with_aliases(
    m: &Map<String, Value>,
    key: &str,
    aliases: &[&str],
    kind: &str,
    links: &mut Vec<crate::types::ObjectLink>,
    id: &str,
    at: &str,
    u: &mut std::collections::BTreeSet<String>,
) -> Result<(), CompatFailure> {
    let pointer = |fallback: &str| {
        if m.contains_key(key) {
            format!("/props/{key}")
        } else {
            aliases
                .iter()
                .find(|a| m.contains_key(**a))
                .map_or_else(|| format!("/props/{fallback}"), |a| format!("/props/{a}"))
        }
    };
    if let Some(v) = relation_val(m, key, aliases, u)? {
        let mut ids = Vec::new();
        if let Some(s) = v.as_str() {
            ids.push(s.to_owned())
        } else if let Some(a) = v.as_array() {
            for x in a {
                ids.push(x.as_str().ok_or_else(|| invalid(&pointer(key)))?.to_owned())
            }
        } else {
            return Err(invalid(&pointer(key)));
        }
        ids.sort();
        ids.dedup();
        for target in ids {
            let mut h = Sha256::new();
            h.update(format!("kosmos-link-v1\0{id}\0{kind}\0{target}").as_bytes());
            links.push(crate::types::ObjectLink {
                id: format!("lnk*{:x}", h.finalize()),
                source_object_id: id.into(),
                target_object_id: target,
                link_type: kind.into(),
                created_at: at.into(),
            });
        }
    }
    Ok(())
}
pub fn relation_single_with_aliases(
    m: &Map<String, Value>,
    key: &str,
    aliases: &[&str],
    kind: &str,
    links: &mut Vec<crate::types::ObjectLink>,
    id: &str,
    at: &str,
    u: &mut std::collections::BTreeSet<String>,
) -> Result<(), CompatFailure> {
    let pointer = || {
        if m.contains_key(key) {
            format!("/props/{key}")
        } else {
            aliases
                .iter()
                .find(|a| m.contains_key(**a))
                .map_or_else(|| format!("/props/{key}"), |a| format!("/props/{a}"))
        }
    };
    if let Some(v) = relation_val(m, key, aliases, u)? {
        if let Some(a) = v.as_array() {
            let mut ids = a.iter().filter_map(Value::as_str).collect::<Vec<_>>();
            ids.sort_unstable();
            ids.dedup();
            if ids.len() > 1 {
                return Err(invalid(&pointer()));
            }
        }
    }
    relation_with_aliases(m, key, aliases, kind, links, id, at, u)
}
pub fn bundle(m: Map<String, Value>) -> Vec<LocalState> {
    if m.is_empty() {
        vec![]
    } else {
        vec![LocalState {
            data_json: Value::Object(m),
        }]
    }
}
pub fn qbundle(m: Map<String, Value>) -> Vec<Quarantine> {
    if m.is_empty() {
        vec![]
    } else {
        vec![Quarantine {
            fields_json: Value::Object(m),
        }]
    }
}
pub fn extensions(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    used: &std::collections::BTreeSet<String>,
    local: &mut Map<String, Value>,
    q: &mut Map<String, Value>,
) -> Result<(), CompatFailure> {
    let mut e = Map::new();
    for (k, v) in m {
        if used.contains(k) {
            continue;
        }
        if is_secret(k) {
            return Err(CompatFailure::SecretField {
                pointer: format!("/props/{k}"),
            });
        }
        if is_local(k) {
            local
                .entry("planning")
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .map(|nested| nested.insert(camel(k), v.clone()));
        } else if is_quarantine(k) {
            q.insert(camel(k), v.clone());
        } else if !is_relation_id(k) {
            if let Some(v) = sanitize_extension(v, &camel(k), local, q) {
                e.insert(camel(k), v);
            }
        }
    }
    if let Some(Value::Object(x)) = o.get_mut("extensions") {
        x.insert("compatibility".into(), Value::Object(e));
    }
    Ok(())
}
fn sanitize_extension(
    v: &Value,
    namespace: &str,
    _local: &mut Map<String, Value>,
    q: &mut Map<String, Value>,
) -> Option<Value> {
    match v {
        Value::Object(m) => Some(Value::Object(
            m.iter()
                .filter_map(|(k, v)| {
                    if is_local(k) || is_quarantine(k) || is_relation_id(k) {
                        q.insert(format!("{namespace}.{}", camel(k)), v.clone());
                        None
                    } else {
                        sanitize_extension(v, &format!("{namespace}.{}", camel(k)), _local, q)
                            .map(|v| (camel(k), v))
                    }
                })
                .collect(),
        )),
        Value::Array(a) => Some(Value::Array(
            a.iter()
                .filter_map(|v| sanitize_extension(v, namespace, _local, q))
                .collect(),
        )),
        _ => Some(v.clone()),
    }
}
pub fn reject_secrets(m: &Map<String, Value>, p: &str) -> Result<(), CompatFailure> {
    for (k, v) in m {
        let pointer = format!("{p}/{k}");
        if is_secret(k) {
            return Err(CompatFailure::SecretField { pointer });
        }
        reject_secrets_value(v, &pointer)?;
    }
    Ok(())
}
fn reject_secrets_value(v: &Value, p: &str) -> Result<(), CompatFailure> {
    match v {
        Value::Object(m) => reject_secrets(m, p),
        Value::Array(a) => {
            for (i, item) in a.iter().enumerate() {
                reject_secrets_value(item, &format!("{p}/{i}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn is_secret(k: &str) -> bool {
    let x = k.to_ascii_lowercase();
    x.contains("password")
        || x.contains("token")
        || x.contains("secret")
        || x.contains("credential")
        || x.contains("api_key")
        || x.contains("apikey")
}
fn is_local(k: &str) -> bool {
    matches!(
        k,
        "exe_path"
            | "exePath"
            | "install_dir"
            | "installDir"
            | "save_path"
            | "savePath"
            | "source_path"
            | "sourcePath"
            | "save_exists"
            | "saveExists"
            | "exe_name"
            | "exeName"
            | "installed"
            | "install_size_bytes"
            | "installSizeBytes"
            | "launch_pid"
            | "launchPid"
            | "window_id"
            | "windowId"
            | "process_state"
            | "processState"
            | "pid"
    )
}
fn is_quarantine(k: &str) -> bool {
    matches!(
        k,
        "source"
            | "source_app"
            | "sourceApp"
            | "source_app_id"
            | "sourceAppId"
            | "rawg_id"
            | "rawgId"
            | "provider"
            | "account_id"
            | "accountId"
            | "external_id"
            | "externalId"
            | "connector_id"
            | "connectorId"
            | "cover_image"
            | "coverImage"
            | "background_image"
            | "backgroundImage"
            | "project_id"
            | "projectId"
            | "workspace_id"
            | "workspaceId"
    )
}
fn is_relation_id(k: &str) -> bool {
    matches!(
        k,
        "project_id"
            | "projectId"
            | "tag_ids"
            | "tagIds"
            | "related_ids"
            | "relatedIds"
            | "source_note_id"
            | "sourceNoteId"
            | "photo_id"
            | "photoId"
            | "taskId"
            | "task_id"
            | "note_ids"
            | "noteIds"
            | "task_ids"
            | "taskIds"
            | "author_person_ids"
            | "authorPersonIds"
            | "related_notes"
    )
}
pub fn camel(k: &str) -> String {
    let mut s = String::new();
    let mut up = false;
    for c in k.chars() {
        if c == '_' {
            up = true
        } else if up {
            s.extend(c.to_uppercase());
            up = false
        } else {
            s.push(c)
        }
    }
    s
}
pub fn validate(obj: &crate::types::ArkObject) -> Result<(), CompatFailure> {
    let regs =
        crate::canonical_types::definitions::canonical_type_registrations().map_err(|_| {
            CompatFailure::DataLossRisk {
                pointer: "/".into(),
            }
        })?;
    let r = regs
        .into_iter()
        .find(|x| x.type_id == obj.type_id && x.version == obj.type_version)
        .ok_or_else(|| invalid("/type_id"))?;
    crate::canonical_types::validation::validate_canonical(&r, &obj.props_json, &obj.content_json)
        .map_err(|e| match e.code {
            crate::canonical_types::validation::CanonicalValidationCode::InvalidField => {
                invalid(&e.pointer)
            }
            crate::canonical_types::validation::CanonicalValidationCode::InvariantViolation => {
                CompatFailure::DataLossRisk { pointer: e.pointer }
            }
        })
}
