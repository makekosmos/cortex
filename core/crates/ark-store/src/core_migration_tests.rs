use rusqlite::Connection;
use serde_json::json;

const OLD_TYPES: &[(&str, &str, &str)] = &[
    ("com.kosmos.task", "task_obj", "core.task"),
    ("com.kosmos.note", "note_obj", "core.note"),
    ("com.kosmos.project", "project_obj", "core.project"),
    ("com.kosmos.tag", "tag_obj", "core.tag"),
    ("com.kosmos.person", "person_obj", "core.person"),
    ("com.kosmos.image", "image_obj", "core.image"),
    ("com.kosmos.time-entry", "time_entry_obj", "core.time-entry"),
    ("com.kosmos.game", "game_obj", "core.game"),
    ("com.kosmos.book", "book_obj", "core.book"),
];

fn seed_old_database(path: &str) -> Result<(), String> {
    let connection = crate::open::open_db(path)?;
    crate::open::init_schema(&connection)?;
    connection
        .execute_batch(
            "CREATE TABLE todos(id TEXT);
             CREATE TABLE projects(id TEXT);
             CREATE TABLE tags(id TEXT);
             CREATE TABLE areas(id TEXT);
             CREATE TABLE headings(id TEXT);",
        )
        .map_err(|error| error.to_string())?;
    let content = json!({
        "content": [{"content": [{"text": "", "type": "text"}], "type": "paragraph"}],
        "type": "doc"
    });
    let task_props = json!({
        "canceledAt": null,
        "checklist": [],
        "completedAt": null,
        "dueAt": null,
        "priority": "none",
        "recurrence": null,
        "reminderAt": null,
        "scheduledAt": null,
        "status": "todo",
        "extensions": {
            "area_id": null, "billable": false, "cancelled_at": null,
            "checklist_items": [], "completed_at": null,
            "created_at": "2026-10-04T22:01:08.744412+00:00", "deadline": null,
            "description": null, "fuel_cost": null, "heading_id": null,
            "is_cancelled": false, "is_completed": false, "is_evening": false,
            "is_someday": false, "is_today": true, "is_trashed": false,
            "price": null, "priority": 0, "project_id": null,
            "recurrence_rule": null, "reminder_date": null, "scheduled_date": null,
            "significance": null, "sort_order": 0, "status": "todo",
            "system_kind": null, "tag_ids": []
        }
    });
    for &(legacy_id, alias, _) in OLD_TYPES {
        for type_id in [legacy_id, alias] {
            connection
                .execute(
                    "INSERT INTO object_types
                     (id,name,schema_json,ui_schema_json,created_at,updated_at,system_locked)
                     VALUES(?1,?1,'{}','{}','2026-10-04','2026-10-04',0)",
                    [type_id],
                )
                .map_err(|error| error.to_string())?;
        }
        for (prefix, type_id) in [("canonical", legacy_id), ("alias", alias)] {
            let id = format!("{prefix}:{type_id}");
            let version = if legacy_id == "com.kosmos.task" {
                "1.1.0"
            } else {
                "0.0.1"
            };
            let props = if legacy_id == "com.kosmos.task" {
                task_props.clone()
            } else {
                json!({"preserve": type_id})
            };
            connection
                .execute(
                    "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,
                     created_at,updated_at) VALUES(?1,?2,?3,?1,?4,?5,'2026-10-04','2026-10-04')",
                    rusqlite::params![id, type_id, version, content.to_string(), props.to_string()],
                )
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[test]
fn startup_migrates_legacy_types_once_and_keeps_the_first_backup() -> Result<(), String> {
    let directory = crate::test_support::test_temp_dir().map_err(|error| error.to_string())?;
    let database = directory.path().join("ark.db");
    let backup = directory.path().join("ark.db.pre-core.bak");
    let database_path = database.to_str().ok_or("database path is not UTF-8")?;
    let backup_path = backup.to_str().ok_or("backup path is not UTF-8")?;
    seed_old_database(database_path)?;

    {
        let _writer =
            crate::writer::SqliteWriter::open_initialized(database_path, backup_path, "device-a")
                .map_err(|error| error.to_string())?;
    }
    let first_backup = std::fs::read(&backup).map_err(|error| error.to_string())?;
    let backup_connection = Connection::open(backup_path).map_err(|error| error.to_string())?;
    let original_types: i64 = backup_connection
        .query_row(
            "SELECT COUNT(*) FROM objects WHERE type_id LIKE 'com.kosmos.%' OR type_id LIKE '%_obj'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    assert_eq!(original_types, 18);
    drop(backup_connection);
    let connection = Connection::open(database_path).map_err(|error| error.to_string())?;
    let migrated: Vec<(String, String, String)> = {
        let mut statement = connection
            .prepare("SELECT id,type_id,type_version FROM objects ORDER BY id")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|error| error.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    let mut expected: Vec<_> = OLD_TYPES
        .iter()
        .flat_map(|(legacy_id, alias, core_id)| {
            [
                (
                    format!("canonical:{legacy_id}"),
                    (*core_id).to_string(),
                    "0.1.0".to_string(),
                ),
                (
                    format!("alias:{alias}"),
                    (*core_id).to_string(),
                    "0.1.0".to_string(),
                ),
            ]
        })
        .collect();
    expected.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(migrated, expected);
    for table in ["todos", "projects", "tags", "areas", "headings"] {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
                [table],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert!(!exists, "{table} was not dropped");
    }
    let sync_vector: String = connection
        .query_row(
            "SELECT value FROM sync_kv WHERE key=?1",
            [crate::VERSION_VECTOR_KEY],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let sync_vector: ark_domain::types::VersionVector =
        serde_json::from_str(&sync_vector).map_err(|error| error.to_string())?;
    assert_eq!(sync_vector.len(), 18);
    assert!(sync_vector.contains_key("canonical:com.kosmos.task"));
    assert!(sync_vector.contains_key("alias:task_obj"));
    let mapped_task_props: String = connection
        .query_row(
            "SELECT props_json FROM objects WHERE id='canonical:com.kosmos.task'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let mapped_task_props: serde_json::Value =
        serde_json::from_str(&mapped_task_props).map_err(|error| error.to_string())?;
    assert_eq!(mapped_task_props["planning"]["kind"], "today");
    let mapped_note_props: String = connection
        .query_row(
            "SELECT props_json FROM objects WHERE id='canonical:com.kosmos.note'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let mapped_note_props: serde_json::Value =
        serde_json::from_str(&mapped_note_props).map_err(|error| error.to_string())?;
    assert_eq!(mapped_note_props, json!({"preserve": "com.kosmos.note"}));
    drop(connection);

    {
        let _writer =
            crate::writer::SqliteWriter::open_initialized(database_path, backup_path, "device-a")
                .map_err(|error| error.to_string())?;
    }
    assert_eq!(
        std::fs::read(&backup).map_err(|error| error.to_string())?,
        first_backup
    );
    let connection = Connection::open(database_path).map_err(|error| error.to_string())?;
    let applied: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM ark_migrations WHERE id='core-types-0.1.0'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    assert_eq!(applied, 1);
    let second_sync_vector: String = connection
        .query_row(
            "SELECT value FROM sync_kv WHERE key=?1",
            [crate::VERSION_VECTOR_KEY],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let second_sync_vector: ark_domain::types::VersionVector =
        serde_json::from_str(&second_sync_vector).map_err(|error| error.to_string())?;
    assert_eq!(second_sync_vector, sync_vector);
    Ok(())
}
