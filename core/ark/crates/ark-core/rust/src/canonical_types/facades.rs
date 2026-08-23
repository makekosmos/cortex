//! Engine-owned typed canonical read/write facades.
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

/// Atomically set, replace, or remove the Book's sole canonical cover link.
///
/// This boundary never materializes an Image. `source_ref` is retained in the
/// wire contract for compatibility, but source conversion belongs to the
/// caller/import pipeline; this facade only links an existing typed Image.
pub fn set_book_cover(
    conn: &Connection,
    book_id: &str,
    source_ref: Option<&str>,
    existing_image_id: Option<&str>,
    _alt_text: &str,
    device_id: &str,
) -> Result<BookCoverMutation, AssetSourceError> {
    if source_ref.is_some() && existing_image_id.is_some() {
        return Err(AssetSourceError::new("conflicting_source"));
    }
    if source_ref.is_some() {
        return Err(AssetSourceError::new("source_requires_existing_image"));
    }
    let book = db::get_object(conn, book_id)
        .map_err(|_| AssetSourceError::new("storage"))?
        .ok_or_else(|| AssetSourceError::new("missing_book"))?;
    if book.type_id != BOOK {
        return Err(AssetSourceError::new("wrong_book_type"));
    }
    validate_object(&book)?;
    let mut image = None;
    let image_id = if let Some(id) = existing_image_id {
        let object = db::get_object(conn, id)
            .map_err(|_| AssetSourceError::new("storage"))?
            .ok_or_else(|| AssetSourceError::new("missing_image"))?;
        if object.type_id != IMAGE {
            return Err(AssetSourceError::new("wrong_image_type"));
        }
        if object.deleted_at.is_some() {
            return Err(AssetSourceError::new("deleted_image"));
        }
        validate_object(&object)?;
        image = Some(object);
        Some(id.to_string())
    } else {
        None
    };
    let existing: Vec<ObjectLink> = db::list_object_links(conn)
        .map_err(|_| AssetSourceError::new("storage"))?
        .into_iter()
        .filter(|l| l.source_object_id == book_id && l.link_type == COVER)
        .collect();
    let desired = image_id.as_ref().map(|id| canonical_link_id(book_id, id));
    let already = desired
        .as_ref()
        .and_then(|id| existing.iter().find(|l| &l.id == id))
        .is_some()
        && existing.len() == 1;
    if already {
        return Ok(BookCoverMutation {
            changed: false,
            book: Some(book),
            image,
            ..Default::default()
        });
    }
    conn.execute_batch("SAVEPOINT canonical_cover")
        .map_err(|_| AssetSourceError::new("storage"))?;
    let result = (|| {
        let mut mutation = BookCoverMutation {
            changed: true,
            book: Some(book.clone()),
            image: image.clone(),
            ..Default::default()
        };

        for link in existing {
            db::delete_object_link(conn, &link.id).map_err(|_| AssetSourceError::new("storage"))?;
            mutation.deleted_link_ids.push(link.id);
        }
        if let (Some(id), Some(link_id)) = (image_id, desired) {
            let link = ObjectLink {
                id: link_id,
                source_object_id: book_id.into(),
                target_object_id: id,
                link_type: COVER.into(),
                created_at: now(device_id),
            };
            db::upsert_object_link(conn, &link).map_err(|_| AssetSourceError::new("storage"))?;
            mutation.links.push(link);
        }
        Ok::<_, AssetSourceError>(mutation)
    })();
    match result {
        Ok(mutation) => {
            conn.execute_batch("RELEASE canonical_cover")
                .map_err(|_| AssetSourceError::new("storage"))?;
            Ok(mutation)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK TO canonical_cover; RELEASE canonical_cover");
            Err(error)
        }
    }
}

/// Canonical ingress for compatibility Todo/Project/Tag writes.  Adapters may
/// construct the legacy wire record, but persistence, HLC and link state stay
/// owned by this facade.
pub fn write_legacy_records(
    conn: &Connection,
    records: &[crate::canonical_types::compatibility::LegacyRecord],
    source_kind: &str,
    device_id: Option<String>,
) -> Result<Vec<SyncEntity>, String> {
    use crate::canonical_types::compatibility::{
        map_legacy_with_context, CanonicalIdentity, LegacySource, MappingContext,
    };
    use std::collections::BTreeMap;
    let mut existing_object_ids = BTreeMap::new();
    for object in db::list_objects(conn)? {
        existing_object_ids.insert(
            object.id,
            CanonicalIdentity::new(object.type_id, object.type_version),
        );
    }
    for record in records {
        let type_id = match record.legacy_type_id.as_str() {
            "task_obj" => "com.kosmos.task",
            "project_obj" => "com.kosmos.project",
            "tag_obj" => "com.kosmos.tag",
            _ => continue,
        };
        existing_object_ids.insert(record.id.clone(), CanonicalIdentity::new(type_id, VERSION));
    }
    let context = MappingContext {
        existing_object_ids,
        existing_links: db::list_object_links(conn)?,
    };
    let mapped = records
        .iter()
        .map(|record| {
            let raw = serde_json::to_vec(record).map_err(|e| e.to_string())?;
            map_legacy_with_context(
                LegacySource {
                    source_kind,
                    source_id: &record.id,
                    raw_json: &raw,
                },
                &context,
            )
            .map_err(|e| format!("{}:{}:{}", e.code(), e.source_id(), e.pointer()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let write_device = device_id.as_deref().unwrap_or("ark-core");
    let mut entities = Vec::new();
    for item in mapped {
        let id = item.object.id.clone();
        db::upsert_object(conn, &item.object)?;
        let hlc = db::bump_sync_version_vector(conn, "object", &id, write_device, false)?;
        conn.execute(
            "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE excluded.hlc > object_sync_versions.hlc",
            params![id, hlc],
        )
        .map_err(|e| e.to_string())?;
        entities.push(SyncEntity {
            entity_type: "object".into(),
            id: id.clone(),
            data: serde_json::to_value(&item.object)
                .map_err(|e| e.to_string())?
                .as_object()
                .cloned()
                .unwrap_or_default(),
            hlc,
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        });
        for link in item.links {
            let link_id = link.id.clone();
            db::upsert_object_link(conn, &link)?;
            let hlc =
                db::bump_sync_version_vector(conn, "object_link", &link_id, write_device, false)?;
            entities.push(SyncEntity {
                entity_type: "object_link".into(),
                id: link_id,
                data: serde_json::to_value(&link)
                    .map_err(|e| e.to_string())?
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
                hlc,
                deleted: None,
                origin_device_id: None,
                origin_seq: None,
            });
        }
    }
    Ok(entities)
}

pub fn delete_legacy_object(
    conn: &Connection,
    id: &str,
    expected_type_id: &str,
    device_id: Option<String>,
) -> Result<SyncEntity, String> {
    let mut object = db::get_object(conn, id)?.ok_or_else(|| "MISSING_OBJECT".to_string())?;
    if object.type_id != expected_type_id {
        return Err("WRONG_TARGET".into());
    }
    let write_device = device_id.as_deref().unwrap_or("ark-core");
    object.deleted_at = Some(HLC::now(write_device).wall_time);
    db::upsert_object(conn, &object)?;
    let hlc = db::bump_sync_version_vector(conn, "object", id, write_device, true)?;
    conn.execute(
        "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,1) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=1 WHERE excluded.hlc > object_sync_versions.hlc",
        params![id, hlc],
    )
    .map_err(|e| e.to_string())?;
    db::record_sync_tombstone(conn, "object", id, &hlc)?;
    Ok(SyncEntity {
        entity_type: "object".into(),
        id: id.into(),
        data: Default::default(),
        hlc,
        deleted: Some(true),
        origin_device_id: None,
        origin_seq: None,
    })
}

pub fn apply_legacy_compat_entity(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
    let current: Option<String> = conn
        .query_row(
            "SELECT hlc FROM object_sync_versions WHERE object_id=?1",
            params![entity.id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if current
        .as_deref()
        .is_some_and(|hlc| HLC::compare_str(&entity.hlc, hlc) != std::cmp::Ordering::Greater)
    {
        return Ok(());
    }
    let legacy_type = match entity.entity_type.as_str() {
        "todo" => "task_obj",
        "project" => "project_obj",
        "tag" => "tag_obj",
        "area" | "heading" => return Err("LegacyPlanningReadOnly".into()),
        _ => return Err(format!("unknown sync entity type '{}'", entity.entity_type)),
    };
    let mut props = entity.data.clone();
    if let Some(title) = props.get("title") {
        if !title.is_string() {
            return Err("invalid type at /title".into());
        }
    }
    let title = props
        .remove("title")
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    props.retain(|key, value| {
        !value.is_null()
            && !(matches!(
                key.as_str(),
                "is_completed"
                    | "isCompleted"
                    | "is_cancelled"
                    | "isCancelled"
                    | "is_someday"
                    | "isSomeday"
            ) && value.as_bool() == Some(false))
    });
    let created_at = props
        .get("createdAt")
        .or_else(|| props.get("created_at"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let updated_at = props
        .get("updatedAt")
        .or_else(|| props.get("updated_at"))
        .and_then(Value::as_str)
        .unwrap_or(&entity.hlc)
        .to_owned();
    // Historical planning rows did not carry rich-text content.  The
    // compatibility contract supplies the canonical empty document rather
    // than rejecting an otherwise valid legacy wire payload.
    let source = json!({"id": entity.id, "legacy_type_id": legacy_type, "title": title, "content": {"type":"doc","content":[{"type":"paragraph"}]}, "props": props, "created_at": created_at, "updated_at": updated_at, "deleted_at": null});
    let mapped = crate::canonical_types::compatibility::map_legacy_source(
        crate::canonical_types::compatibility::LegacySource {
            source_kind: &entity.entity_type,
            source_id: &entity.id,
            raw_json: source.to_string().as_bytes(),
        },
    )
    .map_err(|e| format!("invalid type at {}", e.pointer()))?;
    db::upsert_object(conn, &mapped.object)?;
    for link in mapped.links {
        db::upsert_object_link(conn, &link)?;
    }
    conn.execute("INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE excluded.hlc > object_sync_versions.hlc", params![entity.id, entity.hlc]).map_err(|e| e.to_string())?;
    db::delete_sync_tombstone(conn, &entity.id)
}

pub fn apply_canonical_object_entity(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
    let current: Option<String> = conn
        .query_row(
            "SELECT hlc FROM object_sync_versions WHERE object_id=?1",
            params![entity.id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if current
        .as_deref()
        .is_some_and(|hlc| HLC::compare_str(&entity.hlc, hlc) != std::cmp::Ordering::Greater)
    {
        return Ok(());
    }
    conn.execute_batch("SAVEPOINT canonical_sync_object")
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        if entity.deleted == Some(true) {
            db::delete_object(conn, &entity.id)?;
            conn.execute(
                    "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,1) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=1 WHERE excluded.hlc > object_sync_versions.hlc",
                    params![entity.id, entity.hlc],
                ).map_err(|e| e.to_string())?;
            db::upsert_sync_tombstone(conn, entity)?;
            return Ok(());
        }
        let mut data = entity.data.clone();
        data.insert("id".into(), Value::String(entity.id.clone()));
        let input: ArkObjectWrite =
            serde_json::from_value(Value::Object(data.clone())).map_err(|e| e.to_string())?;
        let input_type_id = input.type_id.clone();
        let input_type_version = input.type_version.clone();
        let canonical_alias = match input_type_id.as_str() {
            "note_obj" => Some("com.kosmos.note"),
            "task_obj" => Some("com.kosmos.task"),
            "project_obj" => Some("com.kosmos.project"),
            "tag_obj" => Some("com.kosmos.tag"),
            "person_obj" => Some("com.kosmos.person"),
            "image_obj" => Some("com.kosmos.image"),
            "time_entry_obj" => Some("com.kosmos.time-entry"),
            "game_obj" => Some("com.kosmos.game"),
            "book_obj" => Some("com.kosmos.book"),
            _ => None,
        };
        let object = if let Some(canonical_type_id) = canonical_alias {
            let props = input.props_json.clone();
            let source = serde_json::json!({
                "id": input.id,
                "legacy_type_id": input.type_id,
                "title": input.title,
                "content": input.content_json,
                "props": props,
                "created_at": input.created_at,
                "updated_at": input.updated_at,
                "deleted_at": input.deleted_at,
            });
            let mapped = crate::canonical_types::compatibility::map_legacy_source(
                crate::canonical_types::compatibility::LegacySource {
                    source_kind: "object",
                    source_id: &entity.id,
                    raw_json: source.to_string().as_bytes(),
                },
            )
            .map_err(|e| format!("{} at {}", e.code(), e.pointer()))?;
            debug_assert_eq!(mapped.object.type_id, canonical_type_id);
            mapped.object
        } else if crate::canonical_types::definitions::canonical_type_registrations()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|registration| registration.type_id == input_type_id)
        {
            crate::canonical_types::ingress::prepare_object(conn, input)
                .map_err(|e| e.to_string())?
        } else {
            // Generic Phase 2 objects have no canonical content contract.
            // Preserve the wire version exactly; resolving a legacy wire
            // version to a synthetic compatibility hash here changes the
            // pending/version contract.
            input.with_type_version(
                input_type_version
                    .unwrap_or_else(|| crate::type_registry::LEGACY_VERSION.to_string()),
            )
        };
        db::upsert_object(conn, &object)?;
        if let Some(raw_links) = data.get("links") {
            let links: Vec<ObjectLink> =
                serde_json::from_value(raw_links.clone()).map_err(|e| e.to_string())?;
            for link in links {
                db::upsert_object_link(conn, &link)?;
            }
        }
        conn.execute(
                "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE excluded.hlc > object_sync_versions.hlc",
                params![object.id, entity.hlc],
            ).map_err(|e| e.to_string())?;
        db::delete_sync_tombstone(conn, &entity.id)
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE canonical_sync_object")
            .map_err(|e| e.to_string()),
        Err(error) => {
            let _ = conn
                .execute_batch("ROLLBACK TO canonical_sync_object; RELEASE canonical_sync_object");
            Err(error)
        }
    }
}
