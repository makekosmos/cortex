use super::*;
use rusqlite::OptionalExtension;

fn object_write_snapshot(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<ObjectWriteSnapshot, String> {
    let identity = conn
        .query_row(
            "SELECT type_id, type_version FROM objects WHERE id = ?1",
            [id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(ObjectWriteSnapshot {
        exists: identity.is_some(),
        type_id: identity.as_ref().map(|value| value.0.clone()),
        type_version: identity.map(|value| value.1),
        revision: db::get_object_revision(conn, id)?,
    })
}

pub(super) async fn list_objects(state: &Arc<ServiceState>) -> Result<Value, String> {
    with_conn(state, |conn| {
        let objects = db::list_objects(conn)?;
        serde_json::to_value(objects).map_err(|e| e.to_string())
    })
}

pub(super) async fn list_object_summaries(state: &Arc<ServiceState>) -> Result<Value, String> {
    with_conn(state, |conn| {
        let objects = db::list_object_summaries(conn)?;
        serde_json::to_value(objects).map_err(|e| e.to_string())
    })
}

pub(super) async fn list_objects_by_type(
    state: &Arc<ServiceState>,
    type_id: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let objects = db::list_objects_by_type(conn, &type_id)?;
        serde_json::to_value(objects).map_err(|e| e.to_string())
    })
}

pub(super) async fn list_object_summaries_by_type(
    state: &Arc<ServiceState>,
    type_id: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let objects = db::list_object_summaries_by_type(conn, &type_id)?;
        serde_json::to_value(objects).map_err(|e| e.to_string())
    })
}

pub(super) async fn list_running_time_entries(
    state: &Arc<ServiceState>,
    source: Option<String>,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let objects = db::list_running_time_entries(conn, source.as_deref())?;
        serde_json::to_value(objects).map_err(|e| e.to_string())
    })
}

pub(super) async fn get_objects_by_ids(
    state: &Arc<ServiceState>,
    ids: Vec<String>,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let objects = db::get_objects_by_ids(conn, &ids)?;
        serde_json::to_value(objects).map_err(|e| e.to_string())
    })
}

pub(super) async fn search_objects(
    state: &Arc<ServiceState>,
    query: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        let results = db::search_objects(conn, &query)?;
        serde_json::to_value(results).map_err(|e| e.to_string())
    })
}

pub(super) async fn get_object(state: &Arc<ServiceState>, id: String) -> Result<Value, String> {
    with_conn(state, |conn| {
        let object = db::get_object(conn, &id)?;
        serde_json::to_value(object).map_err(|e| e.to_string())
    })
}

pub(super) async fn get_object_write_snapshot(
    state: &Arc<ServiceState>,
    id: String,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        serde_json::to_value(object_write_snapshot(conn, &id)?).map_err(|error| error.to_string())
    })
}

pub(super) async fn canonical_game_list(
    state: &Arc<ServiceState>,
    device_id: Option<String>,
) -> Result<Value, String> {
    let device = local_write_device_id(device_id);
    with_conn(state, |conn| {
        serde_json::to_value(crate::canonical_types::game::list_games(conn, &device)?)
            .map_err(|e| e.to_string())
    })
}

pub(super) async fn canonical_game_get(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let device = local_write_device_id(device_id);
    with_conn(state, |conn| {
        serde_json::to_value(crate::canonical_types::game::get_game(conn, &id, &device)?)
            .map_err(|e| e.to_string())
    })
}

pub(super) async fn canonical_game_upsert(
    state: &Arc<ServiceState>,
    game: crate::canonical_types::game::GameUpsertCommand,
    device_id: Option<String>,
) -> Result<Value, String> {
    let device = local_write_device_id(device_id);
    let record = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::CanonicalGameUpsertCommand::new(game, device),
        )
        .await
        .map_err(|error| error.to_string())?;
    if record.changed {
        emit_event(json!({"event":"arrancador.changed"}));
    }
    serde_json::to_value(record).map_err(|e| e.to_string())
}

pub(super) async fn canonical_asset_sources(
    state: &Arc<ServiceState>,
    object_ids: Vec<String>,
) -> Result<Value, String> {
    with_conn(state, |conn| {
        serde_json::to_value(
            crate::canonical_types::facades::asset_sources(conn, &object_ids)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|e| e.to_string())
    })
}

pub(super) async fn canonical_set_book_cover(
    state: &Arc<ServiceState>,
    book_id: String,
    source_ref: Option<String>,
    existing_image_id: Option<String>,
    alt_text: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let book_id_for_event = book_id.clone();
    let device_id = local_write_device_id(device_id);
    let changed = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::LocalBookCoverCommand::new(
                crate::application::object_mutation::LocalBookCoverWrite {
                    book_id: book_id.clone(),
                    source_ref,
                    existing_image_id,
                    alt_text,
                    device_id,
                },
            ),
        )
        .await
        .map_err(|error| error.to_string())?;
    if changed {
        emit_event(json!({
            "event": "canonical_book_cover_changed",
            "bookId": book_id_for_event,
        }));
    }
    Ok(json!(true))
}

pub(super) async fn upsert_object(
    state: &Arc<ServiceState>,
    object: ArkObjectWrite,
    expected_snapshot: Option<ObjectWriteSnapshot>,
    device_id: Option<String>,
) -> Result<Value, String> {
    let object_id = object.id.clone();
    let object_type_id = object.type_id.clone();
    let entity = if object.type_id == "core.task" {
        let commit = get_db_handle(state)?
            .submit(
                db::CommandContext::engine(),
                db::LocalCoreTaskWrite::new(object, expected_snapshot, device_id),
            )
            .await
            .map_err(|error| error.to_string())?;
        let hlc = commit
            .receipt
            .resulting_hlc
            .ok_or_else(|| "object_write_receipt_missing_revision".to_string())?;
        make_sync_entity(
            "object",
            &commit.object.id,
            serde_json::to_value(&commit.object).unwrap_or(json!({})),
            hlc,
            None,
        )
    } else {
        let commit = get_db_handle(state)?
            .submit(
                db::CommandContext::engine(),
                db::LocalObjectUpsertCommand::new(
                    crate::application::object_mutation::LocalObjectUpsert {
                        object,
                        expected_snapshot,
                        device_id: local_write_device_id(device_id),
                    },
                ),
            )
            .await
            .map_err(|error| error.to_string())?;
        commit.entity
    };
    let eid = object_id.clone();
    let etid = object_type_id.clone();
    maybe_broadcast_local_change(state, entity).await;
    emit_event(json!({
        "event": "object_upserted",
        "id": eid,
        "type_id": etid,
    }));
    Ok(json!(true))
}

pub(super) async fn delete_object(
    state: &Arc<ServiceState>,
    id: String,
    expected_snapshot: Option<ObjectWriteSnapshot>,
    device_id: Option<String>,
) -> Result<Value, String> {
    let object_id = id.clone();
    let entity = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::LocalObjectDeleteCommand::new(
                crate::application::object_mutation::LocalObjectDelete {
                    id,
                    expected_snapshot,
                    device_id: local_write_device_id(device_id),
                },
            ),
        )
        .await
        .map_err(|error| error.to_string())?
        .entity;
    let eid = object_id.clone();
    maybe_broadcast_local_change(state, entity).await;
    emit_event(json!({
        "event": "object_deleted",
        "id": eid,
    }));
    Ok(json!(true))
}
