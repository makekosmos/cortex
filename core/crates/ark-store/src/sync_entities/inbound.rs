use ark_application::sync_storage::{InboundApplyOutcome, ReplayNotice};
use ark_domain::hlc::HLC;
use ark_domain::types::{
    ArkObject, ObjectLink, ObjectType, SyncEntity, TrackedApp, UsageDay, UsageEvent, UsageSession,
};
use rusqlite::Connection;
use serde_json::{json, Value};

/// Applies an inbound entity using Store-owned SQL mappings. The caller owns
/// the immediate transaction that also records its receipt and feed entry.
pub fn apply_inbound_entity(
    connection: &Connection,
    entity: &SyncEntity,
) -> Result<(InboundApplyOutcome, Vec<ReplayNotice>), String> {
    let sequenced_usage = ark_domain::types::is_sequenced_usage_entity(&entity.entity_type);
    if sequenced_usage && !crate::usage_sync_log::usage_entity_is_newer(connection, entity)? {
        let (device_id, seq) = crate::usage_sync_log::usage_origin_for_entity(connection, entity)?;
        crate::usage_sync_log::record_usage_sequence(
            connection,
            &entity.entity_type,
            &entity.id,
            &device_id,
            seq,
            &entity.hlc,
            entity.deleted == Some(true),
        )?;
        return Ok((InboundApplyOutcome::Stale, Vec::new()));
    }
    if matches!(entity.entity_type.as_str(), "area" | "heading") {
        return Err("LegacyPlanningReadOnly".into());
    }

    let canonical_type = if matches!(
        entity.entity_type.as_str(),
        "object" | "todo" | "project" | "tag"
    ) {
        "object"
    } else {
        &entity.entity_type
    };
    let deleted = entity.deleted == Some(true);
    let (outcome, notices) = apply_mutation(connection, entity)?;
    if outcome == InboundApplyOutcome::Applied {
        crate::committed_feed::append_committed_change(
            connection,
            canonical_type,
            &entity.id,
            &entity.hlc,
            deleted,
            entity.origin_device_id.as_deref(),
            entity.origin_seq,
        )?;
    }
    Ok((outcome, notices))
}

fn apply_mutation(
    connection: &Connection,
    entity: &SyncEntity,
) -> Result<(InboundApplyOutcome, Vec<ReplayNotice>), String> {
    let sequenced_usage = ark_domain::types::is_sequenced_usage_entity(&entity.entity_type);
    if entity.entity_type == "todo" && entity.deleted == Some(true) {
        let mut canonical = entity.clone();
        canonical.entity_type = "object".into();
        return crate::sync_entities::apply_remote_task_delete(connection, &canonical)
            .map(|outcome| (outcome, Vec::new()));
    }
    if matches!(entity.entity_type.as_str(), "project" | "tag") && entity.deleted == Some(true) {
        let mut canonical = entity.clone();
        canonical.entity_type = "object".into();
        return crate::sync_entities::apply_canonical_object_entity(connection, &canonical)
            .map(|outcome| (outcome, Vec::new()));
    }
    if entity.entity_type == "object" && entity.deleted == Some(true) {
        return crate::sync_entities::apply_canonical_object_entity(connection, entity)
            .map(|outcome| (outcome, Vec::new()));
    }
    if entity.deleted == Some(true) {
        match entity.entity_type.as_str() {
            "todo" => crate::legacy_entities::delete_todo(connection, &entity.id),
            "project" => crate::legacy_entities::delete_project(connection, &entity.id),
            "tag" => crate::legacy_entities::delete_tag(connection, &entity.id),
            "tracked_app" => crate::usage::delete_tracked_app(connection, &entity.id),
            "usage_session" => crate::usage::delete_usage_session(connection, &entity.id),
            "usage_event" => crate::usage::delete_usage_event(connection, &entity.id),
            "usage_day" => crate::usage::delete_usage_day(connection, &entity.id),
            "object_type" => crate::type_registry::delete_object_type(connection, &entity.id),
            "object" => crate::objects::delete_object(connection, &entity.id),
            "object_link" => crate::objects::delete_object_link(connection, &entity.id),
            _ => Err(format!("unknown sync entity type '{}'", entity.entity_type)),
        }?;
        crate::sync_tombstones::upsert(connection, entity)?;
        if sequenced_usage {
            crate::usage_sync_log::finish_usage_entity_sync(connection, entity)?;
        }
        return Ok((InboundApplyOutcome::Applied, Vec::new()));
    }

    let mut full_data = entity.data.clone();
    full_data.insert("id".to_owned(), Value::String(entity.id.clone()));
    let value = Value::Object(full_data);
    let mut notices = Vec::new();
    let mut outcome = InboundApplyOutcome::Applied;
    match entity.entity_type.as_str() {
        "todo" | "project" | "tag" => {
            return apply_legacy_compat_entity(connection, entity)
                .map(|outcome| (outcome, Vec::new()));
        }
        "area" | "heading" => return Err("LegacyPlanningReadOnly".into()),
        "tracked_app" => serde_json::from_value::<TrackedApp>(value)
            .map_err(|e| e.to_string())
            .and_then(|value| crate::usage::upsert_tracked_app(connection, &value))?,
        "usage_session" => serde_json::from_value::<UsageSession>(value)
            .map_err(|e| e.to_string())
            .and_then(|value| crate::usage::upsert_usage_session(connection, &value))?,
        "usage_event" => serde_json::from_value::<UsageEvent>(value)
            .map_err(|e| e.to_string())
            .and_then(|value| crate::usage::upsert_usage_event(connection, &value))?,
        "usage_day" => serde_json::from_value::<UsageDay>(value)
            .map_err(|e| e.to_string())
            .and_then(|value| crate::usage::upsert_usage_day(connection, &value))?,
        "object_type" => {
            let value = serde_json::from_value::<ObjectType>(value).map_err(|e| e.to_string())?;
            notices = crate::type_registry::upsert_object_type(connection, &value)?;
        }
        "object" => {
            let mut object_data = match value {
                Value::Object(map) => map,
                _ => return Err("object payload must be an object".into()),
            };
            object_data
                .entry("typeVersion".to_owned())
                .or_insert_with(|| Value::String("0.0.0-legacy".into()));
            let object = serde_json::from_value::<ArkObject>(Value::Object(object_data))
                .map_err(|e| e.to_string())?;
            if !crate::object_types::is_definition_known(
                connection,
                &object.type_id,
                &object.type_version,
            )? {
                crate::pending_objects::insert_pending_object(connection, entity, &object.type_id)?;
                outcome = InboundApplyOutcome::Pending;
            } else {
                outcome = crate::sync_entities::apply_canonical_object_entity(connection, entity)?;
            }
        }
        "object_link" => {
            let link = serde_json::from_value::<ObjectLink>(value).map_err(|e| e.to_string())?;
            crate::objects::upsert_object_link(connection, &link)?;
        }
        _ => return Err(format!("unknown sync entity type '{}'", entity.entity_type)),
    }
    if outcome != InboundApplyOutcome::Pending {
        crate::sync_tombstones::delete(connection, &entity.id)?;
        if sequenced_usage {
            crate::usage_sync_log::finish_usage_entity_sync(connection, entity)?;
        }
    }
    Ok((outcome, notices))
}

fn apply_legacy_compat_entity(
    connection: &Connection,
    entity: &SyncEntity,
) -> Result<InboundApplyOutcome, String> {
    let current = crate::sync_versions::stored_object_revision(connection, &entity.id)?;
    if current
        .as_deref()
        .is_some_and(|hlc| HLC::compare_str(&entity.hlc, hlc) != std::cmp::Ordering::Greater)
    {
        return Ok(InboundApplyOutcome::Stale);
    }
    let legacy_type = match entity.entity_type.as_str() {
        "todo" => "task_obj",
        "project" => "project_obj",
        "tag" => "tag_obj",
        "area" | "heading" => return Err("LegacyPlanningReadOnly".into()),
        _ => return Err(format!("unknown sync entity type '{}'", entity.entity_type)),
    };
    let mut props = entity.data.clone();
    if props.get("title").is_some_and(|title| !title.is_string()) {
        return Err("invalid type at /title".into());
    }
    let title = props
        .remove("title")
        .and_then(|value| value.as_str().map(str::to_owned))
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
        .unwrap_or("");
    let updated_at = props
        .get("updatedAt")
        .or_else(|| props.get("updated_at"))
        .and_then(Value::as_str)
        .unwrap_or(&entity.hlc);
    let source = json!({
        "id": entity.id,
        "legacy_type_id": legacy_type,
        "title": title,
        "content": {"type":"doc", "content":[{"type":"paragraph"}]},
        "props": props,
        "created_at": created_at,
        "updated_at": updated_at,
        "deleted_at": null
    });
    let mapped = ark_application::canonical_compatibility::map_legacy_source(
        ark_application::canonical_compatibility::LegacySource {
            source_kind: &entity.entity_type,
            source_id: &entity.id,
            raw_json: source.to_string().as_bytes(),
        },
    )
    .map_err(|error| format!("invalid type at {}", error.pointer()))?;
    if mapped.object.type_id == "core.task" {
        let session = crate::task_merge::SqliteTaskMutationSession::new(connection);
        let outcome = ark_application::task_mutation::apply_inbound_task_mutation(
            &session,
            ark_application::task_mutation::InboundTaskMutation::LegacyTodoSnapshot {
                object: &mapped.object,
                links: &mapped.links,
                source_hlc: &entity.hlc,
            },
        )
        .map_err(|error| error.to_string())?;
        return Ok(match outcome {
            ark_application::task_mutation::InboundTaskMutationOutcome::Applied => {
                InboundApplyOutcome::Applied
            }
            ark_application::task_mutation::InboundTaskMutationOutcome::Stale => {
                InboundApplyOutcome::Stale
            }
        });
    }
    crate::sync_entities::apply_inbound_object_snapshot(
        connection,
        &mapped.object,
        &mapped.links,
        &entity.hlc,
        crate::sync_entities::InboundObjectRevision::LegacyCompatibility,
    )
}
