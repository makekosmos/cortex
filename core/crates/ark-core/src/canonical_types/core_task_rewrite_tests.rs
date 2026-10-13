use super::core_task_rewrite::{rewrite_legacy_tasks, CoreTaskRewriteReport};
use crate::db::init_schema;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

const CREATED: &str = "2026-10-04T22:01:08.744412+00:00";
const DONE_AT: &str = "2026-10-11T14:53:27.055486Z";
const CANCELLED_AT: &str = "2026-10-12T09:00:00.000000Z";

fn fresh_db() -> Result<Connection, String> {
    let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
    init_schema(&conn)?;
    Ok(conn)
}

fn backup_path(dir: &tempfile::TempDir) -> String {
    dir.path()
        .join("ark-before-core-task.db")
        .display()
        .to_string()
}

/// Задача 1.1.0 в том виде, как её пишет живая база: верхний уровень и
/// `extensions` согласованы, пустые поля — `null`.
fn legacy_props(
    status: &str,
    today: bool,
    someday: bool,
    evening: bool,
    completed: bool,
    cancelled: bool,
    scheduled: Option<&str>,
) -> Value {
    let done_at = if completed {
        json!(DONE_AT)
    } else {
        Value::Null
    };
    let cancelled_at = if cancelled {
        json!(CANCELLED_AT)
    } else {
        Value::Null
    };
    json!({
        "canceledAt": cancelled_at, "checklist": [], "completedAt": done_at,
        "dueAt": null, "priority": "none", "recurrence": null,
        "reminderAt": null, "scheduledAt": null, "status": status,
        "extensions": {
            "area_id": null, "billable": false, "cancelled_at": cancelled_at,
            "checklist_items": [], "completed_at": done_at, "created_at": CREATED,
            "deadline": null, "description": null, "fuel_cost": null, "heading_id": null,
            "is_cancelled": cancelled, "is_completed": completed, "is_evening": evening,
            "is_someday": someday, "is_today": today, "is_trashed": false,
            "price": null, "priority": 0, "project_id": null, "recurrence_rule": null,
            "reminder_date": null, "scheduled_date": scheduled, "significance": null,
            "sort_order": 0, "status": status, "system_kind": null, "tag_ids": []
        }
    })
}

fn insert_legacy(conn: &Connection, id: &str, props: &Value, trashed: bool) -> Result<(), String> {
    let content =
        r#"{"content":[{"content":[{"text":"","type":"text"}],"type":"paragraph"}],"type":"doc"}"#;
    let deleted_at = trashed.then_some(CREATED);
    conn.execute(
        "INSERT INTO objects (id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at)
         VALUES (?1, 'com.kosmos.task', '1.1.0', ?1, ?2, ?3, ?4, ?4, ?5)",
        params![id, content, props.to_string(), CREATED, deleted_at],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn stored_props(conn: &Connection, id: &str) -> Result<(String, String, Value), String> {
    conn.query_row(
        "SELECT type_id, type_version, props_json FROM objects WHERE id = ?1",
        params![id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    )
    .map_err(|e| e.to_string())
    .and_then(|(t, v, p)| Ok((t, v, serde_json::from_str(&p).map_err(|e| e.to_string())?)))
}

#[test]
fn rewrite_does_nothing_without_legacy_tasks() -> Result<(), String> {
    let conn = fresh_db()?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let report = rewrite_legacy_tasks(&conn, &backup_path(&dir), "device-a")?;
    assert_eq!(report, CoreTaskRewriteReport::default());
    assert!(!std::path::Path::new(&backup_path(&dir)).exists());
    Ok(())
}

#[test]
fn rewrite_maps_every_legacy_state_and_keeps_trash_and_extensions() -> Result<(), String> {
    let conn = fresh_db()?;
    insert_legacy(
        &conn,
        "inbox",
        &legacy_props("inbox", false, false, false, false, false, None),
        false,
    )?;
    insert_legacy(
        &conn,
        "today-evening",
        &legacy_props("todo", true, false, true, false, false, None),
        false,
    )?;
    insert_legacy(
        &conn,
        "someday",
        &legacy_props("todo", false, true, false, false, false, None),
        false,
    )?;
    insert_legacy(
        &conn,
        "anytime",
        &legacy_props("todo", false, false, false, false, false, None),
        false,
    )?;
    insert_legacy(
        &conn,
        "done-today",
        &legacy_props("done", true, false, false, true, false, None),
        false,
    )?;
    insert_legacy(
        &conn,
        "cancelled",
        &legacy_props("todo", false, false, false, false, true, None),
        false,
    )?;
    let mut trashed = legacy_props("todo", true, false, false, false, false, None);
    trashed["extensions"]["is_trashed"] = json!(true);
    insert_legacy(&conn, "trashed", &trashed, true)?;
    let mut priced = legacy_props(
        "todo",
        false,
        false,
        false,
        false,
        false,
        Some("2026-10-20"),
    );
    priced["extensions"]["price"] = json!(150);
    insert_legacy(&conn, "scheduled", &priced, false)?;

    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let report = rewrite_legacy_tasks(&conn, &backup_path(&dir), "device-a")?;
    assert_eq!(
        report,
        CoreTaskRewriteReport {
            rewritten: 8,
            live: 7,
            trashed: 1
        }
    );

    let (type_id, version, props) = stored_props(&conn, "inbox")?;
    assert_eq!((type_id.as_str(), version.as_str()), ("core.task", "0.1.0"));
    assert_eq!(props["state"], json!({"kind": "open", "at": null}));
    assert_eq!(
        props["planning"],
        json!({"kind": "inbox", "period": "any", "date": null})
    );
    assert_eq!(props["reminders"], json!([]));
    assert_eq!(props["recurrence"], Value::Null);
    assert_eq!(props["checklist"], json!([]));
    assert_eq!(props["priority"], json!("none"));
    assert_eq!(props["extensions"]["sort_order"], json!(0));
    assert!(props["extensions"].get("is_today").is_none());
    assert!(props["extensions"].get("status").is_none());

    assert_eq!(
        stored_props(&conn, "today-evening")?.2["planning"],
        json!({"kind": "today", "period": "evening", "date": null})
    );
    assert_eq!(
        stored_props(&conn, "someday")?.2["planning"]["kind"],
        json!("someday")
    );
    assert_eq!(
        stored_props(&conn, "anytime")?.2["planning"]["kind"],
        json!("anytime")
    );
    assert_eq!(
        stored_props(&conn, "done-today")?.2["state"],
        json!({"kind": "done", "at": DONE_AT})
    );
    assert_eq!(
        stored_props(&conn, "done-today")?.2["planning"]["kind"],
        json!("today")
    );
    assert_eq!(
        stored_props(&conn, "cancelled")?.2["state"],
        json!({"kind": "canceled", "at": CANCELLED_AT})
    );
    assert_eq!(
        stored_props(&conn, "scheduled")?.2["planning"],
        json!({"kind": "date", "period": "any", "date": "2026-10-20"})
    );
    assert_eq!(
        stored_props(&conn, "scheduled")?.2["extensions"]["price"],
        json!(150)
    );

    let deleted_at: Option<String> = conn
        .query_row(
            "SELECT deleted_at FROM objects WHERE id = 'trashed'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    assert_eq!(deleted_at.as_deref(), Some(CREATED));
    let legacy_left: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM objects WHERE type_id = 'com.kosmos.task'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    assert_eq!(legacy_left, 0);
    Ok(())
}

#[test]
fn rewrite_writes_a_backup_of_the_legacy_rows_before_changing_them() -> Result<(), String> {
    let conn = fresh_db()?;
    insert_legacy(
        &conn,
        "inbox",
        &legacy_props("inbox", false, false, false, false, false, None),
        false,
    )?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = backup_path(&dir);
    rewrite_legacy_tasks(&conn, &path, "device-a")?;

    let backup = Connection::open(&path).map_err(|e| e.to_string())?;
    let legacy: i64 = backup
        .query_row("SELECT COUNT(*) FROM objects WHERE type_id = 'com.kosmos.task' AND type_version = '1.1.0'", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    assert_eq!(legacy, 1);
    Ok(())
}

#[test]
fn rewrite_stops_before_backup_or_write_on_data_it_cannot_carry() -> Result<(), String> {
    let conn = fresh_db()?;
    insert_legacy(
        &conn,
        "fine",
        &legacy_props("todo", false, false, false, false, false, None),
        false,
    )?;
    let mut linked = legacy_props("todo", false, false, false, false, false, None);
    linked["extensions"]["project_id"] = json!("project-1");
    insert_legacy(&conn, "linked", &linked, false)?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = backup_path(&dir);

    let error = rewrite_legacy_tasks(&conn, &path, "device-a")
        .err()
        .unwrap_or_default();
    assert!(
        error.contains("linked") && error.contains("project_id is not migrated"),
        "{error}"
    );
    assert!(!std::path::Path::new(&path).exists());
    let (type_id, _, _) = stored_props(&conn, "fine")?;
    assert_eq!(type_id, "com.kosmos.task");
    Ok(())
}

#[test]
fn rewrite_refuses_to_overwrite_an_existing_backup() -> Result<(), String> {
    let conn = fresh_db()?;
    insert_legacy(
        &conn,
        "inbox",
        &legacy_props("inbox", false, false, false, false, false, None),
        false,
    )?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = backup_path(&dir);
    std::fs::write(&path, b"existing").map_err(|e| e.to_string())?;

    assert!(rewrite_legacy_tasks(&conn, &path, "device-a").is_err());
    assert_eq!(
        std::fs::read(&path).map_err(|e| e.to_string())?,
        b"existing"
    );
    let (type_id, _, _) = stored_props(&conn, "inbox")?;
    assert_eq!(type_id, "com.kosmos.task");
    Ok(())
}

#[test]
fn rewrite_refuses_scheduled_date_together_with_today() -> Result<(), String> {
    let conn = fresh_db()?;
    insert_legacy(
        &conn,
        "both",
        &legacy_props("todo", true, false, false, false, false, Some("2026-10-20")),
        false,
    )?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    assert!(rewrite_legacy_tasks(&conn, &backup_path(&dir), "device-a").is_err());
    Ok(())
}

#[test]
fn second_rewrite_is_a_noop() -> Result<(), String> {
    let conn = fresh_db()?;
    insert_legacy(
        &conn,
        "inbox",
        &legacy_props("inbox", false, false, false, false, false, None),
        false,
    )?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    rewrite_legacy_tasks(&conn, &backup_path(&dir), "device-a")?;
    let again = rewrite_legacy_tasks(
        &conn,
        &dir.path().join("second.db").display().to_string(),
        "device-a",
    )?;
    assert_eq!(again, CoreTaskRewriteReport::default());
    Ok(())
}
