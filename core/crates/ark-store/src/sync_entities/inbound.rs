use ark_application::sync_storage::{InboundApplyOutcome, ReplayNotice};
use ark_domain::types::{
    ArkObject, ObjectLink, ObjectType, SyncEntity, TrackedApp, UsageDay, UsageEvent, UsageSession,
};
use rusqlite::Connection;
use serde_json::Value;

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
    let canonical_type = entity.entity_type.as_str();
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
    if entity.entity_type == "object" && entity.deleted == Some(true) {
        return crate::sync_entities::apply_canonical_object_entity(connection, entity)
            .map(|outcome| (outcome, Vec::new()));
    }
    if entity.deleted == Some(true) {
        match entity.entity_type.as_str() {
            "tracked_app" => crate::usage::delete_tracked_app(connection, &entity.id),
            "usage_session" => crate::usage::delete_usage_session(connection, &entity.id),
            "usage_event" => crate::usage::delete_usage_event(connection, &entity.id),
            "usage_day" => crate::usage::delete_usage_day(connection, &entity.id),
            "object_type" => crate::type_registry::delete_object_type(connection, &entity.id),
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
