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
    let record = with_write_tx(state, |conn| {
        crate::canonical_types::game::upsert_game(conn, game, &device)
    })?;
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
    let device_id = local_write_device_id(device_id);
    let book_id_for_event = book_id.clone();
    let mutation = with_write_tx(state, |conn| {
        let mutation = crate::canonical_types::facades::set_book_cover(
            conn,
            &book_id,
            source_ref.as_deref(),
            existing_image_id.as_deref(),
            &alt_text,
            &device_id,
        )
        .map_err(|error| error.to_string())?;
        if !mutation.changed {
            return Ok(mutation);
        }
        let book_hlc = record_local_upsert(conn, "object", &book_id, Some(device_id.clone()))?;
        conn.execute(
            "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE excluded.hlc > object_sync_versions.hlc",
            rusqlite::params![book_id, book_hlc],
        )
        .map_err(|e| e.to_string())?;
        for link in &mutation.links {
            let link_hlc =
                record_local_upsert(conn, "object_link", &link.id, Some(device_id.clone()))?;
            conn.execute(
                "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE excluded.hlc > object_sync_versions.hlc",
                rusqlite::params![link.id, link_hlc],
            )
            .map_err(|e| e.to_string())?;
        }
        for id in &mutation.deleted_link_ids {
            record_local_delete(conn, "object_link", id, Some(device_id.clone()))?;
        }
        if let Some(image) = &mutation.image {
            record_local_upsert(conn, "object", &image.id, Some(device_id.clone()))?;
        }
        Ok(mutation)
    })?;
    if mutation.changed {
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
    let entity = with_write_tx(state, |conn| {
        if let Some(expected) = expected_snapshot {
            if object_write_snapshot(conn, &object.id)? != expected {
                return Err("object_conflict:stale_snapshot".to_string());
            }
        }
        let object = crate::canonical_types::ingress::prepare_object(conn, object)
            .map_err(|error| error.to_string())?;
        db::upsert_object(conn, &object)?;
        let hlc = record_local_upsert(conn, "object", &object.id, device_id)?;
        let version_rows = conn
            .execute(
                "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0)
                     ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0
                     WHERE excluded.hlc > object_sync_versions.hlc",
                rusqlite::params![object.id, hlc],
            )
            .map_err(|error| error.to_string())?;
        if version_rows != 1 {
            return Err("object_conflict:stale_revision".to_string());
        }
        Ok(make_sync_entity(
            "object",
            &object.id,
            serde_json::to_value(&object).unwrap_or(json!({})),
            hlc,
            None,
        ))
    })?;
    let eid = object_id.clone();
    let etid = object_type_id.clone();
    let state = Arc::clone(state);
    tokio::spawn(async move {
        broadcast_local_change(&state, entity).await;
    });
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
    let entity = with_write_tx(state, |conn| {
        if let Some(expected) = expected_snapshot {
            if object_write_snapshot(conn, &id)? != expected {
                return Err("object_conflict:stale_snapshot".to_string());
            }
        }
        let type_id = db::get_object(conn, &id)?.map(|object| object.type_id);
        db::delete_object(conn, &id)?;
        let hlc =
            record_local_delete_with_type(conn, "object", &id, device_id, type_id.as_deref())?;
        let version_rows = conn
            .execute(
                "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,1)
                     ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=1
                     WHERE excluded.hlc > object_sync_versions.hlc",
                rusqlite::params![id, hlc],
            )
            .map_err(|error| error.to_string())?;
        if version_rows != 1 {
            return Err("object_conflict:stale_revision".to_string());
        }
        Ok(make_sync_entity(
            "object",
            &id,
            type_id
                .map(|type_id| json!({"typeId": type_id}))
                .unwrap_or_else(|| json!({})),
            hlc,
            Some(true),
        ))
    })?;
    let eid = object_id.clone();
    let state = Arc::clone(state);
    tokio::spawn(async move {
        broadcast_local_change(&state, entity).await;
    });
    emit_event(json!({
        "event": "object_deleted",
        "id": eid,
    }));
    Ok(json!(true))
}
