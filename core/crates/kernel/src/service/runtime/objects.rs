use super::*;

async fn object_read(state: &ServiceState, query: db::ObjectReadQuery) -> Result<Value, String> {
    get_db_handle(state)?
        .query_with_store(db::CommandContext::engine(), query)
        .await
        .map_err(|error| error.to_string())
}

/// `objects.list`. The filters pick the read the six old operations did: ids,
/// then a text query, then the plain listing narrowed by `type_id` and
/// projected to summaries when asked. Two selectors at once is an error rather
/// than a silent precedence rule.
pub(super) async fn list(
    state: &Arc<ServiceState>,
    type_id: Option<String>,
    ids: Option<Vec<String>>,
    query: Option<String>,
    summaries: bool,
) -> Result<Value, String> {
    if ids.is_some() && query.is_some() {
        return Err("objects.list accepts ids or query, not both".to_string());
    }
    let read = match (ids, query, type_id) {
        (Some(ids), _, _) => db::ObjectReadQuery::GetByIds(ids),
        (_, Some(query), _) => db::ObjectReadQuery::Search(query),
        (_, _, Some(type_id)) if summaries => db::ObjectReadQuery::ListSummariesByType(type_id),
        (_, _, Some(type_id)) => db::ObjectReadQuery::ListByType(type_id),
        (_, _, None) if summaries => db::ObjectReadQuery::ListSummaries,
        (_, _, None) => db::ObjectReadQuery::List,
    };
    object_read(state, read).await
}

pub(super) async fn estimate_data_volume(
    state: &Arc<ServiceState>,
    scopes: Vec<crate::service::DataVolumeScope>,
) -> Result<Value, String> {
    let scopes = scopes
        .into_iter()
        .map(::store::data_volume::DataVolumeScope::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let query = ::store::data_volume::DataVolumeQuery::new(scopes)?;
    object_read(state, db::ObjectReadQuery::EstimateDataVolume(query)).await
}

pub(super) async fn list_running_time_entries(
    state: &Arc<ServiceState>,
    source: Option<String>,
) -> Result<Value, String> {
    object_read(state, db::ObjectReadQuery::ListRunningTime(source)).await
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

pub(super) async fn task_merge_conflicts(
    state: &Arc<ServiceState>,
    object_id: String,
    unit: ::domain::task_merge::TaskMergeUnit,
) -> Result<Value, String> {
    let result = get_db_handle(state)?
        .query_with_store(
            db::CommandContext::engine(),
            db::TaskConflictSetQuery::new(object_id, unit),
        )
        .await
        .map_err(|error| error.to_string())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

pub(super) async fn task_merge_append(
    state: &Arc<ServiceState>,
    operation: ::domain::task_merge::TaskMergeOperation,
    expected_conflict_digest: Option<String>,
) -> Result<Value, String> {
    let object_id = operation.object_id.clone();
    let result = get_db_handle(state)?
        .append_task_merge_operation(
            db::CommandContext::engine(),
            db::AppendTaskMergeOperationRequest::new(operation, expected_conflict_digest),
        )
        .await
        .map_err(|error| error.to_string())?;
    if !result.replayed {
        let runtime = state.sync.lock().await.clone();
        if let Some(runtime) = runtime {
            runtime.server.broadcast_task_merge_batches().await;
        }
        // The merge graph is task-only, so the type is known here.
        emit_event(crate::events::objects_changed(
            &object_id,
            Some("core.task"),
            "merge",
        ));
    }
    serde_json::to_value(result).map_err(|error| error.to_string())
}

pub(super) async fn upsert_object(
    state: &Arc<ServiceState>,
    object: ArkObjectWrite,
    expected_snapshot: Option<ObjectWriteSnapshot>,
    operation_id: Option<String>,
    device_id: Option<String>,
) -> Result<Value, String> {
    let object_id = object.id.clone();
    let object_type_id = object.type_id.clone();
    let (entity, replayed) = if object.type_id == "core.task" {
        let command = db::LocalCoreTaskWrite::new(object, expected_snapshot, device_id);
        let result = get_db_handle(state)?
            .submit_local_core_task_write(db::CommandContext::engine(), operation_id, command)
            .await
            .map_err(|error| error.to_string())?;
        let replayed = result.replayed;
        let commit = result.value;
        let hlc = commit
            .receipt
            .resulting_hlc
            .ok_or_else(|| "object_write_receipt_missing_revision".to_string())?;
        (
            make_sync_entity(
                "object",
                &commit.object.id,
                serde_json::to_value(&commit.object).unwrap_or(json!({})),
                hlc,
                None,
            ),
            replayed,
        )
    } else {
        let result = get_db_handle(state)?
            .submit_local_object_upsert(
                db::CommandContext::engine(),
                ::store::object_mutation::LocalObjectUpsert {
                    object,
                    expected_snapshot,
                    device_id: local_write_device_id(device_id),
                },
                operation_id,
            )
            .await
            .map_err(|error| error.to_string())?;
        (result.value.entity, result.replayed)
    };
    if !replayed {
        maybe_broadcast_local_change(state, entity).await;
        emit_event(json!({
            "event": "object_upserted",
            "id": object_id,
            "type_id": object_type_id,
        }));
        emit_event(crate::events::objects_changed(
            &object_id,
            Some(&object_type_id),
            "upsert",
        ));
    }
    Ok(json!(true))
}

pub(super) async fn delete_object(
    state: &Arc<ServiceState>,
    id: String,
    expected_snapshot: Option<ObjectWriteSnapshot>,
    operation_id: Option<String>,
    device_id: Option<String>,
) -> Result<Value, String> {
    let object_id = id.clone();
    let result = get_db_handle(state)?
        .submit_local_object_delete(
            db::CommandContext::engine(),
            crate::application::object_mutation::LocalObjectDelete {
                id,
                expected_snapshot,
                device_id: local_write_device_id(device_id),
            },
            operation_id,
        )
        .await
        .map_err(|error| error.to_string())?;
    if !result.replayed {
        let type_id = result.value.entity.data.get("typeId").cloned();
        maybe_broadcast_local_change(state, result.value.entity).await;
        emit_event(json!({
            "event": "object_deleted",
            "id": object_id,
        }));
        emit_event(crate::events::objects_changed(
            &object_id,
            type_id.as_ref().and_then(Value::as_str),
            "delete",
        ));
    }
    Ok(json!(true))
}
