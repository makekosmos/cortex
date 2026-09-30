use super::*;

pub(super) async fn init(state: &Arc<ServiceState>, db_path: String) -> Result<Value, String> {
    let conn = db::open_db(&db_path)?;
    db::init_schema(&conn)?;
    *state.db.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(StdMutex::new(conn)));
    *state.db_path.lock().unwrap_or_else(|e| e.into_inner()) = Some(db_path);
    Ok(json!(true))
}

pub(super) async fn load_all(state: &Arc<ServiceState>) -> Result<Value, String> {
    with_conn(state, |conn| {
        let data = db::load_all(conn)?;
        serde_json::to_value(data).map_err(|e| e.to_string())
    })
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
            "priority": todo.priority, "scheduled_date": todo.scheduled_date, "deadline": todo.deadline,
            "reminder_date": todo.reminder_date, "is_today": todo.is_today, "is_evening": todo.is_evening,
            "is_someday": todo.is_someday, "is_completed": todo.is_completed, "completed_at": todo.completed_at,
            "is_cancelled": todo.is_cancelled, "cancelled_at": todo.cancelled_at, "checklist_items": todo.checklist_items,
            "recurrence_rule": todo.recurrence_rule, "project_id": todo.project_id, "tag_ids": todo.tag_ids,
        }),
        &todo.created_at,
        None,
    );
    let entities = with_write_tx(state, |conn| write_legacy_graph(conn, &[record], device_id))?;
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!(true))
}

pub(super) async fn delete_todo(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let entity = with_write_tx(state, |conn| {
        tombstone_legacy(conn, &id, "com.kosmos.task", device_id)
    })?;
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}

pub(super) async fn delete_project(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let entity = with_write_tx(state, |conn| {
        tombstone_legacy(conn, &id, "com.kosmos.project", device_id)
    })?;
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}

pub(super) async fn batch_upsert_todos(
    state: &Arc<ServiceState>,
    todos: Vec<TodoItem>,
    device_id: Option<String>,
) -> Result<Value, String> {
    let records = todos.iter().map(|todo| legacy_record(&todo.id, "task_obj", &todo.title, json!({
        "priority": todo.priority, "scheduled_date": todo.scheduled_date, "deadline": todo.deadline,
        "reminder_date": todo.reminder_date, "is_today": todo.is_today, "is_evening": todo.is_evening,
        "is_someday": todo.is_someday, "is_completed": todo.is_completed, "completed_at": todo.completed_at,
        "is_cancelled": todo.is_cancelled, "cancelled_at": todo.cancelled_at, "checklist_items": todo.checklist_items,
        "recurrence_rule": todo.recurrence_rule, "project_id": todo.project_id, "tag_ids": todo.tag_ids,
    }), &todo.created_at, None)).collect::<Vec<_>>();
    let entities = with_write_tx(state, |conn| write_legacy_graph(conn, &records, device_id))?;
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
        json!({"status": project.status, "scheduled_date": project.scheduled_date, "deadline": project.deadline, "color": project.color_tag}),
        &project.created_at,
        None,
    );
    let entities = with_write_tx(state, |conn| write_legacy_graph(conn, &[record], device_id))?;
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
    let entities = with_write_tx(state, |conn| write_legacy_graph(conn, &[record], device_id))?;
    maybe_broadcast_local_changes(state, entities).await;
    Ok(json!(true))
}

pub(super) async fn upsert_tracked_app(
    state: &Arc<ServiceState>,
    tracked_app: TrackedApp,
    device_id: Option<String>,
) -> Result<Value, String> {
    let entity = with_write_tx(state, |conn| {
        db::upsert_tracked_app(conn, &tracked_app)?;
        let hlc = record_local_upsert(conn, "tracked_app", &tracked_app.id, device_id)?;
        Ok(make_sync_entity(
            "tracked_app",
            &tracked_app.id,
            serde_json::to_value(&tracked_app).unwrap_or(json!({})),
            hlc,
            None,
        ))
    })?;
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}

pub(super) async fn delete_tracked_app(
    state: &Arc<ServiceState>,
    id: String,
    device_id: Option<String>,
) -> Result<Value, String> {
    let entity = with_write_tx(state, |conn| {
        db::delete_tracked_app(conn, &id)?;
        let hlc = record_local_delete(conn, "tracked_app", &id, device_id)?;
        Ok(make_sync_entity(
            "tracked_app",
            &id,
            json!({}),
            hlc,
            Some(true),
        ))
    })?;
    maybe_broadcast_local_change(state, entity).await;
    Ok(json!(true))
}
