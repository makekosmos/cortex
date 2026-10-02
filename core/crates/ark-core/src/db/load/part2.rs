
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
