use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{Duration, Utc};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};

use crate::hlc::HLC;
use crate::schema::{CREATE_OBJECT_SEARCH_FTS, CREATE_TABLES};
use crate::sync_server::StorageBackend;
use crate::types::*;

const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";

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

pub fn init_schema(conn: &Connection) -> Result<(), String> {
    check_integrity(conn)?;
    conn.execute_batch(CREATE_TABLES)
        .map_err(|e| e.to_string())?;
    let object_search_fts_enabled = ensure_object_search_fts(conn).is_ok();
    seed_builtin_object_types(conn)?;
    if object_search_fts_enabled {
        rebuild_object_search_fts(conn)?;
    }
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

#[allow(dead_code)]
fn builtin_note_object_type() -> ObjectType {
    ObjectType {
        id: "note_obj".to_string(),
        name: "Заметка".to_string(),
        schema_json: json!({
            "fields": [
                {
                    "id": "description",
                    "label": "Описание",
                    "kind": "long_text",
                    "required": false,
                    "visible": true,
                    "read_only": false,
                },
                {
                    "id": "related_notes",
                    "label": "Связанные заметки",
                    "kind": "relation",
                    "required": false,
                    "visible": true,
                    "read_only": false,
                    "link_type": "related",
                }
            ]
        })
        .to_string(),
        ui_schema_json: json!({
            "visible_fields": ["description", "related_notes"],
            "hidden_fields": ["created_at", "updated_at", "deleted_at"],
            "read_only_fields": [],
        })
        .to_string(),
        created_at: "1970-01-01T00:00:00.000Z".to_string(),
        updated_at: "1970-01-01T00:00:00.000Z".to_string(),
        system_locked: true,
    }
}

#[allow(dead_code)]
fn builtin_game_object_type() -> ObjectType {
    ObjectType {
        id: "game_obj".to_string(),
        name: "Игра".to_string(),
        schema_json: json!({
            "fields": [
                { "id": "description", "label": "Описание", "kind": "long_text", "required": false, "visible": true, "read_only": false },
                { "id": "user_rating", "label": "Оценка", "kind": "number", "required": false, "visible": true, "read_only": false },
                {
                    "id": "play_status",
                    "label": "Статус",
                    "kind": "select",
                    "required": false,
                    "visible": true,
                    "read_only": false,
                    "options": ["not_started", "in_progress", "completed", "abandoned"]
                },
                { "id": "genres", "label": "Жанры", "kind": "text", "required": false, "visible": true, "read_only": false },
                { "id": "cover_image", "label": "Обложка", "kind": "image", "required": false, "visible": true, "read_only": false },
                { "id": "background_image", "label": "Фон", "kind": "image", "required": false, "visible": true, "read_only": false },
                {
                    "id": "related_notes",
                    "label": "Связанные заметки",
                    "kind": "relation",
                    "required": false,
                    "visible": true,
                    "read_only": false,
                    "link_type": "related"
                },
                { "id": "exe_path", "label": "Путь к игре", "kind": "text", "required": false, "visible": true, "read_only": false },
                { "id": "save_path", "label": "Путь к сейвам", "kind": "text", "required": false, "visible": true, "read_only": false },
                { "id": "total_playtime_seconds", "label": "Время игры", "kind": "number", "required": false, "visible": true, "read_only": true },
                { "id": "last_played_at", "label": "Последний запуск", "kind": "date", "required": false, "visible": true, "read_only": true },
                { "id": "play_count", "label": "Запусков", "kind": "number", "required": false, "visible": true, "read_only": true },
                { "id": "save_exists", "label": "Сейв найден", "kind": "boolean", "required": false, "visible": true, "read_only": true },
                { "id": "rawg_id", "label": "RAWG ID", "kind": "text", "required": false, "visible": false, "read_only": true },
                { "id": "exe_name", "label": "Имя exe", "kind": "text", "required": false, "visible": false, "read_only": true }
            ]
        })
        .to_string(),
        ui_schema_json: json!({
            "visible_fields": [
                "description",
                "user_rating",
                "play_status",
                "genres",
                "cover_image",
                "background_image",
                "related_notes",
                "exe_path",
                "save_path",
                "total_playtime_seconds",
                "last_played_at",
                "play_count",
                "save_exists"
            ],
            "hidden_fields": ["created_at", "updated_at", "deleted_at", "rawg_id", "exe_name", "sync_source"],
            "read_only_fields": ["total_playtime_seconds", "last_played_at", "play_count", "save_exists", "rawg_id", "exe_name"],
        })
        .to_string(),
        created_at: "1970-01-01T00:00:00.000Z".to_string(),
        updated_at: "1970-01-01T00:00:00.000Z".to_string(),
        system_locked: true,
    }
}

fn builtin_note_object_type_v2() -> ObjectType {
    ObjectType {
        id: "note_obj".to_string(),
        name: "Заметка".to_string(),
        schema_json: json!({
            "fields": [
                {
                    "id": "description",
                    "label": "Описание",
                    "kind": "long_text",
                    "required": false,
                    "visible": true,
                    "read_only": false,
                    "system": false
                },
                {
                    "id": "related_notes",
                    "label": "Связанные заметки",
                    "kind": "relation",
                    "required": false,
                    "visible": true,
                    "read_only": false,
                    "link_type": "related",
                    "system": false
                }
            ]
        })
        .to_string(),
        ui_schema_json: json!({
            "featured_fields": ["description"],
            "visible_fields": ["description", "related_notes"],
            "hidden_fields": ["created_at", "updated_at", "deleted_at"],
            "read_only_fields": [],
            "field_order": ["description", "related_notes"],
            "header_layout": "inline",
            "default_layout": "page",
            "default_template_id": null,
        })
        .to_string(),
        created_at: "1970-01-01T00:00:00.000Z".to_string(),
        updated_at: "1970-01-01T00:00:00.000Z".to_string(),
        system_locked: true,
    }
}

fn builtin_game_object_type_v2() -> ObjectType {
    ObjectType {
        id: "game_obj".to_string(),
        name: "Игра".to_string(),
        schema_json: json!({
            "fields": [
                { "id": "description", "label": "Описание", "kind": "long_text", "required": false, "visible": true, "read_only": false, "system": false },
                { "id": "user_rating", "label": "Оценка", "kind": "number", "required": false, "visible": true, "read_only": false, "system": false },
                {
                    "id": "play_status",
                    "label": "Статус",
                    "kind": "select",
                    "required": false,
                    "visible": true,
                    "read_only": false,
                    "options": ["not_started", "in_progress", "completed", "abandoned"],
                    "system": false
                },
                { "id": "genres", "label": "Жанры", "kind": "text", "required": false, "visible": true, "read_only": false, "system": false },
                { "id": "cover_image", "label": "Обложка", "kind": "image", "required": false, "visible": true, "read_only": false, "system": false },
                { "id": "background_image", "label": "Фон", "kind": "image", "required": false, "visible": true, "read_only": false, "system": false },
                {
                    "id": "related_notes",
                    "label": "Связанные заметки",
                    "kind": "relation",
                    "required": false,
                    "visible": true,
                    "read_only": false,
                    "link_type": "related",
                    "system": false
                },
                { "id": "exe_path", "label": "Путь к игре", "kind": "text", "required": false, "visible": true, "read_only": false, "system": true },
                { "id": "save_path", "label": "Путь к сейвам", "kind": "text", "required": false, "visible": true, "read_only": false, "system": true },
                { "id": "total_playtime_seconds", "label": "Время игры", "kind": "number", "required": false, "visible": true, "read_only": true, "system": true },
                { "id": "last_played_at", "label": "Последний запуск", "kind": "date", "required": false, "visible": true, "read_only": true, "system": true },
                { "id": "play_count", "label": "Запусков", "kind": "number", "required": false, "visible": true, "read_only": true, "system": true },
                { "id": "save_exists", "label": "Сейв найден", "kind": "boolean", "required": false, "visible": true, "read_only": true, "system": true },
                { "id": "rawg_id", "label": "RAWG ID", "kind": "text", "required": false, "visible": false, "read_only": true, "system": true },
                { "id": "exe_name", "label": "Имя exe", "kind": "text", "required": false, "visible": false, "read_only": true, "system": true }
            ]
        })
        .to_string(),
        ui_schema_json: json!({
            "featured_fields": [],
            "visible_fields": [
                "play_status",
                "genres",
                "total_playtime_seconds",
                "last_played_at"
            ],
            "hidden_fields": [
                "created_at",
                "updated_at",
                "deleted_at",
                "description",
                "user_rating",
                "cover_image",
                "background_image",
                "related_notes",
                "exe_path",
                "save_path",
                "play_count",
                "save_exists",
                "rawg_id",
                "exe_name",
                "sync_source"
            ],
            "read_only_fields": ["total_playtime_seconds", "last_played_at", "play_count", "save_exists", "rawg_id", "exe_name"],
            "field_order": [
                "play_status",
                "genres",
                "total_playtime_seconds",
                "last_played_at",
                "description",
                "user_rating",
                "play_count",
                "save_exists",
                "cover_image",
                "background_image",
                "related_notes",
                "exe_path",
                "save_path",
                "rawg_id",
                "exe_name"
            ],
            "header_layout": "inline",
            "default_layout": "page",
            "default_template_id": null,
        })
        .to_string(),
        created_at: "1970-01-01T00:00:00.000Z".to_string(),
        updated_at: "1970-01-01T00:00:00.000Z".to_string(),
        system_locked: true,
    }
}

fn builtin_time_entry_object_type() -> ObjectType {
    ObjectType {
        id: "time_entry_obj".to_string(),
        name: "Запись времени".to_string(),
        schema_json: json!({
            "fields": [
                { "id": "started_at",            "label": "Начало",          "kind": "date",     "required": true,  "visible": true,  "read_only": false, "system": true },
                { "id": "ended_at",              "label": "Конец",           "kind": "date",     "required": false, "visible": true,  "read_only": false, "system": true },
                { "id": "kind",                  "label": "Тип",             "kind": "select",   "required": true,  "visible": true,  "read_only": false, "options": ["manual", "pomodoro_work", "pomodoro_break"], "system": true },
                { "id": "source",                "label": "Источник",        "kind": "select",   "required": false, "visible": true,  "read_only": false, "options": ["manual", "pomodoro", "imported"], "system": true },
                { "id": "pomodoro_session_id",   "label": "Pomodoro-сессия", "kind": "text",     "required": false, "visible": false, "read_only": true,  "system": true },
                { "id": "billable",              "label": "Оплачиваемое",    "kind": "boolean",  "required": false, "visible": true,  "read_only": false, "system": false },
                { "id": "description",           "label": "Описание",        "kind": "long_text","required": false, "visible": true,  "read_only": false, "system": false },
                { "id": "tags",                  "label": "Теги",            "kind": "relation", "required": false, "visible": true,  "read_only": false, "link_type": "tagged",   "system": false },
                { "id": "for_task",              "label": "Задача",          "kind": "relation", "required": false, "visible": true,  "read_only": false, "link_type": "for-task", "system": false }
            ]
        })
        .to_string(),
        ui_schema_json: json!({
            "featured_fields": ["description"],
            "visible_fields": ["started_at", "ended_at", "kind", "billable", "description", "tags", "for_task"],
            "hidden_fields": ["pomodoro_session_id", "source", "created_at", "updated_at", "deleted_at"],
            "read_only_fields": ["pomodoro_session_id"],
            "field_order": ["started_at", "ended_at", "kind", "description", "tags", "for_task", "billable", "source", "pomodoro_session_id"],
            "header_layout": "inline",
            "default_layout": "page",
            "default_template_id": null,
        })
        .to_string(),
        created_at: "1970-01-01T00:00:00.000Z".to_string(),
        updated_at: "1970-01-01T00:00:00.000Z".to_string(),
        system_locked: true,
    }
}

fn builtin_tag_object_type() -> ObjectType {
    ObjectType {
        id: "tag_obj".to_string(),
        name: "Тег".to_string(),
        schema_json: json!({
            "fields": [
                { "id": "color",       "label": "Цвет",     "kind": "text",     "required": false, "visible": true,  "read_only": false, "system": false },
                { "id": "description", "label": "Описание", "kind": "long_text","required": false, "visible": true,  "read_only": false, "system": false }
            ]
        })
        .to_string(),
        ui_schema_json: json!({
            "featured_fields": [],
            "visible_fields": ["color", "description"],
            "hidden_fields": ["created_at", "updated_at", "deleted_at"],
            "read_only_fields": [],
            "field_order": ["color", "description"],
            "header_layout": "inline",
            "default_layout": "page",
            "default_template_id": null,
        })
        .to_string(),
        created_at: "1970-01-01T00:00:00.000Z".to_string(),
        updated_at: "1970-01-01T00:00:00.000Z".to_string(),
        system_locked: true,
    }
}

fn seed_builtin_object_types(conn: &Connection) -> Result<(), String> {
    for object_type in [
        builtin_note_object_type_v2(),
        builtin_game_object_type_v2(),
        builtin_time_entry_object_type(),
        builtin_tag_object_type(),
    ] {
        seed_builtin_object_type(conn, &object_type)?;
    }
    Ok(())
}

fn seed_builtin_object_type(conn: &Connection, builtin: &ObjectType) -> Result<(), String> {
    let existing = get_object_type(conn, &builtin.id)?;

    let merged = if let Some(existing) = existing {
        ObjectType {
            id: builtin.id.clone(),
            name: builtin.name.clone(),
            schema_json: builtin.schema_json.clone(),
            ui_schema_json: existing.ui_schema_json,
            created_at: existing.created_at,
            updated_at: builtin.updated_at.clone(),
            system_locked: true,
        }
    } else {
        builtin.clone()
    };

    upsert_object_type(conn, &merged)
}

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
        "INSERT OR REPLACE INTO todos
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
             ?22, ?23)",
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
    conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;
    for todo in todos {
        if let Err(e) = upsert_todo(conn, todo) {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }
    conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
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
        "INSERT OR REPLACE INTO projects
            (id, title, notes, status, scheduled_date, deadline, sort_order, color_tag, area_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
        "INSERT OR REPLACE INTO areas (id, title, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4)",
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
        "INSERT OR REPLACE INTO tags (id, title, color, created_at)
         VALUES (?1, ?2, ?3, ?4)",
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
        "INSERT OR REPLACE INTO headings (id, title, sort_order, project_id)
         VALUES (?1, ?2, ?3, ?4)",
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
    conn.execute(
        "INSERT OR REPLACE INTO object_types
            (id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
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
    // Phase 2: после появления типа replay'ить objects, которые ждали этот type_id.
    replay_pending_for_type(conn, &object_type.id)?;
    Ok(())
}

pub fn delete_object_type(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM object_types WHERE id = ?1", params![id])
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

/// Сохранить SyncEntity типа "object" в `sync_pending_objects` до появления нужного type.
pub fn insert_pending_object(
    conn: &Connection,
    entity: &SyncEntity,
    awaited_type_id: &str,
) -> Result<(), String> {
    let payload = serde_json::to_string(entity).map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO sync_pending_objects (id, payload, awaited_type_id, received_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![entity.id, payload, awaited_type_id, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Replay'нуть все pending objects, ожидающие указанного `type_id`. Для каждого:
///   1. parse payload → SyncEntity → ArkObject
///   2. upsert_object
///   3. DELETE из pending
///   4. emit `sync_replay` event
/// Возвращает количество replayed.
pub fn replay_pending_for_type(conn: &Connection, type_id: &str) -> Result<usize, String> {
    let mut stmt = conn
        .prepare("SELECT id, payload FROM sync_pending_objects WHERE awaited_type_id = ?1")
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String)> = stmt
        .query_map(params![type_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    conn.execute_batch("BEGIN IMMEDIATE").map_err(|e| e.to_string())?;
    let mut replayed = 0usize;
    let result = (|| {
        for (entity_id, payload_json) in &rows {
            let entity: SyncEntity =
                serde_json::from_str(payload_json).map_err(|e| e.to_string())?;
            let mut data = entity.data.clone();
            data.insert("id".to_string(), Value::String(entity.id.clone()));
            let object: ArkObject = serde_json::from_value(Value::Object(data))
                .map_err(|e| e.to_string())?;
            upsert_object(conn, &object)?;
            conn.execute(
                "DELETE FROM sync_pending_objects WHERE id = ?1",
                params![entity_id],
            )
            .map_err(|e| e.to_string())?;
            replayed += 1;
        }
        Ok::<_, String>(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }
    for (entity_id, _) in &rows {
        crate::events::emit_event(json!({
            "event": "sync_replay",
            "entity_type": "object",
            "entity_id": entity_id,
            "type_id": type_id,
        }));
    }
    Ok(replayed)
}

/// Сколько объектов сейчас ждёт указанный type_id (utility для тестов / observability).
pub fn count_pending_for_type(conn: &Connection, type_id: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM sync_pending_objects WHERE awaited_type_id = ?1",
        params![type_id],
        |row| row.get::<_, i64>(0),
    )
    .map_err(|e| e.to_string())
}

pub fn list_object_types(conn: &Connection) -> Result<Vec<ObjectType>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked
             FROM object_types
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
         WHERE id = ?1",
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
    conn.execute_batch("SAVEPOINT ark_upsert_object")
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        conn.execute(
            "INSERT OR REPLACE INTO objects
                (id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                object.id,
                object.type_id,
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
        title: row.get(2)?,
        content_json: parse_json_or_default(row.get(3)?),
        props_json: parse_json_or_default(row.get(4)?),
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        deleted_at: row.get(7)?,
    })
}

pub fn list_objects(conn: &Connection) -> Result<Vec<ArkObject>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at
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

pub fn list_objects_by_type(conn: &Connection, type_id: &str) -> Result<Vec<ArkObject>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at
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

pub fn get_objects_by_ids(conn: &Connection, ids: &[String]) -> Result<Vec<ArkObject>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = (1..=ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at
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
        "SELECT id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at
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
            "SELECT objects.id, objects.title, objects.content_json, objects.props_json
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
            let content_json: String = row.get(2)?;
            let props_json: String = row.get(3)?;
            Ok(ArkObject {
                id: row.get(0)?,
                type_id: String::new(),
                title: row.get(1)?,
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
        let title_matches = line_matches_query(&object.title, &normalized_query, &query_terms);
        let body_matches = body
            .lines()
            .any(|line| line_matches_query(line.trim(), &normalized_query, &query_terms));

        if !title_matches && !body_matches {
            continue;
        }

        let title_lower = object.title.to_lowercase();
        let body_lower = body.to_lowercase();
        let score = if title_lower.contains(&normalized_query) {
            400
        } else if title_matches {
            300
        } else if body_lower.contains(&normalized_query) {
            200
        } else {
            100
        };

        let (line, text) =
            build_search_context(&object.title, &body, &normalized_query, &query_terms);
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

    matches.sort_by(|left, right| right.0.cmp(&left.0));
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
    conn.execute(
        "INSERT OR REPLACE INTO object_links
            (id, source_object_id, target_object_id, link_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
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
        "INSERT OR REPLACE INTO tracked_apps
            (id, platform, exe_path, normalized_exe_path, process_name,
             display_name, publisher, icon_ref, first_seen_at, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
        "INSERT OR REPLACE INTO usage_sessions
            (id, tracked_app_id, device_id, device_name, platform, started_at, ended_at,
             foreground_ms, idle_ms, window_title, process_name, exe_path,
             pid_start, pid_end, meta_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                 ?8, ?9, ?10, ?11, ?12,
                 ?13, ?14, ?15)",
        params![
            session.id,
            session.tracked_app_id,
            session.device_id,
            session.device_name,
            session.platform,
            session.started_at,
            session.ended_at,
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
        "INSERT OR REPLACE INTO usage_events
            (id, tracked_app_id, usage_session_id, device_id, device_name, platform,
             occurred_at, kind, window_title, process_name, exe_path, pid,
             is_foreground, is_idle, meta_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6,
                 ?7, ?8, ?9, ?10, ?11, ?12,
                 ?13, ?14, ?15)",
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
                    total_foreground_ms: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    total_idle_ms: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    first_recorded_at: row.get(5)?,
                    last_recorded_at: row.get(6)?,
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
                    COALESCE(SUM(usage_sessions.foreground_ms), 0) AS foreground_ms,
                    COALESCE(SUM(usage_sessions.idle_ms), 0) AS idle_ms,
                    COUNT(usage_sessions.id) AS sessions,
                    MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_seen_at
             FROM tracked_apps
             JOIN usage_sessions ON usage_sessions.tracked_app_id = tracked_apps.id
             GROUP BY tracked_apps.id
             ORDER BY foreground_ms DESC, last_seen_at DESC
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
                foreground_ms: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                idle_ms: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                sessions: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                last_seen_at: row.get(7)?,
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
                foreground_ms: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
                idle_ms: row.get::<_, Option<i64>>(9)?.unwrap_or(0),
                window_title: row.get(10)?,
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
                    SUM(CASE WHEN usage_sessions.foreground_ms > 0 THEN 1 ELSE 0 END) AS session_count
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
                    SUM(CASE WHEN usage_sessions.foreground_ms > 0 THEN 1 ELSE 0 END) AS session_count
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
                CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS total_seconds,
                SUM(CASE WHEN usage_sessions.foreground_ms > 0 THEN 1 ELSE 0 END) AS session_count,
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
                    CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS seconds
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
        "INSERT OR REPLACE INTO sync_kv (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn upsert_sync_tombstone(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
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

pub fn bump_sync_version_vector(
    conn: &Connection,
    entity_id: &str,
    device_id: &str,
) -> Result<String, String> {
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
    conn.execute_batch(
        "DELETE FROM object_links;
         DELETE FROM objects;
         DELETE FROM object_types WHERE system_locked = 0;
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
    let todos = load_all_todos(conn)?;
    let projects = load_all_projects(conn)?;
    let areas = load_all_areas(conn)?;
    let tags = load_all_tags(conn)?;
    let headings = load_all_headings(conn)?;
    let tracked_apps = load_all_tracked_apps(conn)?;
    let usage_sessions = load_all_usage_sessions(conn)?;
    let usage_events = load_all_usage_events(conn)?;
    let objects = list_objects(conn)?;
    let object_types = list_object_types(conn)?;
    let object_links = list_object_links(conn)?;
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
    let mut stmt = conn
        .prepare(
            "SELECT id, tracked_app_id, device_id, device_name, platform, started_at,
                    ended_at, foreground_ms, idle_ms, window_title, process_name,
                    exe_path, pid_start, pid_end, meta_json
             FROM usage_sessions
             ORDER BY started_at DESC, id ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let meta_json_str: String = row.get(14)?;
            let meta_json: Value = serde_json::from_str(&meta_json_str).unwrap_or(json!({}));
            Ok(UsageSession {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                device_id: row.get(2)?,
                device_name: row.get(3)?,
                platform: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                foreground_ms: row.get(7)?,
                idle_ms: row.get(8)?,
                window_title: row.get(9)?,
                process_name: row.get(10)?,
                exe_path: row.get(11)?,
                pid_start: row.get(12)?,
                pid_end: row.get(13)?,
                meta_json,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_all_usage_events(conn: &Connection) -> Result<Vec<UsageEvent>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, tracked_app_id, usage_session_id, device_id, device_name, platform,
                    occurred_at, kind, window_title, process_name, exe_path, pid,
                    is_foreground, is_idle, meta_json
             FROM usage_events
             ORDER BY occurred_at DESC, id ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
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
    pub fn set_device_id(&self, device_id: &str) {
        // Poison recovery: device_id — простой String, poison невозможен от
        // sane code path, но защита cheap и согласована с остальными
        // SqliteStorageBackend lock'ами (см. load_entities / apply_entity).
        let mut guard = self.device_id.lock().unwrap_or_else(|e| e.into_inner());
        *guard = device_id.to_string();
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

        // Helper: convert a Serialize value to serde_json::Map, stripping the "id" key.
        fn to_data_map<T: serde::Serialize>(item: &T) -> serde_json::Map<String, Value> {
            match serde_json::to_value(item) {
                Ok(Value::Object(mut map)) => {
                    map.remove("id");
                    map
                }
                _ => serde_json::Map::new(),
            }
        }

        // Use the HLC already stored in the version vector if present;
        // otherwise stamp a fresh `HLC::now(device_id)` so the initial sync
        // has a defined ordering. This mirrors `DelphiStorage.loadEntities`
        // (`vector[todo.id] || HLC.now(this.deviceId).toString()`).
        let hlc_for = |id: &str| -> String {
            if let Some(existing) = vector.get(id) {
                existing.clone()
            } else {
                HLC::now(device_id).to_string()
            }
        };

        if let Ok(todos) = load_all_todos(conn) {
            for todo in &todos {
                entities.push(SyncEntity {
                    entity_type: "todo".to_string(),
                    id: todo.id.clone(),
                    data: to_data_map(todo),
                    hlc: hlc_for(&todo.id),
                    deleted: None,
                });
            }
        }

        if let Ok(projects) = load_all_projects(conn) {
            for project in &projects {
                entities.push(SyncEntity {
                    entity_type: "project".to_string(),
                    id: project.id.clone(),
                    data: to_data_map(project),
                    hlc: hlc_for(&project.id),
                    deleted: None,
                });
            }
        }

        if let Ok(areas) = load_all_areas(conn) {
            for area in &areas {
                entities.push(SyncEntity {
                    entity_type: "area".to_string(),
                    id: area.id.clone(),
                    data: to_data_map(area),
                    hlc: hlc_for(&area.id),
                    deleted: None,
                });
            }
        }

        if let Ok(tags) = load_all_tags(conn) {
            for tag in &tags {
                entities.push(SyncEntity {
                    entity_type: "tag".to_string(),
                    id: tag.id.clone(),
                    data: to_data_map(tag),
                    hlc: hlc_for(&tag.id),
                    deleted: None,
                });
            }
        }

        if let Ok(headings) = load_all_headings(conn) {
            for heading in &headings {
                entities.push(SyncEntity {
                    entity_type: "heading".to_string(),
                    id: heading.id.clone(),
                    data: to_data_map(heading),
                    hlc: hlc_for(&heading.id),
                    deleted: None,
                });
            }
        }

        if let Ok(tracked_apps) = load_all_tracked_apps(conn) {
            for tracked_app in &tracked_apps {
                entities.push(SyncEntity {
                    entity_type: "tracked_app".to_string(),
                    id: tracked_app.id.clone(),
                    data: to_data_map(tracked_app),
                    hlc: hlc_for(&tracked_app.id),
                    deleted: None,
                });
            }
        }

        if let Ok(usage_sessions) = load_all_usage_sessions(conn) {
            for session in &usage_sessions {
                entities.push(SyncEntity {
                    entity_type: "usage_session".to_string(),
                    id: session.id.clone(),
                    data: to_data_map(session),
                    hlc: hlc_for(&session.id),
                    deleted: None,
                });
            }
        }

        if let Ok(usage_events) = load_all_usage_events(conn) {
            for event in &usage_events {
                entities.push(SyncEntity {
                    entity_type: "usage_event".to_string(),
                    id: event.id.clone(),
                    data: to_data_map(event),
                    hlc: hlc_for(&event.id),
                    deleted: None,
                });
            }
        }

        if let Ok(object_types) = list_object_types(conn) {
            for object_type in &object_types {
                entities.push(SyncEntity {
                    entity_type: "object_type".to_string(),
                    id: object_type.id.clone(),
                    data: to_data_map(object_type),
                    hlc: hlc_for(&object_type.id),
                    deleted: None,
                });
            }
        }

        if let Ok(objects) = list_objects(conn) {
            for object in &objects {
                entities.push(SyncEntity {
                    entity_type: "object".to_string(),
                    id: object.id.clone(),
                    data: to_data_map(object),
                    hlc: hlc_for(&object.id),
                    deleted: None,
                });
            }
        }

        if let Ok(object_links) = list_object_links(conn) {
            for object_link in &object_links {
                entities.push(SyncEntity {
                    entity_type: "object_link".to_string(),
                    id: object_link.id.clone(),
                    data: to_data_map(object_link),
                    hlc: hlc_for(&object_link.id),
                    deleted: None,
                });
            }
        }

        if let Ok(tombstones) = load_sync_tombstones(conn) {
            entities.extend(tombstones);
        }

        entities
    }

    fn apply_entity_blocking(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
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
                "object_type" => delete_object_type(conn, &entity.id),
                "object" => delete_object(conn, &entity.id),
                "object_link" => delete_object_link(conn, &entity.id),
                _ => Err(format!("unknown sync entity type '{}'", entity.entity_type)),
            }?;
            return upsert_sync_tombstone(conn, entity);
        }

        let mut full_data = entity.data.clone();
        full_data.insert("id".to_string(), Value::String(entity.id.clone()));
        let value = Value::Object(full_data);

        let result = match entity.entity_type.as_str() {
            "todo" => serde_json::from_value::<TodoItem>(value)
                .map_err(|e| e.to_string())
                .and_then(|todo| upsert_todo(conn, &todo)),
            "project" => serde_json::from_value::<Project>(value)
                .map_err(|e| e.to_string())
                .and_then(|project| upsert_project(conn, &project)),
            "area" => serde_json::from_value::<Area>(value)
                .map_err(|e| e.to_string())
                .and_then(|area| upsert_area(conn, &area)),
            "tag" => serde_json::from_value::<Tag>(value)
                .map_err(|e| e.to_string())
                .and_then(|tag| upsert_tag(conn, &tag)),
            "heading" => serde_json::from_value::<Heading>(value)
                .map_err(|e| e.to_string())
                .and_then(|heading| upsert_heading(conn, &heading)),
            "tracked_app" => serde_json::from_value::<TrackedApp>(value)
                .map_err(|e| e.to_string())
                .and_then(|tracked_app| upsert_tracked_app(conn, &tracked_app)),
            "usage_session" => serde_json::from_value::<UsageSession>(value)
                .map_err(|e| e.to_string())
                .and_then(|session| upsert_usage_session(conn, &session)),
            "usage_event" => serde_json::from_value::<UsageEvent>(value)
                .map_err(|e| e.to_string())
                .and_then(|event| upsert_usage_event(conn, &event)),
            "object_type" => serde_json::from_value::<ObjectType>(value)
                .map_err(|e| e.to_string())
                .and_then(|object_type| upsert_object_type(conn, &object_type)),
            "object" => (|| -> Result<(), String> {
                let object = serde_json::from_value::<ArkObject>(value.clone())
                    .map_err(|e| e.to_string())?;
                // Phase 2: hold-and-replay для schema drift. Если type_id неизвестен,
                // кладём payload в sync_pending_objects, эмитим sync_error, treat as applied
                // (version_vector advances). При появлении object_type — replay в upsert_object_type.
                if !is_object_type_known(conn, &object.type_id)? {
                    insert_pending_object(conn, entity, &object.type_id)?;
                    crate::events::emit_event(json!({
                        "event": "sync_error",
                        "code": "unknown_type_id",
                        "entity_type": "object",
                        "entity_id": entity.id,
                        "awaited_type_id": object.type_id,
                    }));
                    return Ok(());
                }
                upsert_object(conn, &object)
            })(),
            "object_link" => serde_json::from_value::<ObjectLink>(value)
                .map_err(|e| e.to_string())
                .and_then(|link| upsert_object_link(conn, &link)),
            _ => Err(format!("unknown sync entity type '{}'", entity.entity_type)),
        };
        result?;
        delete_sync_tombstone(conn, &entity.id)
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
        assert!(object_types.iter().any(|item| item.id == "note_obj"));
        assert!(object_types.iter().any(|item| item.id == "game_obj"));
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
        assert_eq!(snapshot.top_apps[0].foreground_ms, 1_200);
        assert_eq!(snapshot.recent_sessions[0].id, "session-analytics");
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
        let object_type = make_object_type("book_obj", "Книга");
        upsert_object_type(&conn, &object_type).unwrap();

        let note = make_object("obj-1", "note_obj", "Первая заметка");
        let book = make_object("obj-2", "book_obj", "Clean Code");
        upsert_object(&conn, &note).unwrap();
        upsert_object(&conn, &book).unwrap();

        let link = make_object_link("link-1", "obj-1", "obj-2");
        upsert_object_link(&conn, &link).unwrap();

        let data = load_all(&conn).unwrap();
        assert!(data.object_types.iter().any(|item| item.id == "note_obj"));
        assert!(data.object_types.iter().any(|item| item.id == "game_obj"));
        assert!(data.object_types.iter().any(|item| item.id == "book_obj"));
        assert_eq!(data.objects.len(), 2);
        assert_eq!(data.object_links.len(), 1);
        assert_eq!(data.object_links[0].source_object_id, "obj-1");

        delete_object_link(&conn, "link-1").unwrap();
        delete_object(&conn, "obj-2").unwrap();
        delete_object_type(&conn, "book_obj").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.object_links.len(), 0);
        assert_eq!(data.objects.len(), 1);
        assert!(!data.object_types.iter().any(|item| item.id == "book_obj"));
    }

    #[test]
    fn object_query_helpers_filter_by_type_and_ids() {
        let conn = setup_db();
        let object_type = make_object_type("book_obj", "Book");
        upsert_object_type(&conn, &object_type).unwrap();
        upsert_object(&conn, &make_object("obj-1", "note_obj", "Journal")).unwrap();
        upsert_object(&conn, &make_object("obj-2", "book_obj", "Clean Code")).unwrap();
        upsert_object(&conn, &make_object("obj-3", "book_obj", "Rust Book")).unwrap();

        let books = list_objects_by_type(&conn, "book_obj").unwrap();
        assert_eq!(books.len(), 2);
        assert!(books.iter().all(|object| object.type_id == "book_obj"));

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
        assert!(data.object_types.iter().any(|item| item.id == "note_obj"));
        assert!(data.object_types.iter().any(|item| item.id == "game_obj"));
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
        backend.set_device_id("device-under-test");
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
        assert_eq!(found.entity_type, "todo");
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
        };
        backend.apply_entity(&tombstone).await.unwrap();

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        let found = loaded
            .iter()
            .find(|e| e.id == "tbk2")
            .expect("deleted entity should be emitted as a tombstone");
        assert_eq!(found.entity_type, "todo");
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
        let first = bump_sync_version_vector(&conn, "obj-local", "device-local").unwrap();
        let second = bump_sync_version_vector(&conn, "obj-local", "device-local").unwrap();

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
        }
    }

    #[test]
    fn phase2_is_object_type_known_false_when_missing() {
        let conn = setup_db();
        assert_eq!(is_object_type_known(&conn, "nonexistent").unwrap(), false);
    }

    #[test]
    fn phase2_is_object_type_known_true_after_upsert() {
        let conn = setup_db();
        let object_type = phase2_make_object_type("type_x", "Type X");
        upsert_object_type(&conn, &object_type).unwrap();
        assert_eq!(is_object_type_known(&conn, "type_x").unwrap(), true);
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
            let object_type = make_object_type("note_obj", "Note");
            upsert_object_type(&conn, &object_type).unwrap();
            for i in 0..100 {
                let mut obj = make_object(&format!("obj-{i}"), "note_obj", &format!("Title {i}"));
                obj.props_json = json!({ "n": i, "padding": "x".repeat(200) });
                upsert_object(&conn, &obj).unwrap();
            }
        }
        // Портим data pages (offset 8192+, после header page и schema page) —
        // SQLite header останется валидным, integrity_check обнаружит
        // повреждённые btree pages.
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap();
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
        let object_type = make_object_type("note_obj", "Note");
        upsert_object_type(&conn, &object_type).unwrap();
        let mut object = make_object("obj-backup", "note_obj", "Backup test");
        object.props_json = json!({"tag": "test"});
        upsert_object(&conn, &object).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("ark.db.backup");
        let dest_str = dest.to_str().unwrap();
        backup_to_file(&conn, dest_str).expect("backup должен пройти");

        assert!(dest.exists(), "файл backup'а должен существовать");
        let backup_conn = open_db(dest_str).unwrap();
        let objects = list_objects(&backup_conn).unwrap();
        assert_eq!(objects.len(), 1, "backup должен содержать оригинальный объект");
        assert_eq!(objects[0].id, "obj-backup");
        assert_eq!(objects[0].title, "Backup test");
    }
}
