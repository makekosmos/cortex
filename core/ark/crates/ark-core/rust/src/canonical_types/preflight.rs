//! Read-only Phase 3 source inventory and mapper preflight.
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

fn envelope(
    id: String,
    source_kind: &str,
    title: String,
    content: Value,
    props: Value,
    created: String,
    updated: String,
    deleted: Option<String>,
) -> Value {
    json!({"id":id,"legacy_type_id":source_kind,"title":title,"content":content,"props":props,"created_at":created,"updated_at":updated,"deleted_at":deleted})
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
        let mut stmt = conn.prepare("SELECT id,title,content_json,props_json,created_at,updated_at,deleted_at FROM objects WHERE type_id=?1 ORDER BY id") .map_err(|e| e.to_string())?;
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
            let value = envelope(
                id.clone(),
                kind,
                title,
                content,
                props,
                created,
                updated,
                deleted,
            );
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

fn native_inventory(conn: &Connection, out: &mut Vec<SourceRecord>) -> Result<(), String> {
    // Areas and headings are planning-only compatibility rows; the frozen contract
    // explicitly keeps them in place and never fabricates canonical objects.
    let mut todo = conn.prepare("SELECT id,title,notes,priority,scheduled_date,deadline,reminder_date,is_someday,is_completed,completed_at,is_cancelled,cancelled_at,heading_id,project_id,area_id,tag_ids,checklist_items,recurrence_rule,created_at FROM todos ORDER BY id").map_err(|e| e.to_string())?;
    for row in todo
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, i64>(10)?,
                r.get::<_, Option<String>>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, String>(15)?,
                r.get::<_, String>(16)?,
                r.get::<_, Option<String>>(17)?,
                r.get::<_, String>(18)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (
            id,
            title,
            notes,
            priority,
            scheduled,
            deadline,
            reminder,
            someday,
            completed,
            completed_at,
            cancelled,
            cancelled_at,
            heading,
            project,
            area,
            tags,
            checklist,
            recurrence,
            created,
        ) = row.map_err(|e| e.to_string())?;
        let mut props = serde_json::Map::new();
        props.insert("notes".into(), json!(notes));
        props.insert("priority".into(), json!(priority));
        props.insert("scheduled_date".into(), json!(scheduled));
        props.insert("deadline".into(), json!(deadline));
        props.insert("reminder_date".into(), json!(reminder));
        if someday != 0 {
            props.insert("is_someday".into(), json!(true));
        }
        if completed != 0 {
            props.insert("is_completed".into(), json!(true));
        }
        props.insert("completed_at".into(), json!(completed_at));
        if cancelled != 0 {
            props.insert("is_cancelled".into(), json!(true));
        }
        props.insert("cancelled_at".into(), json!(cancelled_at));
        props.insert("heading_id".into(), json!(heading));
        if let Some(project) = project {
            props.insert("project_id".into(), json!(project));
        }
        props.insert("area_id".into(), json!(area));
        props.insert(
            "tag_ids".into(),
            serde_json::from_str::<Value>(&tags).unwrap_or(Value::Array(Vec::new())),
        );
        props.insert(
            "checklist_items".into(),
            serde_json::from_str::<Value>(&checklist).unwrap_or(Value::Array(Vec::new())),
        );
        props.insert(
            "recurrence_rule".into(),
            recurrence
                .and_then(|v| serde_json::from_str::<Value>(&v).ok())
                .unwrap_or(Value::Null),
        );
        let props = Value::Object(props);
        let value = envelope(
            id.clone(),
            "task_obj",
            title,
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            props,
            created.clone(),
            created,
            None,
        );
        let bytes = compact(&value)?;
        out.push(SourceRecord {
            source_kind: SourceKind::Native("todos".into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes.clone(),
            raw_source: bytes,
        });
    }
    let mut projects=conn.prepare("SELECT id,title,notes,status,scheduled_date,deadline,color_tag,area_id,created_at FROM projects ORDER BY id").map_err(|e|e.to_string())?;
    for row in projects
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (id, title, notes, status, scheduled, deadline, color, area, created) =
            row.map_err(|e| e.to_string())?;
        let value = envelope(
            id.clone(),
            "project_obj",
            title,
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            json!({"notes":notes,"status":status,"scheduled_date":scheduled,"deadline":deadline,"color_tag":color,"area_id":area}),
            created.clone(),
            created,
            None,
        );
        let bytes = compact(&value)?;
        out.push(SourceRecord {
            source_kind: SourceKind::Native("projects".into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes.clone(),
            raw_source: bytes,
        });
    }
    let mut tags = conn
        .prepare("SELECT id,title,color,created_at FROM tags ORDER BY id")
        .map_err(|e| e.to_string())?;
    for row in tags
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (id, title, color, created) = row.map_err(|e| e.to_string())?;
        let value = envelope(
            id.clone(),
            "tag_obj",
            title,
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            json!({"color":color}),
            created.clone(),
            created,
            None,
        );
        let bytes = compact(&value)?;
        out.push(SourceRecord {
            source_kind: SourceKind::Native("tags".into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes.clone(),
            raw_source: bytes,
        });
    }
    let mut areas = conn
        .prepare("SELECT id,title,created_at FROM areas ORDER BY id")
        .map_err(|e| e.to_string())?;
    for row in areas
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (id, title, created) = row.map_err(|e| e.to_string())?;
        let bytes = compact(&envelope(
            id.clone(),
            "project_obj",
            title,
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            json!({}),
            created.clone(),
            created,
            None,
        ))?;
        out.push(SourceRecord {
            source_kind: SourceKind::Native("areas".into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes.clone(),
            raw_source: bytes,
        });
    }
    let mut headings = conn
        .prepare("SELECT id,title,project_id FROM headings ORDER BY id")
        .map_err(|e| e.to_string())?;
    for row in headings
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (id, title, project) = row.map_err(|e| e.to_string())?;
        let bytes = compact(&envelope(
            id.clone(),
            "project_obj",
            title,
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            json!({"project_id":project}),
            String::new(),
            String::new(),
            None,
        ))?;
        out.push(SourceRecord {
            source_kind: SourceKind::Native("headings".into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes.clone(),
            raw_source: bytes,
        });
    }
    Ok(())
}

pub fn inventory_sources(conn: &Connection) -> Result<Vec<SourceRecord>, String> {
    let mut out = Vec::new();
    let mut blocked = Vec::new();
    generic_inventory(conn, &mut out, &mut blocked)?;
    native_inventory(conn, &mut out)?;
    out.sort_by(|a, b| {
        (a.source_kind.name(), &a.source_id).cmp(&(b.source_kind.name(), &b.source_id))
    });
    Ok(out)
}

fn context(conn: &Connection) -> Result<MappingContext, String> {
    let regs = canonical_type_registrations()?;
    let aliases: BTreeMap<String, CanonicalIdentity> = regs
        .iter()
        .flat_map(|r| {
            r.aliases.iter().map(move |a| {
                (
                    a.alias.clone(),
                    CanonicalIdentity::new(r.type_id.clone(), r.version.clone()),
                )
            })
        })
        .collect();
    let mut ids = BTreeMap::new();
    let mut stmt = conn
        .prepare("SELECT id,type_id,type_version FROM objects")
        .map_err(|e| e.to_string())?;
    for row in stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (id, type_id, version) = row.map_err(|e| e.to_string())?;
        ids.insert(
            id,
            aliases
                .get(&type_id)
                .cloned()
                .unwrap_or_else(|| CanonicalIdentity::new(type_id, version)),
        );
    }
    Ok(MappingContext {
        existing_object_ids: ids,
        existing_links: crate::db::list_object_links(conn)?,
    })
}

pub fn preflight_phase3(conn: &Connection) -> Result<MigrationPreflightReport, String> {
    let mut blocked = Vec::new();
    let mut records = Vec::new();
    generic_inventory(conn, &mut records, &mut blocked)?;
    native_inventory(conn, &mut records)?;
    records.sort_by(|a, b| {
        (a.source_kind.name(), &a.source_id).cmp(&(b.source_kind.name(), &b.source_id))
    });
    let inventory_bytes = compact(&Value::Array(records.iter().map(|r| json!({"sourceKind":r.source_kind.name(),"sourceId":r.source_id,"sourceHash":r.source_hash})).collect()))?;
    let inventory_hash = hash_bytes(&inventory_bytes);
    let mut errors = blocked.clone();
    let ctx = context(conn)?;
    for record in &records {
        if record.canonical_bytes.is_empty() || matches!(record.source_kind, SourceKind::Native(_))
        {
            continue;
        }
        if let Err(error) = map_legacy_with_context(
            LegacySource {
                source_kind: record.source_kind.name(),
                source_id: &record.source_id,
                raw_json: &record.canonical_bytes,
            },
            &ctx,
        ) {
            errors.push(BlockedItem {
                source_kind: error.source_kind().into(),
                source_id: error.source_id().into(),
                pointer: error.pointer().into(),
                code: error.code().into(),
                raw_source: error.raw_source().to_vec(),
            });
        }
    }
    errors.sort_by(|a, b| {
        (&a.source_kind, &a.source_id, &a.pointer, &a.code).cmp(&(
            &b.source_kind,
            &b.source_id,
            &b.pointer,
            &b.code,
        ))
    });
    let mut types = Vec::new();
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
        "todos",
        "projects",
        "areas",
        "tags",
        "headings",
    ] {
        let ids: Vec<String> = records
            .iter()
            .filter(|r| r.source_kind.name() == kind)
            .map(|r| r.source_id.clone())
            .collect();
        if !ids.is_empty() {
            types.push(PreflightType {
                type_id: kind.into(),
                source_kind: kind.into(),
                before_count: ids.len(),
                before_ids: ids,
                source_hash: hash_bytes(&compact(&json!(kind))?),
            });
        }
    }
    Ok(MigrationPreflightReport {
        contract_version: "phase3-canonical-v1".into(),
        status: if errors.is_empty() {
            "ready"
        } else {
            "blocked"
        }
        .into(),
        source_inventory_hash: inventory_hash,
        types,
        blocked,
        transformations: Vec::new(),
        errors,
    })
}
