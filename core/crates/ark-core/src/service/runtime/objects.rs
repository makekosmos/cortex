use super::*;

async fn object_read(state: &ServiceState, query: db::ObjectReadQuery) -> Result<Value, String> {
    get_db_handle(state)?
        .query(db::CommandContext::engine(), query)
        .await
        .map_err(|error| error.to_string())
}

pub(super) async fn list_objects(state: &Arc<ServiceState>) -> Result<Value, String> {
    object_read(state, db::ObjectReadQuery::List).await
}

pub(super) async fn list_object_summaries(state: &Arc<ServiceState>) -> Result<Value, String> {
    object_read(state, db::ObjectReadQuery::ListSummaries).await
}

pub(super) async fn list_objects_by_type(
    state: &Arc<ServiceState>,
    type_id: String,
) -> Result<Value, String> {
    object_read(state, db::ObjectReadQuery::ListByType(type_id)).await
}

pub(super) async fn list_object_summaries_by_type(
    state: &Arc<ServiceState>,
    type_id: String,
) -> Result<Value, String> {
    object_read(state, db::ObjectReadQuery::ListSummariesByType(type_id)).await
}

pub(super) async fn list_running_time_entries(
    state: &Arc<ServiceState>,
    source: Option<String>,
) -> Result<Value, String> {
    object_read(state, db::ObjectReadQuery::ListRunningTime(source)).await
}

pub(super) async fn get_objects_by_ids(
    state: &Arc<ServiceState>,
    ids: Vec<String>,
) -> Result<Value, String> {
    object_read(state, db::ObjectReadQuery::GetByIds(ids)).await
}

pub(super) async fn search_objects(
    state: &Arc<ServiceState>,
    query: String,
) -> Result<Value, String> {
    object_read(state, db::ObjectReadQuery::Search(query)).await
}

pub(super) async fn get_object(state: &Arc<ServiceState>, id: String) -> Result<Value, String> {
    object_read(state, db::ObjectReadQuery::Get(id)).await
}

pub(super) async fn get_object_write_snapshot(
    state: &Arc<ServiceState>,
    id: String,
) -> Result<Value, String> {
    object_read(state, db::ObjectReadQuery::WriteSnapshot(id)).await
}

pub(super) async fn canonical_game_list(
    state: &Arc<ServiceState>,
    device_id: Option<String>,
) -> Result<Value, String> {
    let device = local_write_device_id(device_id);
    object_read(state, db::ObjectReadQuery::CanonicalGames(device)).await
}

pub(super) async fn canonical_game_get(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let device = local_write_device_id(device_id);
    object_read(state, db::ObjectReadQuery::CanonicalGame(id, device)).await
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
    object_read(
        state,
        db::ObjectReadQuery::CanonicalAssetSources(object_ids),
    )
    .await
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
