use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};
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
    conn.execute_batch(CREATE_TABLES).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Todo CRUD
// ---------------------------------------------------------------------------

pub fn upsert_todo(conn: &Connection, todo: &TodoItem) -> Result<(), String> {
    let tag_ids_json = serde_json::to_string(&todo.tag_ids).map_err(|e| e.to_string())?;
    let checklist_json =
        serde_json::to_string(&todo.checklist_items).map_err(|e| e.to_string())?;
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
        "DELETE FROM todos; DELETE FROM projects; DELETE FROM areas; DELETE FROM tags; DELETE FROM headings; DELETE FROM sync_kv;",
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
    Ok(LoadAllData {
        todos,
        projects,
        areas,
        tags,
        headings,
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
            let checklist_items: Value =
                serde_json::from_str(&checklist_str).unwrap_or(json!([]));
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

    #[test]
    fn test_schema_creation() {
        let conn = setup_db();
        // Verify tables exist
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('todos','projects','areas','tags','headings','sync_kv')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 6);
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
        set_sync_kv(&conn, "k", "v").unwrap();
        clear_all(&conn).unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 0);
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
        assert_eq!(found.data.get("title").and_then(|v| v.as_str()), Some("Roundtrip"));
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
