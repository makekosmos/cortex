use super::*;

pub(super) async fn handle(request: Request) -> Result<Value, String> {
    match request {
        Request::Init { db_path } => {
            let conn = db::open_db(&db_path)?;
            db::init_schema(&conn)?;
            *DB.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(StdMutex::new(conn)));
            *DB_PATH.lock().unwrap_or_else(|e| e.into_inner()) = Some(db_path);
            Ok(json!(true))
        }

        Request::LoadAll => with_conn(|conn| {
            let data = db::load_all(conn)?;
            serde_json::to_value(data).map_err(|e| e.to_string())
        }),

        // Legacy entity write handlers: до 2026-05-18 они не вызывали
        // record_local_upsert/record_local_delete → sync_kv.version_vector не
        // двигался, peers не видели локальных правок todo/project/area/tag/
        // heading через LAN sync. Тихий data loss bug. Все новые handler'ы
        // (UpsertObject, UpsertUsageSession и т.д.) делают это правильно;
        // приводим legacy к тому же контракту.
        // 2026-06-17: все write-handlers теперь рассылают LiveChange через
        // broadcast_local_change — фикс live-sync gap.
        Request::UpsertTodo { todo, device_id } => {
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
            let entities = with_write_tx(|conn| write_legacy_graph(conn, &[record], device_id))?;
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(entity).await;
                }
            });
            Ok(json!(true))
        }

        Request::DeleteTodo { id, device_id } => {
            let entity =
                with_write_tx(|conn| tombstone_legacy(conn, &id, "com.kosmos.task", device_id))?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::DeleteProject { id, device_id } => {
            let entity =
                with_write_tx(|conn| tombstone_legacy(conn, &id, "com.kosmos.project", device_id))?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::BatchUpsertTodos { todos, device_id } => {
            let records = todos.iter().map(|todo| legacy_record(&todo.id, "task_obj", &todo.title, json!({
                "priority": todo.priority, "scheduled_date": todo.scheduled_date, "deadline": todo.deadline,
                "reminder_date": todo.reminder_date, "is_today": todo.is_today, "is_evening": todo.is_evening,
                "is_someday": todo.is_someday, "is_completed": todo.is_completed, "completed_at": todo.completed_at,
                "is_cancelled": todo.is_cancelled, "cancelled_at": todo.cancelled_at, "checklist_items": todo.checklist_items,
                "recurrence_rule": todo.recurrence_rule, "project_id": todo.project_id, "tag_ids": todo.tag_ids,
            }), &todo.created_at, None)).collect::<Vec<_>>();
            let entities = with_write_tx(|conn| write_legacy_graph(conn, &records, device_id))?;
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(entity).await;
                }
            });
            Ok(json!(true))
        }

        Request::UpsertProject { project, device_id } => {
            let record = legacy_record(
                &project.id,
                "project_obj",
                &project.title,
                json!({"status": project.status, "scheduled_date": project.scheduled_date, "deadline": project.deadline, "color": project.color_tag}),
                &project.created_at,
                None,
            );
            let entities = with_write_tx(|conn| write_legacy_graph(conn, &[record], device_id))?;
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(entity).await;
                }
            });
            Ok(json!(true))
        }

        Request::UpsertArea {
            area: _area,
            device_id: _device_id,
        } => Err("LegacyPlanningReadOnly".to_string()),

        Request::UpsertTag { tag, device_id } => {
            let record = legacy_record(
                &tag.id,
                "tag_obj",
                &tag.title,
                json!({"color": tag.color}),
                &tag.created_at,
                None,
            );
            let entities = with_write_tx(|conn| write_legacy_graph(conn, &[record], device_id))?;
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(entity).await;
                }
            });
            Ok(json!(true))
        }

        Request::UpsertHeading {
            heading: _heading,
            device_id: _device_id,
        } => Err("LegacyPlanningReadOnly".to_string()),

        Request::DeleteHeading {
            id: _id,
            device_id: _device_id,
        } => Err("LegacyPlanningReadOnly".to_string()),

        Request::UpsertTrackedApp {
            tracked_app,
            device_id,
        } => {
            let entity = with_write_tx(|conn| {
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
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::DeleteTrackedApp { id, device_id } => {
            let entity = with_write_tx(|conn| {
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
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        _ => unreachable!("request routed to the wrong runtime handler"),
    }
}

