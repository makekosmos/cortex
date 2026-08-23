// ---------------------------------------------------------------------------
// Todo CRUD
// ---------------------------------------------------------------------------

pub fn upsert_todo(conn: &Connection, todo: &TodoItem) -> Result<(), String> {
    let tag_ids_json = serde_json::to_string(&todo.tag_ids).map_err(|e| e.to_string())?;
    let checklist_json = serde_json::to_string(&todo.checklist_items).map_err(|e| e.to_string())?;
    let recurrence_json: Option<String> = todo
        .recurrence_rule
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO todos
            (id, title, notes, priority, scheduled_date, deadline, reminder_date,
             is_today, is_evening, is_someday, is_completed, completed_at,
             is_cancelled, cancelled_at, is_trashed, sort_order,
             heading_id, project_id, area_id, tag_ids, checklist_items,
             recurrence_rule, created_at)
         VALUES
            (?1, ?2, ?3, ?4, ?5, ?6, ?7,
             ?8, ?9, ?10, ?11, ?12,
             ?13, ?14, ?15, ?16,
             ?17, ?18, ?19, ?20, ?21,
             ?22, ?23)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            notes = excluded.notes,
            priority = excluded.priority,
            scheduled_date = excluded.scheduled_date,
            deadline = excluded.deadline,
            reminder_date = excluded.reminder_date,
            is_today = excluded.is_today,
            is_evening = excluded.is_evening,
            is_someday = excluded.is_someday,
            is_completed = excluded.is_completed,
            completed_at = excluded.completed_at,
            is_cancelled = excluded.is_cancelled,
            cancelled_at = excluded.cancelled_at,
            is_trashed = excluded.is_trashed,
            sort_order = excluded.sort_order,
            heading_id = excluded.heading_id,
            project_id = excluded.project_id,
            area_id = excluded.area_id,
            tag_ids = excluded.tag_ids,
            checklist_items = excluded.checklist_items,
            recurrence_rule = excluded.recurrence_rule,
            created_at = excluded.created_at",
        params![
            todo.id,
            todo.title,
            todo.notes,
            todo.priority,
            todo.scheduled_date,
            todo.deadline,
            todo.reminder_date,
            todo.is_today as i64,
            todo.is_evening as i64,
            todo.is_someday as i64,
            todo.is_completed as i64,
            todo.completed_at,
            todo.is_cancelled as i64,
            todo.cancelled_at,
            todo.is_trashed as i64,
            todo.sort_order,
            todo.heading_id,
            todo.project_id,
            todo.area_id,
            tag_ids_json,
            checklist_json,
            recurrence_json,
            todo.created_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn batch_upsert_todos(conn: &Connection, todos: &[TodoItem]) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT ark_batch_upsert_todos")
        .map_err(|e| e.to_string())?;
    for todo in todos {
        if let Err(e) = upsert_todo(conn, todo) {
            rollback_savepoint(conn, "ark_batch_upsert_todos");
            return Err(e);
        }
    }
    conn.execute_batch("RELEASE SAVEPOINT ark_batch_upsert_todos")
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_todo(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM todos WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Project CRUD
// ---------------------------------------------------------------------------

pub fn upsert_project(conn: &Connection, project: &Project) -> Result<(), String> {
    conn.execute(
        "INSERT INTO projects
            (id, title, notes, status, scheduled_date, deadline, sort_order, color_tag, area_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            notes = excluded.notes,
            status = excluded.status,
            scheduled_date = excluded.scheduled_date,
            deadline = excluded.deadline,
            sort_order = excluded.sort_order,
            color_tag = excluded.color_tag,
            area_id = excluded.area_id,
            created_at = excluded.created_at",
        params![
            project.id,
            project.title,
            project.notes,
            project.status,
            project.scheduled_date,
            project.deadline,
            project.sort_order,
            project.color_tag,
            project.area_id,
            project.created_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_project(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM projects WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Area CRUD
// ---------------------------------------------------------------------------

pub fn upsert_area(conn: &Connection, area: &Area) -> Result<(), String> {
    conn.execute(
        "INSERT INTO areas (id, title, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            sort_order = excluded.sort_order,
            created_at = excluded.created_at",
        params![area.id, area.title, area.sort_order, area.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tag CRUD
// ---------------------------------------------------------------------------

pub fn upsert_tag(conn: &Connection, tag: &Tag) -> Result<(), String> {
    conn.execute(
        "INSERT INTO tags (id, title, color, created_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            color = excluded.color,
            created_at = excluded.created_at",
        params![tag.id, tag.title, tag.color, tag.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Heading CRUD
// ---------------------------------------------------------------------------

pub fn upsert_heading(conn: &Connection, heading: &Heading) -> Result<(), String> {
    conn.execute(
        "INSERT INTO headings (id, title, sort_order, project_id)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            sort_order = excluded.sort_order,
            project_id = excluded.project_id",
        params![
            heading.id,
            heading.title,
            heading.sort_order,
            heading.project_id
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_heading(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM headings WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
