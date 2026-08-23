// ---------------------------------------------------------------------------
// Bulk operations
// ---------------------------------------------------------------------------

pub fn clear_all(conn: &Connection) -> Result<(), String> {
    if object_search_fts_exists(conn).unwrap_or(false) {
        conn.execute("DELETE FROM object_search_fts", [])
            .map_err(|e| e.to_string())?;
    }
    let phase3_completed = phase3_migration_completed(conn)?;
    for table in [
        "object_migration_quarantine",
        "object_local_state",
        "canonical_migration_items",
        "canonical_migration_runs",
        "object_sync_versions",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if exists {
            conn.execute(&format!("DELETE FROM {table}"), [])
                .map_err(|e| e.to_string())?;
        } else if phase3_completed {
            return Err(format!("clear_all missing owned table: {table}"));
        }
    }
    conn.execute_batch(
        "DELETE FROM object_links;
         DELETE FROM objects;
         DELETE FROM sync_pending_objects;
         DELETE FROM object_type_aliases WHERE canonical_type_id IN (SELECT id FROM object_types WHERE system_locked = 0);
         DELETE FROM object_type_versions WHERE type_id IN (SELECT id FROM object_types WHERE system_locked = 0);
         DELETE FROM object_types WHERE system_locked = 0;
         DELETE FROM usage_days;
         DELETE FROM usage_events;
         DELETE FROM usage_sessions;
         DELETE FROM tracked_apps;
         DELETE FROM todos;
         DELETE FROM projects;
         DELETE FROM areas;
         DELETE FROM tags;
         DELETE FROM headings;
         DELETE FROM sync_tombstones;
         DELETE FROM sync_kv;",
    )
    .map_err(|e| e.to_string())
}

pub fn delete_trashed(conn: &Connection) -> Result<usize, String> {
    conn.execute("DELETE FROM todos WHERE is_trashed = 1", [])
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Load all
// ---------------------------------------------------------------------------

pub fn load_all(conn: &Connection) -> Result<LoadAllData, String> {
    let mut todos = load_all_todos(conn)?;
    let mut projects = load_all_projects(conn)?;
    let mut areas = load_all_areas(conn)?;
    let mut tags = load_all_tags(conn)?;
    let mut headings = load_all_headings(conn)?;
    let mut tracked_apps = load_all_tracked_apps(conn)?;
    let mut usage_sessions = load_all_usage_sessions(conn)?;
    let mut usage_events = load_all_usage_events(conn)?;
    let mut objects = list_objects(conn)?;
    let mut object_types = list_object_types(conn)?;
    let mut object_type_summaries = type_registry::list_type_summaries(conn)?;
    let mut object_type_versions = type_registry::list_all_type_versions(conn)?;
    let mut object_type_aliases = type_registry::list_aliases(conn)?;
    let mut object_links = list_object_links(conn)?;

    // The export contract is deterministic even when SQLite's query planner or
    // insertion order changes.  Keep the legacy collections and the additive
    // registry collections stable by their primary-key tuples.
    todos.sort_by(|a, b| a.id.cmp(&b.id));
    projects.sort_by(|a, b| a.id.cmp(&b.id));
    areas.sort_by(|a, b| a.id.cmp(&b.id));
    tags.sort_by(|a, b| a.id.cmp(&b.id));
    headings.sort_by(|a, b| a.id.cmp(&b.id));
    tracked_apps.sort_by(|a, b| a.id.cmp(&b.id));
    usage_sessions.sort_by(|a, b| a.id.cmp(&b.id));
    usage_events.sort_by(|a, b| a.id.cmp(&b.id));
    objects.sort_by(|a, b| a.id.cmp(&b.id));
    object_types.sort_by(|a, b| a.id.cmp(&b.id));
    object_type_summaries.sort_by(|a, b| a.type_id.cmp(&b.type_id));
    object_type_versions.sort_by(|a, b| {
        a.type_id
            .cmp(&b.type_id)
            .then_with(|| a.version.cmp(&b.version))
    });
    object_type_aliases.sort_by(|a, b| a.alias.cmp(&b.alias));
    object_links.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(LoadAllData {
        todos,
        projects,
        areas,
        tags,
        headings,
        tracked_apps,
        usage_sessions,
        usage_events,
        objects,
        object_types,
        object_type_summaries,
        object_type_versions,
        object_type_aliases,
        object_links,
    })
}

fn load_all_todos(conn: &Connection) -> Result<Vec<TodoItem>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, notes, priority, scheduled_date, deadline, reminder_date,
                    is_today, is_evening, is_someday, is_completed, completed_at,
                    is_cancelled, cancelled_at, is_trashed, sort_order,
                    heading_id, project_id, area_id, tag_ids, checklist_items,
                    recurrence_rule, created_at
             FROM todos",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let tag_ids_str: String = row.get(19)?;
            let checklist_str: String = row.get(20)?;
            let recurrence_str: Option<String> = row.get(21)?;

            let tag_ids: Vec<String> = serde_json::from_str(&tag_ids_str).unwrap_or_default();
            let checklist_items: Value = serde_json::from_str(&checklist_str).unwrap_or(json!([]));
            let recurrence_rule: Option<Value> = recurrence_str
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());

            Ok(TodoItem {
                id: row.get(0)?,
                title: row.get(1)?,
                notes: row.get(2)?,
                priority: row.get(3)?,
                scheduled_date: row.get(4)?,
                deadline: row.get(5)?,
                reminder_date: row.get(6)?,
                is_today: row.get::<_, i64>(7)? != 0,
                is_evening: row.get::<_, i64>(8)? != 0,
                is_someday: row.get::<_, i64>(9)? != 0,
                is_completed: row.get::<_, i64>(10)? != 0,
                completed_at: row.get(11)?,
                is_cancelled: row.get::<_, i64>(12)? != 0,
                cancelled_at: row.get(13)?,
                is_trashed: row.get::<_, i64>(14)? != 0,
                sort_order: row.get(15)?,
                heading_id: row.get(16)?,
                project_id: row.get(17)?,
                area_id: row.get(18)?,
                tag_ids,
                checklist_items,
                recurrence_rule,
                created_at: row.get(22)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_all_projects(conn: &Connection) -> Result<Vec<Project>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, notes, status, scheduled_date, deadline,
                    sort_order, color_tag, area_id, created_at
             FROM projects",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                title: row.get(1)?,
                notes: row.get(2)?,
                status: row.get(3)?,
                scheduled_date: row.get(4)?,
                deadline: row.get(5)?,
                sort_order: row.get(6)?,
                color_tag: row.get(7)?,
                area_id: row.get(8)?,
                created_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_all_areas(conn: &Connection) -> Result<Vec<Area>, String> {
    let mut stmt = conn
        .prepare("SELECT id, title, sort_order, created_at FROM areas")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Area {
                id: row.get(0)?,
                title: row.get(1)?,
                sort_order: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_all_tags(conn: &Connection) -> Result<Vec<Tag>, String> {
    let mut stmt = conn
        .prepare("SELECT id, title, color, created_at FROM tags")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Tag {
                id: row.get(0)?,
                title: row.get(1)?,
                color: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_all_headings(conn: &Connection) -> Result<Vec<Heading>, String> {
    let mut stmt = conn
        .prepare("SELECT id, title, sort_order, project_id FROM headings")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Heading {
                id: row.get(0)?,
                title: row.get(1)?,
                sort_order: row.get(2)?,
                project_id: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_all_tracked_apps(conn: &Connection) -> Result<Vec<TrackedApp>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, platform, exe_path, normalized_exe_path, process_name,
                    display_name, publisher, icon_ref, first_seen_at, last_seen_at
             FROM tracked_apps
             ORDER BY last_seen_at DESC, id ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(TrackedApp {
                id: row.get(0)?,
                platform: row.get(1)?,
                exe_path: row.get(2)?,
                normalized_exe_path: row.get(3)?,
                process_name: row.get(4)?,
                display_name: row.get(5)?,
                publisher: row.get(6)?,
                icon_ref: row.get(7)?,
                first_seen_at: row.get(8)?,
                last_seen_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_all_usage_sessions(conn: &Connection) -> Result<Vec<UsageSession>, String> {
    load_usage_sessions_page(conn, -1, 0)
}

fn load_usage_session(conn: &Connection, id: &str) -> Result<Option<UsageSession>, String> {
    conn.query_row(
        "SELECT id, tracked_app_id, device_id, device_name, platform, started_at,
                ended_at, runtime_ms, foreground_ms, idle_ms, window_title, process_name,
                exe_path, pid_start, pid_end, meta_json
         FROM usage_sessions WHERE id = ?1",
        params![id],
        |row| {
            let meta_json: String = row.get(15)?;
            Ok(UsageSession {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                device_id: row.get(2)?,
                device_name: row.get(3)?,
                platform: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                runtime_ms: row.get(7)?,
                foreground_ms: row.get(8)?,
                idle_ms: row.get(9)?,
                window_title: row.get(10)?,
                process_name: row.get(11)?,
                exe_path: row.get(12)?,
                pid_start: row.get(13)?,
                pid_end: row.get(14)?,
                meta_json: serde_json::from_str(&meta_json).unwrap_or_else(|_| json!({})),
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn load_usage_sessions_page(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<UsageSession>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, tracked_app_id, device_id, device_name, platform, started_at,
                    ended_at, runtime_ms, foreground_ms, idle_ms, window_title, process_name,
                    exe_path, pid_start, pid_end, meta_json
             FROM usage_sessions
             ORDER BY id ASC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit, offset], |row| {
            let meta_json_str: String = row.get(15)?;
            let meta_json: Value = serde_json::from_str(&meta_json_str).unwrap_or(json!({}));
            Ok(UsageSession {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                device_id: row.get(2)?,
                device_name: row.get(3)?,
                platform: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                runtime_ms: row.get(7)?,
                foreground_ms: row.get(8)?,
                idle_ms: row.get(9)?,
                window_title: row.get(10)?,
                process_name: row.get(11)?,
                exe_path: row.get(12)?,
                pid_start: row.get(13)?,
                pid_end: row.get(14)?,
                meta_json,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_all_usage_events(conn: &Connection) -> Result<Vec<UsageEvent>, String> {
    load_usage_events_page(conn, -1, 0)
}

fn load_usage_event(conn: &Connection, id: &str) -> Result<Option<UsageEvent>, String> {
    conn.query_row(
        "SELECT id, tracked_app_id, usage_session_id, device_id, device_name, platform,
                occurred_at, kind, window_title, process_name, exe_path, pid,
                is_foreground, is_idle, meta_json
         FROM usage_events WHERE id = ?1",
        params![id],
        |row| {
            let meta_json: String = row.get(14)?;
            Ok(UsageEvent {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                usage_session_id: row.get(2)?,
                device_id: row.get(3)?,
                device_name: row.get(4)?,
                platform: row.get(5)?,
                occurred_at: row.get(6)?,
                kind: row.get(7)?,
                window_title: row.get(8)?,
                process_name: row.get(9)?,
                exe_path: row.get(10)?,
                pid: row.get(11)?,
                is_foreground: row.get::<_, i64>(12)? != 0,
                is_idle: row.get::<_, i64>(13)? != 0,
                meta_json: serde_json::from_str(&meta_json).unwrap_or_else(|_| json!({})),
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn load_usage_events_page(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<UsageEvent>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, tracked_app_id, usage_session_id, device_id, device_name, platform,
                    occurred_at, kind, window_title, process_name, exe_path, pid,
                    is_foreground, is_idle, meta_json
             FROM usage_events
             ORDER BY id ASC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit, offset], |row| {
            let meta_json_str: String = row.get(14)?;
            let meta_json: Value = serde_json::from_str(&meta_json_str).unwrap_or(json!({}));
            Ok(UsageEvent {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                usage_session_id: row.get(2)?,
                device_id: row.get(3)?,
                device_name: row.get(4)?,
                platform: row.get(5)?,
                occurred_at: row.get(6)?,
                kind: row.get(7)?,
                window_title: row.get(8)?,
                process_name: row.get(9)?,
                exe_path: row.get(10)?,
                pid: row.get(11)?,
                is_foreground: row.get::<_, i64>(12)? != 0,
                is_idle: row.get::<_, i64>(13)? != 0,
                meta_json,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

