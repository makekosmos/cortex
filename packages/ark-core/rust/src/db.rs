use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::hlc::HLC;
use crate::schema::CREATE_TABLES;
use crate::sync_server::StorageBackend;
use crate::types::*;

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

pub fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(CREATE_TABLES).map_err(|e| e.to_string())?;
    seed_builtin_object_types(conn)
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

fn seed_builtin_object_types(conn: &Connection) -> Result<(), String> {
    for object_type in [builtin_note_object_type_v2(), builtin_game_object_type_v2()] {
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
    Ok(())
}

pub fn delete_object_type(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM object_types WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
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

pub fn upsert_object(conn: &Connection, object: &ArkObject) -> Result<(), String> {
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
    Ok(())
}

pub fn delete_object(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM objects WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
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
        .query_map([], |row| {
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
        })
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
        |row| {
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
        },
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
    let mut objects = list_objects(conn)?;
    objects.retain(|object| object.deleted_at.is_none());

    let mut matches = Vec::new();

    for object in objects {
        let body = extract_plain_text_from_value(&object.content_json);
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

        let (line, text) = build_search_context(&object.title, &body, &normalized_query, &query_terms);
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
            format!("Название: {}", build_snippet(title.trim(), normalized_query, query_terms)),
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

// ---------------------------------------------------------------------------
// Bulk operations
// ---------------------------------------------------------------------------

pub fn clear_all(conn: &Connection) -> Result<(), String> {
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
        *self.device_id.lock().unwrap() = device_id.to_string();
    }

    pub fn device_id(&self) -> String {
        self.device_id.lock().unwrap().clone()
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

        entities
    }

    fn apply_entity_blocking(conn: &Connection, entity: &SyncEntity) {
        if entity.deleted == Some(true) {
            let _ = match entity.entity_type.as_str() {
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
                _ => Ok(()),
            };
            return;
        }

        let mut full_data = entity.data.clone();
        full_data.insert("id".to_string(), Value::String(entity.id.clone()));
        let value = Value::Object(full_data);

        let _ = match entity.entity_type.as_str() {
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
            "object" => serde_json::from_value::<ArkObject>(value)
                .map_err(|e| e.to_string())
                .and_then(|object| upsert_object(conn, &object)),
            "object_link" => serde_json::from_value::<ObjectLink>(value)
                .map_err(|e| e.to_string())
                .and_then(|link| upsert_object_link(conn, &link)),
            _ => Ok(()),
        };
    }
}

#[async_trait::async_trait]
impl StorageBackend for SqliteStorageBackend {
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity> {
        let conn = self.conn.clone();
        let vector = vector.clone();
        let device_id = self.device_id();
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap();
            Self::collect_entities_blocking(&guard, &vector, &device_id)
        })
        .await
        .unwrap_or_default()
    }

    async fn apply_entity(&self, entity: &SyncEntity) {
        let conn = self.conn.clone();
        let entity = entity.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap();
            Self::apply_entity_blocking(&guard, &entity);
        })
        .await;
    }

    async fn get_kv(&self, key: &str) -> Option<String> {
        let conn = self.conn.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap();
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
            let guard = conn.lock().unwrap();
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
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('todos','projects','areas','tags','headings','tracked_apps','usage_sessions','usage_events','sync_kv')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 9);
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
        backend.apply_entity(&entity).await;

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
    async fn storage_backend_roundtrip_usage_entities() {
        let backend = make_backend();
        backend.apply_entity(&sync_tracked_app("app-sync")).await;
        backend
            .apply_entity(&sync_usage_session("session-sync", "app-sync"))
            .await;
        backend
            .apply_entity(&sync_usage_event(
                "event-sync",
                "app-sync",
                Some("session-sync"),
            ))
            .await;

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
            .await;
        backend
            .apply_entity(&sync_object("obj-sync-b", "game_obj", "Ark game"))
            .await;
        backend
            .apply_entity(&sync_object_link("link-sync", "obj-sync-a", "obj-sync-b"))
            .await;

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
        backend.apply_entity(&entity).await;

        // Now apply a tombstone.
        let tombstone = SyncEntity {
            entity_type: "todo".to_string(),
            id: "tbk2".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-02T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: Some(true),
        };
        backend.apply_entity(&tombstone).await;

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        assert!(
            loaded.iter().all(|e| e.id != "tbk2"),
            "deleted entity should be absent from load_entities",
        );
    }

    #[tokio::test]
    async fn storage_backend_delete_usage_entity_removes_entity() {
        let backend = make_backend();
        backend.apply_entity(&sync_tracked_app("app-del")).await;
        backend
            .apply_entity(&sync_usage_session("session-del", "app-del"))
            .await;
        backend
            .apply_entity(&sync_usage_event(
                "event-del",
                "app-del",
                Some("session-del"),
            ))
            .await;

        let tombstone = SyncEntity {
            entity_type: "usage_event".to_string(),
            id: "event-del".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-02T00:00:00.000Z:000001:peer-a".to_string(),
            deleted: Some(true),
        };
        backend.apply_entity(&tombstone).await;

        let empty_vector: VersionVector = std::collections::HashMap::new();
        let loaded = backend.load_entities(&empty_vector).await;
        assert!(
            loaded.iter().all(|e| e.id != "event-del"),
            "deleted usage event should be absent from load_entities",
        );
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
        backend.apply_entity(&entity).await;

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
}
