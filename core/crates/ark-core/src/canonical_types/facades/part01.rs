// Engine-owned typed canonical read/write facades.
use crate::canonical_types::definitions::canonical_type_registrations;
use crate::canonical_types::validation::validate_canonical;
use crate::db;
use crate::hlc::HLC;
use crate::types::SyncEntity;
use crate::types::{ArkObject, ArkObjectWrite, ObjectLink};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const IMAGE: &str = "com.kosmos.image";
const BOOK: &str = "com.kosmos.book";
const VERSION: &str = "1.0.0";
const COVER: &str = "cover-image";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetSourceError {
    pub code: &'static str,
    pub pointer: Option<String>,
}
impl AssetSourceError {
    pub const MAX_IDS: usize = 256;
    fn new(code: &'static str) -> Self {
        Self {
            code,
            pointer: None,
        }
    }
    fn at(code: &'static str, pointer: impl Into<String>) -> Self {
        Self {
            code,
            pointer: Some(pointer.into()),
        }
    }
}
impl std::fmt::Display for AssetSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "canonical_facade:{}", self.code)?;
        if let Some(p) = &self.pointer {
            write!(f, ":{p}")?;
        }
        Ok(())
    }
}
impl std::error::Error for AssetSourceError {}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssetSource {
    pub object_id: String,
    pub token: Option<String>,
    pub source_ref: String,
    pub source_kind: String,
}

fn registration(type_id: &str) -> Result<crate::type_registry::TypeRegistration, AssetSourceError> {
    canonical_type_registrations()
        .map_err(|_| AssetSourceError::new("definition_invariant"))?
        .into_iter()
        .find(|r| r.type_id == type_id && r.version == VERSION)
        .ok_or_else(|| AssetSourceError::new("wrong_type"))
}
fn validate_object(object: &ArkObject) -> Result<(), AssetSourceError> {
    if object.type_version != VERSION {
        return Err(AssetSourceError::new("wrong_version"));
    }
    let r = registration(&object.type_id)?;
    validate_canonical(&r, &object.props_json, &object.content_json)
        .map_err(|e| AssetSourceError::at("malformed_canonical", e.pointer))
}
/// Return only declared local/quarantine image and rich-text source candidates.
pub fn asset_sources(
    conn: &Connection,
    object_ids: &[String],
) -> Result<Vec<AssetSource>, AssetSourceError> {
    if object_ids.len() > AssetSourceError::MAX_IDS {
        return Err(AssetSourceError::new("too_many_object_ids"));
    }
    let mut seen = HashSet::new();
    if object_ids.iter().any(|id| !seen.insert(id)) {
        return Err(AssetSourceError::new("duplicate_object_id"));
    }
    let mut out = Vec::new();
    for id in object_ids {
        let Some(object) =
            db::get_object(conn, id).map_err(|_| AssetSourceError::new("storage"))?
        else {
            continue;
        };
        if object.deleted_at.is_some() {
            continue;
        }
        validate_object(&object)?;
        if object.type_id == IMAGE {
            let local: Option<String> = conn.query_row(
                "SELECT json_extract(data_json, '$.image.sourcePath') FROM object_local_state WHERE object_id=?1 ORDER BY device_id LIMIT 1", params![id], |r| r.get(0)).optional().map_err(|_| AssetSourceError::new("storage"))?.flatten();
            if let Some(source_ref) = local {
                out.push(AssetSource {
                    object_id: id.clone(),
                    token: None,
                    source_ref,
                    source_kind: "image-local".into(),
                });
                continue;
            }
            let quarantine: Option<String> = conn.query_row(
                "SELECT fields_json FROM object_migration_quarantine WHERE object_id=?1 ORDER BY contract_version LIMIT 1", params![id], |r| r.get(0)).optional().map_err(|_| AssetSourceError::new("storage"))?.flatten();
            if let Some(fields) = quarantine {
                let parsed_fields = serde_json::from_str::<Value>(&fields)
                    .map_err(|_| AssetSourceError::new("malformed_source"))?;
                let source_ref = parsed_fields
                    .get("image")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AssetSourceError::at("malformed_source", "/fields/image"))?;
                if !source_ref.is_empty() {
                    out.push(AssetSource {
                        object_id: id.clone(),
                        token: None,
                        source_ref: source_ref.into(),
                        source_kind: "image-quarantine".into(),
                    });
                }
            }
        }
        let local_data: Option<String> = conn
            .query_row(
                "SELECT data_json FROM object_local_state WHERE object_id=?1 ORDER BY device_id LIMIT 1",
                params![id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| AssetSourceError::new("storage"))?;
        if let Some(images) = local_data
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|value| value.get("richTextImages").cloned())
        {
            if let Some(map) = images.as_object() {
                for (token, source) in map {
                    let source_ref = source.as_str().ok_or_else(|| {
                        AssetSourceError::at("malformed_source", "/content/richTextImages")
                    })?;
                    if !token.starts_with("kosmos-local://richtext-image/") {
                        return Err(AssetSourceError::at(
                            "malformed_source",
                            "/content/richTextImages",
                        ));
                    }
                    out.push(AssetSource {
                        object_id: id.clone(),
                        token: Some(token.clone()),
                        source_ref: source_ref.into(),
                        source_kind: "richtext-local".into(),
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| {
        (&a.object_id, &a.token, &a.source_kind).cmp(&(&b.object_id, &b.token, &b.source_kind))
    });
    Ok(out)
}

#[derive(Debug, Clone, Default)]
pub struct BookCoverMutation {
    pub changed: bool,
    pub book: Option<ArkObject>,
    pub image: Option<ArkObject>,
    pub links: Vec<ObjectLink>,
    pub deleted_link_ids: Vec<String>,
}

fn digest_id(prefix: &str, bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{prefix}{:x}", h.finalize())
}
fn canonical_link_id(book_id: &str, image_id: &str) -> String {
    digest_id(
        "lnk*",
        format!("kosmos-link-v1\0{book_id}\0{COVER}\0{image_id}").as_bytes(),
    )
}

fn now(device: &str) -> String {
    HLC::now(device).wall_time
}
