use ark_domain::types::{Project, Tag, TodoItem};
use rusqlite::{params, Connection};

pub fn upsert_todo(connection: &Connection, todo: &TodoItem) -> Result<(), String> {
    let tag_ids_json = serde_json::to_string(&todo.tag_ids).map_err(|error| error.to_string())?;
    let checklist_json =
        serde_json::to_string(&todo.checklist_items).map_err(|error| error.to_string())?;
    let recurrence_json: Option<String> = todo
        .recurrence_rule
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| error.to_string())?;

    connection
        .execute(
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
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn batch_upsert_todos(connection: &Connection, todos: &[TodoItem]) -> Result<(), String> {
    connection
        .execute_batch("SAVEPOINT ark_batch_upsert_todos")
        .map_err(|error| error.to_string())?;
    for todo in todos {
        if let Err(error) = upsert_todo(connection, todo) {
            let _ = connection.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_batch_upsert_todos; \
                 RELEASE SAVEPOINT ark_batch_upsert_todos",
            );
            return Err(error);
        }
    }
    connection
        .execute_batch("RELEASE SAVEPOINT ark_batch_upsert_todos")
        .map_err(|error| error.to_string())
}

pub fn delete_todo(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM todos WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn upsert_project(connection: &Connection, project: &Project) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO projects
                (id, title, notes, status, scheduled_date, deadline, sort_order, color_tag,
                 area_id, created_at)
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
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn delete_project(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM projects WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn upsert_tag(connection: &Connection, tag: &Tag) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO tags (id, title, color, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                color = excluded.color,
                created_at = excluded.created_at",
            params![tag.id, tag.title, tag.color, tag.created_at],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn delete_tag(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM tags WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::load_all_todos;
    use super::{batch_upsert_todos, delete_todo, upsert_todo};
    use ark_domain::types::TodoItem;
    use rusqlite::Connection;
    use serde_json::json;

    fn todo(id: &str, title: &str) -> TodoItem {
        TodoItem {
            id: id.into(),
            title: title.into(),
            notes: None,
            priority: 0,
            scheduled_date: None,
            deadline: None,
            reminder_date: None,
            is_today: false,
            is_evening: false,
            is_someday: false,
            is_completed: false,
            completed_at: None,
            is_cancelled: false,
            cancelled_at: None,
            is_trashed: false,
            sort_order: 0,
            heading_id: None,
            project_id: None,
            area_id: None,
            tag_ids: Vec::new(),
            checklist_items: json!([]),
            recurrence_rule: None,
            created_at: "2026-01-01T00:00:00.000Z".into(),
        }
    }

    fn database() -> Connection {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("Store schema creates");
        connection
    }

    #[test]
    fn legacy_todo_persistence_runs_without_core_and_roundtrips() {
        let connection = database();

        upsert_todo(&connection, &todo("todo-1", "Keep this task")).expect("Store persists todo");
        assert_eq!(
            load_all_todos(&connection).expect("todo rows decode")[0].title,
            "Keep this task"
        );

        delete_todo(&connection, "todo-1").expect("Store deletes todo");
        assert!(load_all_todos(&connection)
            .expect("todo rows decode")
            .is_empty());
    }

    #[test]
    fn malformed_persisted_todo_json_is_reported_with_its_field() {
        for (field, value) in [
            ("tag_ids", "not-json"),
            ("checklist_items", "not-json"),
            ("recurrence_rule", "not-json"),
        ] {
            let connection = database();
            connection
                .execute(
                    &format!(
                        "INSERT INTO todos(id,title,{field},created_at) \
                         VALUES('todo-1','task',?1,'2026-01-01T00:00:00.000Z')"
                    ),
                    [value],
                )
                .expect("corrupt legacy row is representable");

            let error = load_all_todos(&connection).expect_err("corrupt JSON must be reported");
            assert!(
                error.contains(field),
                "error should identify {field}: {error}"
            );
        }
    }

    #[test]
    fn batch_upsert_rolls_back_partial_writes_inside_outer_transaction() {
        let connection = database();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_second_todo BEFORE INSERT ON todos
                 WHEN NEW.id = 'todo-2' BEGIN SELECT RAISE(ABORT, 'injected failure'); END;
                 BEGIN IMMEDIATE;",
            )
            .expect("failure trigger and outer transaction start");

        let result = batch_upsert_todos(
            &connection,
            &[todo("todo-1", "First"), todo("todo-2", "Rejected")],
        );

        assert!(result.is_err(), "injected second-row failure is returned");
        assert!(
            load_all_todos(&connection)
                .expect("outer transaction remains readable")
                .is_empty(),
            "savepoint rolls the first row back without aborting its caller transaction"
        );
        connection
            .execute_batch("ROLLBACK")
            .expect("caller-owned transaction rolls back");
    }
}
