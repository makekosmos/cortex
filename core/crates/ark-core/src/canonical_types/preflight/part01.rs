// Read-only Phase 3 source inventory and mapper preflight.
use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::compatibility::{
    map_legacy_with_context, CanonicalIdentity, LegacySource, MappingContext,
};
use super::definitions::canonical_type_registrations;

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

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn canonical_json(value: &Value) -> Value {
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
struct SourceEnvelope<'a> {
    id: String,
    source_kind: &'a str,
    title: String,
    content: Value,
    props: Value,
    created: String,
    updated: String,
    deleted: Option<String>,
}

// Serialises one preflight record field-by-field.
fn envelope(e: SourceEnvelope<'_>) -> Value {
    json!({"id":e.id,"legacy_type_id":e.source_kind,"title":e.title,"content":e.content,"props":e.props,"created_at":e.created,"updated_at":e.updated,"deleted_at":e.deleted})
}

fn generic_inventory(
    conn: &Connection,
    out: &mut Vec<SourceRecord>,
    blocked: &mut Vec<BlockedItem>,
) -> Result<(), String> {
    for kind in [
        "note_obj",
        "task_obj",
        "project_obj",
        "tag_obj",
        "person_obj",
        "image_obj",
        "time_entry_obj",
        "game_obj",
        "book_obj",
    ] {
        let mut stmt = conn.prepare(concat!("SELECT id,title,content_json,props_json,created_at,updated_at,deleted_at ","FROM objects WHERE type_id=?1 ORDER BY id")) .map_err(|e| e.to_string())?;
        for row in stmt
            .query_map([kind], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?.into_bytes(),
                    r.get::<_, String>(3)?.into_bytes(),
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|e| e.to_string())?
        {
            let (id, title, content_raw, props_raw, created, updated, deleted) =
                row.map_err(|e| e.to_string())?;
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
                deleted
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
    }
    Ok(())
}
