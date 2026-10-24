use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

mod task_mapping;
use task_mapping::{map_legacy_task, LegacyTask};

const MIGRATION_ID: &str = "core-types-0.1.0";
const CORE_TYPES: &[(&str, &str, &str)] = &[
    ("com.kosmos.task", "task_obj", "core.task"),
    ("com.kosmos.note", "note_obj", "core.note"),
    ("com.kosmos.project", "project_obj", "core.project"),
    ("com.kosmos.tag", "tag_obj", "core.tag"),
    ("com.kosmos.person", "person_obj", "core.person"),
    ("com.kosmos.image", "image_obj", "core.image"),
    (
        "com.kosmos.time-entry",
        "time_entry_obj",
        "core.time-entry",
    ),
    ("com.kosmos.game", "game_obj", "core.game"),
    ("com.kosmos.book", "book_obj", "core.book"),
];
const LEGACY_TABLES: &[&str] = &["todos", "projects", "tags", "areas", "headings"];

pub fn backup_before_migration(connection: &Connection, backup_path: &str) -> Result<(), String> {
    if migration_applied(connection)? || !has_legacy_data(connection)? {
        return Ok(());
    }
    let reserved = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(backup_path)
    {
        Ok(file) => {
            drop(file);
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(format!("core migration backup create failed: {error}")),
    };
    let result = (|| {
        let mut destination = Connection::open(backup_path).map_err(|error| error.to_string())?;
        let backup = rusqlite::backup::Backup::new(connection, &mut destination)
            .map_err(|error| error.to_string())?;
        backup
            .run_to_completion(
                64,
                std::time::Duration::from_millis(10),
                None::<fn(rusqlite::backup::Progress)>,
            )
            .map_err(|error| error.to_string())
    })();
    if result.is_err() && reserved {
        let _ = std::fs::remove_file(backup_path);
    }
    result.map_err(|error| format!("core migration backup failed: {error}"))
}

pub fn migrate(connection: &mut Connection, device_id: &str) -> Result<(), String> {
    if migration_applied(connection)? {
        return Ok(());
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("core migration begin failed: {error}"))?;
    let result = apply(&transaction, device_id);
    match result {
        Ok(()) => transaction
            .commit()
            .map_err(|error| format!("core migration commit failed: {error}")),
        Err(error) => Err(error),
    }
}

fn apply(transaction: &Transaction<'_>, device_id: &str) -> Result<(), String> {
    transaction
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS ark_migrations (
                 id TEXT PRIMARY KEY,
                 applied_at TEXT NOT NULL
             )",
        )
        .map_err(|error| error.to_string())?;
    let applied: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM ark_migrations WHERE id=?1)",
            [MIGRATION_ID],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if applied {
        return Ok(());
    }

    for &(legacy_id, alias, core_id) in CORE_TYPES {
        let objects = load_objects(transaction, legacy_id, alias)?;
        for (type_id, row) in objects {
            let props_json = if core_id == "core.task" {
                map_legacy_task(&row)?.to_string()
            } else {
                row.props_json.clone()
            };
            let changed = transaction
                .execute(
                    "UPDATE objects SET type_id=?1,type_version='0.1.0',props_json=?2
                     WHERE id=?3 AND type_id=?4",
                    params![core_id, props_json, row.id, type_id],
                )
                .map_err(|error| error.to_string())?;
            if changed != 1 {
                return Err(format!("object {} changed during core migration", row.id));
            }
            crate::sync_versions::bump_sync_version_vector(
                transaction,
                "object",
                &row.id,
                device_id,
                false,
            )?;
        }
    }

    for table in LEGACY_TABLES {
        transaction
            .execute_batch(&format!("DROP TABLE IF EXISTS {table}"))
            .map_err(|error| error.to_string())?;
    }
    transaction
        .execute(
            "INSERT INTO ark_migrations(id,applied_at) VALUES (?1,?2)",
            params![MIGRATION_ID, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn load_objects(
    connection: &Connection,
    canonical_id: &str,
    alias: &str,
) -> Result<Vec<(String, LegacyTask)>, String> {
    if !table_exists(connection, "objects")? {
        return Ok(Vec::new());
    }
    let mut statement = connection
        .prepare(
            "SELECT id,type_id,type_version,content_json,props_json,deleted_at FROM objects
             WHERE type_id IN (?1,?2) ORDER BY id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![canonical_id, alias], |row| {
            Ok((
                row.get::<_, String>(1)?,
                LegacyTask {
                    id: row.get(0)?,
                    type_version: row.get(2)?,
                    content_json: row.get(3)?,
                    props_json: row.get(4)?,
                    deleted_at: row.get(5)?,
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn migration_applied(connection: &Connection) -> Result<bool, String> {
    if !table_exists(connection, "ark_migrations")? {
        return Ok(false);
    }
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM ark_migrations WHERE id=?1)",
            [MIGRATION_ID],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

fn has_legacy_data(connection: &Connection) -> Result<bool, String> {
    for table in LEGACY_TABLES {
        if table_exists(connection, table)? {
            return Ok(true);
        }
    }
    if !table_exists(connection, "objects")? {
        return Ok(false);
    }
    for &(canonical_id, alias, _) in CORE_TYPES {
        let found: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM objects WHERE type_id IN (?1,?2))",
                params![canonical_id, alias],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if found {
            return Ok(true);
        }
    }
    Ok(false)
}

fn table_exists(connection: &Connection, name: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
            [name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "table existence query returned no row".to_string())
}
