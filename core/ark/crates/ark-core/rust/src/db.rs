use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{Duration, Utc};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};

use crate::canonical_types::migration;
pub use crate::canonical_types::pending::{
    count_pending_for_type, insert_pending_object, replay_pending_for_type,
};
use crate::hlc::HLC;
use crate::schema::{CREATE_OBJECT_SEARCH_FTS, CREATE_TABLES};
use crate::sync_server::StorageBackend;
use crate::type_registry;
use crate::types::*;

const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";
const USAGE_SEQUENCE_MIGRATION_KEY: &str = "usage_sync.sequence_v1";

pub fn is_sequenced_usage_entity(entity_type: &str) -> bool {
    matches!(entity_type, "usage_session" | "usage_event" | "usage_day")
}

fn usage_cursor_key(device_id: &str) -> String {
    format!("@usage:{device_id}")
}

// ---------------------------------------------------------------------------
// Open and init
// ---------------------------------------------------------------------------

pub fn open_db(path: &str) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// Online SQLite integrity check. Возвращает Err если DB corrupted.
/// Вызывается из init_schema до миграций — silent corruption хуже чем
/// fail-loud при старте (см. 2026-05-18 hardening loop AC2).
pub fn check_integrity(conn: &Connection) -> Result<(), String> {
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| format!("integrity_check query failed: {e}"))?;
    if result != "ok" {
        return Err(format!("ARK DB integrity check failed: {result}"));
    }
    Ok(())
}

fn phase3_migration_completed(conn: &Connection) -> Result<bool, String> {
    let table_exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='canonical_migration_runs')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !table_exists {
        return Ok(false);
    }
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM canonical_migration_runs WHERE contract_version='phase3-canonical-v1' AND status='completed')",
        [],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

/// Initializes the schema and all Phase 2 prerequisites, but does not run Phase 3.
#[doc(hidden)]
pub fn init_schema_prerequisites_for_phase3(conn: &Connection) -> Result<(), String> {
    check_integrity(conn)?;
    // Prevent repeated replacement of the large sync version-vector value
    // from leaving permanent freelist growth. Existing databases adopt this
    // mode after the explicit maintenance VACUUM.
    conn.pragma_update(None, "auto_vacuum", "FULL")
        .map_err(|e| e.to_string())?;
    conn.execute_batch(CREATE_TABLES)
        .map_err(|e| e.to_string())?;
    ensure_usage_runtime_ms(conn)?;
    conn.execute_batch("SAVEPOINT ark_phase2_init")
        .map_err(|e| e.to_string())?;
    let phase2_result = (|| {
        type_registry::migrate_phase2(conn)?;
        crate::data_platform::ensure_schema(conn)?;
        let object_search_fts_enabled = ensure_object_search_fts(conn).is_ok();
        if object_search_fts_enabled {
            rebuild_object_search_fts(conn)?;
        }
        Ok::<bool, String>(object_search_fts_enabled)
    })();
    match phase2_result {
        Ok(_) => conn
            .execute_batch("RELEASE SAVEPOINT ark_phase2_init")
            .map_err(|e| e.to_string())?,
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_phase2_init; RELEASE SAVEPOINT ark_phase2_init",
            );
            return Err(error);
        }
    }
    Ok(())
}

pub fn init_schema(conn: &Connection) -> Result<(), String> {
    init_schema_prerequisites_for_phase3(conn)?;
    migration::migrate_phase3(conn)
        .map(|_| ())
        .map_err(|error| format!("phase3 migration during init_schema failed: {error:?}"))
}

fn ensure_usage_runtime_ms(conn: &Connection) -> Result<(), String> {
    let has_runtime_ms = conn
        .prepare("PRAGMA table_info(usage_sessions)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .iter()
        .any(|name| name == "runtime_ms");

    if !has_runtime_ms {
        conn.execute_batch(
            "ALTER TABLE usage_sessions ADD COLUMN runtime_ms INTEGER NOT NULL DEFAULT 0;",
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "UPDATE usage_sessions
         SET runtime_ms = COALESCE(foreground_ms, 0) + COALESCE(idle_ms, 0)
         WHERE runtime_ms = 0 AND (COALESCE(foreground_ms, 0) > 0 OR COALESCE(idle_ms, 0) > 0)",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Online backup ARK DB в указанный destination path. Source connection
/// может оставаться live (SQLite Online Backup API safe для concurrent
/// readers/writer). Вызывается из db_backup scheduler в kepler-backend
/// через `Request::DbBackup`.
pub fn backup_to_file(conn: &Connection, dest_path: &str) -> Result<(), String> {
    conn.backup(rusqlite::DatabaseName::Main, dest_path, None)
        .map_err(|e| format!("backup_to_file failed: {e}"))
}

/// Online backup через ОТДЕЛЬНЫЙ read-коннекшн к `src_db_path` — НЕ держит
/// глобальный DB mutex, поэтому обычные ARK ops продолжают работать во время
/// копирования. Копирование идёт chunk'ами по `pages_per_step` страниц с паузой
/// `pause` между шагами, чтобы тяжёлый I/O не насыщал диск (стабильность ПК
/// важнее скорости бэкапа). WAL + busy_timeout source DB дают консистентный
/// снапшот при concurrent writer. Вызывается из `Request::DbBackup` на отдельном
/// background-priority потоке (см. ark-core-rpc main.rs).
pub fn backup_to_file_chunked(
    src_db_path: &str,
    dest_path: &str,
    pages_per_step: i32,
    pause: std::time::Duration,
) -> Result<(), String> {
    let src = open_db(src_db_path)?;
    let mut dst = Connection::open(dest_path)
        .map_err(|e| format!("backup_to_file_chunked open dest failed: {e}"))?;
    let backup = rusqlite::backup::Backup::new(&src, &mut dst)
        .map_err(|e| format!("backup_to_file_chunked init failed: {e}"))?;
    backup
        .run_to_completion(
            pages_per_step,
            pause,
            None::<fn(rusqlite::backup::Progress)>,
        )
        .map_err(|e| format!("backup_to_file_chunked run failed: {e}"))
}

// Legacy registry definitions are owned by canonical_types::definitions.
// Historical rows remain readable through the Phase 3 migration/archive path.

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
// Generic object model CRUD
// ---------------------------------------------------------------------------

fn serialize_json(value: &Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| e.to_string())
}

fn parse_json_or_default(raw: String) -> Value {
    serde_json::from_str(&raw).unwrap_or_else(|_| json!({}))
}

pub fn upsert_object_type(conn: &Connection, object_type: &ObjectType) -> Result<(), String> {
    // A canonical ID cannot reuse an existing alias name. Check this before
    // mutating object_types so the legacy write remains fail-closed and atomic.
    let has_aliases: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_aliases')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if has_aliases
        && conn
            .query_row(
                "SELECT 1 FROM object_type_aliases WHERE alias=?1 LIMIT 1",
                params![object_type.id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .is_some()
    {
        return Err("alias collides with canonical type".into());
    }

    conn.execute(
        "INSERT INTO object_types
            (id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            schema_json = excluded.schema_json,
            ui_schema_json = excluded.ui_schema_json,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at,
            system_locked = excluded.system_locked",
        params![
            object_type.id,
            object_type.name,
            object_type.schema_json,
            object_type.ui_schema_json,
            object_type.created_at,
            object_type.updated_at,
            object_type.system_locked as i64,
        ],
    )
    .map_err(|e| e.to_string())?;
    let has_versions: bool = conn
        .query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_versions')", [], |row| row.get::<_, i64>(0))
        .map_err(|e| e.to_string())? != 0;
    if has_versions {
        type_registry::ensure_legacy_type_version(
            conn,
            &object_type.id,
            &object_type.schema_json,
            &object_type.ui_schema_json,
            &object_type.created_at,
        )?;
        let (compat_version, full_hash) = type_registry::legacy_compatibility_version(
            &object_type.schema_json,
            &object_type.ui_schema_json,
        )?;
        let existing_hash = conn
            .query_row(
                "SELECT schema_hash FROM object_type_versions WHERE type_id=?1 AND version=?2",
                params![object_type.id, compat_version],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(existing_hash) = existing_hash {
            if existing_hash != full_hash {
                return Err("legacy compatibility version hash conflict".into());
            }
        } else {
            conn.execute("INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES (?1,?2,?3,?4,'{}','[]','{}',?5,?6)", params![object_type.id, compat_version, object_type.schema_json, object_type.ui_schema_json, full_hash, object_type.created_at]).map_err(|e| e.to_string())?;
        }
        conn.execute(
            "UPDATE object_types SET current_version=?1,status='active' WHERE id=?2",
            params![compat_version, object_type.id],
        )
        .map_err(|e| e.to_string())?;
    }
    let has_current_version: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('object_types') WHERE name='current_version')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    let current_version = if has_current_version {
        conn.query_row(
            "SELECT current_version FROM object_types WHERE id=?1",
            params![object_type.id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?
    } else {
        "0.0.0-legacy".to_string()
    };
    replay_pending_for_type(conn, &object_type.id, &current_version)?;
    if current_version != type_registry::LEGACY_VERSION {
        replay_pending_for_type(conn, &object_type.id, type_registry::LEGACY_VERSION)?;
    }
    Ok(())
}

pub fn delete_object_type(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE object_types SET status='deprecated' WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase 2: schema drift hold-and-replay (sync_pending_objects)
// ---------------------------------------------------------------------------

/// Проверить, существует ли `object_type` с указанным id.
pub fn is_object_type_known(conn: &Connection, type_id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM object_types WHERE id = ?1",
        params![type_id],
        |_| Ok(true),
    )
    .optional()
    .map(|opt| opt.unwrap_or(false))
    .map_err(|e| e.to_string())
}

pub fn is_object_definition_known(
    conn: &Connection,
    type_id: &str,
    type_version: &str,
) -> Result<bool, String> {
    let Ok((canonical_type_id, canonical_version)) =
        crate::type_registry::resolve_object_type_identity(conn, type_id, Some(type_version))
    else {
        return Ok(false);
    };
    conn.query_row(
        "SELECT 1 FROM object_type_versions WHERE type_id=?1 AND version=?2",
        params![canonical_type_id, canonical_version],
        |_| Ok(true),
    )
    .optional()
    .map(|opt| opt.unwrap_or(false))
    .map_err(|e| e.to_string())
}

pub fn list_object_types(conn: &Connection) -> Result<Vec<ObjectType>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked
             FROM object_types
             WHERE status != 'deprecated'
             ORDER BY system_locked DESC, name ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ObjectType {
                id: row.get(0)?,
                name: row.get(1)?,
                schema_json: row.get(2)?,
                ui_schema_json: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                system_locked: row.get::<_, i64>(6)? != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_object_type(conn: &Connection, id: &str) -> Result<Option<ObjectType>, String> {
    conn.query_row(
        "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked
         FROM object_types
         WHERE id = ?1 AND status != 'deprecated'",
        params![id],
        |row| {
            Ok(ObjectType {
                id: row.get(0)?,
                name: row.get(1)?,
                schema_json: row.get(2)?,
                ui_schema_json: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                system_locked: row.get::<_, i64>(6)? != 0,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn ensure_object_search_fts(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(CREATE_OBJECT_SEARCH_FTS)
        .map_err(|e| e.to_string())
}

fn object_search_fts_exists(conn: &Connection) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM sqlite_master
             WHERE type = 'table' AND name = 'object_search_fts'
         )",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(|e| e.to_string())
}

fn rebuild_object_search_fts(conn: &Connection) -> Result<(), String> {
    if !object_search_fts_exists(conn)? {
        return Ok(());
    }

    conn.execute("DELETE FROM object_search_fts", [])
        .map_err(|e| e.to_string())?;
    for object in list_objects(conn)? {
        index_object_for_search(conn, &object)?;
    }
    Ok(())
}

fn index_object_for_search(conn: &Connection, object: &ArkObject) -> Result<(), String> {
    if !object_search_fts_exists(conn)? {
        return Ok(());
    }

    conn.execute(
        "DELETE FROM object_search_fts WHERE object_id = ?1",
        params![object.id],
    )
    .map_err(|e| e.to_string())?;

    if object.deleted_at.is_some() {
        return Ok(());
    }

    conn.execute(
        "INSERT INTO object_search_fts (object_id, title, body, props)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            object.id,
            object.title,
            extract_plain_text_from_value(&object.content_json),
            extract_plain_text_from_value(&object.props_json),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn remove_object_from_search(conn: &Connection, id: &str) -> Result<(), String> {
    if !object_search_fts_exists(conn)? {
        return Ok(());
    }

    conn.execute(
        "DELETE FROM object_search_fts WHERE object_id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn rollback_savepoint(conn: &Connection, name: &str) {
    let _ = conn.execute_batch(&format!(
        "ROLLBACK TO SAVEPOINT {name}; RELEASE SAVEPOINT {name};"
    ));
}

pub fn upsert_object(conn: &Connection, object: &ArkObject) -> Result<(), String> {
    let registry_ready: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_versions')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if registry_ready {
        let (canonical_type_id, canonical_version) =
            crate::type_registry::resolve_object_type_identity(
                conn,
                &object.type_id,
                Some(&object.type_version),
            )?;
        let mut canonical_object = object.clone();
        canonical_object.type_id = canonical_type_id;
        canonical_object.type_version = canonical_version;
        return upsert_object_inner(conn, &canonical_object);
    }
    upsert_object_inner(conn, object)
}

fn upsert_object_inner(conn: &Connection, object: &ArkObject) -> Result<(), String> {
    let has_type_version: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('objects') WHERE name='type_version')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if !has_type_version {
        conn.execute_batch(
            "ALTER TABLE objects ADD COLUMN type_version TEXT NOT NULL DEFAULT '0.0.0-legacy'",
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute_batch("SAVEPOINT ark_upsert_object")
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        // Do not use SQLite REPLACE here: it deletes the old row first and cascades.
        // См. postmortems.md § 2026-06-04.
        conn.execute(
            "INSERT INTO objects
                (id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                type_id = excluded.type_id,
                type_version = excluded.type_version,
                title = excluded.title,
                content_json = excluded.content_json,
                props_json = excluded.props_json,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at",
            params![
                object.id,
                object.type_id,
                object.type_version,
                object.title,
                serialize_json(&object.content_json)?,
                serialize_json(&object.props_json)?,
                object.created_at,
                object.updated_at,
                object.deleted_at,
            ],
        )
        .map_err(|e| e.to_string())?;
        index_object_for_search(conn, object)
    })();

    if let Err(error) = result {
        rollback_savepoint(conn, "ark_upsert_object");
        return Err(error);
    }

    conn.execute_batch("RELEASE SAVEPOINT ark_upsert_object")
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_object(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT ark_delete_object")
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        conn.execute("DELETE FROM objects WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        remove_object_from_search(conn, id)
    })();

    if let Err(error) = result {
        rollback_savepoint(conn, "ark_delete_object");
        return Err(error);
    }

    conn.execute_batch("RELEASE SAVEPOINT ark_delete_object")
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn map_ark_object_row(row: &Row<'_>) -> rusqlite::Result<ArkObject> {
    Ok(ArkObject {
        id: row.get(0)?,
        type_id: row.get(1)?,
        type_version: row.get(2)?,
        title: row.get(3)?,
        content_json: parse_json_or_default(row.get(4)?),
        props_json: parse_json_or_default(row.get(5)?),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
    })
}

fn map_ark_object_summary_row(row: &Row<'_>) -> rusqlite::Result<ArkObjectSummary> {
    Ok(ArkObjectSummary {
        id: row.get(0)?,
        type_id: row.get(1)?,
        type_version: row.get(2)?,
        title: row.get(3)?,
        props_json: parse_json_or_default(row.get(4)?),
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        deleted_at: row.get(7)?,
    })
}

pub fn list_objects(conn: &Connection) -> Result<Vec<ArkObject>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
             FROM objects
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_ark_object_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn list_object_summaries(conn: &Connection) -> Result<Vec<ArkObjectSummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, type_version, title, props_json, created_at, updated_at, deleted_at
             FROM objects
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_ark_object_summary_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn canonical_query_type_id(conn: &Connection, type_id: &str) -> Result<String, String> {
    let registry_ready: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_aliases')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if registry_ready {
        return Ok(crate::type_registry::resolve_type_id(conn, type_id)?
            .unwrap_or_else(|| type_id.to_string()));
    }
    Ok(type_id.to_string())
}

pub fn list_objects_by_type(conn: &Connection, type_id: &str) -> Result<Vec<ArkObject>, String> {
    let type_id = canonical_query_type_id(conn, type_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
             FROM objects
             WHERE type_id = ?1
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![type_id], map_ark_object_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn list_object_summaries_by_type(
    conn: &Connection,
    type_id: &str,
) -> Result<Vec<ArkObjectSummary>, String> {
    let type_id = canonical_query_type_id(conn, type_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, type_version, title, props_json, created_at, updated_at, deleted_at
             FROM objects
             WHERE type_id = ?1
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![type_id], map_ark_object_summary_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Возвращает только running time_entry_obj (props.endedAt IS NULL),
/// опционально отфильтрованных по props.source. Без обхода всех записей
/// типа — фильтр на SQL уровне через `json_extract`. Hot path для
/// focus widget'а (`stopManualStopwatch`).
///
/// `deleted_at IS NULL` — чтобы tombstones не возвращались как running.
/// Сортировка: новейшие startedAt сверху (DESC), как у callers'ов раньше.
pub fn list_running_time_entries(
    conn: &Connection,
    source_filter: Option<&str>,
) -> Result<Vec<ArkObject>, String> {
    let time_entry_type_id = canonical_query_type_id(conn, "time_entry_obj")?;
    let base_sql =
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
         FROM objects
         WHERE type_id = ?1
           AND deleted_at IS NULL
           AND json_extract(props_json, '$.endedAt') IS NULL";
    let order = " ORDER BY json_extract(props_json, '$.startedAt') DESC";
    if let Some(source) = source_filter {
        let sql = format!("{base_sql} AND json_extract(props_json, '$.source') = ?2{order}");
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![time_entry_type_id, source], map_ark_object_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    } else {
        let sql = format!("{base_sql}{order}");
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![time_entry_type_id], map_ark_object_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

pub fn get_objects_by_ids(conn: &Connection, ids: &[String]) -> Result<Vec<ArkObject>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = (1..=ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
         FROM objects
         WHERE id IN ({placeholders})
         ORDER BY updated_at DESC, created_at DESC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(ids.iter()), map_ark_object_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_object(conn: &Connection, id: &str) -> Result<Option<ArkObject>, String> {
    conn.query_row(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
         FROM objects
         WHERE id = ?1",
        params![id],
        map_ark_object_row,
    )
    .optional()
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub file: String,
    pub line: usize,
    pub text: String,
    pub entry_id: String,
}

pub fn search_objects(conn: &Connection, query: &str) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let normalized_query = query.trim().to_lowercase();
    let query_terms = tokenize_search_text(query);
    if let Some(fts_query) = build_fts_match_query(&query_terms) {
        if let Ok(results) =
            search_objects_with_fts(conn, &fts_query, &normalized_query, &query_terms)
        {
            return Ok(results);
        }
    }

    search_objects_fallback(conn, &normalized_query, &query_terms)
}

fn search_objects_with_fts(
    conn: &Connection,
    fts_query: &str,
    normalized_query: &str,
    query_terms: &[String],
) -> Result<Vec<SearchResult>, String> {
    if !object_search_fts_exists(conn)? {
        return Err("object_search_fts is not available".to_string());
    }

    let mut stmt = conn
        .prepare(
            "SELECT objects.id, objects.type_id, objects.type_version, objects.title, objects.content_json, objects.props_json
             FROM object_search_fts
             JOIN objects ON objects.id = object_search_fts.object_id
             WHERE object_search_fts MATCH ?1
               AND objects.deleted_at IS NULL
             ORDER BY bm25(object_search_fts), objects.updated_at DESC, objects.created_at DESC
             LIMIT 30",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![fts_query], |row| {
            let content_json: String = row.get(4)?;
            let props_json: String = row.get(5)?;
            Ok(ArkObject {
                id: row.get(0)?,
                type_id: row.get(1)?,
                type_version: row.get(2)?,
                title: row.get(3)?,
                content_json: parse_json_or_default(content_json),
                props_json: parse_json_or_default(props_json),
                created_at: String::new(),
                updated_at: String::new(),
                deleted_at: None,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for object in rows {
        let object = object.map_err(|e| e.to_string())?;
        let body = object_search_body(&object);
        let (line, text) =
            build_search_context(&object.title, &body, normalized_query, query_terms);
        results.push(SearchResult {
            file: String::new(),
            line,
            text,
            entry_id: object.id,
        });
    }
    Ok(results)
}

fn search_objects_fallback(
    conn: &Connection,
    normalized_query: &str,
    query_terms: &[String],
) -> Result<Vec<SearchResult>, String> {
    let mut objects = list_objects(conn)?;
    objects.retain(|object| object.deleted_at.is_none());

    let mut matches = Vec::new();

    for object in objects {
        let body = object_search_body(&object);
        let title_matches = line_matches_query(&object.title, normalized_query, query_terms);
        let body_matches = body
            .lines()
            .any(|line| line_matches_query(line.trim(), normalized_query, query_terms));

        if !title_matches && !body_matches {
            continue;
        }

        let title_lower = object.title.to_lowercase();
        let body_lower = body.to_lowercase();
        let score = if title_lower.contains(normalized_query) {
            400
        } else if title_matches {
            300
        } else if body_lower.contains(normalized_query) {
            200
        } else {
            100
        };

        let (line, text) =
            build_search_context(&object.title, &body, normalized_query, query_terms);
        matches.push((
            score,
            SearchResult {
                file: String::new(),
                line,
                text,
                entry_id: object.id,
            },
        ));
    }

    matches.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    matches.truncate(30);

    Ok(matches.into_iter().map(|(_, result)| result).collect())
}

fn build_fts_match_query(query_terms: &[String]) -> Option<String> {
    if query_terms.is_empty() {
        return None;
    }

    let terms = query_terms
        .iter()
        .map(|term| format!("{term}*"))
        .collect::<Vec<_>>();
    Some(terms.join(" AND "))
}

fn object_search_body(object: &ArkObject) -> String {
    let content = extract_plain_text_from_value(&object.content_json);
    let props = extract_plain_text_from_value(&object.props_json);

    match (content.is_empty(), props.is_empty()) {
        (true, true) => String::new(),
        (false, true) => content,
        (true, false) => props,
        (false, false) => format!("{content}\n{props}"),
    }
}

fn build_search_context(
    title: &str,
    body: &str,
    normalized_query: &str,
    query_terms: &[String],
) -> (usize, String) {
    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if line_matches_query(trimmed, normalized_query, query_terms) {
            return (
                index + 1,
                build_snippet(trimmed, normalized_query, query_terms),
            );
        }
    }

    if line_matches_query(title, normalized_query, query_terms) {
        return (
            0,
            format!(
                "Название: {}",
                build_snippet(title.trim(), normalized_query, query_terms)
            ),
        );
    }

    let first_line = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();

    if !first_line.is_empty() {
        return (1, truncate_snippet(first_line, 140));
    }

    (0, format!("Название: {}", title.trim()))
}

fn line_matches_query(line: &str, normalized_query: &str, query_terms: &[String]) -> bool {
    if line.is_empty() {
        return false;
    }

    let line_lower = line.to_lowercase();
    if line_lower.contains(normalized_query) {
        return true;
    }

    let line_terms = tokenize_search_text(line);
    query_terms.iter().any(|query_term| {
        line_terms
            .iter()
            .any(|line_term| line_term.starts_with(query_term))
    })
}

fn build_snippet(line: &str, normalized_query: &str, query_terms: &[String]) -> String {
    let line_lower = line.to_lowercase();
    let direct_match = line_lower.find(normalized_query);
    let term_match = query_terms.iter().find_map(|term| line_lower.find(term));
    let match_start = direct_match.or(term_match).unwrap_or(0);
    let snippet_radius = 56;

    let start = char_boundary_before(line, match_start.saturating_sub(snippet_radius));
    let end = char_boundary_after(
        line,
        (match_start + normalized_query.len() + snippet_radius).min(line.len()),
    );
    let snippet = line[start..end].trim();

    if start == 0 && end == line.len() {
        truncate_snippet(snippet, 140)
    } else {
        let mut result = String::new();
        if start > 0 {
            result.push_str("...");
        }
        result.push_str(snippet);
        if end < line.len() {
            result.push_str("...");
        }
        result
    }
}

fn truncate_snippet(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        return line.to_string();
    }

    let truncated = line.chars().take(max_chars).collect::<String>();
    format!("{}...", truncated.trim_end())
}

fn char_boundary_before(value: &str, index: usize) -> usize {
    let mut safe_index = index.min(value.len());
    while safe_index > 0 && !value.is_char_boundary(safe_index) {
        safe_index -= 1;
    }
    safe_index
}

fn char_boundary_after(value: &str, index: usize) -> usize {
    let mut safe_index = index.min(value.len());
    while safe_index < value.len() && !value.is_char_boundary(safe_index) {
        safe_index += 1;
    }
    safe_index.min(value.len())
}

fn tokenize_search_text(value: &str) -> Vec<String> {
    value
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_lowercase())
        .collect()
}

fn extract_plain_text_from_value(value: &Value) -> String {
    let mut output = String::new();
    collect_plain_text(value, &mut output);
    output.trim().to_string()
}

fn collect_plain_text(node: &Value, output: &mut String) {
    if let Some(node_type) = node.get("type").and_then(|value| value.as_str()) {
        match node_type {
            "text" => {
                if let Some(text) = node.get("text").and_then(|value| value.as_str()) {
                    output.push_str(text);
                    output.push(' ');
                }
            }
            "hardBreak" => output.push('\n'),
            _ => {}
        }
    }

    if let Some(text) = node.as_str() {
        output.push_str(text);
        output.push(' ');
        return;
    }

    if let Some(children) = node.get("content").and_then(|value| value.as_array()) {
        for child in children {
            collect_plain_text(child, output);
        }
    }

    if let Some(values) = node.as_array() {
        for value in values {
            collect_plain_text(value, output);
        }
    }

    if let Some(values) = node.as_object() {
        let is_rich_text_text_node = matches!(
            node.get("type").and_then(|value| value.as_str()),
            Some("text")
        );
        for (key, value) in values {
            if key == "type" || key == "content" || (key == "text" && is_rich_text_text_node) {
                continue;
            }
            collect_plain_text(value, output);
        }
    }

    if matches!(
        node.get("type").and_then(|value| value.as_str()),
        Some("paragraph" | "heading" | "codeBlock" | "blockquote" | "listItem")
    ) {
        output.push('\n');
    }
}

pub fn upsert_object_link(conn: &Connection, link: &ObjectLink) -> Result<(), String> {
    // Do not use SQLite REPLACE here: it deletes the old row first.
    // См. postmortems.md § 2026-06-04.
    conn.execute(
        "INSERT INTO object_links
            (id, source_object_id, target_object_id, link_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            source_object_id = excluded.source_object_id,
            target_object_id = excluded.target_object_id,
            link_type = excluded.link_type,
            created_at = excluded.created_at",
        params![
            link.id,
            link.source_object_id,
            link.target_object_id,
            link.link_type,
            link.created_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_object_link(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM object_links WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_object_links(conn: &Connection) -> Result<Vec<ObjectLink>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, source_object_id, target_object_id, link_type, created_at
             FROM object_links
             ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ObjectLink {
                id: row.get(0)?,
                source_object_id: row.get(1)?,
                target_object_id: row.get(2)?,
                link_type: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Usage tracking CRUD
// ---------------------------------------------------------------------------

pub fn upsert_tracked_app(conn: &Connection, tracked_app: &TrackedApp) -> Result<(), String> {
    conn.execute(
        "INSERT INTO tracked_apps
            (id, platform, exe_path, normalized_exe_path, process_name,
             display_name, publisher, icon_ref, first_seen_at, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
            platform = excluded.platform,
            exe_path = excluded.exe_path,
            normalized_exe_path = excluded.normalized_exe_path,
            process_name = excluded.process_name,
            display_name = excluded.display_name,
            publisher = excluded.publisher,
            icon_ref = excluded.icon_ref,
            first_seen_at = excluded.first_seen_at,
            last_seen_at = excluded.last_seen_at",
        params![
            tracked_app.id,
            tracked_app.platform,
            tracked_app.exe_path,
            tracked_app.normalized_exe_path,
            tracked_app.process_name,
            tracked_app.display_name,
            tracked_app.publisher,
            tracked_app.icon_ref,
            tracked_app.first_seen_at,
            tracked_app.last_seen_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_tracked_app(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM tracked_apps WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn upsert_usage_session(conn: &Connection, session: &UsageSession) -> Result<(), String> {
    let meta_json = serde_json::to_string(&session.meta_json).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO usage_sessions
            (id, tracked_app_id, device_id, device_name, platform, started_at, ended_at,
             runtime_ms, foreground_ms, idle_ms, window_title, process_name, exe_path,
             pid_start, pid_end, meta_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                 ?8, ?9, ?10, ?11, ?12, ?13,
                 ?14, ?15, ?16)
         ON CONFLICT(id) DO UPDATE SET
            tracked_app_id = excluded.tracked_app_id,
            device_id = excluded.device_id,
            device_name = excluded.device_name,
            platform = excluded.platform,
            started_at = excluded.started_at,
            ended_at = excluded.ended_at,
            runtime_ms = excluded.runtime_ms,
            foreground_ms = excluded.foreground_ms,
            idle_ms = excluded.idle_ms,
            window_title = excluded.window_title,
            process_name = excluded.process_name,
            exe_path = excluded.exe_path,
            pid_start = excluded.pid_start,
            pid_end = excluded.pid_end,
            meta_json = excluded.meta_json",
        params![
            session.id,
            session.tracked_app_id,
            session.device_id,
            session.device_name,
            session.platform,
            session.started_at,
            session.ended_at,
            session.runtime_ms,
            session.foreground_ms,
            session.idle_ms,
            session.window_title,
            session.process_name,
            session.exe_path,
            session.pid_start,
            session.pid_end,
            meta_json,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_usage_session(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM usage_sessions WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn upsert_usage_event(conn: &Connection, event: &UsageEvent) -> Result<(), String> {
    let meta_json = serde_json::to_string(&event.meta_json).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO usage_events
            (id, tracked_app_id, usage_session_id, device_id, device_name, platform,
             occurred_at, kind, window_title, process_name, exe_path, pid,
             is_foreground, is_idle, meta_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6,
                 ?7, ?8, ?9, ?10, ?11, ?12,
                 ?13, ?14, ?15)
         ON CONFLICT(id) DO UPDATE SET
            tracked_app_id = excluded.tracked_app_id,
            usage_session_id = excluded.usage_session_id,
            device_id = excluded.device_id,
            device_name = excluded.device_name,
            platform = excluded.platform,
            occurred_at = excluded.occurred_at,
            kind = excluded.kind,
            window_title = excluded.window_title,
            process_name = excluded.process_name,
            exe_path = excluded.exe_path,
            pid = excluded.pid,
            is_foreground = excluded.is_foreground,
            is_idle = excluded.is_idle,
            meta_json = excluded.meta_json",
        params![
            event.id,
            event.tracked_app_id,
            event.usage_session_id,
            event.device_id,
            event.device_name,
            event.platform,
            event.occurred_at,
            event.kind,
            event.window_title,
            event.process_name,
            event.exe_path,
            event.pid,
            event.is_foreground as i64,
            event.is_idle as i64,
            meta_json,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_usage_event(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM usage_events WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn upsert_usage_day(conn: &Connection, day: &UsageDay) -> Result<(), String> {
    let payload_json = serde_json::to_string(&day.payload_json).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO usage_days (id, device_id, day, payload_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            device_id = excluded.device_id,
            day = excluded.day,
            payload_json = excluded.payload_json,
            updated_at = excluded.updated_at",
        params![day.id, day.device_id, day.day, payload_json, day.updated_at,],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_usage_day(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM usage_days WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn load_usage_day(conn: &Connection, id: &str) -> Result<Option<UsageDay>, String> {
    conn.query_row(
        "SELECT id, device_id, day, payload_json, updated_at
         FROM usage_days WHERE id = ?1",
        params![id],
        |row| {
            let payload: String = row.get(3)?;
            Ok(UsageDay {
                id: row.get(0)?,
                device_id: row.get(1)?,
                day: row.get(2)?,
                payload_json: serde_json::from_str(&payload)
                    .unwrap_or_else(|_| json!({ "a": [], "t": [], "s": [] })),
                updated_at: row.get(4)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn payload_strings(payload: &Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn payload_spans(payload: &Value) -> Vec<Vec<i64>> {
    payload
        .get("s")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_array)
                .filter_map(|span| span.iter().map(Value::as_i64).collect::<Option<Vec<_>>>())
                .filter(|span| span.len() == 5 && span[1] > 0)
                .collect()
        })
        .unwrap_or_default()
}

fn dictionary_index(values: &mut Vec<String>, value: &str) -> i64 {
    if let Some(index) = values.iter().position(|item| item == value) {
        index as i64
    } else {
        values.push(value.to_string());
        (values.len() - 1) as i64
    }
}

fn merge_usage_spans(mut spans: Vec<Vec<i64>>) -> Vec<Vec<i64>> {
    spans.sort_by_key(|span| span[0]);
    let mut merged: Vec<Vec<i64>> = Vec::with_capacity(spans.len());
    for span in spans {
        if let Some(previous) = merged.last_mut() {
            let previous_end = previous[0].saturating_add(previous[1]);
            if previous[2..] == span[2..] && span[0] <= previous_end {
                let span_end = span[0].saturating_add(span[1]);
                previous[1] = previous[1].max(span_end.saturating_sub(previous[0]));
                continue;
            }
        }
        merged.push(span);
    }
    merged
}

fn upsert_usage_day_fragment(
    conn: &Connection,
    write: &UsageSpanWrite,
    day: &str,
    day_start_unix: i64,
    fragment_start: i64,
    fragment_end: i64,
) -> Result<UsageDay, String> {
    let id = format!("usage-day:{}:{day}", write.device_id);
    let mut usage_day = load_usage_day(conn, &id)?.unwrap_or_else(|| UsageDay {
        id,
        device_id: write.device_id.clone(),
        day: day.to_string(),
        payload_json: json!({ "a": [], "t": [], "s": [] }),
        updated_at: write.updated_at.clone(),
    });
    let mut apps = payload_strings(&usage_day.payload_json, "a");
    let mut titles = payload_strings(&usage_day.payload_json, "t");
    let mut spans = payload_spans(&usage_day.payload_json);
    let app_index = dictionary_index(&mut apps, &write.tracked_app_id);
    let title_index = write
        .window_title
        .as_deref()
        .map(|title| dictionary_index(&mut titles, title))
        .unwrap_or(-1);
    let start_second = fragment_start.saturating_sub(day_start_unix);
    let duration_seconds = fragment_end.saturating_sub(fragment_start);
    let replacement = vec![
        start_second,
        duration_seconds,
        app_index,
        title_index,
        write.flags,
    ];
    if let Some(existing) = spans
        .iter_mut()
        .find(|span| span[0] == start_second && span[2] == app_index)
    {
        *existing = replacement;
    } else {
        spans.push(replacement);
    }
    usage_day.payload_json = json!({
        "a": apps,
        "t": titles,
        "s": merge_usage_spans(spans),
    });
    usage_day.updated_at.clone_from(&write.updated_at);
    upsert_usage_day(conn, &usage_day)?;
    Ok(usage_day)
}

pub fn upsert_usage_span(
    conn: &Connection,
    write: &UsageSpanWrite,
) -> Result<Vec<UsageDay>, String> {
    if write.ended_at_unix <= write.started_at_unix {
        return Ok(Vec::new());
    }
    let mut cursor = write.started_at_unix;
    let mut days = Vec::new();
    while cursor < write.ended_at_unix {
        let date = chrono::DateTime::from_timestamp(cursor, 0)
            .ok_or_else(|| format!("invalid usage span timestamp: {cursor}"))?
            .date_naive();
        let day = date.format("%Y-%m-%d").to_string();
        let day_start_unix = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| format!("invalid usage day: {day}"))?
            .and_utc()
            .timestamp();
        let fragment_end = write
            .ended_at_unix
            .min(day_start_unix.saturating_add(86_400));
        days.push(upsert_usage_day_fragment(
            conn,
            write,
            &day,
            day_start_unix,
            cursor,
            fragment_end,
        )?);
        cursor = fragment_end;
    }
    Ok(days)
}

pub fn get_usage_title_total(conn: &Connection, query: &str) -> Result<UsageTitleTotal, String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(UsageTitleTotal {
            active_seconds: 0,
            idle_seconds: 0,
        });
    }
    let mut statement = conn
        .prepare("SELECT payload_json FROM usage_days ORDER BY day ASC")
        .map_err(|error| error.to_string())?;
    let payloads = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    let mut total = UsageTitleTotal {
        active_seconds: 0,
        idle_seconds: 0,
    };
    for payload in payloads {
        let payload: Value = serde_json::from_str(&payload.map_err(|error| error.to_string())?)
            .unwrap_or_else(|_| json!({}));
        let titles = payload_strings(&payload, "t");
        for span in payload_spans(&payload) {
            let title_matches = usize::try_from(span[3])
                .ok()
                .and_then(|index| titles.get(index))
                .is_some_and(|title| title.to_lowercase().contains(&query));
            if !title_matches {
                continue;
            }
            if span[4] & 1 == 1 {
                total.idle_seconds = total.idle_seconds.saturating_add(span[1]);
            } else {
                total.active_seconds = total.active_seconds.saturating_add(span[1]);
            }
        }
    }
    Ok(total)
}

fn clamp_positive_i64(value: i64, fallback: i64) -> i64 {
    if value > 0 {
        value
    } else {
        fallback
    }
}

fn iso_date_days_ago(days_ago: i64) -> String {
    (Utc::now().date_naive() - Duration::days(days_ago))
        .format("%Y-%m-%d")
        .to_string()
}

fn enumerate_dates(range_days: i64) -> Vec<String> {
    (0..range_days)
        .map(|index| iso_date_days_ago(range_days - index - 1))
        .collect()
}

pub fn load_usage_analytics(
    conn: &Connection,
    range_days: i64,
    top_apps_limit: i64,
    recent_sessions_limit: i64,
) -> Result<UsageAnalyticsSnapshot, String> {
    let range_days = clamp_positive_i64(range_days, 21);
    let top_apps_limit = clamp_positive_i64(top_apps_limit, 8);
    let recent_sessions_limit = clamp_positive_i64(recent_sessions_limit, 24);
    let range_start = iso_date_days_ago(range_days - 1);
    let range_end = iso_date_days_ago(0);

    let summary = conn
        .query_row(
            "SELECT COUNT(DISTINCT tracked_apps.id) AS tracked_app_count,
                    COUNT(DISTINCT usage_sessions.id) AS session_count,
                    (SELECT COUNT(*) FROM usage_events) AS event_count,
                    COALESCE(SUM(usage_sessions.runtime_ms), 0) AS total_runtime_ms,
                    COALESCE(SUM(usage_sessions.foreground_ms), 0) AS total_foreground_ms,
                    COALESCE(SUM(usage_sessions.idle_ms), 0) AS total_idle_ms,
                    MIN(usage_sessions.started_at) AS first_recorded_at,
                    MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_recorded_at
             FROM tracked_apps
             LEFT JOIN usage_sessions ON usage_sessions.tracked_app_id = tracked_apps.id",
            [],
            |row| {
                Ok(UsageSummary {
                    tracked_app_count: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    session_count: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    event_count: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    total_runtime_ms: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    total_foreground_ms: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    total_idle_ms: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    first_recorded_at: row.get(6)?,
                    last_recorded_at: row.get(7)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT SUBSTR(COALESCE(ended_at, started_at), 1, 10) AS date,
                    COALESCE(SUM(foreground_ms), 0) AS foreground_ms,
                    COALESCE(SUM(idle_ms), 0) AS idle_ms,
                    COUNT(*) AS sessions
             FROM usage_sessions
             WHERE SUBSTR(COALESCE(ended_at, started_at), 1, 10) BETWEEN ?1 AND ?2
             GROUP BY date
             ORDER BY date ASC",
        )
        .map_err(|e| e.to_string())?;
    let trend_rows = stmt
        .query_map(params![range_start, range_end], |row| {
            Ok(DailyTrendPoint {
                date: row.get(0)?,
                foreground_ms: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                idle_ms: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                sessions: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let trend_by_date = trend_rows
        .into_iter()
        .map(|point| (point.date.clone(), point))
        .collect::<std::collections::HashMap<_, _>>();
    let daily_trend = enumerate_dates(range_days)
        .into_iter()
        .map(|date| {
            trend_by_date
                .get(&date)
                .cloned()
                .unwrap_or(DailyTrendPoint {
                    date,
                    foreground_ms: 0,
                    idle_ms: 0,
                    sessions: 0,
                })
        })
        .collect();

    let mut stmt = conn
        .prepare(
            "SELECT CAST(STRFTIME('%w', started_at) AS INTEGER) AS weekday,
                    CAST(STRFTIME('%H', started_at) AS INTEGER) AS hour,
                    COALESCE(SUM(foreground_ms), 0) AS foreground_ms
             FROM usage_sessions
             WHERE SUBSTR(COALESCE(ended_at, started_at), 1, 10) BETWEEN ?1 AND ?2
             GROUP BY weekday, hour",
        )
        .map_err(|e| e.to_string())?;
    let hourly_heatmap = stmt
        .query_map(params![range_start, range_end], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|row| match row {
            Ok((Some(weekday), Some(hour), foreground_ms)) => Some(Ok(HourlyHeatmapCell {
                weekday,
                hour,
                foreground_ms,
            })),
            Ok((_, _, _)) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT tracked_apps.id,
                    tracked_apps.display_name,
                    tracked_apps.process_name,
                    tracked_apps.normalized_exe_path AS normalized_path,
                    tracked_apps.icon_ref,
                    COALESCE(SUM(usage_sessions.runtime_ms), 0) AS runtime_ms,
                    COALESCE(SUM(usage_sessions.foreground_ms), 0) AS foreground_ms,
                    COALESCE(SUM(usage_sessions.idle_ms), 0) AS idle_ms,
                    COUNT(usage_sessions.id) AS sessions,
                    MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_seen_at
             FROM tracked_apps
             JOIN usage_sessions ON usage_sessions.tracked_app_id = tracked_apps.id
             GROUP BY tracked_apps.id
             ORDER BY runtime_ms DESC, foreground_ms DESC, last_seen_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let top_apps = stmt
        .query_map(params![top_apps_limit], |row| {
            let display_name: Option<String> = row.get(1)?;
            let process_name: String = row.get(2)?;
            Ok(TopAppEntry {
                id: row.get(0)?,
                display_name: display_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(&process_name)
                    .to_string(),
                process_name,
                normalized_path: row.get(3)?,
                icon_ref: row.get(4)?,
                runtime_ms: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                foreground_ms: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                idle_ms: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
                sessions: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
                last_seen_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT usage_sessions.id,
                    usage_sessions.tracked_app_id,
                    tracked_apps.display_name,
                    usage_sessions.process_name,
                    usage_sessions.platform,
                    usage_sessions.device_name,
                    usage_sessions.started_at,
                    usage_sessions.ended_at,
                    usage_sessions.runtime_ms,
                    usage_sessions.foreground_ms,
                    usage_sessions.idle_ms,
                    usage_sessions.window_title
             FROM usage_sessions
             JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
             ORDER BY usage_sessions.started_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let recent_sessions = stmt
        .query_map(params![recent_sessions_limit], |row| {
            let display_name: Option<String> = row.get(2)?;
            let process_name: String = row.get(3)?;
            Ok(RecentSessionEntry {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                display_name: display_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(&process_name)
                    .to_string(),
                process_name,
                platform: row.get(4)?,
                device_name: row.get(5)?,
                started_at: row.get(6)?,
                ended_at: row.get(7)?,
                runtime_ms: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
                foreground_ms: row.get::<_, Option<i64>>(9)?.unwrap_or(0),
                idle_ms: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                window_title: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(UsageAnalyticsSnapshot {
        generated_at: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        summary,
        daily_trend,
        hourly_heatmap,
        top_apps,
        recent_sessions,
    })
}

fn clamp_usage_process_limit(limit: i64) -> i64 {
    limit.clamp(1, 25)
}

fn normalize_usage_binding_value(match_type: &str, value: &str) -> String {
    let trimmed = value.trim();
    if match_type == "exe_path" {
        trimmed.replace('/', "\\").to_lowercase()
    } else {
        trimmed.to_lowercase()
    }
}

fn build_usage_process_candidate(
    tracked_app_id: String,
    display_name: Option<String>,
    exe_path: Option<String>,
    process_name: Option<String>,
    last_seen_at: Option<String>,
    session_count: i64,
) -> Option<UsageProcessCandidate> {
    let exe_path = exe_path.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let process_name = process_name.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let (binding_match_type, binding_match_value) = match (&exe_path, &process_name) {
        (Some(value), _) => ("exe_path".to_string(), value.clone()),
        (None, Some(value)) => ("process_name".to_string(), value.clone()),
        (None, None) => return None,
    };
    let binding_normalized_value =
        normalize_usage_binding_value(&binding_match_type, &binding_match_value);
    let display_name = display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(process_name.as_deref())
        .or(exe_path.as_deref())
        .unwrap_or(&tracked_app_id)
        .to_string();

    Some(UsageProcessCandidate {
        tracked_app_id,
        display_name,
        exe_path,
        process_name,
        last_seen_at,
        session_count,
        binding_match_type,
        binding_match_value,
        binding_normalized_value,
    })
}

fn map_usage_process_candidate_row(
    row: &Row<'_>,
) -> rusqlite::Result<Option<UsageProcessCandidate>> {
    let tracked_app_id: String = row.get(0)?;
    let display_name: Option<String> = row.get(1)?;
    let exe_path: Option<String> = row.get(2)?;
    let process_name: Option<String> = row.get(3)?;
    let last_seen_at: Option<String> = row.get(4)?;
    let session_count = row.get::<_, Option<i64>>(5)?.unwrap_or(0);
    Ok(build_usage_process_candidate(
        tracked_app_id,
        display_name,
        exe_path,
        process_name,
        last_seen_at,
        session_count,
    ))
}

pub fn list_recent_usage_processes(
    conn: &Connection,
    limit: i64,
) -> Result<Vec<UsageProcessCandidate>, String> {
    let limit = clamp_usage_process_limit(limit);
    let mut stmt = conn
        .prepare(
            "SELECT tracked_apps.id AS tracked_app_id,
                    NULLIF(COALESCE(tracked_apps.display_name, tracked_apps.process_name, tracked_apps.exe_path), '') AS display_name,
                    NULLIF(tracked_apps.exe_path, '') AS exe_path,
                    NULLIF(tracked_apps.process_name, '') AS process_name,
                    MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_seen_at,
                    SUM(CASE WHEN usage_sessions.runtime_ms > 0 THEN 1 ELSE 0 END) AS session_count
             FROM usage_sessions
             JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
             WHERE NULLIF(COALESCE(tracked_apps.exe_path, tracked_apps.process_name), '') IS NOT NULL
             GROUP BY tracked_apps.id, tracked_apps.display_name, tracked_apps.exe_path, tracked_apps.process_name
             ORDER BY last_seen_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit], map_usage_process_candidate_row)
        .map_err(|e| e.to_string())?;
    rows.filter_map(|row| match row {
        Ok(Some(candidate)) => Some(Ok(candidate)),
        Ok(None) => None,
        Err(error) => Some(Err(error)),
    })
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
}

pub fn search_usage_processes(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<UsageProcessCandidate>, String> {
    let trimmed = query.trim().to_lowercase();
    if trimmed.is_empty() {
        return list_recent_usage_processes(conn, limit);
    }

    let limit = clamp_usage_process_limit(limit);
    let pattern = format!("%{trimmed}%");
    let mut stmt = conn
        .prepare(
            "SELECT tracked_apps.id AS tracked_app_id,
                    NULLIF(COALESCE(tracked_apps.display_name, tracked_apps.process_name, tracked_apps.exe_path), '') AS display_name,
                    NULLIF(tracked_apps.exe_path, '') AS exe_path,
                    NULLIF(tracked_apps.process_name, '') AS process_name,
                    MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_seen_at,
                    SUM(CASE WHEN usage_sessions.runtime_ms > 0 THEN 1 ELSE 0 END) AS session_count
             FROM usage_sessions
             JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
             WHERE (
                LOWER(COALESCE(tracked_apps.display_name, '')) LIKE ?1
                OR LOWER(COALESCE(tracked_apps.process_name, '')) LIKE ?1
                OR LOWER(COALESCE(tracked_apps.exe_path, '')) LIKE ?1
             )
             GROUP BY tracked_apps.id, tracked_apps.display_name, tracked_apps.exe_path, tracked_apps.process_name
             ORDER BY last_seen_at DESC
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![pattern, limit], map_usage_process_candidate_row)
        .map_err(|e| e.to_string())?;
    rows.filter_map(|row| match row {
        Ok(Some(candidate)) => Some(Ok(candidate)),
        Ok(None) => None,
        Err(error) => Some(Err(error)),
    })
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
}

#[derive(Debug)]
struct UsageGameBindingIndex {
    game_names_by_id: HashMap<String, String>,
    game_ids_by_path: HashMap<String, String>,
    game_ids_by_process_name: HashMap<String, String>,
}

#[derive(Debug)]
struct UsageTrackedAggregateRow {
    normalized_path: Option<String>,
    normalized_process_name: Option<String>,
    total_seconds: i64,
    session_count: i64,
    last_played: Option<String>,
}

#[derive(Debug)]
struct UsageTrackedDailyRow {
    normalized_path: Option<String>,
    normalized_process_name: Option<String>,
    date: String,
    seconds: i64,
}

fn build_usage_game_binding_index(bindings: &[UsageGamePlaytimeBinding]) -> UsageGameBindingIndex {
    let mut index = UsageGameBindingIndex {
        game_names_by_id: HashMap::new(),
        game_ids_by_path: HashMap::new(),
        game_ids_by_process_name: HashMap::new(),
    };

    for binding in bindings {
        let game_id = binding.game_id.trim();
        if game_id.is_empty() {
            continue;
        }

        let game_name = binding.game_name.trim();
        index.game_names_by_id.insert(
            game_id.to_string(),
            if game_name.is_empty() {
                game_id.to_string()
            } else {
                game_name.to_string()
            },
        );

        let normalized = normalize_usage_binding_value(&binding.match_type, &binding.match_value);
        if normalized.is_empty() {
            continue;
        }

        if binding.match_type == "exe_path" {
            index
                .game_ids_by_path
                .insert(normalized, game_id.to_string());
        } else if binding.match_type == "process_name" {
            index
                .game_ids_by_process_name
                .insert(normalized, game_id.to_string());
        }
    }

    index
}

fn append_usage_binding_where_clause(
    index: &UsageGameBindingIndex,
    params: &mut Vec<String>,
) -> Option<String> {
    let mut clauses = Vec::new();

    if !index.game_ids_by_path.is_empty() {
        let placeholders = (0..index.game_ids_by_path.len())
            .map(|offset| format!("?{}", params.len() + offset + 1))
            .collect::<Vec<_>>()
            .join(", ");
        clauses.push(format!(
            "tracked_apps.normalized_exe_path IN ({placeholders})"
        ));
        params.extend(index.game_ids_by_path.keys().cloned());
    }

    if !index.game_ids_by_process_name.is_empty() {
        let placeholders = (0..index.game_ids_by_process_name.len())
            .map(|offset| format!("?{}", params.len() + offset + 1))
            .collect::<Vec<_>>()
            .join(", ");
        clauses.push(format!(
            "LOWER(COALESCE(tracked_apps.process_name, '')) IN ({placeholders})"
        ));
        params.extend(index.game_ids_by_process_name.keys().cloned());
    }

    if clauses.is_empty() {
        None
    } else {
        Some(format!("WHERE ({})", clauses.join(" OR ")))
    }
}

fn find_usage_game_for_tracked_row(
    normalized_path: Option<&str>,
    normalized_process_name: Option<&str>,
    index: &UsageGameBindingIndex,
) -> Option<(String, String)> {
    if let Some(path) = normalized_path {
        let normalized = normalize_usage_binding_value("exe_path", path);
        if let Some(game_id) = index.game_ids_by_path.get(&normalized) {
            let game_name = index
                .game_names_by_id
                .get(game_id)
                .cloned()
                .unwrap_or_else(|| game_id.clone());
            return Some((game_id.clone(), game_name));
        }
    }

    let process_name = normalized_process_name
        .map(|value| normalize_usage_binding_value("process_name", value))
        .unwrap_or_default();
    if process_name.is_empty() {
        return None;
    }

    index
        .game_ids_by_process_name
        .get(&process_name)
        .map(|game_id| {
            let game_name = index
                .game_names_by_id
                .get(game_id)
                .cloned()
                .unwrap_or_else(|| game_id.clone());
            (game_id.clone(), game_name)
        })
}

fn map_usage_tracked_aggregate_row(row: &Row<'_>) -> rusqlite::Result<UsageTrackedAggregateRow> {
    Ok(UsageTrackedAggregateRow {
        normalized_path: row.get(0)?,
        normalized_process_name: row.get(1)?,
        total_seconds: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
        session_count: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
        last_played: row.get(4)?,
    })
}

fn map_usage_tracked_daily_row(row: &Row<'_>) -> rusqlite::Result<UsageTrackedDailyRow> {
    Ok(UsageTrackedDailyRow {
        normalized_path: row.get(0)?,
        normalized_process_name: row.get(1)?,
        date: row.get(2)?,
        seconds: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
    })
}

pub fn load_usage_game_playtime_summary(
    conn: &Connection,
    bindings: &[UsageGamePlaytimeBinding],
    range_start: Option<&str>,
    range_end: Option<&str>,
) -> Result<UsageGamePlaytimeSummary, String> {
    let index = build_usage_game_binding_index(bindings);
    let mut aggregate_params = Vec::new();
    let Some(aggregate_where_sql) =
        append_usage_binding_where_clause(&index, &mut aggregate_params)
    else {
        return Ok(UsageGamePlaytimeSummary {
            aggregates: Vec::new(),
            daily_totals: Vec::new(),
            per_game_totals: Vec::new(),
        });
    };

    let aggregate_sql = format!(
        "SELECT tracked_apps.normalized_exe_path AS normalized_path,
                LOWER(COALESCE(tracked_apps.process_name, '')) AS normalized_process_name,
                CAST(SUM(usage_sessions.runtime_ms) / 1000 AS INTEGER) AS total_seconds,
                SUM(CASE WHEN usage_sessions.runtime_ms > 0 THEN 1 ELSE 0 END) AS session_count,
                MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_played
         FROM usage_sessions
         JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
         {aggregate_where_sql}
         GROUP BY tracked_apps.id, tracked_apps.normalized_exe_path, normalized_process_name"
    );
    let mut stmt = conn.prepare(&aggregate_sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params_from_iter(aggregate_params.iter()),
            map_usage_tracked_aggregate_row,
        )
        .map_err(|e| e.to_string())?;
    let tracked_rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut aggregates_by_game: HashMap<String, UsageGamePlaytimeAggregate> = HashMap::new();
    for row in tracked_rows {
        let Some((game_id, game_name)) = find_usage_game_for_tracked_row(
            row.normalized_path.as_deref(),
            row.normalized_process_name.as_deref(),
            &index,
        ) else {
            continue;
        };

        let entry =
            aggregates_by_game
                .entry(game_id.clone())
                .or_insert(UsageGamePlaytimeAggregate {
                    game_id,
                    game_name,
                    total_seconds: 0,
                    session_count: 0,
                    last_played: None,
                });
        entry.total_seconds += row.total_seconds;
        entry.session_count += row.session_count;
        let should_update_last_played = match (&row.last_played, &entry.last_played) {
            (Some(next), Some(current)) => next > current,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if should_update_last_played {
            entry.last_played = row.last_played;
        }
    }
    let mut aggregates = aggregates_by_game.into_values().collect::<Vec<_>>();
    aggregates.sort_by(|left, right| {
        right
            .total_seconds
            .cmp(&left.total_seconds)
            .then_with(|| left.game_name.cmp(&right.game_name))
    });

    let mut daily_totals = Vec::new();
    let mut per_game_totals = Vec::new();
    if let (Some(range_start), Some(range_end)) = (range_start, range_end) {
        let mut daily_params = Vec::new();
        let Some(mut daily_where_sql) =
            append_usage_binding_where_clause(&index, &mut daily_params)
        else {
            return Ok(UsageGamePlaytimeSummary {
                aggregates,
                daily_totals,
                per_game_totals,
            });
        };
        let start_placeholder = daily_params.len() + 1;
        let end_placeholder = daily_params.len() + 2;
        daily_where_sql.push_str(&format!(
            " AND SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10)
                  BETWEEN ?{start_placeholder} AND ?{end_placeholder}"
        ));
        daily_params.push(range_start.to_string());
        daily_params.push(range_end.to_string());

        let daily_sql = format!(
            "SELECT tracked_apps.normalized_exe_path AS normalized_path,
                    LOWER(COALESCE(tracked_apps.process_name, '')) AS normalized_process_name,
                    SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10) AS date,
                    CAST(SUM(usage_sessions.runtime_ms) / 1000 AS INTEGER) AS seconds
             FROM usage_sessions
             JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
             {daily_where_sql}
             GROUP BY tracked_apps.id, tracked_apps.normalized_exe_path, normalized_process_name, date
             HAVING seconds > 0
             ORDER BY date ASC, tracked_apps.id ASC"
        );
        let mut stmt = conn.prepare(&daily_sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                params_from_iter(daily_params.iter()),
                map_usage_tracked_daily_row,
            )
            .map_err(|e| e.to_string())?;
        let tracked_daily_rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut daily_totals_by_date: HashMap<String, i64> = HashMap::new();
        let mut per_game_totals_by_id: HashMap<String, UsageGameRangeTotal> = HashMap::new();
        for row in tracked_daily_rows {
            let Some((game_id, game_name)) = find_usage_game_for_tracked_row(
                row.normalized_path.as_deref(),
                row.normalized_process_name.as_deref(),
                &index,
            ) else {
                continue;
            };

            *daily_totals_by_date.entry(row.date.clone()).or_insert(0) += row.seconds;
            let entry =
                per_game_totals_by_id
                    .entry(game_id.clone())
                    .or_insert(UsageGameRangeTotal {
                        game_id,
                        game_name,
                        seconds: 0,
                    });
            entry.seconds += row.seconds;
        }
        daily_totals = daily_totals_by_date
            .into_iter()
            .map(|(date, seconds)| UsageGameDailyTotal { date, seconds })
            .collect();
        daily_totals.sort_by(|left, right| left.date.cmp(&right.date));

        per_game_totals = per_game_totals_by_id.into_values().collect();
        per_game_totals.sort_by(|left, right| {
            right
                .seconds
                .cmp(&left.seconds)
                .then_with(|| left.game_name.cmp(&right.game_name))
        });
    }

    Ok(UsageGamePlaytimeSummary {
        aggregates,
        daily_totals,
        per_game_totals,
    })
}

// ---------------------------------------------------------------------------
// sync_kv
// ---------------------------------------------------------------------------

pub fn get_sync_kv(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM sync_kv WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn set_sync_kv(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO sync_kv (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn upsert_sync_tombstone(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
    conn.execute(
        "INSERT INTO sync_tombstones (id, entity_type, hlc, deleted_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            entity_type = excluded.entity_type,
            hlc = excluded.hlc,
            deleted_at = excluded.deleted_at
         WHERE excluded.hlc >= sync_tombstones.hlc",
        params![entity.id, entity.entity_type, entity.hlc, entity.hlc],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_sync_tombstone(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM sync_tombstones WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn ensure_usage_sequence_migrated(conn: &Connection, device_id: &str) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT usage_sequence_migration")
        .map_err(|e| e.to_string())?;
    match ensure_usage_sequence_migrated_inner(conn, device_id) {
        Ok(()) => {
            conn.execute_batch("RELEASE usage_sequence_migration")
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO usage_sequence_migration; RELEASE usage_sequence_migration",
            );
            Err(error)
        }
    }
}

fn ensure_usage_sequence_migrated_inner(conn: &Connection, device_id: &str) -> Result<(), String> {
    if get_sync_kv(conn, USAGE_SEQUENCE_MIGRATION_KEY)?.as_deref() == Some("1") {
        return Ok(());
    }

    let raw = get_sync_kv(conn, VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(value).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_default();
    let mut statement = conn
        .prepare(
            "SELECT entity_type, id FROM (
                SELECT 'usage_session' AS entity_type, id FROM usage_sessions
                UNION ALL
                SELECT 'usage_event', id FROM usage_events
                UNION ALL
                SELECT 'usage_day', id FROM usage_days
             )
             ORDER BY entity_type, id",
        )
        .map_err(|e| e.to_string())?;
    let refs = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(statement);

    let wall_time = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    let mut seq = 0u64;
    for (entity_type, entity_id) in refs {
        seq = seq.saturating_add(1);
        let hlc = HLC::new(wall_time.clone(), seq, device_id.to_string()).to_string();
        conn.execute(
            "INSERT OR REPLACE INTO usage_sync_versions(entity_type, entity_id, hlc)
             VALUES (?1, ?2, ?3)",
            params![entity_type, entity_id, hlc],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO usage_sync_log
                (device_id, seq, entity_type, entity_id, hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            params![device_id, seq as i64, entity_type, entity_id, hlc],
        )
        .map_err(|e| e.to_string())?;
        vector.remove(&entity_id);
    }
    conn.execute(
        "INSERT OR IGNORE INTO usage_sync_heads(device_id, max_seq) VALUES (?1, 0)",
        params![device_id],
    )
    .map_err(|e| e.to_string())?;
    let mut head = conn
        .query_row(
            "SELECT max_seq FROM usage_sync_heads WHERE device_id = ?1",
            params![device_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        .max(0) as u64;
    while conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM usage_sync_log WHERE device_id = ?1 AND seq = ?2
             )",
            params![device_id, head.saturating_add(1) as i64],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0
    {
        head = head.saturating_add(1);
    }
    conn.execute(
        "UPDATE usage_sync_heads SET max_seq = ?2 WHERE device_id = ?1",
        params![device_id, head as i64],
    )
    .map_err(|e| e.to_string())?;
    vector.insert(usage_cursor_key(device_id), head.to_string());
    set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|e| e.to_string())?,
    )?;
    set_sync_kv(conn, USAGE_SEQUENCE_MIGRATION_KEY, "1")
}

fn next_usage_sequence(conn: &Connection, device_id: &str) -> Result<u64, String> {
    conn.query_row(
        "INSERT INTO usage_sync_heads(device_id, max_seq) VALUES (?1, 1)
         ON CONFLICT(device_id) DO UPDATE SET max_seq = max_seq + 1
         RETURNING max_seq",
        params![device_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value.max(0) as u64)
    .map_err(|e| e.to_string())
}

fn record_usage_sequence(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: &str,
    seq: u64,
    hlc: &str,
    deleted: bool,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO usage_sync_log
            (device_id, seq, entity_type, entity_id, hlc, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            device_id,
            seq as i64,
            entity_type,
            entity_id,
            hlc,
            deleted as i64
        ],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO usage_sync_heads(device_id, max_seq) VALUES (?1, ?2)
         ON CONFLICT(device_id) DO UPDATE SET max_seq = MAX(max_seq, excluded.max_seq)",
        params![device_id, seq as i64],
    )
    .map_err(|e| e.to_string())?;

    let raw = get_sync_kv(conn, VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();
    let cursor_key = usage_cursor_key(device_id);
    let mut contiguous = vector
        .get(&cursor_key)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    while conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM usage_sync_log WHERE device_id = ?1 AND seq = ?2
             )",
            params![device_id, contiguous.saturating_add(1) as i64],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0
    {
        contiguous = contiguous.saturating_add(1);
    }
    vector.insert(cursor_key, contiguous.to_string());
    vector.remove(entity_id);
    set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|e| e.to_string())?,
    )
}

fn usage_origin_for_entity(
    conn: &Connection,
    entity: &SyncEntity,
) -> Result<(String, u64), String> {
    if let (Some(device_id), Some(seq)) = (&entity.origin_device_id, entity.origin_seq) {
        return Ok((device_id.clone(), seq));
    }
    if let Some(existing) = conn
        .query_row(
            "SELECT device_id, seq FROM usage_sync_log
             WHERE entity_type = ?1 AND entity_id = ?2 AND hlc = ?3
             LIMIT 1",
            params![entity.entity_type, entity.id, entity.hlc],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        return Ok((existing.0, existing.1.max(0) as u64));
    }
    let parsed = HLC::from_string(&entity.hlc);
    let source_device_id = if parsed.device_id.is_empty() {
        "legacy-usage".to_string()
    } else {
        parsed.device_id
    };
    let device_id = format!("legacy:{source_device_id}");
    let seq = next_usage_sequence(conn, &device_id)?;
    Ok((device_id, seq))
}

fn usage_entity_is_newer(conn: &Connection, entity: &SyncEntity) -> Result<bool, String> {
    let local = conn
        .query_row(
            "SELECT hlc FROM usage_sync_versions
             WHERE entity_type = ?1 AND entity_id = ?2",
            params![entity.entity_type, entity.id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(local
        .as_deref()
        .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc)))
}

fn finish_usage_entity_sync(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
    conn.execute(
        "INSERT INTO usage_sync_versions(entity_type, entity_id, hlc)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET hlc = excluded.hlc
         WHERE excluded.hlc > usage_sync_versions.hlc",
        params![entity.entity_type, entity.id, entity.hlc],
    )
    .map_err(|e| e.to_string())?;
    let (device_id, seq) = usage_origin_for_entity(conn, entity)?;
    record_usage_sequence(
        conn,
        &entity.entity_type,
        &entity.id,
        &device_id,
        seq,
        &entity.hlc,
        entity.deleted == Some(true),
    )
}

pub fn bump_sync_version_vector(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: &str,
    deleted: bool,
) -> Result<String, String> {
    if is_sequenced_usage_entity(entity_type) {
        ensure_usage_sequence_migrated(conn, device_id)?;
        let seq = next_usage_sequence(conn, device_id)?;
        let hlc = HLC::new(
            Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            seq,
            device_id.to_string(),
        )
        .to_string();
        conn.execute(
            "INSERT INTO usage_sync_versions(entity_type, entity_id, hlc)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(entity_type, entity_id) DO UPDATE SET hlc = excluded.hlc",
            params![entity_type, entity_id, hlc],
        )
        .map_err(|e| e.to_string())?;
        record_usage_sequence(conn, entity_type, entity_id, device_id, seq, &hlc, deleted)?;
        return Ok(hlc);
    }

    let raw = get_sync_kv(conn, VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(value).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_default();

    let now = HLC::now(device_id);
    let next = match vector.get(entity_id) {
        Some(existing) => {
            let previous = HLC::from_string(existing);
            if HLC::compare(&now, &previous).is_gt() {
                now
            } else {
                HLC::new(
                    previous.wall_time,
                    previous.counter.saturating_add(1),
                    device_id.to_string(),
                )
            }
        }
        None => now,
    };
    let hlc = next.to_string();
    vector.insert(entity_id.to_string(), hlc.clone());
    set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|e| e.to_string())?,
    )?;
    Ok(hlc)
}

pub fn record_sync_tombstone(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    hlc: &str,
) -> Result<(), String> {
    let entity = SyncEntity {
        entity_type: entity_type.to_string(),
        id: entity_id.to_string(),
        data: serde_json::Map::new(),
        hlc: hlc.to_string(),
        deleted: Some(true),
        origin_device_id: None,
        origin_seq: None,
    };
    upsert_sync_tombstone(conn, &entity)
}

fn load_sync_tombstones(conn: &Connection) -> Result<Vec<SyncEntity>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, entity_type, hlc
             FROM sync_tombstones
             ORDER BY hlc ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SyncEntity {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                hlc: row.get(2)?,
                data: serde_json::Map::new(),
                deleted: Some(true),
                origin_device_id: None,
                origin_seq: None,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

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

// ---------------------------------------------------------------------------
// Delete helpers (for StorageBackend)
// ---------------------------------------------------------------------------

pub fn delete_area(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM areas WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_tag(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM tags WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SqliteStorageBackend
//
// Wraps a shared rusqlite::Connection behind `Arc<Mutex<_>>` and implements
// the async `StorageBackend` trait expected by `sync_server::SyncServer`
// and `sync_client::SyncClient`. Blocking rusqlite work is wrapped in
// `tokio::task::spawn_blocking` so it doesn't stall the async runtime.
// ---------------------------------------------------------------------------

fn load_usage_sequence_page(
    conn: &Connection,
    remote_vector: &VersionVector,
    offset: usize,
    limit: usize,
) -> Result<Vec<SyncEntity>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let mut cursors = remote_vector
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix("@usage:")
                .and_then(|device_id| value.parse::<i64>().ok().map(|seq| (device_id, seq)))
        })
        .collect::<Vec<_>>();
    cursors.sort_unstable_by(|a, b| a.0.cmp(b.0));

    let mut sql = String::from(
        "SELECT device_id, seq, entity_type, entity_id
         FROM usage_sync_log WHERE seq > ",
    );
    let mut values = Vec::<rusqlite::types::Value>::new();
    if cursors.is_empty() {
        sql.push('0');
    } else {
        sql.push_str("CASE device_id ");
        for (device_id, seq) in cursors {
            sql.push_str("WHEN ? THEN ? ");
            values.push(device_id.to_string().into());
            values.push(seq.into());
        }
        sql.push_str("ELSE 0 END");
    }
    sql.push_str(" ORDER BY device_id, seq LIMIT ? OFFSET ?");
    values.push((limit as i64).into());
    values.push((offset as i64).into());

    let mut statement = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let refs = statement
        .query_map(params_from_iter(values), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(statement);

    fn to_data_map<T: serde::Serialize>(item: &T) -> serde_json::Map<String, Value> {
        match serde_json::to_value(item) {
            Ok(Value::Object(mut map)) => {
                map.remove("id");
                map
            }
            _ => serde_json::Map::new(),
        }
    }

    let mut entities = Vec::with_capacity(refs.len());
    for (origin_device_id, seq, entity_type, entity_id) in refs {
        let hlc = conn
            .query_row(
                "SELECT hlc FROM usage_sync_versions
                 WHERE entity_type = ?1 AND entity_id = ?2",
                params![entity_type, entity_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| HLC::now(&origin_device_id).to_string());
        let deleted = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sync_tombstones
                    WHERE entity_type = ?1 AND id = ?2
                 )",
                params![entity_type, entity_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            != 0;
        let data = if deleted {
            Some(serde_json::Map::new())
        } else {
            match entity_type.as_str() {
                "usage_session" => load_usage_session(conn, &entity_id)?
                    .as_ref()
                    .map(to_data_map),
                "usage_event" => load_usage_event(conn, &entity_id)?
                    .as_ref()
                    .map(to_data_map),
                "usage_day" => load_usage_day(conn, &entity_id)?.as_ref().map(to_data_map),
                _ => None,
            }
        };
        let Some(data) = data else {
            continue;
        };
        entities.push(SyncEntity {
            entity_type,
            id: entity_id,
            data,
            hlc,
            deleted: deleted.then_some(true),
            origin_device_id: Some(origin_device_id),
            origin_seq: Some(seq.max(0) as u64),
        });
    }
    Ok(entities)
}

pub struct SqliteStorageBackend {
    conn: Arc<Mutex<rusqlite::Connection>>,
    device_id: Arc<Mutex<String>>,
}

impl SqliteStorageBackend {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self {
            conn,
            device_id: Arc::new(Mutex::new(String::new())),
        }
    }

    /// Set the device id used to stamp HLCs on entities that don't yet carry
    /// one in the version vector. Matches the TS `DelphiStorage` behaviour
    /// (`HLC.now(deviceId)` when `vector[id]` is missing).
    pub fn set_device_id(&self, device_id: &str) -> Result<(), String> {
        // Poison recovery: device_id — простой String, poison невозможен от
        // sane code path, но защита cheap и согласована с остальными
        // SqliteStorageBackend lock'ами (см. load_entities / apply_entity).
        let mut guard = self.device_id.lock().unwrap_or_else(|e| e.into_inner());
        *guard = device_id.to_string();
        drop(guard);
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        ensure_usage_sequence_migrated(&conn, device_id)
    }

    pub fn device_id(&self) -> String {
        self.device_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn collect_entities_blocking(
        conn: &Connection,
        vector: &VersionVector,
        device_id: &str,
    ) -> Vec<SyncEntity> {
        let mut entities = Vec::new();
        let mut offset = 0;
        loop {
            let page = Self::collect_entities_page_blocking(conn, vector, device_id, offset, 100);
            if page.is_empty() {
                break;
            }
            offset += page.len();
            entities.extend(page);
        }
        entities
    }

    fn collect_entities_page_blocking(
        conn: &Connection,
        remote_vector: &VersionVector,
        device_id: &str,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity> {
        if limit == 0 {
            return Vec::new();
        }

        fn to_data_map<T: serde::Serialize>(item: &T) -> serde_json::Map<String, Value> {
            match serde_json::to_value(item) {
                Ok(Value::Object(mut map)) => {
                    map.remove("id");
                    map
                }
                _ => serde_json::Map::new(),
            }
        }

        let local_vector = remote_vector;
        let make_entity = |entity_type: &str, id: &str, data| SyncEntity {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
            data,
            hlc: local_vector
                .get(id)
                .cloned()
                .unwrap_or_else(|| HLC::now(device_id).to_string()),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };

        let fixed_count: usize = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM todos) +
                    (SELECT COUNT(*) FROM projects) +
                    (SELECT COUNT(*) FROM areas) +
                    (SELECT COUNT(*) FROM tags) +
                    (SELECT COUNT(*) FROM headings) +
                    (SELECT COUNT(*) FROM tracked_apps) +
                    (SELECT COUNT(*) FROM object_types) +
                    (SELECT COUNT(*) FROM objects) +
                    (SELECT COUNT(*) FROM object_links) +
                    (SELECT COUNT(*) FROM sync_tombstones)",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let mut fixed = Vec::new();
        macro_rules! add_entities {
            ($items:expr, $kind:literal) => {
                if let Ok(items) = $items {
                    for item in &items {
                        fixed.push(make_entity($kind, &item.id, to_data_map(item)));
                    }
                }
            };
        }
        if offset < fixed_count {
            add_entities!(load_all_todos(conn), "todo");
            add_entities!(load_all_projects(conn), "project");
            add_entities!(load_all_areas(conn), "area");
            add_entities!(load_all_tags(conn), "tag");
            add_entities!(load_all_headings(conn), "heading");
            add_entities!(load_all_tracked_apps(conn), "tracked_app");
            add_entities!(list_object_types(conn), "object_type");
            add_entities!(list_objects(conn), "object");
            add_entities!(list_object_links(conn), "object_link");
            if let Ok(tombstones) = load_sync_tombstones(conn) {
                fixed.extend(tombstones);
            }
            fixed.sort_by(|a, b| {
                a.entity_type
                    .cmp(&b.entity_type)
                    .then_with(|| a.id.cmp(&b.id))
            });
        }

        let mut entities: Vec<_> = fixed.iter().skip(offset).take(limit).cloned().collect();
        if entities.len() == limit {
            return entities;
        }

        let usage_offset = offset.saturating_sub(fixed_count);
        let remaining = limit - entities.len();
        match load_usage_sequence_page(conn, remote_vector, usage_offset, remaining) {
            Ok(usage) => entities.extend(usage),
            Err(error) => eprintln!("[ark-core] failed to load usage sync page: {error}"),
        }
        entities
    }

    fn apply_entity_blocking(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
        conn.execute_batch("SAVEPOINT sync_entity_apply")
            .map_err(|e| e.to_string())?;
        match Self::apply_entity_blocking_inner(conn, entity) {
            Ok(()) => {
                conn.execute_batch("RELEASE sync_entity_apply")
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            Err(error) => {
                let _ =
                    conn.execute_batch("ROLLBACK TO sync_entity_apply; RELEASE sync_entity_apply");
                Err(error)
            }
        }
    }

    fn apply_entity_blocking_inner(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
        let sequenced_usage = is_sequenced_usage_entity(&entity.entity_type);
        if sequenced_usage && !usage_entity_is_newer(conn, entity)? {
            let (device_id, seq) = usage_origin_for_entity(conn, entity)?;
            return record_usage_sequence(
                conn,
                &entity.entity_type,
                &entity.id,
                &device_id,
                seq,
                &entity.hlc,
                entity.deleted == Some(true),
            );
        }
        if matches!(entity.entity_type.as_str(), "area" | "heading") {
            return Err("LegacyPlanningReadOnly".into());
        }
        if matches!(entity.entity_type.as_str(), "todo" | "project" | "tag")
            && entity.deleted == Some(true)
        {
            let mut canonical = entity.clone();
            canonical.entity_type = "object".into();
            return crate::canonical_types::facades::apply_canonical_object_entity(
                conn, &canonical,
            );
        }

        if entity.deleted == Some(true) {
            match entity.entity_type.as_str() {
                "todo" => delete_todo(conn, &entity.id),
                "project" => delete_project(conn, &entity.id),
                "area" => delete_area(conn, &entity.id),
                "tag" => delete_tag(conn, &entity.id),
                "heading" => delete_heading(conn, &entity.id),
                "tracked_app" => delete_tracked_app(conn, &entity.id),
                "usage_session" => delete_usage_session(conn, &entity.id),
                "usage_event" => delete_usage_event(conn, &entity.id),
                "usage_day" => delete_usage_day(conn, &entity.id),
                "object_type" => delete_object_type(conn, &entity.id),
                "object" => delete_object(conn, &entity.id),
                "object_link" => delete_object_link(conn, &entity.id),
                _ => Err(format!("unknown sync entity type '{}'", entity.entity_type)),
            }?;
            upsert_sync_tombstone(conn, entity)?;
            if sequenced_usage {
                finish_usage_entity_sync(conn, entity)?;
            }
            return Ok(());
        }

        let mut full_data = entity.data.clone();
        full_data.insert("id".to_string(), Value::String(entity.id.clone()));
        let value = Value::Object(full_data);

        let result = match entity.entity_type.as_str() {
            "todo" | "project" | "tag" => {
                crate::canonical_types::facades::apply_legacy_compat_entity(conn, entity)
            }
            "area" | "heading" => Err("LegacyPlanningReadOnly".into()),
            "tracked_app" => serde_json::from_value::<TrackedApp>(value)
                .map_err(|e| e.to_string())
                .and_then(|tracked_app| upsert_tracked_app(conn, &tracked_app)),
            "usage_session" => serde_json::from_value::<UsageSession>(value)
                .map_err(|e| e.to_string())
                .and_then(|session| upsert_usage_session(conn, &session)),
            "usage_event" => serde_json::from_value::<UsageEvent>(value)
                .map_err(|e| e.to_string())
                .and_then(|event| upsert_usage_event(conn, &event)),
            "usage_day" => serde_json::from_value::<UsageDay>(value)
                .map_err(|e| e.to_string())
                .and_then(|day| upsert_usage_day(conn, &day)),
            "object_type" => serde_json::from_value::<ObjectType>(value)
                .map_err(|e| e.to_string())
                .and_then(|object_type| upsert_object_type(conn, &object_type)),
            "object" => {
                // The canonical ingress owns identity resolution and validation;
                // pending replay uses this same branch after the registry arrives.
                let mut object_data = match value.clone() {
                    Value::Object(map) => map,
                    _ => return Err("object payload must be an object".into()),
                };
                object_data
                    .entry("typeVersion".to_string())
                    .or_insert_with(|| Value::String("0.0.0-legacy".to_string()));
                let object = serde_json::from_value::<ArkObject>(Value::Object(object_data))
                    .map_err(|e| e.to_string())?;
                if !is_object_definition_known(conn, &object.type_id, &object.type_version)? {
                    insert_pending_object(conn, entity, &object.type_id)?;
                    crate::events::emit_event(json!({
                        "event": "sync_error",
                        "code": "unknown_type_version",
                        "entity_type": "object",
                        "entity_id": entity.id,
                        "awaited_type_id": object.type_id,
                        "awaited_type_version": object.type_version,
                    }));
                    Ok(())
                } else {
                    crate::canonical_types::facades::apply_canonical_object_entity(conn, entity)
                }
            }
            "object_link" => serde_json::from_value::<ObjectLink>(value)
                .map_err(|e| e.to_string())
                .and_then(|link| upsert_object_link(conn, &link)),
            _ => Err(format!("unknown sync entity type '{}'", entity.entity_type)),
        };
        result?;
        delete_sync_tombstone(conn, &entity.id)?;
        if sequenced_usage {
            finish_usage_entity_sync(conn, entity)?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl StorageBackend for SqliteStorageBackend {
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity> {
        let conn = self.conn.clone();
        let vector = vector.clone();
        let device_id = self.device_id();
        tokio::task::spawn_blocking(move || {
            // Poison recovery: если earlier panic заполучил lock, мы всё
            // равно можем читать. Это backend для load (read-only path),
            // данные внутри guard'а целы. Без recovery каждый последующий
            // sync round возвращает empty list → multi-device sync silent
            // фейлится навсегда после первого panic'а в этом процессе.
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            Self::collect_entities_blocking(&guard, &vector, &device_id)
        })
        .await
        .unwrap_or_default()
    }

    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity> {
        let conn = self.conn.clone();
        let vector = vector.clone();
        let device_id = self.device_id();
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            Self::collect_entities_page_blocking(&guard, &vector, &device_id, offset, limit)
        })
        .await
        .unwrap_or_default()
    }

    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String> {
        let conn = self.conn.clone();
        let entity = entity.clone();
        tokio::task::spawn_blocking(move || {
            // Apply — write path. Poison recovery acceptable: SQLite
            // transactions atomic, partially-applied state не возможен.
            // Альтернатива (return Err) делает sync неработоспособным до
            // process restart.
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            Self::apply_entity_blocking(&guard, &entity)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    async fn get_kv(&self, key: &str) -> Option<String> {
        let conn = self.conn.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            get_sync_kv(&guard, &key).unwrap_or(None)
        })
        .await
        .unwrap_or(None)
    }

    async fn set_kv(&self, key: &str, value: &str) {
        let conn = self.conn.clone();
        let key = key.to_string();
        let value = value.to_string();
        let _ = tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            let _ = set_sync_kv(&guard, &key, &value);
        })
        .await;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    fn make_todo(id: &str, title: &str) -> TodoItem {
        TodoItem {
            id: id.to_string(),
            title: title.to_string(),
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
            tag_ids: vec![],
            checklist_items: json!([]),
            recurrence_rule: None,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        }
    }

    fn make_tracked_app(id: &str) -> TrackedApp {
        TrackedApp {
            id: id.to_string(),
            platform: "windows".to_string(),
            exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
            normalized_exe_path: r"c:\\apps\\demo\\demo.exe".to_string(),
            process_name: "demo.exe".to_string(),
            display_name: Some("Demo App".to_string()),
            publisher: Some("Demo Corp".to_string()),
            icon_ref: None,
            first_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
            last_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
        }
    }

    fn make_usage_session(id: &str, tracked_app_id: &str) -> UsageSession {
        UsageSession {
            id: id.to_string(),
            tracked_app_id: tracked_app_id.to_string(),
            device_id: "device-1".to_string(),
            device_name: "Test Device".to_string(),
            platform: "windows".to_string(),
            started_at: "2026-01-01T00:00:00.000Z".to_string(),
            ended_at: Some("2026-01-01T00:10:00.000Z".to_string()),
            runtime_ms: 600_000,
            foreground_ms: 600_000,
            idle_ms: 0,
            window_title: Some("Demo Window".to_string()),
            process_name: "demo.exe".to_string(),
            exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
            pid_start: Some(1234),
            pid_end: Some(1234),
            meta_json: json!({"note": "session"}),
        }
    }

    fn make_usage_event(id: &str, tracked_app_id: &str, session_id: Option<&str>) -> UsageEvent {
        UsageEvent {
            id: id.to_string(),
            tracked_app_id: tracked_app_id.to_string(),
            usage_session_id: session_id.map(|value| value.to_string()),
            device_id: "device-1".to_string(),
            device_name: "Test Device".to_string(),
            platform: "windows".to_string(),
            occurred_at: "2026-01-01T00:05:00.000Z".to_string(),
            kind: "foreground".to_string(),
            window_title: Some("Demo Window".to_string()),
            process_name: "demo.exe".to_string(),
            exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
            pid: Some(1234),
            is_foreground: true,
            is_idle: false,
            meta_json: json!({"note": "event"}),
        }
    }

    fn make_object(id: &str, type_id: &str, title: &str) -> ArkObject {
        ArkObject {
            id: id.to_string(),
            type_id: type_id.to_string(),
            type_version: "0.0.0-legacy".to_string(),
            title: title.to_string(),
            content_json: json!({
                "type": "doc",
                "content": [{ "type": "paragraph" }]
            }),
            props_json: json!({
                "description": format!("Description for {title}")
            }),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            deleted_at: None,
        }
    }

    fn make_object_type(id: &str, name: &str) -> ObjectType {
        ObjectType {
            id: id.to_string(),
            name: name.to_string(),
            schema_json: json!({
                "fields": [
                    { "id": "description", "label": "Описание", "kind": "long_text", "required": false, "visible": true, "read_only": false }
                ]
            })
            .to_string(),
            ui_schema_json: json!({
                "visible_fields": ["description"],
                "hidden_fields": ["created_at", "updated_at", "deleted_at"],
                "read_only_fields": [],
            })
            .to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            system_locked: false,
        }
    }

    fn make_object_link(id: &str, source_object_id: &str, target_object_id: &str) -> ObjectLink {
        ObjectLink {
            id: id.to_string(),
            source_object_id: source_object_id.to_string(),
            target_object_id: target_object_id.to_string(),
            link_type: "related".to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn test_schema_creation() {
        let conn = setup_db();
        // Verify tables exist
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('todos','projects','areas','tags','headings','tracked_apps','usage_sessions','usage_events','sync_kv','sync_tombstones')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 10);
        let object_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('object_types', 'objects', 'object_links')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(object_count, 3);
    }

    #[test]
    fn test_init_schema_migrates_existing_db_without_destroying_data() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE todos (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                notes TEXT,
                priority INTEGER NOT NULL DEFAULT 0,
                scheduled_date TEXT,
                deadline TEXT,
                reminder_date TEXT,
                is_today INTEGER NOT NULL DEFAULT 0,
                is_evening INTEGER NOT NULL DEFAULT 0,
                is_someday INTEGER NOT NULL DEFAULT 0,
                is_completed INTEGER NOT NULL DEFAULT 0,
                completed_at TEXT,
                is_cancelled INTEGER NOT NULL DEFAULT 0,
                cancelled_at TEXT,
                is_trashed INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                heading_id TEXT,
                project_id TEXT,
                area_id TEXT,
                tag_ids TEXT NOT NULL DEFAULT '[]',
                checklist_items TEXT NOT NULL DEFAULT '[]',
                recurrence_rule TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE projects (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                notes TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                scheduled_date TEXT,
                deadline TEXT,
                sort_order INTEGER NOT NULL DEFAULT 0,
                color_tag TEXT,
                area_id TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE areas (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE tags (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                color TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE headings (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                project_id TEXT NOT NULL
            );
            CREATE TABLE sync_kv (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )
        .unwrap();

        let todo = make_todo("legacy-todo", "Keep me");
        upsert_todo(&conn, &todo).unwrap();
        set_sync_kv(&conn, "legacy-key", "legacy-value").unwrap();

        init_schema(&conn).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 1);
        assert_eq!(data.todos[0].id, "legacy-todo");
        assert_eq!(data.todos[0].title, "Keep me");
        assert_eq!(
            get_sync_kv(&conn, "legacy-key").unwrap(),
            Some("legacy-value".to_string())
        );

        let usage_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table'
                   AND name IN ('tracked_apps', 'usage_sessions', 'usage_events')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(usage_table_count, 3);
        let object_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table'
                   AND name IN ('object_types', 'objects', 'object_links')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(object_table_count, 3);
        let tombstone_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table' AND name = 'sync_tombstones'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tombstone_table_count, 1);
        let object_types = list_object_types(&conn).unwrap();
        assert!(object_types.iter().any(|item| item.id == "com.kosmos.note"));
        assert!(object_types.iter().any(|item| item.id == "com.kosmos.task"));
        let migrated: (String, String) = conn
            .query_row(
                "SELECT type_id, type_version FROM objects WHERE id = 'legacy-todo'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            migrated,
            ("com.kosmos.task".to_string(), "1.0.0".to_string()),
        );
        let canonical_type_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM object_types WHERE id = 'com.kosmos.task'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(canonical_type_exists, 1);
    }

    #[test]
    fn test_init_schema_adds_usage_runtime_ms_to_existing_sessions() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE tracked_apps (
                id TEXT PRIMARY KEY,
                platform TEXT NOT NULL,
                exe_path TEXT NOT NULL,
                normalized_exe_path TEXT NOT NULL,
                process_name TEXT NOT NULL,
                display_name TEXT,
                publisher TEXT,
                icon_ref TEXT,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL
            );
            CREATE TABLE usage_sessions (
                id TEXT PRIMARY KEY,
                tracked_app_id TEXT NOT NULL,
                device_id TEXT NOT NULL,
                device_name TEXT NOT NULL,
                platform TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                foreground_ms INTEGER NOT NULL DEFAULT 0,
                idle_ms INTEGER NOT NULL DEFAULT 0,
                window_title TEXT,
                process_name TEXT NOT NULL,
                exe_path TEXT NOT NULL,
                pid_start INTEGER,
                pid_end INTEGER,
                meta_json TEXT NOT NULL DEFAULT '{}'
            );
            INSERT INTO tracked_apps
                (id, platform, exe_path, normalized_exe_path, process_name, display_name, first_seen_at, last_seen_at)
            VALUES
                ('app-legacy', 'windows', 'C:\\Games\\Legacy\\legacy.exe', 'c:\\games\\legacy\\legacy.exe',
                 'legacy.exe', 'Legacy', '2026-01-01T00:00:00.000Z', '2026-01-01T01:00:00.000Z');
            INSERT INTO usage_sessions
                (id, tracked_app_id, device_id, device_name, platform, started_at, ended_at,
                 foreground_ms, idle_ms, window_title, process_name, exe_path)
            VALUES
                ('session-legacy', 'app-legacy', 'device-1', 'Device', 'windows',
                 '2026-01-01T00:00:00.000Z', '2026-01-01T01:00:00.000Z',
                 2400000, 300000, 'Legacy', 'legacy.exe', 'C:\\Games\\Legacy\\legacy.exe');
            ",
        )
        .unwrap();

        init_schema(&conn).unwrap();

        let runtime_ms: i64 = conn
            .query_row(
                "SELECT runtime_ms FROM usage_sessions WHERE id = 'session-legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(runtime_ms, 2_700_000);
        let data = load_all(&conn).unwrap();
        assert_eq!(data.usage_sessions[0].runtime_ms, 2_700_000);
    }

    #[test]
    fn test_todo_crud() {
        let conn = setup_db();
        let todo = make_todo("t1", "Buy milk");
        upsert_todo(&conn, &todo).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 1);
        assert_eq!(data.todos[0].title, "Buy milk");

        delete_todo(&conn, "t1").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 0);
    }

    #[test]
    fn test_batch_upsert() {
        let conn = setup_db();
        let todos = vec![make_todo("t1", "A"), make_todo("t2", "B")];
        batch_upsert_todos(&conn, &todos).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 2);
    }

    #[test]
    fn test_project_crud() {
        let conn = setup_db();
        let project = Project {
            id: "p1".to_string(),
            title: "My Project".to_string(),
            notes: None,
            status: "active".to_string(),
            scheduled_date: None,
            deadline: None,
            sort_order: 0,
            color_tag: None,
            area_id: None,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        };
        upsert_project(&conn, &project).unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].title, "My Project");

        delete_project(&conn, "p1").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.projects.len(), 0);
    }

    #[test]
    fn test_area_crud() {
        let conn = setup_db();
        let area = Area {
            id: "a1".to_string(),
            title: "Work".to_string(),
            sort_order: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        };
        upsert_area(&conn, &area).unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.areas.len(), 1);
    }

    #[test]
    fn test_tag_crud() {
        let conn = setup_db();
        let tag = Tag {
            id: "tg1".to_string(),
            title: "urgent".to_string(),
            color: Some("red".to_string()),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        };
        upsert_tag(&conn, &tag).unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.tags.len(), 1);
        assert_eq!(data.tags[0].color, Some("red".to_string()));
    }

    #[test]
    fn test_heading_crud() {
        let conn = setup_db();
        let heading = Heading {
            id: "h1".to_string(),
            title: "Section 1".to_string(),
            sort_order: 0,
            project_id: "p1".to_string(),
        };
        upsert_heading(&conn, &heading).unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.headings.len(), 1);

        delete_heading(&conn, "h1").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.headings.len(), 0);
    }

    #[test]
    fn test_usage_tracking_crud() {
        let conn = setup_db();
        let tracked_app = make_tracked_app("app-1");
        let session = make_usage_session("session-1", &tracked_app.id);
        let event = make_usage_event("event-1", &tracked_app.id, Some(&session.id));

        upsert_tracked_app(&conn, &tracked_app).unwrap();
        upsert_usage_session(&conn, &session).unwrap();
        upsert_usage_event(&conn, &event).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.tracked_apps.len(), 1);
        assert_eq!(data.usage_sessions.len(), 1);
        assert_eq!(data.usage_events.len(), 1);
        assert_eq!(data.tracked_apps[0].platform, "windows");
        assert_eq!(data.usage_sessions[0].tracked_app_id, "app-1");
        assert_eq!(data.usage_events[0].kind, "foreground");

        delete_usage_event(&conn, "event-1").unwrap();
        delete_usage_session(&conn, "session-1").unwrap();
        delete_tracked_app(&conn, "app-1").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.tracked_apps.len(), 0);
        assert_eq!(data.usage_sessions.len(), 0);
        assert_eq!(data.usage_events.len(), 0);
    }

    #[test]
    fn usage_analytics_snapshot_includes_summary_and_zero_filled_trend() {
        let conn = setup_db();
        let mut tracked_app = make_tracked_app("app-analytics");
        tracked_app.display_name = None;
        upsert_tracked_app(&conn, &tracked_app).unwrap();

        let today = Utc::now().date_naive();
        let today_start = format!("{}T10:00:00.000Z", today.format("%Y-%m-%d"));
        let today_end = format!("{}T10:30:00.000Z", today.format("%Y-%m-%d"));
        let mut session = make_usage_session("session-analytics", &tracked_app.id);
        session.started_at = today_start.clone();
        session.ended_at = Some(today_end);
        session.runtime_ms = 1_500;
        session.foreground_ms = 1_200;
        session.idle_ms = 300;
        upsert_usage_session(&conn, &session).unwrap();

        let mut event = make_usage_event("event-analytics", &tracked_app.id, Some(&session.id));
        event.occurred_at = today_start;
        upsert_usage_event(&conn, &event).unwrap();

        let snapshot = load_usage_analytics(&conn, 3, 5, 5).unwrap();

        assert_eq!(snapshot.summary.tracked_app_count, 1);
        assert_eq!(snapshot.summary.session_count, 1);
        assert_eq!(snapshot.summary.event_count, 1);
        assert_eq!(snapshot.summary.total_runtime_ms, 1_500);
        assert_eq!(snapshot.summary.total_foreground_ms, 1_200);
        assert_eq!(snapshot.summary.total_idle_ms, 300);
        assert_eq!(snapshot.daily_trend.len(), 3);
        assert_eq!(
            snapshot
                .daily_trend
                .iter()
                .filter(|point| point.sessions == 0)
                .count(),
            2,
            "range should include zero-filled days without sessions"
        );
        assert_eq!(snapshot.daily_trend.last().unwrap().sessions, 1);
        assert_eq!(snapshot.top_apps[0].display_name, "demo.exe");
        assert_eq!(snapshot.top_apps[0].icon_ref, None);
        assert_eq!(snapshot.top_apps[0].runtime_ms, 1_500);
        assert_eq!(snapshot.top_apps[0].foreground_ms, 1_200);
        assert_eq!(snapshot.recent_sessions[0].id, "session-analytics");
        assert_eq!(snapshot.recent_sessions[0].runtime_ms, 1_500);
        assert_eq!(snapshot.hourly_heatmap[0].foreground_ms, 1_200);
    }

    #[test]
    fn usage_process_queries_return_recent_and_search_candidates() {
        let conn = setup_db();
        let mut app = make_tracked_app("app-process");
        app.display_name = Some("Nebula Game".to_string());
        app.exe_path = r"C:/Games/Nebula/nebula.exe".to_string();
        app.normalized_exe_path = r"c:\games\nebula\nebula.exe".to_string();
        app.process_name = "Nebula.exe".to_string();
        upsert_tracked_app(&conn, &app).unwrap();
        let mut session = make_usage_session("session-process", &app.id);
        session.started_at = "2026-04-26T10:00:00.000Z".to_string();
        session.ended_at = Some("2026-04-26T11:00:00.000Z".to_string());
        session.runtime_ms = 3_600_000;
        session.foreground_ms = 3_600_000;
        upsert_usage_session(&conn, &session).unwrap();

        let recent = list_recent_usage_processes(&conn, 10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].tracked_app_id, "app-process");
        assert_eq!(recent[0].binding_match_type, "exe_path");
        assert_eq!(
            recent[0].binding_normalized_value,
            r"c:\games\nebula\nebula.exe"
        );
        assert_eq!(recent[0].session_count, 1);

        let search_results = search_usage_processes(&conn, "nebula", 10).unwrap();
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].display_name, "Nebula Game");
        assert!(search_usage_processes(&conn, "missing", 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn usage_game_playtime_summary_matches_bindings_and_range() {
        let conn = setup_db();
        let mut main_app = make_tracked_app("tracked-main");
        main_app.exe_path = r"C:\Games\Chronicle\Chronicle.exe".to_string();
        main_app.normalized_exe_path = r"c:\games\chronicle\chronicle.exe".to_string();
        main_app.process_name = "chronicle.exe".to_string();
        upsert_tracked_app(&conn, &main_app).unwrap();

        let mut alt_app = make_tracked_app("tracked-alt");
        alt_app.exe_path = r"C:\Games\Chronicle\Chronicle_DX12.exe".to_string();
        alt_app.normalized_exe_path = r"c:\games\chronicle\chronicle_dx12.exe".to_string();
        alt_app.process_name = "chronicle_dx12.exe".to_string();
        upsert_tracked_app(&conn, &alt_app).unwrap();

        let mut helper_app = make_tracked_app("tracked-helper");
        helper_app.exe_path = "".to_string();
        helper_app.normalized_exe_path = "".to_string();
        helper_app.process_name = "Chronicle Helper.exe".to_string();
        upsert_tracked_app(&conn, &helper_app).unwrap();

        let mut other_app = make_tracked_app("tracked-other");
        other_app.exe_path = r"C:\Other\Other.exe".to_string();
        other_app.normalized_exe_path = r"c:\other\other.exe".to_string();
        other_app.process_name = "other.exe".to_string();
        upsert_tracked_app(&conn, &other_app).unwrap();

        let sessions = [
            (
                "session-main",
                "tracked-main",
                "2026-04-20T10:00:00.000Z",
                1_800_000,
            ),
            (
                "session-alt",
                "tracked-alt",
                "2026-04-20T11:00:00.000Z",
                3_600_000,
            ),
            (
                "session-helper",
                "tracked-helper",
                "2026-04-21T12:00:00.000Z",
                600_000,
            ),
            (
                "session-other",
                "tracked-other",
                "2026-04-20T13:00:00.000Z",
                9_000_000,
            ),
        ];
        for (session_id, tracked_app_id, started_at, foreground_ms) in sessions {
            let mut session = make_usage_session(session_id, tracked_app_id);
            session.started_at = started_at.to_string();
            session.ended_at = Some(started_at.replace(":00.000Z", ":30.000Z"));
            session.runtime_ms = foreground_ms;
            session.foreground_ms = foreground_ms;
            upsert_usage_session(&conn, &session).unwrap();
        }

        let bindings = vec![
            UsageGamePlaytimeBinding {
                game_id: "game-1".to_string(),
                game_name: "Chronicle".to_string(),
                match_type: "exe_path".to_string(),
                match_value: r"C:\Games\Chronicle\Chronicle.exe".to_string(),
            },
            UsageGamePlaytimeBinding {
                game_id: "game-1".to_string(),
                game_name: "Chronicle".to_string(),
                match_type: "exe_path".to_string(),
                match_value: r"C:\Games\Chronicle\Chronicle_DX12.exe".to_string(),
            },
            UsageGamePlaytimeBinding {
                game_id: "game-1".to_string(),
                game_name: "Chronicle".to_string(),
                match_type: "process_name".to_string(),
                match_value: "chronicle helper.exe".to_string(),
            },
        ];

        let summary = load_usage_game_playtime_summary(
            &conn,
            &bindings,
            Some("2026-04-20"),
            Some("2026-04-21"),
        )
        .unwrap();

        assert_eq!(summary.aggregates.len(), 1);
        assert_eq!(summary.aggregates[0].game_id, "game-1");
        assert_eq!(summary.aggregates[0].total_seconds, 6_000);
        assert_eq!(summary.aggregates[0].session_count, 3);
        assert_eq!(
            summary.daily_totals,
            vec![
                UsageGameDailyTotal {
                    date: "2026-04-20".to_string(),
                    seconds: 5_400,
                },
                UsageGameDailyTotal {
                    date: "2026-04-21".to_string(),
                    seconds: 600,
                },
            ]
        );
        assert_eq!(summary.per_game_totals.len(), 1);
        assert_eq!(summary.per_game_totals[0].seconds, 6_000);
    }

    #[test]
    fn test_object_model_crud() {
        let conn = setup_db();
        let object_type = make_object_type("crud_fixture_book_type", "Книга");
        upsert_object_type(&conn, &object_type).unwrap();
        upsert_object_type(&conn, &make_object_type("crud_fixture_type", "Заметка")).unwrap();

        let note = make_object("obj-1", "crud_fixture_type", "Первая заметка");
        let book = make_object("obj-2", "crud_fixture_book_type", "Clean Code");
        upsert_object(&conn, &note).unwrap();
        upsert_object(&conn, &book).unwrap();

        let link = make_object_link("link-1", "obj-1", "obj-2");
        upsert_object_link(&conn, &link).unwrap();

        let data = load_all(&conn).unwrap();
        assert!(data
            .object_types
            .iter()
            .any(|item| item.id == "com.kosmos.note"));
        assert!(data
            .object_types
            .iter()
            .any(|item| item.id == "com.kosmos.game"));
        assert!(data
            .object_types
            .iter()
            .any(|item| item.id == "crud_fixture_book_type"));
        assert_eq!(data.objects.len(), 2);
        assert_eq!(data.object_links.len(), 1);
        assert_eq!(data.object_links[0].source_object_id, "obj-1");

        delete_object_link(&conn, "link-1").unwrap();
        delete_object(&conn, "obj-2").unwrap();
        delete_object_type(&conn, "crud_fixture_book_type").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.object_links.len(), 0);
        assert_eq!(data.objects.len(), 1);
        assert!(!data
            .object_types
            .iter()
            .any(|item| item.id == "crud_fixture_book_type"));
    }

    #[test]
    fn upsert_object_preserves_existing_links() {
        // Regression: 2026-06-04. SQLite REPLACE deletes the old object row first.
        let conn = setup_db();
        let note = make_object("obj-note", "note_obj", "Первая заметка");
        let task = make_object("obj-task", "note_obj", "Задача");
        let tag = make_object("obj-tag", "note_obj", "Тег");
        upsert_object(&conn, &note).unwrap();
        upsert_object(&conn, &task).unwrap();
        upsert_object(&conn, &tag).unwrap();

        upsert_object_link(&conn, &make_object_link("link-out", "obj-note", "obj-task")).unwrap();
        upsert_object_link(&conn, &make_object_link("link-in", "obj-tag", "obj-note")).unwrap();

        let mut updated_note = make_object("obj-note", "note_obj", "Обновленная заметка");
        updated_note.updated_at = "2026-01-02T00:00:00.000Z".to_string();
        upsert_object(&conn, &updated_note).unwrap();

        let links = list_object_links(&conn).unwrap();
        assert_eq!(links.len(), 2);
        assert!(links.iter().any(|link| {
            link.id == "link-out"
                && link.source_object_id == "obj-note"
                && link.target_object_id == "obj-task"
        }));
        assert!(links.iter().any(|link| {
            link.id == "link-in"
                && link.source_object_id == "obj-tag"
                && link.target_object_id == "obj-note"
        }));
        assert_eq!(
            get_object(&conn, "obj-note").unwrap().unwrap().title,
            "Обновленная заметка"
        );
    }

    #[test]
    fn upsert_object_type_preserves_existing_objects_and_links() {
        // Regression: 2026-06-04. SQLite REPLACE cascades through object_types -> objects -> links.
        let conn = setup_db();
        let object_type = make_object_type("custom_note", "Custom Note");
        upsert_object_type(&conn, &object_type).unwrap();
        upsert_object(&conn, &make_object("obj-a", "custom_note", "A")).unwrap();
        upsert_object(&conn, &make_object("obj-b", "custom_note", "B")).unwrap();
        upsert_object_link(&conn, &make_object_link("link-custom", "obj-a", "obj-b")).unwrap();

        let mut updated_type = make_object_type("custom_note", "Custom Note Updated");
        updated_type.updated_at = "2026-01-02T00:00:00.000Z".to_string();
        upsert_object_type(&conn, &updated_type).unwrap();

        assert_eq!(list_objects_by_type(&conn, "custom_note").unwrap().len(), 2);
        let links = list_object_links(&conn).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].id, "link-custom");
        assert_eq!(
            get_object_type(&conn, "custom_note").unwrap().unwrap().name,
            "Custom Note Updated"
        );
    }

    #[test]
    fn upsert_object_link_updates_existing_row_in_place() {
        // Regression: 2026-06-04. Link upsert should update, not delete+insert.
        let conn = setup_db();
        upsert_object(&conn, &make_object("obj-a", "note_obj", "A")).unwrap();
        upsert_object(&conn, &make_object("obj-b", "note_obj", "B")).unwrap();
        upsert_object(&conn, &make_object("obj-c", "note_obj", "C")).unwrap();
        upsert_object_link(&conn, &make_object_link("link-1", "obj-a", "obj-b")).unwrap();

        let before_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM object_links WHERE id = ?1",
                params!["link-1"],
                |row| row.get(0),
            )
            .unwrap();

        let mut updated_link = make_object_link("link-1", "obj-a", "obj-c");
        updated_link.link_type = "tagged".to_string();
        upsert_object_link(&conn, &updated_link).unwrap();

        let after_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM object_links WHERE id = ?1",
                params!["link-1"],
                |row| row.get(0),
            )
            .unwrap();
        let links = list_object_links(&conn).unwrap();
        assert_eq!(before_rowid, after_rowid);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target_object_id, "obj-c");
        assert_eq!(links[0].link_type, "tagged");
    }

    #[test]
    fn object_query_helpers_filter_by_type_and_ids() {
        let conn = setup_db();
        let object_type = make_object_type("query_fixture_book_type", "Book");
        upsert_object_type(&conn, &object_type).unwrap();
        upsert_object_type(&conn, &make_object_type("query_fixture_type", "Journal")).unwrap();
        upsert_object(
            &conn,
            &make_object("obj-1", "query_fixture_type", "Journal"),
        )
        .unwrap();
        upsert_object(
            &conn,
            &make_object("obj-2", "query_fixture_book_type", "Clean Code"),
        )
        .unwrap();
        upsert_object(
            &conn,
            &make_object("obj-3", "query_fixture_book_type", "Rust Book"),
        )
        .unwrap();

        let books = list_objects_by_type(&conn, "query_fixture_book_type").unwrap();
        assert_eq!(books.len(), 2);
        assert!(books
            .iter()
            .all(|object| object.type_id == "query_fixture_book_type"));

        let ids = vec![
            "obj-3".to_string(),
            "missing".to_string(),
            "obj-1".to_string(),
        ];
        let objects = get_objects_by_ids(&conn, &ids).unwrap();
        let returned_ids = objects
            .iter()
            .map(|object| object.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(returned_ids.len(), 2);
        assert!(returned_ids.contains(&"obj-3"));
        assert!(returned_ids.contains(&"obj-1"));
        assert!(get_objects_by_ids(&conn, &[]).unwrap().is_empty());
    }

    #[test]
    fn object_summary_queries_skip_body_and_filter_by_type() {
        let conn = setup_db();
        let object_type = make_object_type("summary_fixture_book_type", "Book");
        upsert_object_type(&conn, &object_type).unwrap();
        upsert_object_type(&conn, &make_object_type("summary_fixture_type", "Journal")).unwrap();

        let mut older = make_object("obj-1", "summary_fixture_type", "Journal");
        older.content_json = json!({ "text": "large body that must not be selected by summaries" });
        older.props_json = json!({ "kind": "note" });
        older.created_at = "2026-01-01T00:00:00.000Z".to_string();
        older.updated_at = "2026-01-01T00:00:00.000Z".to_string();
        upsert_object(&conn, &older).unwrap();

        let mut newer = make_object("obj-2", "summary_fixture_book_type", "Clean Code");
        newer.content_json = json!({ "text": "another body" });
        newer.props_json = json!({ "kind": "book" });
        newer.created_at = "2026-01-02T00:00:00.000Z".to_string();
        newer.updated_at = "2026-01-02T00:00:00.000Z".to_string();
        upsert_object(&conn, &newer).unwrap();

        let summaries = list_object_summaries(&conn).unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].id, "obj-2");
        assert_eq!(summaries[0].props_json, json!({ "kind": "book" }));
        assert_eq!(summaries[1].id, "obj-1");

        let book_summaries =
            list_object_summaries_by_type(&conn, "summary_fixture_book_type").unwrap();
        assert_eq!(book_summaries.len(), 1);
        assert_eq!(book_summaries[0].id, "obj-2");
        assert_eq!(book_summaries[0].type_id, "summary_fixture_book_type");
    }

    fn make_time_entry(
        id: &str,
        started_at: &str,
        ended_at: Option<&str>,
        source: &str,
    ) -> ArkObject {
        ArkObject {
            id: id.to_string(),
            type_id: "time_entry_obj".to_string(),
            type_version: "0.0.0-legacy".to_string(),
            title: format!("entry {id}"),
            content_json: json!({}),
            props_json: json!({
                "startedAt": started_at,
                "endedAt": ended_at,
                "source": source,
            }),
            created_at: started_at.to_string(),
            updated_at: started_at.to_string(),
            deleted_at: None,
        }
    }

    #[test]
    fn list_running_time_entries_empty_db_returns_empty() {
        let conn = setup_db();
        let result = list_running_time_entries(&conn, None).unwrap();
        assert!(result.is_empty());
        let result_filtered = list_running_time_entries(&conn, Some("manual")).unwrap();
        assert!(result_filtered.is_empty());
    }

    #[test]
    fn list_running_time_entries_returns_only_running() {
        let conn = setup_db();
        upsert_object(
            &conn,
            &make_time_entry("te-running", "2026-05-20T10:00:00.000Z", None, "manual"),
        )
        .unwrap();
        upsert_object(
            &conn,
            &make_time_entry(
                "te-done",
                "2026-05-20T08:00:00.000Z",
                Some("2026-05-20T09:00:00.000Z"),
                "manual",
            ),
        )
        .unwrap();

        let result = list_running_time_entries(&conn, None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "te-running");
    }

    #[test]
    fn list_running_time_entries_filter_by_source() {
        let conn = setup_db();
        upsert_object(
            &conn,
            &make_time_entry("te-manual", "2026-05-20T10:00:00.000Z", None, "manual"),
        )
        .unwrap();
        upsert_object(
            &conn,
            &make_time_entry("te-pomo", "2026-05-20T11:00:00.000Z", None, "pomodoro"),
        )
        .unwrap();
        upsert_object(
            &conn,
            &make_time_entry(
                "te-break",
                "2026-05-20T12:00:00.000Z",
                None,
                "pomodoro_break",
            ),
        )
        .unwrap();

        let manual = list_running_time_entries(&conn, Some("manual")).unwrap();
        assert_eq!(manual.len(), 1);
        assert_eq!(manual[0].id, "te-manual");

        let pomo = list_running_time_entries(&conn, Some("pomodoro")).unwrap();
        assert_eq!(pomo.len(), 1);
        assert_eq!(pomo[0].id, "te-pomo");

        let breaks = list_running_time_entries(&conn, Some("pomodoro_break")).unwrap();
        assert_eq!(breaks.len(), 1);
        assert_eq!(breaks[0].id, "te-break");

        // Без фильтра — все три, отсортированы DESC по startedAt.
        let all = list_running_time_entries(&conn, None).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, "te-break");
        assert_eq!(all[1].id, "te-pomo");
        assert_eq!(all[2].id, "te-manual");
    }

    #[test]
    fn list_running_time_entries_excludes_deleted() {
        let conn = setup_db();
        let mut deleted = make_time_entry("te-del", "2026-05-20T10:00:00.000Z", None, "manual");
        deleted.deleted_at = Some("2026-05-20T10:30:00.000Z".to_string());
        upsert_object(&conn, &deleted).unwrap();

        let result = list_running_time_entries(&conn, None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_object_search_fts_rebuilds_existing_objects_on_init() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_TABLES).unwrap();

        let object_type = make_object_type("search_obj", "Searchable");
        upsert_object_type(&conn, &object_type).unwrap();
        let mut object = make_object("obj-existing-search", "search_obj", "Existing Object");
        object.props_json = json!({
            "description": "preexisting nebula archive"
        });
        upsert_object(&conn, &object).unwrap();

        assert!(
            !object_search_fts_exists(&conn).unwrap(),
            "legacy DB setup should not have the FTS table yet"
        );

        init_schema(&conn).unwrap();

        let results = search_objects(&conn, "nebula").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry_id, "obj-existing-search");
    }

    #[test]
    fn test_object_search_fts_updates_and_removes_index_entries() {
        let conn = setup_db();
        let object_type = make_object_type("search_obj", "Searchable");
        upsert_object_type(&conn, &object_type).unwrap();

        let mut object = make_object("obj-search-update", "search_obj", "Search Target");
        object.props_json = json!({ "description": "alpha marker" });
        upsert_object(&conn, &object).unwrap();
        assert_eq!(
            search_objects(&conn, "alpha").unwrap()[0].entry_id,
            object.id
        );

        object.props_json = json!({ "description": "beta marker" });
        object.updated_at = "2026-01-02T00:00:00.000Z".to_string();
        upsert_object(&conn, &object).unwrap();
        assert!(
            search_objects(&conn, "alpha").unwrap().is_empty(),
            "old FTS text should be removed on object update"
        );
        assert_eq!(
            search_objects(&conn, "beta").unwrap()[0].entry_id,
            object.id
        );

        delete_object(&conn, &object.id).unwrap();
        assert!(
            search_objects(&conn, "beta").unwrap().is_empty(),
            "deleted objects should be removed from the FTS index"
        );
    }

    #[test]
    fn test_object_search_is_punctuation_safe_and_falls_back_without_fts() {
        let conn = setup_db();
        let object_type = make_object_type("search_obj", "Searchable");
        upsert_object_type(&conn, &object_type).unwrap();
        let mut object = make_object("obj-punctuation-search", "search_obj", "C++ Primer");
        object.props_json = json!({
            "description": "boss-fight co-op notes"
        });
        upsert_object(&conn, &object).unwrap();

        let punctuation_results = search_objects(&conn, "boss-fight??").unwrap();
        assert_eq!(punctuation_results.len(), 1);
        assert_eq!(punctuation_results[0].entry_id, "obj-punctuation-search");

        conn.execute_batch("DROP TABLE object_search_fts").unwrap();
        let fallback_results = search_objects(&conn, "co-op").unwrap();
        assert_eq!(fallback_results.len(), 1);
        assert_eq!(fallback_results[0].entry_id, "obj-punctuation-search");
    }

    #[test]
    fn test_sync_kv() {
        let conn = setup_db();
        assert_eq!(get_sync_kv(&conn, "foo").unwrap(), None);
        set_sync_kv(&conn, "foo", "bar").unwrap();
        assert_eq!(get_sync_kv(&conn, "foo").unwrap(), Some("bar".to_string()));
    }

    #[test]
    fn test_clear_all() {
        let conn = setup_db();
        upsert_todo(&conn, &make_todo("t1", "X")).unwrap();
        upsert_tracked_app(&conn, &make_tracked_app("app-clear")).unwrap();
        upsert_usage_session(&conn, &make_usage_session("session-clear", "app-clear")).unwrap();
        upsert_usage_event(
            &conn,
            &make_usage_event("event-clear", "app-clear", Some("session-clear")),
        )
        .unwrap();
        set_sync_kv(&conn, "k", "v").unwrap();
        clear_all(&conn).unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 0);
        assert_eq!(data.tracked_apps.len(), 0);
        assert_eq!(data.usage_sessions.len(), 0);
        assert_eq!(data.usage_events.len(), 0);
        assert_eq!(data.objects.len(), 0);
        assert_eq!(data.object_links.len(), 0);
        assert!(data
            .object_types
            .iter()
            .any(|item| item.id == "com.kosmos.note"));
        assert!(data
            .object_types
            .iter()
            .any(|item| item.id == "com.kosmos.game"));
        assert_eq!(get_sync_kv(&conn, "k").unwrap(), None);
    }

    #[test]
    fn test_delete_trashed() {
        let conn = setup_db();
        let mut todo = make_todo("t1", "Keep");
        upsert_todo(&conn, &todo).unwrap();

        todo.id = "t2".to_string();
        todo.title = "Trash me".to_string();
        todo.is_trashed = true;
        upsert_todo(&conn, &todo).unwrap();

        let count = delete_trashed(&conn).unwrap();
        assert_eq!(count, 1);
        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 1);
        assert_eq!(data.todos[0].id, "t1");
    }

    #[test]
    fn test_todo_with_tags_and_checklist() {
        let conn = setup_db();
        let todo = TodoItem {
            tag_ids: vec!["tag1".to_string(), "tag2".to_string()],
            checklist_items: json!([{"id": "c1", "title": "Step 1", "isCompleted": false}]),
            recurrence_rule: Some(json!({"frequency": "daily", "interval": 1})),
            ..make_todo("t1", "Complex")
        };
        upsert_todo(&conn, &todo).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos[0].tag_ids, vec!["tag1", "tag2"]);
        assert!(data.todos[0].checklist_items.is_array());
        assert!(data.todos[0].recurrence_rule.is_some());
    }

    // -----------------------------------------------------------------------
    // SqliteStorageBackend roundtrip tests (AC3)
    // -----------------------------------------------------------------------

    fn make_backend() -> SqliteStorageBackend {
        let conn = setup_db();
        let shared = Arc::new(Mutex::new(conn));
        let backend = SqliteStorageBackend::new(shared);
        backend.set_device_id("device-under-test").unwrap();
        backend
    }

    fn sync_todo(id: &str, title: &str) -> SyncEntity {
        let todo = make_todo(id, title);
        let value = serde_json::to_value(&todo).unwrap();
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
        };
        map.remove("id");
        SyncEntity {
            entity_type: "todo".to_string(),
            id: id.to_string(),
            data: map,
            hlc: "2026-01-01T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }

    fn sync_tracked_app(id: &str) -> SyncEntity {
        let tracked_app = make_tracked_app(id);
        let value = serde_json::to_value(&tracked_app).unwrap();
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
        };
        map.remove("id");
        SyncEntity {
            entity_type: "tracked_app".to_string(),
            id: id.to_string(),
            data: map,
            hlc: "2026-01-01T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }

    fn sync_usage_session(id: &str, tracked_app_id: &str) -> SyncEntity {
        let session = make_usage_session(id, tracked_app_id);
        let value = serde_json::to_value(&session).unwrap();
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
        };
        map.remove("id");
        SyncEntity {
            entity_type: "usage_session".to_string(),
            id: id.to_string(),
            data: map,
            hlc: "2026-01-01T00:00:00.000Z:000002:peer-a".to_string(),
            deleted: None,
            origin_device_id: Some("peer-a".to_string()),
            origin_seq: Some(2),
        }
    }

    fn sync_usage_event(id: &str, tracked_app_id: &str, session_id: Option<&str>) -> SyncEntity {
        let event = make_usage_event(id, tracked_app_id, session_id);
        let value = serde_json::to_value(&event).unwrap();
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
        };
        map.remove("id");
        SyncEntity {
            entity_type: "usage_event".to_string(),
            id: id.to_string(),
            data: map,
            hlc: "2026-01-01T00:00:00.000Z:000003:peer-a".to_string(),
            deleted: None,
            origin_device_id: Some("peer-a".to_string()),
            origin_seq: Some(3),
        }
    }

    fn sync_object(id: &str, type_id: &str, title: &str) -> SyncEntity {
        let object = make_object(id, type_id, title);
        let value = serde_json::to_value(&object).unwrap();
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
        };
        map.remove("id");
        SyncEntity {
            entity_type: "object".to_string(),
            id: id.to_string(),
            data: map,
            hlc: "2026-01-01T00:00:00.000Z:000010:peer-a".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }

    fn sync_object_link(id: &str, source_object_id: &str, target_object_id: &str) -> SyncEntity {
        let object_link = make_object_link(id, source_object_id, target_object_id);
        let value = serde_json::to_value(&object_link).unwrap();
        let mut map = match value {
            Value::Object(m) => m,
            _ => unreachable!(),
        };
        map.remove("id");
        SyncEntity {
            entity_type: "object_link".to_string(),
            id: id.to_string(),
            data: map,
            hlc: "2026-01-01T00:00:00.000Z:000011:peer-a".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }

    #[tokio::test]
    async fn storage_backend_roundtrip_todo() {
        let backend = make_backend();
        let entity = sync_todo("tbk1", "Roundtrip");
        backend.apply_entity(&entity).await.unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        let found = loaded
            .iter()
            .find(|e| e.id == "tbk1")
            .expect("inserted todo should be loaded");
        assert_eq!(found.entity_type, "object");
        assert_eq!(
            found.data.get("title").and_then(|v| v.as_str()),
            Some("Roundtrip")
        );
    }

    #[tokio::test]
    async fn storage_backend_rejects_invalid_sync_payload() {
        let backend = make_backend();
        let mut entity = sync_todo("tbk-invalid", "Invalid");
        entity.data.insert("title".to_string(), json!(123));

        let err = backend
            .apply_entity(&entity)
            .await
            .expect_err("invalid payload should return an apply error");
        assert!(
            err.contains("invalid type") || err.contains("expected"),
            "unexpected error: {err}",
        );
    }

    #[tokio::test]
    async fn storage_backend_rejects_unknown_sync_entity_type() {
        let backend = make_backend();
        let mut entity = sync_todo("tbk-unknown", "Unknown");
        entity.entity_type = "unknown_entity".to_string();

        let err = backend
            .apply_entity(&entity)
            .await
            .expect_err("unknown entity type should return an apply error");
        assert!(
            err.contains("unknown sync entity type"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn storage_backend_roundtrip_usage_entities() {
        let backend = make_backend();
        backend
            .apply_entity(&sync_tracked_app("app-sync"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_usage_session("session-sync", "app-sync"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_usage_event(
                "event-sync",
                "app-sync",
                Some("session-sync"),
            ))
            .await
            .unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;

        let tracked_app = loaded
            .iter()
            .find(|e| e.entity_type == "tracked_app" && e.id == "app-sync")
            .expect("inserted tracked app should be loaded");
        assert_eq!(
            tracked_app.data.get("displayName").and_then(|v| v.as_str()),
            Some("Demo App")
        );

        let session = loaded
            .iter()
            .find(|e| e.entity_type == "usage_session" && e.id == "session-sync")
            .expect("inserted usage session should be loaded");
        assert_eq!(
            session.data.get("trackedAppId").and_then(|v| v.as_str()),
            Some("app-sync")
        );

        let event = loaded
            .iter()
            .find(|e| e.entity_type == "usage_event" && e.id == "event-sync")
            .expect("inserted usage event should be loaded");
        assert_eq!(
            event.data.get("kind").and_then(|v| v.as_str()),
            Some("foreground")
        );
    }

    #[tokio::test]
    async fn storage_backend_pages_large_usage_history() {
        let conn = setup_db();
        upsert_tracked_app(&conn, &make_tracked_app("app-paged")).unwrap();
        for index in 0..250 {
            upsert_usage_event(
                &conn,
                &make_usage_event(&format!("event-paged-{index:03}"), "app-paged", None),
            )
            .unwrap();
        }
        let backend = SqliteStorageBackend::new(Arc::new(Mutex::new(conn)));
        backend.set_device_id("device-paged").unwrap();
        let vector = VersionVector::new();
        let mut offset = 0;
        let mut usage_event_ids = std::collections::HashSet::new();

        loop {
            let page = backend.load_entities_page(&vector, offset, 100).await;
            assert!(page.len() <= 100);
            if page.is_empty() {
                break;
            }
            offset += page.len();
            usage_event_ids.extend(
                page.into_iter()
                    .filter(|entity| entity.entity_type == "usage_event")
                    .map(|entity| entity.id),
            );
        }

        assert_eq!(usage_event_ids.len(), 250);
    }

    #[tokio::test]
    async fn storage_backend_roundtrip_object_entities() {
        let backend = make_backend();
        backend
            .apply_entity(&sync_object("obj-sync-a", "note_obj", "Ark note"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_object("obj-sync-b", "game_obj", "Ark game"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_object_link("link-sync", "obj-sync-a", "obj-sync-b"))
            .await
            .unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;

        let note = loaded
            .iter()
            .find(|e| e.entity_type == "object" && e.id == "obj-sync-a")
            .expect("inserted object should be loaded");
        assert_eq!(
            note.data.get("title").and_then(|v| v.as_str()),
            Some("Ark note")
        );

        let link = loaded
            .iter()
            .find(|e| e.entity_type == "object_link" && e.id == "link-sync")
            .expect("inserted object link should be loaded");
        assert_eq!(
            link.data.get("sourceObjectId").and_then(|v| v.as_str()),
            Some("obj-sync-a")
        );
    }

    #[tokio::test]
    async fn storage_backend_delete_removes_entity() {
        let backend = make_backend();
        let entity = sync_todo("tbk2", "To delete");
        backend.apply_entity(&entity).await.unwrap();

        // Now apply a tombstone.
        let tombstone = SyncEntity {
            entity_type: "todo".to_string(),
            id: "tbk2".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-02T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        };
        backend.apply_entity(&tombstone).await.unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        let found = loaded
            .iter()
            .find(|e| e.id == "tbk2")
            .expect("deleted entity should be emitted as a tombstone");
        assert_eq!(found.entity_type, "object");
        assert_eq!(found.deleted, Some(true));
        assert_eq!(found.hlc, "2026-01-02T00:00:00.000Z:000001:peer-a");
        assert!(found.data.is_empty());
    }

    #[tokio::test]
    async fn storage_backend_tombstone_survives_backend_recreation() {
        let conn = setup_db();
        let shared = Arc::new(Mutex::new(conn));
        let backend = SqliteStorageBackend::new(shared.clone());

        let tombstone = SyncEntity {
            entity_type: "todo".to_string(),
            id: "tbk-recreated".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-02T00:00:00.000Z:000002:peer-a".to_string(),
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        };
        backend.apply_entity(&tombstone).await.unwrap();

        let recreated = SqliteStorageBackend::new(shared);
        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = recreated.load_entities(&empty_vector).await;
        assert!(
            loaded
                .iter()
                .any(|e| e.id == "tbk-recreated" && e.deleted == Some(true)),
            "tombstone should survive recreating the storage backend",
        );
    }

    #[tokio::test]
    async fn storage_backend_live_entity_clears_tombstone() {
        let backend = make_backend();
        let tombstone = SyncEntity {
            entity_type: "todo".to_string(),
            id: "tbk-resurrected".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-02T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        };
        backend.apply_entity(&tombstone).await.unwrap();

        let mut live = sync_todo("tbk-resurrected", "Live again");
        live.hlc = "2026-01-03T00:00:00.000Z:000001:peer-a".to_string();
        backend.apply_entity(&live).await.unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        let matching: Vec<&SyncEntity> = loaded
            .iter()
            .filter(|e| e.id == "tbk-resurrected")
            .collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].deleted, None);
        assert_eq!(
            matching[0].data.get("title").and_then(|v| v.as_str()),
            Some("Live again")
        );
    }

    #[test]
    fn local_sync_version_bump_persists_hlc_for_entity() {
        let conn = setup_db();
        let first =
            bump_sync_version_vector(&conn, "object", "obj-local", "device-local", false).unwrap();
        let second =
            bump_sync_version_vector(&conn, "object", "obj-local", "device-local", false).unwrap();

        assert!(HLC::is_newer(&second, &first) || second > first);
        let raw = get_sync_kv(&conn, VERSION_VECTOR_KEY)
            .unwrap()
            .expect("version vector should be stored");
        let vector: VersionVector = serde_json::from_str(&raw).unwrap();
        assert_eq!(vector.get("obj-local"), Some(&second));
        assert!(second.ends_with(":device-local"));
    }

    #[test]
    fn local_sync_tombstone_is_persisted_and_can_be_cleared() {
        let conn = setup_db();
        record_sync_tombstone(
            &conn,
            "object",
            "obj-deleted",
            "2026-01-02T00:00:00.000Z:000001:device-local",
        )
        .unwrap();

        let tombstones = load_sync_tombstones(&conn).unwrap();
        assert!(
            tombstones.iter().any(|entity| {
                entity.entity_type == "object"
                    && entity.id == "obj-deleted"
                    && entity.deleted == Some(true)
            }),
            "local delete should leave a durable tombstone",
        );

        delete_sync_tombstone(&conn, "obj-deleted").unwrap();
        let tombstones = load_sync_tombstones(&conn).unwrap();
        assert!(
            tombstones.iter().all(|entity| entity.id != "obj-deleted"),
            "local live upsert should be able to clear prior tombstone",
        );
    }

    #[tokio::test]
    async fn storage_backend_delete_usage_entity_removes_entity() {
        let backend = make_backend();
        backend
            .apply_entity(&sync_tracked_app("app-del"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_usage_session("session-del", "app-del"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_usage_event(
                "event-del",
                "app-del",
                Some("session-del"),
            ))
            .await
            .unwrap();

        let tombstone = SyncEntity {
            entity_type: "usage_event".to_string(),
            id: "event-del".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-02T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: Some(true),
            origin_device_id: Some("peer-a".to_string()),
            origin_seq: Some(4),
        };
        backend.apply_entity(&tombstone).await.unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        let found = loaded
            .iter()
            .find(|e| e.id == "event-del")
            .expect("deleted usage event should be emitted as a tombstone");
        assert_eq!(found.entity_type, "usage_event");
        assert_eq!(found.deleted, Some(true));
        assert!(found.data.is_empty());
    }

    #[tokio::test]
    async fn storage_backend_kv_roundtrip() {
        let backend = make_backend();
        assert_eq!(backend.get_kv("foo").await, None);
        backend.set_kv("foo", "bar").await;
        assert_eq!(backend.get_kv("foo").await, Some("bar".to_string()));
    }

    #[tokio::test]
    async fn storage_backend_uses_stored_hlc_when_present() {
        let backend = make_backend();
        let entity = sync_todo("tbk3", "With HLC");
        backend.apply_entity(&entity).await.unwrap();

        let mut vector: VersionVector = std::collections::HashMap::new();
        vector.insert(
            "tbk3".to_string(),
            "2026-03-01T00:00:00.000Z:000005:peer-b".to_string(),
        );

        let loaded = backend.load_entities(&vector).await;
        let found = loaded
            .iter()
            .find(|e| e.id == "tbk3")
            .expect("entity present");
        assert_eq!(
            found.hlc,
            "2026-03-01T00:00:00.000Z:000005:peer-b".to_string(),
            "load_entities should prefer HLC from the passed version vector",
        );
    }

    // -----------------------------------------------------------------------
    // Phase 2: hold-and-replay (sync_pending_objects)
    // -----------------------------------------------------------------------

    fn phase2_make_object_type(id: &str, name: &str) -> ObjectType {
        ObjectType {
            id: id.to_string(),
            name: name.to_string(),
            schema_json: "{}".to_string(),
            ui_schema_json: "{}".to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            system_locked: false,
        }
    }

    fn phase2_make_sync_entity_object(id: &str, type_id: &str, title: &str) -> SyncEntity {
        let mut data = serde_json::Map::new();
        data.insert("typeId".to_string(), Value::String(type_id.to_string()));
        data.insert(
            "typeVersion".to_string(),
            Value::String("0.0.0-legacy".to_string()),
        );
        data.insert("title".to_string(), Value::String(title.to_string()));
        data.insert("contentJson".to_string(), json!({}));
        data.insert("propsJson".to_string(), json!({}));
        data.insert(
            "createdAt".to_string(),
            Value::String("2026-01-01T00:00:00.000Z".to_string()),
        );
        data.insert(
            "updatedAt".to_string(),
            Value::String("2026-01-01T00:00:00.000Z".to_string()),
        );
        SyncEntity {
            entity_type: "object".to_string(),
            id: id.to_string(),
            data,
            hlc: "2026-01-01T00:00:00.000Z:000001:test".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }

    #[test]
    fn phase2_is_object_type_known_false_when_missing() {
        let conn = setup_db();
        assert!(!is_object_type_known(&conn, "nonexistent").unwrap());
    }

    #[test]
    fn phase2_is_object_type_known_true_after_upsert() {
        let conn = setup_db();
        let object_type = phase2_make_object_type("type_x", "Type X");
        upsert_object_type(&conn, &object_type).unwrap();
        assert!(is_object_type_known(&conn, "type_x").unwrap());
    }

    #[test]
    fn phase2_insert_pending_object_persists_payload() {
        let conn = setup_db();
        let entity = phase2_make_sync_entity_object("obj-1", "future_type", "Pending object");
        insert_pending_object(&conn, &entity, "future_type").unwrap();
        assert_eq!(count_pending_for_type(&conn, "future_type").unwrap(), 1);
        assert_eq!(count_pending_for_type(&conn, "other_type").unwrap(), 0);
    }

    #[test]
    fn phase2_insert_pending_overwrites_same_id() {
        // Если sync приносит обновлённую version того же object'а, REPLACE'ит,
        // не дублирует.
        let conn = setup_db();
        let e1 = phase2_make_sync_entity_object("obj-1", "future_type", "First");
        let e2 = phase2_make_sync_entity_object("obj-1", "future_type", "Second");
        insert_pending_object(&conn, &e1, "future_type").unwrap();
        insert_pending_object(&conn, &e2, "future_type").unwrap();
        assert_eq!(count_pending_for_type(&conn, "future_type").unwrap(), 1);
    }

    #[test]
    fn phase2_replay_runs_when_type_appears() {
        let conn = setup_db();
        // 1. Pending object для типа, который ещё не существует.
        let entity = phase2_make_sync_entity_object("obj-1", "type_late", "Awaiting type");
        insert_pending_object(&conn, &entity, "type_late").unwrap();
        assert_eq!(count_pending_for_type(&conn, "type_late").unwrap(), 1);

        // Object table должна быть пустой.
        let objects_before = list_objects(&conn).unwrap();
        assert_eq!(objects_before.len(), 0);

        // 2. Создаём тип — должен сработать auto-replay.
        let object_type = phase2_make_object_type("type_late", "Late Type");
        upsert_object_type(&conn, &object_type).unwrap();

        // 3. Pending очищен, object материализован.
        assert_eq!(count_pending_for_type(&conn, "type_late").unwrap(), 0);
        let objects_after = list_objects(&conn).unwrap();
        assert_eq!(objects_after.len(), 1);
        assert_eq!(objects_after[0].id, "obj-1");
        assert_eq!(objects_after[0].title, "Awaiting type");
        assert_eq!(objects_after[0].type_id, "type_late");
    }

    #[test]
    fn phase2_replay_handles_multiple_pending_same_type() {
        let conn = setup_db();
        for i in 0..5 {
            let e = phase2_make_sync_entity_object(
                &format!("obj-{i}"),
                "batch_type",
                &format!("Obj {i}"),
            );
            insert_pending_object(&conn, &e, "batch_type").unwrap();
        }
        assert_eq!(count_pending_for_type(&conn, "batch_type").unwrap(), 5);

        upsert_object_type(&conn, &phase2_make_object_type("batch_type", "Batch")).unwrap();

        assert_eq!(count_pending_for_type(&conn, "batch_type").unwrap(), 0);
        assert_eq!(list_objects(&conn).unwrap().len(), 5);
    }

    #[test]
    fn phase2_replay_only_targets_matching_type() {
        let conn = setup_db();
        insert_pending_object(
            &conn,
            &phase2_make_sync_entity_object("obj-a", "type_a", "A"),
            "type_a",
        )
        .unwrap();
        insert_pending_object(
            &conn,
            &phase2_make_sync_entity_object("obj-b", "type_b", "B"),
            "type_b",
        )
        .unwrap();

        upsert_object_type(&conn, &phase2_make_object_type("type_a", "Type A")).unwrap();

        // Только obj-a replayed; obj-b всё ещё в pending.
        assert_eq!(count_pending_for_type(&conn, "type_a").unwrap(), 0);
        assert_eq!(count_pending_for_type(&conn, "type_b").unwrap(), 1);
        let objects = list_objects(&conn).unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "obj-a");
    }

    // --- Hardening (2026-05-18) ---

    #[test]
    fn check_integrity_passes_on_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        check_integrity(&conn).expect("пустая DB должна проходить integrity_check");
    }

    #[test]
    fn check_integrity_passes_after_init_schema() {
        let conn = setup_db();
        check_integrity(&conn).expect("DB после init_schema должна быть целостной");
    }

    #[test]
    fn init_schema_fails_on_corrupted_db() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        // Создаём валидную DB и наполняем данными (нужно ≥ 1 data page чтобы
        // повредить не header).
        {
            let conn = open_db(path.to_str().unwrap()).unwrap();
            init_schema(&conn).unwrap();
            let object_type = make_object_type("corruption_fixture_type", "Note");
            upsert_object_type(&conn, &object_type).unwrap();
            for i in 0..100 {
                let mut obj = make_object(
                    &format!("obj-{i}"),
                    "corruption_fixture_type",
                    &format!("Title {i}"),
                );
                obj.props_json = json!({ "n": i, "padding": "x".repeat(200) });
                upsert_object(&conn, &obj).unwrap();
            }
        }
        // Портим data pages (offset 8192+, после header page и schema page) —
        // SQLite header останется валидным, integrity_check обнаружит
        // повреждённые btree pages.
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            file.seek(SeekFrom::Start(8192)).unwrap();
            file.write_all(&[0xFF; 4096]).unwrap();
            file.flush().unwrap();
        }
        // Открываем снова. open_db может пройти (header интактен) или fail
        // на PRAGMA journal_mode. Если открыт — init_schema fail'ит на
        // integrity_check. Любой путь — fail-loud.
        let open_result = open_db(path.to_str().unwrap());
        if let Ok(conn) = open_result {
            let result = init_schema(&conn);
            assert!(
                result.is_err(),
                "init_schema должен fail при corrupted DB; результат: {result:?}"
            );
            let err = result.unwrap_err();
            assert!(
                err.contains("integrity")
                    || err.contains("corruption")
                    || err.contains("malformed"),
                "ошибка должна упоминать integrity/corruption/malformed, получили: {err}"
            );
        }
        // else: open_db уже fail'ил — тоже acceptable fail-loud path.
    }

    #[test]
    fn backup_to_file_creates_valid_copy() {
        let conn = setup_db();
        // Положим test данные.
        let object_type = make_object_type("backup_fixture_type", "Note");
        upsert_object_type(&conn, &object_type).unwrap();
        let mut object = make_object("obj-backup", "backup_fixture_type", "Backup test");
        object.props_json = json!({"tag": "test"});
        upsert_object(&conn, &object).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("ark.db.backup");
        let dest_str = dest.to_str().unwrap();
        backup_to_file(&conn, dest_str).expect("backup должен пройти");

        assert!(dest.exists(), "файл backup'а должен существовать");
        let backup_conn = open_db(dest_str).unwrap();
        let objects = list_objects(&backup_conn).unwrap();
        assert_eq!(
            objects.len(),
            1,
            "backup должен содержать оригинальный объект"
        );
        assert_eq!(objects[0].id, "obj-backup");
        assert_eq!(objects[0].title, "Backup test");
    }

    #[test]
    fn backup_to_file_chunked_creates_valid_copy_via_separate_connection() {
        let tmp = tempfile::tempdir().unwrap();
        let src_path = tmp.path().join("ark.db");
        let src_str = src_path.to_str().unwrap();
        {
            // Файловый source: chunked backup открывает его по пути отдельным
            // коннекшном (а не из переданного &Connection).
            let conn = open_db(src_str).unwrap();
            init_schema(&conn).unwrap();
            upsert_object_type(
                &conn,
                &make_object_type("backup_chunk_fixture_type", "Note"),
            )
            .unwrap();
            upsert_object(
                &conn,
                &make_object("obj-chunked", "backup_chunk_fixture_type", "Chunked backup"),
            )
            .unwrap();
        }

        let dest = tmp.path().join("ark.db.backup");
        let dest_str = dest.to_str().unwrap();
        backup_to_file_chunked(src_str, dest_str, 4, std::time::Duration::from_millis(0))
            .expect("chunked backup должен пройти");

        assert!(dest.exists(), "файл backup'а должен существовать");
        let backup_conn = open_db(dest_str).unwrap();
        let objects = list_objects(&backup_conn).unwrap();
        assert_eq!(
            objects.len(),
            1,
            "backup должен содержать оригинальный объект"
        );
        assert_eq!(objects[0].id, "obj-chunked");
        assert_eq!(objects[0].title, "Chunked backup");
    }

    // Фаза B — тесты вложенности (RED до замены BEGIN→SAVEPOINT).

    #[test]
    fn batch_upsert_todos_nests_in_outer_transaction() {
        let conn = setup_db();
        // Открываем внешнюю транзакцию — имитируем вызов из with_write_tx.
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        let todos = vec![
            make_todo("bt1", "Nested todo A"),
            make_todo("bt2", "Nested todo B"),
        ];
        // До Фазы B падает: "cannot start a transaction within a transaction".
        // После Фазы B (SAVEPOINT) должно пройти без ошибки.
        batch_upsert_todos(&conn, &todos)
            .expect("batch_upsert_todos должен работать внутри внешней транзакции");
        conn.execute_batch("COMMIT").unwrap();
        // Проверяем, что todo действительно записаны.
        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 2, "оба todo должны быть записаны");
    }

    #[test]
    fn upsert_object_type_nests_in_outer_transaction() {
        let conn = setup_db();
        let ot = make_object_type("ot-nested", "Тип вложенный");
        // Открываем внешнюю транзакцию — имитируем вызов из with_write_tx.
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        // До Фазы B падает через replay_pending_for_type → BEGIN IMMEDIATE:
        // "cannot start a transaction within a transaction".
        // После Фазы B (SAVEPOINT) должно пройти без ошибки.
        upsert_object_type(&conn, &ot)
            .expect("upsert_object_type должен работать внутри внешней транзакции");
        conn.execute_batch("COMMIT").unwrap();
        // Проверяем, что тип записан.
        let types = list_object_types(&conn).unwrap();
        assert!(
            types.iter().any(|t| t.id == "ot-nested"),
            "object_type должен быть записан"
        );
    }

    #[test]
    fn compact_usage_span_is_idempotent_and_dictionary_encoded() {
        let conn = setup_db();
        let mut write = UsageSpanWrite {
            device_id: "device-compact".to_string(),
            started_at_unix: 1_767_225_600,
            ended_at_unix: 1_767_225_630,
            tracked_app_id: "browser".to_string(),
            window_title: Some("Facebook".to_string()),
            flags: 0,
            updated_at: "2026-01-01T00:00:30.000Z".to_string(),
        };
        upsert_usage_span(&conn, &write).unwrap();
        write.ended_at_unix += 30;
        upsert_usage_span(&conn, &write).unwrap();

        let day = load_usage_day(&conn, "usage-day:device-compact:2026-01-01")
            .unwrap()
            .unwrap();
        assert_eq!(day.payload_json["a"], json!(["browser"]));
        assert_eq!(day.payload_json["t"], json!(["Facebook"]));
        assert_eq!(day.payload_json["s"], json!([[0, 60, 0, 0, 0]]));
        assert_eq!(
            get_usage_title_total(&conn, "face").unwrap(),
            UsageTitleTotal {
                active_seconds: 60,
                idle_seconds: 0,
            }
        );
    }

    #[test]
    fn compact_usage_span_splits_at_utc_midnight_and_syncs() {
        let source = setup_db();
        let write = UsageSpanWrite {
            device_id: "device-midnight".to_string(),
            started_at_unix: 1_767_311_990,
            ended_at_unix: 1_767_312_010,
            tracked_app_id: "reader".to_string(),
            window_title: Some("Book".to_string()),
            flags: 1,
            updated_at: "2026-01-02T00:00:10.000Z".to_string(),
        };
        let days = upsert_usage_span(&source, &write).unwrap();
        assert_eq!(days.len(), 2);
        ensure_usage_sequence_migrated(&source, "source-device").unwrap();

        let entities = SqliteStorageBackend::collect_entities_blocking(
            &source,
            &VersionVector::new(),
            "source-device",
        );
        let usage_days = entities
            .iter()
            .filter(|entity| entity.entity_type == "usage_day")
            .collect::<Vec<_>>();
        assert_eq!(usage_days.len(), 2);

        let peer = setup_db();
        for entity in usage_days {
            SqliteStorageBackend::apply_entity_blocking(&peer, entity).unwrap();
        }
        assert_eq!(
            peer.query_row("SELECT COUNT(*) FROM usage_days", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn usage_sequence_cursor_waits_for_missing_entries() {
        let conn = setup_db();

        record_usage_sequence(
            &conn,
            "usage_day",
            "day-2",
            "peer-device",
            2,
            "2026-01-02T00:00:00.000Z:000002:peer-device",
            false,
        )
        .unwrap();
        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(vector.get("@usage:peer-device"), Some(&"0".to_string()));

        record_usage_sequence(
            &conn,
            "usage_day",
            "day-1",
            "peer-device",
            1,
            "2026-01-01T00:00:00.000Z:000001:peer-device",
            false,
        )
        .unwrap();
        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(vector.get("@usage:peer-device"), Some(&"2".to_string()));
    }

    #[test]
    fn local_usage_versions_use_one_device_cursor() {
        let conn = setup_db();
        let first =
            bump_sync_version_vector(&conn, "usage_day", "day-a", "local-device", false).unwrap();
        let second =
            bump_sync_version_vector(&conn, "usage_day", "day-b", "local-device", false).unwrap();

        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(vector.get("@usage:local-device"), Some(&"2".to_string()));
        assert!(!vector.contains_key("day-a"));
        assert!(!vector.contains_key("day-b"));
        assert!(HLC::is_newer(&second, &first));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM usage_sync_log", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            2
        );
    }

    #[test]
    fn legacy_usage_replay_keeps_one_synthetic_sequence() {
        let conn = setup_db();
        let day = UsageDay {
            id: "usage-day:legacy:2026-01-01".to_string(),
            device_id: "legacy".to_string(),
            day: "2026-01-01".to_string(),
            payload_json: json!({"a": ["browser"], "t": ["Example"], "s": [[0, 30, 0, 0, 0]]}),
            updated_at: "2026-01-01T00:00:30.000Z".to_string(),
        };
        let mut data = serde_json::to_value(&day)
            .unwrap()
            .as_object()
            .unwrap()
            .clone();
        data.remove("id");
        let entity = SyncEntity {
            entity_type: "usage_day".to_string(),
            id: day.id.clone(),
            data,
            hlc: "2026-01-01T00:00:30.000Z:000007:old-peer".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };

        SqliteStorageBackend::apply_entity_blocking(&conn, &entity).unwrap();
        SqliteStorageBackend::apply_entity_blocking(&conn, &entity).unwrap();

        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM usage_sync_log", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(
            vector.get("@usage:legacy:old-peer").map(String::as_str),
            Some("1")
        );
        assert!(!vector.contains_key(&day.id));
    }

    #[tokio::test]
    async fn storage_backend_versioned_object_matrix_holds_unknown_payload_and_replays_exactly() {
        let backend = make_backend();
        let conn = backend.conn.clone();
        {
            let guard = conn.lock().unwrap();
            upsert_object_type(&guard, &make_object_type("remote-type", "Remote")).unwrap();
        }
        let mut old = sync_object("remote-old", "remote-type", "old-wire");
        old.data.remove("typeVersion");
        backend.apply_entity(&old).await.unwrap();
        assert_eq!(
            get_object(&conn.lock().unwrap(), "remote-old")
                .unwrap()
                .unwrap()
                .type_version,
            type_registry::LEGACY_VERSION
        );
        let mut unknown = sync_object("remote-unknown", "remote-type", "unknown-wire");
        unknown.data.insert("typeVersion".into(), json!("9.9.9"));
        unknown
            .data
            .insert("futurePayload".into(), json!({"keep":true}));
        backend.apply_entity(&unknown).await.unwrap();
        let guard = conn.lock().unwrap();
        assert!(get_object(&guard, "remote-unknown").unwrap().is_none());
        let payload: String = guard
            .query_row(
                "SELECT payload FROM sync_pending_objects WHERE id='remote-unknown'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(payload.contains("futurePayload"));
        drop(guard);
        let replayed = {
            let guard = conn.lock().unwrap();
            replay_pending_for_type(&guard, "remote-type", type_registry::LEGACY_VERSION).unwrap()
        };
        assert_eq!(replayed, 0);
        assert!(get_object(&conn.lock().unwrap(), "remote-unknown")
            .unwrap()
            .is_none());
    }
}
