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

pub(super) async fn handle(request: Request) -> Result<Value, String> {
    match request {
        Request::ListObjects => with_conn(|conn| {
            let objects = db::list_objects(conn)?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::ListObjectSummaries => with_conn(|conn| {
            let objects = db::list_object_summaries(conn)?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::ListObjectsByType { type_id } => with_conn(|conn| {
            let objects = db::list_objects_by_type(conn, &type_id)?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::ListObjectSummariesByType { type_id } => with_conn(|conn| {
            let objects = db::list_object_summaries_by_type(conn, &type_id)?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::ListRunningTimeEntries { source } => with_conn(|conn| {
            let objects = db::list_running_time_entries(conn, source.as_deref())?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::GetObjectsByIds { ids } => with_conn(|conn| {
            let objects = db::get_objects_by_ids(conn, &ids)?;
            serde_json::to_value(objects).map_err(|e| e.to_string())
        }),
        Request::SearchObjects { query } => with_conn(|conn| {
            let results = db::search_objects(conn, &query)?;
            serde_json::to_value(results).map_err(|e| e.to_string())
        }),
        Request::GetObject { id } => with_conn(|conn| {
            let object = db::get_object(conn, &id)?;
            serde_json::to_value(object).map_err(|e| e.to_string())
        }),
        Request::GetObjectWriteSnapshot { id } => with_conn(|conn| {
            serde_json::to_value(object_write_snapshot(conn, &id)?)
                .map_err(|error| error.to_string())
        }),
        Request::CanonicalGameList { device_id } => {
            let device = local_write_device_id(device_id);
            with_conn(|conn| {
                serde_json::to_value(ark_core::canonical_types::game::list_games(conn, &device)?)
                    .map_err(|e| e.to_string())
            })
        }
        Request::CanonicalGameGet { id, device_id } => {
            let device = local_write_device_id(device_id);
            with_conn(|conn| {
                serde_json::to_value(ark_core::canonical_types::game::get_game(
                    conn, &id, &device,
                )?)
                .map_err(|e| e.to_string())
            })
        }
        Request::CanonicalGameUpsert { game, device_id } => {
            let device = local_write_device_id(device_id);
            let record = with_write_tx(|conn| {
                ark_core::canonical_types::game::upsert_game(conn, game, &device)
            })?;
            if record.changed {
                emit_event(json!({"event":"arrancador.changed"}));
            }
            serde_json::to_value(record).map_err(|e| e.to_string())
        }
        Request::CanonicalAssetSources { object_ids } => with_conn(|conn| {
            serde_json::to_value(
                ark_core::canonical_types::facades::asset_sources(conn, &object_ids)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|e| e.to_string())
        }),
        Request::CanonicalSetBookCover {
            book_id,
            source_ref,
            existing_image_id,
            alt_text,
            device_id,
        } => {
            let device_id = local_write_device_id(device_id);
            let book_id_for_event = book_id.clone();
            let mutation = with_write_tx(|conn| {
                let mutation = ark_core::canonical_types::facades::set_book_cover(
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
                let book_hlc =
                    record_local_upsert(conn, "object", &book_id, Some(device_id.clone()))?;
                conn.execute(
                    "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE excluded.hlc > object_sync_versions.hlc",
                    rusqlite::params![book_id, book_hlc],
                )
                .map_err(|e| e.to_string())?;
                for link in &mutation.links {
                    let link_hlc = record_local_upsert(
                        conn,
                        "object_link",
                        &link.id,
                        Some(device_id.clone()),
                    )?;
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
        Request::UpsertObject {
            object,
            expected_snapshot,
            device_id,
        } => {
            let object_id = object.id.clone();
            let object_type_id = object.type_id.clone();
            let entity = with_write_tx(|conn| {
                if let Some(expected) = expected_snapshot {
                    if object_write_snapshot(conn, &object.id)? != expected {
                        return Err("object_conflict:stale_snapshot".to_string());
                    }
                }
                let object = ark_core::canonical_types::ingress::prepare_object(conn, object)
                    .map_err(|error| error.to_string())?;
                db::upsert_object(conn, &object)?;
                let hlc = record_local_upsert(conn, "object", &object.id, device_id)?;
                let version_rows = conn.execute(
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
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            emit_event(json!({
                "event": "object_upserted",
                "id": eid,
                "type_id": etid,
            }));
            Ok(json!(true))
        }
        Request::DeleteObject {
            id,
            expected_snapshot,
            device_id,
        } => {
            let object_id = id.clone();
            let entity = with_write_tx(|conn| {
                if let Some(expected) = expected_snapshot {
                    if object_write_snapshot(conn, &id)? != expected {
                        return Err("object_conflict:stale_snapshot".to_string());
                    }
                }
                let type_id = db::get_object(conn, &id)?.map(|object| object.type_id);
                db::delete_object(conn, &id)?;
                let hlc = record_local_delete_with_type(
                    conn,
                    "object",
                    &id,
                    device_id,
                    type_id.as_deref(),
                )?;
                let version_rows = conn.execute(
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
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            emit_event(json!({
                "event": "object_deleted",
                "id": eid,
            }));
            Ok(json!(true))
        }
        _ => unreachable!("request routed to the wrong runtime handler"),
    }
}
