use super::{ObjectPlanError, PlannedItem};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub fn stable_link_id(source: &str, kind: &str, target: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"kosmos-link-v1\0");
    h.update(source.as_bytes());
    h.update(b"\0");
    h.update(kind.as_bytes());
    h.update(b"\0");
    h.update(target.as_bytes());
    format!("lnk*{:x}", h.finalize())
}

fn canonical_json(v: &Value) -> Value {
    match v {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), canonical_json(value)))
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

fn json_bytes(value: &Value) -> Result<Vec<u8>, ObjectPlanError> {
    serde_json::to_vec(&canonical_json(value))
        .map_err(|error| ObjectPlanError::Storage(error.to_string()))
}

fn digest(item: &PlannedItem) -> Result<String, ObjectPlanError> {
    let value = serde_json::json!({
        "object": item.mapped.object,
        "links": item.mapped.links,
        "local": item.mapped.local_state,
        "quarantine": item.mapped.quarantine,
    });
    Ok(format!("{:x}", Sha256::digest(json_bytes(&value)?)))
}

fn same_json(left: &str, right: &Value) -> Result<bool, ObjectPlanError> {
    let parsed: Value =
        serde_json::from_str(left).map_err(|error| ObjectPlanError::Storage(error.to_string()))?;
    Ok(canonical_json(&parsed) == canonical_json(right))
}

pub(crate) fn apply_one(
    conn: &Connection,
    item: &PlannedItem,
    _now: &str,
) -> Result<String, ObjectPlanError> {
    let mapped = &item.mapped;
    let canonical_hash = digest(item)?;
    let object = &mapped.object;

    if let Some((type_id, version, title, content, props, created, updated, deleted)) = conn
        .query_row(
            concat!(
                "SELECT type_id,type_version,title,content_json,props_json,created_at,",
                "updated_at,deleted_at FROM objects WHERE id=?1",
            ),
            [object.id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            },
        )
        .optional()
        .map_err(|error| ObjectPlanError::Storage(error.to_string()))?
    {
        let legacy_upgrade = matches!(
            (type_id.as_str(), object.type_id.as_str()),
            ("note_obj", "com.kosmos.note")
                | ("task_obj", "com.kosmos.task")
                | ("project_obj", "com.kosmos.project")
                | ("tag_obj", "com.kosmos.tag")
                | ("person_obj", "com.kosmos.person")
                | ("image_obj", "com.kosmos.image")
                | ("time_entry_obj", "com.kosmos.time-entry")
                | ("game_obj", "com.kosmos.game")
                | ("book_obj", "com.kosmos.book")
        );
        let exact = (type_id == object.type_id || legacy_upgrade)
            && version == object.type_version
            && title == object.title
            && same_json(&content, &object.content_json)?
            && same_json(&props, &object.props_json)?
            && created == object.created_at
            && updated == object.updated_at
            && deleted == object.deleted_at;
        if legacy_upgrade {
            conn.execute(
                concat!(
                    "UPDATE objects SET type_id=?2,type_version=?3,title=?4,content_json=?5,",
                    "props_json=?6,created_at=?7,updated_at=?8,deleted_at=?9 WHERE id=?1",
                ),
                params![
                    object.id,
                    object.type_id,
                    object.type_version,
                    object.title,
                    String::from_utf8(
                        json_bytes(&object.content_json)?
                    ).map_err(|error| ObjectPlanError::Storage(error.to_string()))?,
                    String::from_utf8(
                        json_bytes(&object.props_json)?
                    ).map_err(|error| ObjectPlanError::Storage(error.to_string()))?,
                    object.created_at,
                    object.updated_at,
                    object.deleted_at,
                ],
            ).map_err(|error| ObjectPlanError::Storage(error.to_string()))?;
        } else if !exact {
            return Err(ObjectPlanError::CanonicalConflict {
                source_kind: item.source_kind.clone(),
                source_id: item.source_id.clone(),
            });
        }
    } else {
        conn.execute(
            concat!(
                "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
                "created_at,updated_at,deleted_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            ),
            params![
                object.id,
                object.type_id,
                object.type_version,
                object.title,
                String::from_utf8(
                    json_bytes(&object.content_json)?
                ).map_err(|error| ObjectPlanError::Storage(error.to_string()))?,
                String::from_utf8(
                    json_bytes(&object.props_json)?
                ).map_err(|error| ObjectPlanError::Storage(error.to_string()))?,
                object.created_at,
                object.updated_at,
                object.deleted_at,
            ],
        )
        .map_err(|error| ObjectPlanError::Storage(error.to_string()))?;
    }

    for link in &mapped.links {
        let deterministic_id = stable_link_id(
            &link.source_object_id,
            &link.link_type,
            &link.target_object_id,
        );
        let equivalent = conn
            .query_row(
                concat!(
                    "SELECT id FROM object_links WHERE source_object_id=?1 AND link_type=?2 AND ",
                    "target_object_id=?3",
                ),
                params![link.source_object_id, link.link_type, link.target_object_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| ObjectPlanError::Storage(error.to_string()))?;
        if equivalent.is_some() {
            continue;
        }
        if let Some(existing) = conn
            .query_row(
                "SELECT source_object_id,link_type,target_object_id FROM object_links WHERE id=?1",
                [deterministic_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| ObjectPlanError::Storage(error.to_string()))?
        {
            if existing
                != (
                    link.source_object_id.clone(),
                    link.link_type.clone(),
                    link.target_object_id.clone(),
                )
            {
                return Err(ObjectPlanError::CanonicalConflict {
                    source_kind: item.source_kind.clone(),
                    source_id: item.source_id.clone(),
                });
            }
            continue;
        }
        conn.execute(
            concat!(
                "INSERT INTO object_links(id,source_object_id,target_object_id,link_type,",
                "created_at) VALUES(?1,?2,?3,?4,?5)",
            ),
            params![
                deterministic_id,
                link.source_object_id,
                link.target_object_id,
                link.link_type,
                link.created_at,
            ],
        )
        .map_err(|error| ObjectPlanError::Storage(error.to_string()))?;
    }

    for state in &mapped.local_state {
        let data = String::from_utf8(json_bytes(&state.data_json)?)
            .map_err(|error| ObjectPlanError::Storage(error.to_string()))?;
        conn.execute(
            concat!(
                "INSERT INTO object_local_state(object_id,device_id,data_json,updated_at) ",
                "VALUES(?1,?2,?3,?4) ON CONFLICT(object_id,device_id) DO UPDATE SET ",
                "data_json=excluded.data_json,updated_at=excluded.updated_at WHERE ",
                "data_json<>excluded.data_json",
            ),
            params![object.id, "migration", data, object.updated_at],
        )
        .map_err(|error| ObjectPlanError::Storage(error.to_string()))?;
    }

    if !mapped.quarantine.is_empty() {
        let fields = Value::Array(
            mapped
                .quarantine
                .iter()
                .map(|entry| entry.fields_json.clone())
                .collect(),
        );
        let fields = String::from_utf8(json_bytes(&fields)?)
            .map_err(|error| ObjectPlanError::Storage(error.to_string()))?;
        conn.execute(
            concat!(
                "INSERT INTO object_migration_quarantine(object_id,contract_version,",
                "source_type_id,fields_json,source_hash,updated_at) VALUES(?1,",
                "'phase3-canonical-v1',?2,?3,?4,?5) ON CONFLICT(object_id,contract_version) ",
                "DO UPDATE SET fields_json=excluded.fields_json,",
                "source_hash=excluded.source_hash,updated_at=excluded.updated_at WHERE ",
                "fields_json<>excluded.fields_json OR source_hash<>excluded.source_hash",
            ),
            params![object.id, object.type_id, fields, item.source_hash, object.updated_at],
        )
        .map_err(|error| ObjectPlanError::Storage(error.to_string()))?;
    }

    conn.execute(
        concat!(
            "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,?3) ON ",
            "CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=excluded.deleted ",
            "WHERE hlc<>excluded.hlc OR deleted<>excluded.deleted",
        ),
        params![object.id, object.updated_at, i64::from(object.deleted_at.is_some())],
    )
    .map_err(|error| ObjectPlanError::Storage(error.to_string()))?;
    Ok(canonical_hash)
}
