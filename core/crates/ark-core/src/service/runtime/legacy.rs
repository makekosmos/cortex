use super::*;

pub(super) async fn init(state: &Arc<ServiceState>, db_path: String) -> Result<Value, String> {
    // Re-initialization closes the current owner before opening another one;
    // two actor writer connections must never overlap for the live database.
    handle_stop_sync(state).await;
    *state.db.lock().unwrap_or_else(|error| error.into_inner()) = None;
    let previous_owner = {
        state
            .db_owner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    };
    if let Some(mut owner) = previous_owner {
        owner
            .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(30))
            .await
            .map_err(|error| format!("database owner shutdown failed: {error}"))?;
    }
    let (db, owner) = db::DbHandle::open(db_path.clone(), local_write_device_id(None))
        .await
        .map_err(|error| error.to_string())?;
    *state
        .db_owner
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(owner);
    *state.db.lock().unwrap_or_else(|e| e.into_inner()) = Some(db);
    *state.db_path.lock().unwrap_or_else(|e| e.into_inner()) = Some(db_path);
    Ok(json!(true))
}

pub(super) async fn load_all(state: &Arc<ServiceState>) -> Result<Value, String> {
    get_db_handle(state)?
        .query_with_store(db::CommandContext::engine(), db::ObjectReadQuery::LoadAll)
        .await
        .map_err(|error| error.to_string())
}

// Legacy entity write handlers: до 2026-05-18 они не вызывали
// record_local_upsert/record_local_delete → sync_kv.version_vector не
// двигался, peers не видели локальных правок todo/project/area/tag/
// heading через LAN sync. Тихий data loss bug. Все новые handler'ы
// (UpsertObject, UpsertUsageSession и т.д.) делают это правильно;
// приводим legacy к тому же контракту.
// 2026-06-17: все write-handlers теперь рассылают LiveChange через
// broadcast_local_change — фикс live-sync gap.
pub(super) async fn upsert_todo(
    state: &Arc<ServiceState>,
    todo: TodoItem,
    device_id: Option<String>,
) -> Result<Value, String> {
    let record = legacy_record(
        &todo.id,
        "task_obj",
        &todo.title,
        json!({
            "priority": todo.priority, "scheduled_date": todo.scheduled_date,
            "deadline": todo.deadline, "reminder_date": todo.reminder_date,
            "is_today": todo.is_today, "is_evening": todo.is_evening,
            "is_someday": todo.is_someday, "is_completed": todo.is_completed,
            "completed_at": todo.completed_at, "is_cancelled": todo.is_cancelled,
            "cancelled_at": todo.cancelled_at, "checklist_items": todo.checklist_items,
            "recurrence_rule": todo.recurrence_rule, "project_id": todo.project_id,
            "tag_ids": todo.tag_ids,
        }),
        &todo.created_at,
        None,
    );
    let entities = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::LegacyGraphWriteCommand::new(
                crate::application::legacy_mutation::LegacyGraphWrite {
                    records: vec![record],
                    device_id,
                },
            ),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!(true))
}

pub(super) async fn delete_todo(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let entity = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::LegacyDeleteCommand::new(crate::application::legacy_mutation::LegacyDelete {
                id,
                kind: crate::application::legacy_mutation::LegacyObjectKind::Task,
                device_id,
            }),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}

pub(super) async fn delete_project(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let entity = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::LegacyDeleteCommand::new(crate::application::legacy_mutation::LegacyDelete {
                id,
                kind: crate::application::legacy_mutation::LegacyObjectKind::Project,
                device_id,
            }),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}

pub(super) async fn batch_upsert_todos(
    state: &Arc<ServiceState>,
    todos: Vec<TodoItem>,
    device_id: Option<String>,
) -> Result<Value, String> {
    let records = todos
        .iter()
        .map(|todo| {
            legacy_record(
                &todo.id,
                "task_obj",
                &todo.title,
                json!({
                    "priority": todo.priority, "scheduled_date": todo.scheduled_date,
                    "deadline": todo.deadline, "reminder_date": todo.reminder_date,
                    "is_today": todo.is_today, "is_evening": todo.is_evening,
                    "is_someday": todo.is_someday, "is_completed": todo.is_completed,
                    "completed_at": todo.completed_at, "is_cancelled": todo.is_cancelled,
                    "cancelled_at": todo.cancelled_at,
                    "checklist_items": todo.checklist_items,
                    "recurrence_rule": todo.recurrence_rule,
                    "project_id": todo.project_id, "tag_ids": todo.tag_ids,
                }),
                &todo.created_at,
                None,
            )
        })
        .collect::<Vec<_>>();
    let entities = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::LegacyGraphWriteCommand::new(
                crate::application::legacy_mutation::LegacyGraphWrite { records, device_id },
            ),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!(true))
}

pub(super) async fn upsert_project(
    state: &Arc<ServiceState>,
    project: Project,
    device_id: Option<String>,
) -> Result<Value, String> {
    let record = legacy_record(
        &project.id,
        "project_obj",
        &project.title,
        json!(
            {"status": project.status,
            "scheduled_date": project.scheduled_date,
            "deadline": project.deadline,
            "color": project.color_tag}
        ),
        &project.created_at,
        None,
    );
    let entities = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::LegacyGraphWriteCommand::new(
                crate::application::legacy_mutation::LegacyGraphWrite {
                    records: vec![record],
                    device_id,
                },
            ),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!(true))
}

pub(super) async fn upsert_tag(
    state: &Arc<ServiceState>,
    tag: Tag,
    device_id: Option<String>,
) -> Result<Value, String> {
    let record = legacy_record(
        &tag.id,
        "tag_obj",
        &tag.title,
        json!({"color": tag.color}),
        &tag.created_at,
        None,
    );
    let entities = get_db_handle(state)?
        .submit(
            db::CommandContext::engine(),
            db::LegacyGraphWriteCommand::new(
                crate::application::legacy_mutation::LegacyGraphWrite {
                    records: vec![record],
                    device_id,
                },
            ),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!(true))
}

pub(super) async fn upsert_tracked_app(
    state: &Arc<ServiceState>,
    tracked_app: TrackedApp,
    device_id: Option<String>,
) -> Result<Value, String> {
    use ark_application::tracked_app_mutation::{TrackedAppMutation, TrackedAppUpsert};

    let entity = get_db_handle(state)?
        .submit_tracked_app_mutation(
            db::CommandContext::engine(),
            TrackedAppMutation::Upsert(Box::new(TrackedAppUpsert {
                tracked_app,
                device_id: local_write_device_id(device_id),
            })),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}

pub(super) async fn delete_tracked_app(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    use ark_application::tracked_app_mutation::{TrackedAppDelete, TrackedAppMutation};

    let entity = get_db_handle(state)?
        .submit_tracked_app_mutation(
            db::CommandContext::engine(),
            TrackedAppMutation::Delete(TrackedAppDelete {
                id,
                device_id: local_write_device_id(device_id),
            }),
        )
        .await
        .map_err(|error| error.to_string())?;
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}
