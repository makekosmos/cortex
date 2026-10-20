use crate::hlc::HLC;
use crate::types::SyncEntity;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub fn apply_legacy_compat_entity(
    conn: &Connection,
    entity: &SyncEntity,
) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
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
        return Ok(ark_application::sync_storage::InboundApplyOutcome::Stale);
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
        !(value.is_null()
            || matches!(
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
    let source = json!(
        {"id": entity.id,
        "legacy_type_id": legacy_type,
        "title": title,
        "content": {"type":"doc",
        "content":[{"type":"paragraph"}]},
        "props": props,
        "created_at": created_at,
        "updated_at": updated_at,
        "deleted_at": null});
    let mapped = ark_application::canonical_compatibility::map_legacy_source(
        ark_application::canonical_compatibility::LegacySource {
            source_kind: &entity.entity_type,
            source_id: &entity.id,
            raw_json: source.to_string().as_bytes(),
        },
    )
    .map_err(|e| format!("invalid type at {}", e.pointer()))?;
    if mapped.object.type_id == "core.task" {
        let session = ark_store::task_merge::SqliteTaskMutationSession::new(conn);
        let outcome = crate::application::task_mutation::apply_inbound_task_mutation(
            &session,
            crate::application::task_mutation::InboundTaskMutation::LegacyTodoSnapshot {
                object: &mapped.object,
                links: &mapped.links,
                source_hlc: &entity.hlc,
            },
        )
        .map_err(|error| error.to_string())?;
        return Ok(match outcome {
            crate::application::task_mutation::InboundTaskMutationOutcome::Applied => {
                ark_application::sync_storage::InboundApplyOutcome::Applied
            }
            crate::application::task_mutation::InboundTaskMutationOutcome::Stale => {
                ark_application::sync_storage::InboundApplyOutcome::Stale
            }
        });
    }
    ark_store::sync_entities::apply_inbound_object_snapshot(
        conn,
        &mapped.object,
        &mapped.links,
        &entity.hlc,
        ark_store::sync_entities::InboundObjectRevision::LegacyCompatibility,
    )
}
