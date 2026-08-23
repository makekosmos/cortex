// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async {
        if let Err(e) = serve().await {
            eprintln!("ark-core-rpc fatal: {e}");
            std::process::exit(1);
        }
    });
}

async fn serve() -> Result<(), String> {
    // Event channel + stdout writer
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Value>();
    set_event_sender(event_tx);

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            write_event_line(&event);
        }
    });

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        let line: String = line;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(trimmed) {
            Ok(raw) => {
                let request_id = request_id_from_value(&raw);
                match serde_json::from_value::<Request>(raw) {
                    Ok(req) => match handle_request(req).await {
                        Ok(data) => response_ok(data, request_id),
                        Err(e) => response_error(e, request_id),
                    },
                    Err(e) => response_error(e.to_string(), request_id),
                }
            }
            Err(e) => response_error(e.to_string(), None),
        };
        write_response_line(&response);
    }
    Ok(())
}

fn request_id_from_value(value: &Value) -> Option<Value> {
    // SDK кладёт envelope-id в `_req_id`. Старый формат (`id`) тоже принимаем
    // для обратной совместимости с прежним протоколом.
    value
        .get("_req_id")
        .cloned()
        .or_else(|| value.get("id").cloned())
}

fn response_ok(data: Value, request_id: Option<Value>) -> Value {
    response_with_optional_id(json!({ "ok": true, "data": data }), request_id)
}

fn response_error(error: String, request_id: Option<Value>) -> Value {
    response_with_optional_id(json!({ "ok": false, "error": error }), request_id)
}

fn response_with_optional_id(mut response: Value, request_id: Option<Value>) -> Value {
    if let (Value::Object(map), Some(id)) = (&mut response, request_id) {
        // Эхо envelope-id под `_req_id`. Старый формат (`id`) тоже дублируем
        // для SDK-версий, читающих legacy-поле.
        map.insert("_req_id".to_string(), id.clone());
        map.insert("id".to_string(), id);
    }
    response
}

// ---------------------------------------------------------------------------
// Stdout writers (shared lock to prevent interleaving of response + event)
// ---------------------------------------------------------------------------

static STDOUT_LOCK: StdMutex<()> = StdMutex::new(());

fn write_line(value: &Value) {
    let json = match serde_json::to_string(value) {
        Ok(s) => s,
        Err(_) => return,
    };
    let _guard = STDOUT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(json.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

fn write_response_line(value: &Value) {
    write_line(value);
}

fn write_event_line(value: &Value) {
    write_line(value);
}

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

fn with_conn<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    let outer = DB.lock().unwrap_or_else(|e| e.into_inner());
    let shared = outer
        .as_ref()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())?
        .clone();
    drop(outer);
    let inner = shared.lock().unwrap_or_else(|e| e.into_inner());
    f(&inner)
}

/// Выполняет write-замыкание в одной SQLite-транзакции поверх shared conn.
/// COMMIT при Ok, ROLLBACK при Err. Гарантирует атомарность entity + FTS + sync-meta:
/// вложенные SAVEPOINT внутри db::* работают внутри этого BEGIN, при ошибке откатывается всё.
fn with_write_tx<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    with_conn(|conn| {
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| e.to_string())?;
        match f(conn) {
            Ok(v) => {
                conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
                Ok(v)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    })
}

fn get_shared_conn() -> Result<Arc<StdMutex<rusqlite::Connection>>, String> {
    let guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| "Database not initialized. Call Init first.".to_string())
}

// --- DB backup: background-priority thread + chunking config ---------------

/// Страниц за один шаг online-backup. Меньше шаг → мягче для диска. Override
/// `ARK_BACKUP_PAGES_PER_STEP`.
fn backup_pages_per_step() -> i32 {
    std::env::var("ARK_BACKUP_PAGES_PER_STEP")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(256)
}

/// Пауза (мс) между шагами online-backup, чтобы не насыщать диск. Override
/// `ARK_BACKUP_PAUSE_MS`.
fn backup_pause_ms() -> u64 {
    std::env::var("ARK_BACKUP_PAUSE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5)
}

/// Thread-scoped background priority (CPU + I/O) для backup-потока.
/// `THREAD_MODE_BACKGROUND_BEGIN` на Windows; no-op иначе. RAII (Drop → END).
/// НЕ process-wide — ark-core-rpc обслуживает интерактивные ARK ops.
#[cfg(windows)]
mod background_priority {
    pub struct BackgroundThreadGuard {
        active: bool,
    }
    impl BackgroundThreadGuard {
        pub fn enter() -> Self {
            use windows::Win32::System::Threading::{
                GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
            };
            // SAFETY: меняем приоритет только текущего потока через псевдо-handle.
            let ok = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
            Self { active: ok.is_ok() }
        }
    }
    impl Drop for BackgroundThreadGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            use windows::Win32::System::Threading::{
                GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_END,
            };
            // SAFETY: симметричный END для ранее успешного BEGIN на том же потоке.
            unsafe {
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
            }
        }
    }
}

#[cfg(not(windows))]
mod background_priority {
    pub struct BackgroundThreadGuard;
    impl BackgroundThreadGuard {
        pub fn enter() -> Self {
            Self
        }
    }
}

fn local_write_device_id(device_id: Option<String>) -> String {
    device_id
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "ark-core-rpc-local".to_string())
}

fn record_local_upsert(
    conn: &rusqlite::Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: Option<String>,
) -> Result<String, String> {
    let device_id = local_write_device_id(device_id);
    let hlc = db::bump_sync_version_vector(conn, entity_type, entity_id, &device_id, false)?;
    db::delete_sync_tombstone(conn, entity_id)?;
    Ok(hlc)
}

fn record_local_delete(
    conn: &rusqlite::Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: Option<String>,
) -> Result<String, String> {
    let device_id = local_write_device_id(device_id);
    let hlc = db::bump_sync_version_vector(conn, entity_type, entity_id, &device_id, true)?;
    db::record_sync_tombstone(conn, entity_type, entity_id, &hlc)?;
    Ok(hlc)
}

/// Строит SyncEntity из сырого serde_json::Value объекта (уже сериализованного).
/// `data_value` ожидается `Value::Object`; "id" удаляется как в `to_data_map` в db.rs.
/// Для delete: передавай `Value::Object(Map::new())` + `deleted = Some(true)`.
fn make_sync_entity(
    entity_type: &str,
    id: &str,
    data_value: Value,
    hlc: String,
    deleted: Option<bool>,
) -> SyncEntity {
    let data = match data_value {
        Value::Object(mut map) => {
            map.remove("id");
            map
        }
        _ => serde_json::Map::new(),
    };
    let origin = db::is_sequenced_usage_entity(entity_type).then(|| HLC::from_string(&hlc));
    SyncEntity {
        entity_type: entity_type.to_string(),
        id: id.to_string(),
        data,
        hlc,
        deleted,
        origin_device_id: origin.as_ref().map(|value| value.device_id.clone()),
        origin_seq: origin.map(|value| value.counter),
    }
}

/// Fan-out локального изменения всем подключённым пирам (server sessions +
/// outbound clients + relay/iroh). Не применяет entity к storage и не
/// перебивает HLC — данные уже записаны через `record_local_*`.
/// Если SYNC не запущен — тихий no-op.
async fn broadcast_local_change(entity: SyncEntity) {
    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return,
        }
    };

    runtime
        .server
        .broadcast_live_change(entity.clone(), None)
        .await;

    let clients: Vec<_> = runtime.clients.lock().await.values().cloned().collect();
    for client in clients {
        client.broadcast_live_change(entity.clone()).await;
    }
    if let Some(relay) = runtime.relay.as_ref() {
        // Ошибку отправки логируем, но не пробрасываем — live broadcast best-effort.
        if let Err(e) = relay.broadcast_live_change(entity.clone()) {
            eprintln!("[ark-core] broadcast_local_change relay error: {e}");
        }
    }
}

fn legacy_content() -> Value {
    json!({"type":"doc","content":[{"type":"paragraph"}]})
}

fn legacy_record(
    id: &str,
    legacy_type_id: &str,
    title: &str,
    props: Value,
    created_at: &str,
    deleted_at: Option<String>,
) -> ark_core::canonical_types::compatibility::LegacyRecord {
    ark_core::canonical_types::compatibility::LegacyRecord {
        id: id.into(),
        legacy_type_id: legacy_type_id.into(),
        title: title.into(),
        content: legacy_content(),
        props,
        created_at: created_at.into(),
        updated_at: created_at.into(),
        deleted_at,
    }
}

fn write_legacy_graph(
    conn: &rusqlite::Connection,
    records: &[ark_core::canonical_types::compatibility::LegacyRecord],
    device_id: Option<String>,
) -> Result<Vec<SyncEntity>, String> {
    ark_core::canonical_types::facades::write_legacy_records(conn, records, "rpc", device_id)
}

fn tombstone_legacy(
    conn: &rusqlite::Connection,
    id: &str,
    expected_type_id: &str,
    device_id: Option<String>,
) -> Result<SyncEntity, String> {
    ark_core::canonical_types::facades::delete_legacy_object(conn, id, expected_type_id, device_id)
}

// ---------------------------------------------------------------------------

async fn handle_request(request: Request) -> Result<Value, String> {
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

        Request::UpsertUsageSession {
            usage_session,
            device_id,
        } => {
            let entity = with_write_tx(|conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::upsert_usage_session(conn, &usage_session)?;
                let hlc = record_local_upsert(conn, "usage_session", &usage_session.id, device_id)?;
                Ok(make_sync_entity(
                    "usage_session",
                    &usage_session.id,
                    serde_json::to_value(&usage_session).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::DeleteUsageSession { id, device_id } => {
            let entity = with_write_tx(|conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::delete_usage_session(conn, &id)?;
                let hlc = record_local_delete(conn, "usage_session", &id, device_id)?;
                Ok(make_sync_entity(
                    "usage_session",
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

        Request::UpsertUsageEvent {
            usage_event,
            device_id,
        } => {
            let entity = with_write_tx(|conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::upsert_usage_event(conn, &usage_event)?;
                let hlc = record_local_upsert(conn, "usage_event", &usage_event.id, device_id)?;
                Ok(make_sync_entity(
                    "usage_event",
                    &usage_event.id,
                    serde_json::to_value(&usage_event).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }

        Request::DeleteUsageEvent { id, device_id } => {
            let entity = with_write_tx(|conn| {
                db::ensure_usage_sequence_migrated(
                    conn,
                    &local_write_device_id(device_id.clone()),
                )?;
                db::delete_usage_event(conn, &id)?;
                let hlc = record_local_delete(conn, "usage_event", &id, device_id)?;
                Ok(make_sync_entity(
                    "usage_event",
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
        Request::UpsertUsageSpan { usage_span } => {
            let entities = with_write_tx(|conn| {
                db::ensure_usage_sequence_migrated(conn, &usage_span.device_id)?;
                db::upsert_usage_span(conn, &usage_span)?
                    .into_iter()
                    .map(|day| {
                        let hlc = record_local_upsert(
                            conn,
                            "usage_day",
                            &day.id,
                            Some(usage_span.device_id.clone()),
                        )?;
                        Ok(make_sync_entity(
                            "usage_day",
                            &day.id,
                            serde_json::to_value(&day).unwrap_or(json!({})),
                            hlc,
                            None,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()
            })?;
            let ids = entities
                .iter()
                .map(|entity| entity.id.clone())
                .collect::<Vec<_>>();
            tokio::spawn(async move {
                for entity in entities {
                    broadcast_local_change(entity).await;
                }
            });
            Ok(json!({ "usageDayIds": ids }))
        }
        Request::GetUsageTitleTotal { query } => with_conn(|conn| {
            serde_json::to_value(db::get_usage_title_total(conn, &query)?)
                .map_err(|error| error.to_string())
        }),
        Request::GetUsageAnalytics {
            range_days,
            top_apps_limit,
            recent_sessions_limit,
        } => with_conn(|conn| {
            let snapshot = db::load_usage_analytics(
                conn,
                range_days.unwrap_or(21),
                top_apps_limit.unwrap_or(8),
                recent_sessions_limit.unwrap_or(24),
            )?;
            serde_json::to_value(snapshot).map_err(|e| e.to_string())
        }),
        Request::ListRecentUsageProcesses { limit } => with_conn(|conn| {
            let candidates = db::list_recent_usage_processes(conn, limit.unwrap_or(10))?;
            serde_json::to_value(candidates).map_err(|e| e.to_string())
        }),
        Request::SearchUsageProcesses { query, limit } => with_conn(|conn| {
            let candidates = db::search_usage_processes(conn, &query, limit.unwrap_or(10))?;
            serde_json::to_value(candidates).map_err(|e| e.to_string())
        }),
        Request::GetUsageGamePlaytimeSummary {
            bindings,
            range_start,
            range_end,
        } => with_conn(|conn| {
            let summary = db::load_usage_game_playtime_summary(
                conn,
                &bindings,
                range_start.as_deref(),
                range_end.as_deref(),
            )?;
            serde_json::to_value(summary).map_err(|e| e.to_string())
        }),
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
        Request::UpsertObject { object, device_id } => {
            let object_id = object.id.clone();
            let object_type_id = object.type_id.clone();
            let entity = with_write_tx(|conn| {
                let object = ark_core::canonical_types::ingress::prepare_object(conn, object)
                    .map_err(|error| error.to_string())?;
                db::upsert_object(conn, &object)?;
                let hlc = record_local_upsert(conn, "object", &object.id, device_id)?;
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
        Request::DeleteObject { id, device_id } => {
            let object_id = id.clone();
            let entity = with_write_tx(|conn| {
                db::delete_object(conn, &id)?;
                let hlc = record_local_delete(conn, "object", &id, device_id)?;
                Ok(make_sync_entity("object", &id, json!({}), hlc, Some(true)))
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
        Request::TypesList => with_conn(|conn| {
            serde_json::to_value(ark_core::type_registry::list_type_summaries(conn)?)
                .map_err(|e| e.to_string())
        }),
        Request::TypesRegisterPackageDefinitions { registrations } => with_write_tx(|conn| {
            for registration in &registrations {
                ark_core::type_registry::register_type(conn, registration)?;
            }
            Ok(json!(true))
        }),
        Request::TypesGet { type_id, version } => with_conn(|conn| {
            serde_json::to_value(ark_core::type_registry::get_type(
                conn,
                &type_id,
                version.as_deref(),
            )?)
            .map_err(|e| e.to_string())
        }),
        Request::TypesListVersions { type_id } => with_conn(|conn| {
            let Some(canonical) = ark_core::type_registry::resolve_type_id(conn, &type_id)? else {
                return Ok(json!([]));
            };
            serde_json::to_value(ark_core::type_registry::list_type_versions(
                conn, &canonical,
            )?)
            .map_err(|e| e.to_string())
        }),
        Request::TypesResolveAlias { alias } => with_conn(|conn| {
            serde_json::to_value(ark_core::type_registry::resolve_alias(conn, &alias)?)
                .map_err(|e| e.to_string())
        }),
        Request::ListObjectTypes => with_conn(|conn| {
            let object_types = db::list_object_types(conn)?;
            serde_json::to_value(object_types).map_err(|e| e.to_string())
        }),
        Request::GetObjectType { id } => with_conn(|conn| {
            let object_type = db::get_object_type(conn, &id)?;
            serde_json::to_value(object_type).map_err(|e| e.to_string())
        }),
        Request::UpsertObjectType {
            object_type,
            device_id,
        } => {
            // replay_pending_for_type теперь использует SAVEPOINT ark_replay_pending
            // вместо BEGIN IMMEDIATE, поэтому вкладывается в транзакцию из with_write_tx.
            // entity-строка + sync-meta записываются атомарно.
            let entity = with_write_tx(|conn| {
                db::upsert_object_type(conn, &object_type)?;
                let hlc = record_local_upsert(conn, "object_type", &object_type.id, device_id)?;
                Ok(make_sync_entity(
                    "object_type",
                    &object_type.id,
                    serde_json::to_value(&object_type).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }
        Request::DeleteObjectType { id, device_id } => {
            let entity = with_write_tx(|conn| {
                db::delete_object_type(conn, &id)?;
                let hlc = record_local_delete(conn, "object_type", &id, device_id)?;
                Ok(make_sync_entity(
                    "object_type",
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
        Request::ListObjectLinks => with_conn(|conn| {
            let object_links = db::list_object_links(conn)?;
            serde_json::to_value(object_links).map_err(|e| e.to_string())
        }),
        Request::UpsertObjectLink {
            object_link,
            device_id,
        } => {
            let entity = with_write_tx(|conn| {
                db::upsert_object_link(conn, &object_link)?;
                let hlc = record_local_upsert(conn, "object_link", &object_link.id, device_id)?;
                Ok(make_sync_entity(
                    "object_link",
                    &object_link.id,
                    serde_json::to_value(&object_link).unwrap_or(json!({})),
                    hlc,
                    None,
                ))
            })?;
            tokio::spawn(async move {
                broadcast_local_change(entity).await;
            });
            Ok(json!(true))
        }
        Request::DeleteObjectLink { id, device_id } => {
            let entity = with_write_tx(|conn| {
                db::delete_object_link(conn, &id)?;
                let hlc = record_local_delete(conn, "object_link", &id, device_id)?;
                Ok(make_sync_entity(
                    "object_link",
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

        Request::GetSyncKv { key } => with_conn(|conn| {
            let value = db::get_sync_kv(conn, &key)?;
            Ok(json!(value))
        }),

        Request::SetSyncKv { key, value } => with_conn(|conn| {
            db::set_sync_kv(conn, &key, &value)?;
            Ok(json!(true))
        }),

        Request::ExternalRefsUpsert {
            connector_id,
            account_id,
            external_type,
            external_id,
            object_id,
            revision,
            hash,
            state,
        } => with_write_tx(|conn| {
            ark_core::data_platform::ensure_schema(conn)?;
            ark_core::data_platform::upsert_external_ref(
                conn,
                &connector_id,
                &account_id,
                &external_type,
                &external_id,
                &object_id,
                revision.as_deref(),
                hash.as_deref(),
                &state,
            )?;
            Ok(json!(true))
        }),

        Request::ClearAll => with_conn(|conn| {
            db::clear_all(conn)?;
            Ok(json!(null))
        }),

        Request::DeleteTrashed => with_conn(|conn| {
            let count = db::delete_trashed(conn)?;
            Ok(json!(count))
        }),

        Request::DbBackup { dest_path } => {
            let src_path = DB_PATH
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .ok_or_else(|| "Database not initialized. Call Init first.".to_string())?;
            let pages = backup_pages_per_step();
            let pause = std::time::Duration::from_millis(backup_pause_ms());
            let dest_for_thread = dest_path.clone();
            // Fire-and-forget: копирование идёт на отдельном background-priority
            // потоке через ОТДЕЛЬНЫЙ read-коннекшн (не держит глобальный DB
            // mutex) — серийный RPC-loop сразу свободен для других запросов.
            // Завершение сообщается событием `db_backup_result`; kepler-backend
            // ждёт его, чтобы записать last_backup_ts + ротацию.
            std::thread::Builder::new()
                .name("ark-db-backup".to_string())
                .spawn(move || {
                    let _bg = background_priority::BackgroundThreadGuard::enter();
                    let result =
                        db::backup_to_file_chunked(&src_path, &dest_for_thread, pages, pause);
                    let event = match &result {
                        Ok(()) => json!({
                            "event": "db_backup_result",
                            "ok": true,
                            "dest": dest_for_thread,
                        }),
                        Err(e) => json!({
                            "event": "db_backup_result",
                            "ok": false,
                            "dest": dest_for_thread,
                            "error": e,
                        }),
                    };
                    emit_event(event);
                })
                .map_err(|e| format!("spawn db_backup thread failed: {e}"))?;
            Ok(json!({ "started": true, "dest": dest_path }))
        }

        Request::StartSync {
            space_id,
            device_id,
            device_name,
            port,
            seed_addresses,
            relay_url,
            relay_api_key,
            auth_secret,
            use_iroh,
            iroh_peer_ticket,
        } => {
            handle_start_sync(
                space_id,
                device_id,
                device_name,
                port,
                seed_addresses,
                relay_url,
                relay_api_key,
                auth_secret,
                use_iroh,
                iroh_peer_ticket,
            )
            .await
        }

        Request::StopSync => {
            handle_stop_sync().await;
            Ok(json!(true))
        }

        Request::BroadcastChange { entity } => handle_broadcast_change(entity).await,

        Request::GetConnectedPeers => handle_get_connected_peers().await,

        Request::GetSyncSnapshot => handle_get_sync_snapshot().await,

        Request::DisconnectPeer { device_id } => handle_disconnect_peer(device_id).await,

        Request::ConnectWithPairingCode { pairing_code } => {
            handle_connect_with_pairing_code(pairing_code).await
        }

        Request::LeaveSpace => {
            handle_stop_sync().await;
            // Also purge any persisted self-reference peer records so the
            // next start_sync on this or any other space does not inherit
            // phantom records. The underlying SyncServer did this inside its
            // own `start()` but we may be leaving without restarting.
            Ok(json!(true))
        }

        Request::AddSeedPeer { addresses } => handle_add_seed_peer(addresses).await,

        Request::GetOwnAddresses { port } => {
            let port = port.unwrap_or(LAN_SYNC_PORT);
            Ok(json!(get_own_addresses(port)))
        }

        Request::GetHostDeviceName => Ok(json!(get_host_device_name())),

        Request::GetOwnIrohTicket => handle_get_own_iroh_ticket().await,
    }
}
