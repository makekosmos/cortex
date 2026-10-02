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
