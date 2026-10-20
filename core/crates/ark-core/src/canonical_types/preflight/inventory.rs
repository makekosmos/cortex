use ark_application::source_snapshot::SourceSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

// Read-only Phase 3 source inventory and mapper preflight.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", content = "name")]
pub enum SourceKind {
    Legacy(String),
    Native(String),
}

impl SourceKind {
    pub fn name(&self) -> &str {
        match self {
            Self::Legacy(v) | Self::Native(v) => v,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceRecord {
    pub source_kind: SourceKind,
    pub source_id: String,
    pub source_hash: String,
    pub canonical_bytes: Vec<u8>,
    pub raw_source: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockedItem {
    pub source_kind: String,
    pub source_id: String,
    pub pointer: String,
    pub code: String,
    pub raw_source: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreflightType {
    pub type_id: String,
    pub source_kind: String,
    pub before_count: usize,
    pub before_ids: Vec<String>,
    pub source_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationPreflightReport {
    #[serde(rename = "contractVersion")]
    pub contract_version: String,
    pub status: String,
    #[serde(rename = "sourceInventoryHash")]
    pub source_inventory_hash: String,
    pub types: Vec<PreflightType>,
    pub blocked: Vec<BlockedItem>,
    pub transformations: Vec<Value>,
    pub errors: Vec<BlockedItem>,
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(k, v)| (k.clone(), canonical_json(v)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

pub(crate) fn compact(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&canonical_json(value)).map_err(|e| e.to_string())
}

/// One preflight record's fields — the record's own columns before
/// serialisation into the envelope JSON.
pub(crate) struct SourceEnvelope<'a> {
    pub(crate) id: String,
    pub(crate) source_kind: &'a str,
    pub(crate) title: String,
    pub(crate) content: Value,
    pub(crate) props: Value,
    pub(crate) created: String,
    pub(crate) updated: String,
    pub(crate) deleted: Option<String>,
}

// Serialises one preflight record field-by-field.
pub(crate) fn envelope(e: SourceEnvelope<'_>) -> Value {
    json!(
        {"id":e.id,
        "legacy_type_id":e.source_kind,
        "title":e.title,
        "content":e.content,
        "props":e.props,
        "created_at":e.created,
        "updated_at":e.updated,
        "deleted_at":e.deleted})
}

pub(crate) fn generic_inventory(
    snapshot: &SourceSnapshot,
    out: &mut Vec<SourceRecord>,
    blocked: &mut Vec<BlockedItem>,
) -> Result<(), String> {
    for row in &snapshot.objects {
        let kind = row.type_id.as_str();
        let id = row.id.clone();
        let title = row.title.clone();
        let content_raw = row.content_json.as_bytes().to_vec();
        let props_raw = row.props_json.as_bytes().to_vec();
        let created = row.created_at.clone();
        let updated = row.updated_at.clone();
        let deleted = row.deleted_at.clone();
        let content = serde_json::from_slice::<Value>(&content_raw);
        let props = serde_json::from_slice::<Value>(&props_raw);
        let (content, props) = match (content, props) {
            (Ok(content), Ok(props)) => (content, props),
            _ => {
                let raw = if serde_json::from_slice::<Value>(&content_raw).is_err() {
                    content_raw.clone()
                } else {
                    props_raw.clone()
                };
                blocked.push(BlockedItem {
                    source_kind: kind.into(),
                    source_id: id.clone(),
                    pointer: if serde_json::from_slice::<Value>(&content_raw).is_err() {
                        "/content_json"
                    } else {
                        "/props_json"
                    }
                    .into(),
                    code: "MalformedJson".into(),
                    raw_source: raw.clone(),
                });
                out.push(SourceRecord {
                    source_kind: SourceKind::Legacy(kind.into()),
                    source_id: id,
                    source_hash: hash_bytes(&raw),
                    canonical_bytes: Vec::new(),
                    raw_source: raw,
                });
                continue;
            }
        };
        let value = envelope(SourceEnvelope {
            id: id.clone(),
            source_kind: kind,
            title,
            content,
            props,
            created,
            updated,
            deleted,
        });
        let bytes = compact(&value)?;
        // raw_source is the exact stored JSON object columns, not the canonicalized envelope.
        let raw = [content_raw.as_slice(), b"\0", props_raw.as_slice()].concat();
        out.push(SourceRecord {
            source_kind: SourceKind::Legacy(kind.into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes,
            raw_source: raw,
        });
    }
    Ok(())
}
