#[cfg(test)]
use rusqlite::TransactionBehavior;
use rusqlite::{params, Connection, Transaction};
use serde_json::{json, Value};

use super::SqliteStorageBackend;
use crate::db::{
    delete_object, delete_object_link, delete_object_type, delete_project, delete_sync_tombstone,
    delete_todo, delete_tracked_app, delete_usage_day, delete_usage_event, delete_usage_session,
    finish_usage_entity_sync, insert_pending_object, is_object_definition_known,
    is_sequenced_usage_entity, record_usage_sequence, upsert_object_link, upsert_object_type,
    upsert_sync_tombstone, upsert_tracked_app, upsert_usage_day, upsert_usage_event,
    upsert_usage_session, usage_entity_is_newer, usage_origin_for_entity,
};
use crate::types::{
    ArkObject, ObjectLink, ObjectType, SyncEntity, TrackedApp, UsageDay, UsageEvent, UsageSession,
};

// ---------------------------------------------------------------------------
// Delete helpers (for StorageBackend)
// ---------------------------------------------------------------------------

pub fn delete_tag(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM tags WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

impl SqliteStorageBackend {
    #[cfg(test)]
    pub(in crate::db) fn apply_entity_blocking(
        conn: &Connection,
        entity: &SyncEntity,
    ) -> Result<(), String> {
        let transaction = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;
        let (_, replay_notices) = Self::apply_entity_in_transaction(&transaction, entity)?;
        transaction.commit().map_err(|error| error.to_string())?;
        for notice in replay_notices {
            crate::events::emit_event(notice.event_payload());
        }
        Ok(())
    }

    pub(in crate::db) fn apply_entity_in_transaction(
        transaction: &Transaction<'_>,
        entity: &SyncEntity,
    ) -> Result<
        (
            crate::application::sync_storage::InboundApplyOutcome,
            Vec<crate::canonical_types::pending::ReplayNotice>,
        ),
        String,
    > {
        Self::apply_entity_blocking_inner(transaction, entity)
    }

    pub(in crate::db) fn apply_authenticated_entity_in_transaction(
        transaction: &Transaction<'_>,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
    ) -> Result<
        (
            crate::application::sync_storage::InboundApplyOutcome,
            Vec<crate::canonical_types::pending::ReplayNotice>,
        ),
        String,
    > {
        let removed: Vec<String> = crate::db::get_sync_kv(transaction, "sync.removed_peers")?
            .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
            .transpose()?
            .unwrap_or_default();
        if removed.iter().any(|peer| peer == authenticated_peer_id) {
            return Err("sync_peer_removed".into());
        }
        Self::apply_entity_blocking_inner(transaction, entity)
    }

    fn apply_entity_blocking_inner(
        conn: &Connection,
        entity: &SyncEntity,
    ) -> Result<
        (
            crate::application::sync_storage::InboundApplyOutcome,
            Vec<crate::canonical_types::pending::ReplayNotice>,
        ),
        String,
    > {
        let canonical_object = matches!(
            entity.entity_type.as_str(),
            "object" | "todo" | "project" | "tag"
        );
        let canonical_type = if canonical_object {
            "object"
        } else {
            &entity.entity_type
        };
        let deleted = entity.deleted == Some(true);

        let (outcome, notices) = Self::apply_entity_mutation_inner(conn, entity)?;

        if outcome == crate::application::sync_storage::InboundApplyOutcome::Applied {
            crate::db::append_committed_change(
                conn,
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

    fn apply_entity_mutation_inner(
        conn: &Connection,
        entity: &SyncEntity,
    ) -> Result<
        (
            crate::application::sync_storage::InboundApplyOutcome,
            Vec<crate::canonical_types::pending::ReplayNotice>,
        ),
        String,
    > {
        let sequenced_usage = is_sequenced_usage_entity(&entity.entity_type);
        if sequenced_usage && !usage_entity_is_newer(conn, entity)? {
            let (device_id, seq) = usage_origin_for_entity(conn, entity)?;
            record_usage_sequence(
                conn,
                &entity.entity_type,
                &entity.id,
                &device_id,
                seq,
                &entity.hlc,
                entity.deleted == Some(true),
            )?;
            return Ok((
                crate::application::sync_storage::InboundApplyOutcome::Stale,
                Vec::new(),
            ));
        }
        if matches!(entity.entity_type.as_str(), "area" | "heading") {
            return Err("LegacyPlanningReadOnly".into());
        }
        if matches!(entity.entity_type.as_str(), "todo" | "project" | "tag")
            && entity.deleted == Some(true)
        {
            let mut canonical = entity.clone();
            canonical.entity_type = "object".into();
            return crate::canonical_types::facades::apply_canonical_object_entity(
                conn, &canonical,
            )
            .map(|outcome| (outcome, Vec::new()));
        }
        if entity.entity_type == "object" && entity.deleted == Some(true) {
            return crate::canonical_types::facades::apply_canonical_object_entity(conn, entity)
                .map(|outcome| (outcome, Vec::new()));
        }

        if entity.deleted == Some(true) {
            match entity.entity_type.as_str() {
                "todo" => delete_todo(conn, &entity.id),
                "project" => delete_project(conn, &entity.id),
                "tag" => delete_tag(conn, &entity.id),
                "tracked_app" => delete_tracked_app(conn, &entity.id),
                "usage_session" => delete_usage_session(conn, &entity.id),
                "usage_event" => delete_usage_event(conn, &entity.id),
                "usage_day" => delete_usage_day(conn, &entity.id),
                "object_type" => delete_object_type(conn, &entity.id),
                "object" => delete_object(conn, &entity.id),
                "object_link" => delete_object_link(conn, &entity.id),
                _ => Err(format!("unknown sync entity type '{}'", entity.entity_type)),
            }?;
            upsert_sync_tombstone(conn, entity)?;
            if sequenced_usage {
                finish_usage_entity_sync(conn, entity)?;
            }
            return Ok((
                crate::application::sync_storage::InboundApplyOutcome::Applied,
                Vec::new(),
            ));
        }

        let mut full_data = entity.data.clone();
        full_data.insert("id".to_string(), Value::String(entity.id.clone()));
        let value = Value::Object(full_data);

        let mut replay_notices = Vec::new();
        let mut outcome = crate::application::sync_storage::InboundApplyOutcome::Applied;
        let result = match entity.entity_type.as_str() {
            "todo" | "project" | "tag" => {
                crate::canonical_types::facades::apply_legacy_compat_entity(conn, entity).map(
                    |applied| {
                        outcome = applied;
                    },
                )
            }
            "area" | "heading" => Err("LegacyPlanningReadOnly".into()),
            "tracked_app" => serde_json::from_value::<TrackedApp>(value)
                .map_err(|e| e.to_string())
                .and_then(|tracked_app| upsert_tracked_app(conn, &tracked_app)),
            "usage_session" => serde_json::from_value::<UsageSession>(value)
                .map_err(|e| e.to_string())
                .and_then(|session| upsert_usage_session(conn, &session)),
            "usage_event" => serde_json::from_value::<UsageEvent>(value)
                .map_err(|e| e.to_string())
                .and_then(|event| upsert_usage_event(conn, &event)),
            "usage_day" => serde_json::from_value::<UsageDay>(value)
                .map_err(|e| e.to_string())
                .and_then(|day| upsert_usage_day(conn, &day)),
            "object_type" => serde_json::from_value::<ObjectType>(value)
                .map_err(|e| e.to_string())
                .and_then(|object_type| {
                    replay_notices = upsert_object_type(conn, &object_type)?;
                    Ok(())
                }),
            "object" => {
                // The canonical ingress owns identity resolution and validation;
                // pending replay uses this same branch after the registry arrives.
                let mut object_data = match value.clone() {
                    Value::Object(map) => map,
                    _ => return Err("object payload must be an object".into()),
                };
                object_data
                    .entry("typeVersion".to_string())
                    .or_insert_with(|| Value::String("0.0.0-legacy".to_string()));
                let object = serde_json::from_value::<ArkObject>(Value::Object(object_data))
                    .map_err(|e| e.to_string())?;
                if !is_object_definition_known(conn, &object.type_id, &object.type_version)? {
                    insert_pending_object(conn, entity, &object.type_id)?;
                    crate::events::emit_event(json!({
                        "event": "sync_error",
                        "code": "unknown_type_version",
                        "entity_type": "object",
                        "entity_id": entity.id,
                        "awaited_type_id": object.type_id,
                        "awaited_type_version": object.type_version,
                    }));
                    outcome = crate::application::sync_storage::InboundApplyOutcome::Pending;
                    Ok(())
                } else {
                    crate::canonical_types::facades::apply_canonical_object_entity(conn, entity)
                        .map(|applied| {
                            outcome = applied;
                        })
                }
            }
            "object_link" => serde_json::from_value::<ObjectLink>(value)
                .map_err(|e| e.to_string())
                .and_then(|link| upsert_object_link(conn, &link)),
            _ => Err(format!("unknown sync entity type '{}'", entity.entity_type)),
        };
        result?;
        delete_sync_tombstone(conn, &entity.id)?;
        if sequenced_usage {
            finish_usage_entity_sync(conn, entity)?;
        }
        Ok((outcome, replay_notices))
    }
}

pub(super) fn is_full_core_task(entity: &SyncEntity) -> bool {
    entity.entity_type == "object"
        && entity.deleted != Some(true)
        && entity.data.get("typeId").and_then(Value::as_str) == Some("core.task")
        && entity.data.get("typeVersion").and_then(Value::as_str) == Some("0.1.0")
        && entity.data.contains_key("contentJson")
        && entity.data.contains_key("propsJson")
}
