use ark_domain::hlc::HLC;
use ark_domain::types::{ArkObjectWrite, ObjectLink, SyncEntity};
use rusqlite::{params, Connection};
use serde_json::Value;

use super::InboundObjectRevision;

pub fn apply_canonical_object_entity(
    connection: &Connection,
    entity: &SyncEntity,
) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
    let type_id = entity
        .data
        .get("typeId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .map(Some)
        .unwrap_or(
            crate::objects::get_object(connection, &entity.id)?.map(|object| object.type_id),
        );
    let task_session = crate::task_merge::SqliteTaskMutationSession::new(connection);
    if entity.deleted == Some(true) && type_id.as_deref() == Some("core.task") {
        return apply_remote_task_delete(connection, entity);
    }
    let current = crate::sync_versions::object_revision(connection, &entity.id)?;
    if current
        .as_deref()
        .is_some_and(|hlc| HLC::compare_str(&entity.hlc, hlc) != std::cmp::Ordering::Greater)
    {
        return Ok(ark_application::sync_storage::InboundApplyOutcome::Stale);
    }
    connection
        .execute_batch("SAVEPOINT canonical_sync_object")
        .map_err(|error| error.to_string())?;
    let result = (|| {
        if entity.deleted == Some(true) {
            crate::objects::delete_object(connection, &entity.id)?;
            connection
                .execute(
                    concat!(
                        "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,1) ",
                        "ON ",
                        "CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=1 WHERE ",
                        "excluded.hlc > object_sync_versions.hlc"
                    ),
                    params![entity.id, entity.hlc],
                )
                .map_err(|error| error.to_string())?;
            crate::sync_tombstones::upsert(connection, entity)?;
            return Ok(None);
        }

        let mut data = entity.data.clone();
        data.insert("id".into(), Value::String(entity.id.clone()));
        let input: ArkObjectWrite = serde_json::from_value(Value::Object(data.clone()))
            .map_err(|error| error.to_string())?;
        let input_type_id = input.type_id.clone();
        let input_type_version = input.type_version.clone();
        let canonical_alias = match input_type_id.as_str() {
            "note_obj" => Some("com.kosmos.note"),
            "task_obj" => Some("core.task"),
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
            let source = serde_json::json!({
                "id": input.id,
                "legacy_type_id": input.type_id,
                "title": input.title,
                "content": input.content_json,
                "props": input.props_json,
                "created_at": input.created_at,
                "updated_at": input.updated_at,
                "deleted_at": input.deleted_at,
            });
            let mapped = ark_application::canonical_compatibility::map_legacy_source(
                ark_application::canonical_compatibility::LegacySource {
                    source_kind: "object",
                    source_id: &entity.id,
                    raw_json: source.to_string().as_bytes(),
                },
            )
            .map_err(|error| format!("{} at {}", error.code(), error.pointer()))?;
            debug_assert_eq!(mapped.object.type_id, canonical_type_id);
            mapped.object
        } else if ark_domain::canonical_type::definitions::canonical_type_registrations()
            .map_err(|error| error.to_string())?
            .iter()
            .any(|registration| registration.type_id == input_type_id)
        {
            crate::object_write::prepare_object(connection, input)
                .map_err(|error| error.to_string())?
        } else {
            // Preserve the wire version for generic Phase 2 objects.
            input.with_type_version(input_type_version.unwrap_or_else(|| {
                ark_domain::canonical_type::registry::LEGACY_VERSION.to_string()
            }))
        };

        let links: Vec<ObjectLink> = data
            .get("links")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        if object.type_id == "core.task" {
            let outcome = ark_application::task_mutation::apply_inbound_task_mutation(
                &task_session,
                ark_application::task_mutation::InboundTaskMutation::CanonicalEntitySnapshot {
                    object: &object,
                    links: &links,
                    source_hlc: &entity.hlc,
                },
            )
            .map_err(|error| error.to_string())?;
            return Ok(Some(match outcome {
                ark_application::task_mutation::InboundTaskMutationOutcome::Applied => {
                    ark_application::sync_storage::InboundApplyOutcome::Applied
                }
                ark_application::task_mutation::InboundTaskMutationOutcome::Stale => {
                    ark_application::sync_storage::InboundApplyOutcome::Stale
                }
            }));
        }
        let outcome = crate::sync_entities::apply_inbound_object_snapshot(
            connection,
            &object,
            &links,
            &entity.hlc,
            InboundObjectRevision::Canonical,
        )?;
        Ok(
            (outcome != ark_application::sync_storage::InboundApplyOutcome::Applied)
                .then_some(outcome),
        )
    })();

    match result {
        Ok(outcome) => connection
            .execute_batch("RELEASE canonical_sync_object")
            .map(|()| {
                outcome.unwrap_or(ark_application::sync_storage::InboundApplyOutcome::Applied)
            })
            .map_err(|error| error.to_string()),
        Err(error) => {
            let _ = connection
                .execute_batch("ROLLBACK TO canonical_sync_object; RELEASE canonical_sync_object");
            Err(error)
        }
    }
}

pub fn apply_remote_task_delete(
    connection: &Connection,
    entity: &SyncEntity,
) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
    connection
        .execute_batch("SAVEPOINT canonical_sync_task_delete")
        .map_err(|error| error.to_string())?;
    let session = crate::task_merge::SqliteTaskMutationSession::new(connection);
    let result = ark_application::task_mutation::apply_inbound_task_mutation(
        &session,
        ark_application::task_mutation::InboundTaskMutation::RemoteHardDelete { entity },
    )
    .map_err(|error| error.to_string());
    match result {
        Ok(outcome) => {
            connection
                .execute_batch("RELEASE canonical_sync_task_delete")
                .map_err(|error| error.to_string())?;
            Ok(match outcome {
                ark_application::task_mutation::InboundTaskMutationOutcome::Applied => {
                    ark_application::sync_storage::InboundApplyOutcome::Applied
                }
                ark_application::task_mutation::InboundTaskMutationOutcome::Stale => {
                    ark_application::sync_storage::InboundApplyOutcome::Stale
                }
            })
        }
        Err(error) => {
            let _ = connection.execute_batch(
                "ROLLBACK TO canonical_sync_task_delete; RELEASE canonical_sync_task_delete",
            );
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply_canonical_object_entity;
    use crate::schema::CREATE_TABLES;
    use ark_application::sync_storage::InboundApplyOutcome;
    use ark_domain::types::SyncEntity;
    use rusqlite::Connection;
    use serde_json::{json, Map};

    #[test]
    fn canonical_entity_apply_keeps_wire_version_for_generic_types() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(CREATE_TABLES)
            .expect("create schema");
        connection
            .execute_batch(
                "CREATE TABLE object_sync_versions (
                    object_id TEXT PRIMARY KEY,
                    hlc TEXT NOT NULL,
                    deleted INTEGER NOT NULL DEFAULT 0
                );",
            )
            .expect("create object sync versions");
        connection
            .execute(
                concat!(
                    "INSERT INTO object_types(id,name,schema_json,ui_schema_json,",
                    "created_at,updated_at) VALUES('custom','Custom','{}','{}','now','now')"
                ),
                [],
            )
            .expect("insert custom type");
        let entity = SyncEntity {
            entity_type: "object".into(),
            id: "custom-1".into(),
            data: Map::from_iter([
                ("typeId".into(), json!("custom")),
                ("typeVersion".into(), json!("2.3.4")),
                ("title".into(), json!("Custom object")),
                ("props".into(), json!({})),
                ("links".into(), json!([])),
                ("createdAt".into(), json!("2026-10-20T10:00:00.000Z")),
                ("updatedAt".into(), json!("2026-10-20T10:00:00.000Z")),
            ]),
            hlc: "2026-10-20T10:00:00.000Z-0000000000@device".into(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };
        assert_eq!(
            apply_canonical_object_entity(&connection, &entity).expect("apply object"),
            InboundApplyOutcome::Applied
        );
        let version: String = connection
            .query_row(
                "SELECT type_version FROM objects WHERE id='custom-1'",
                [],
                |row| row.get(0),
            )
            .expect("read persisted version");
        assert_eq!(version, "2.3.4");
    }
}
