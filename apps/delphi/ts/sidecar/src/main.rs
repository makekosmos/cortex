use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Global connection state
// ---------------------------------------------------------------------------

static DB: Mutex<Option<Connection>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Data structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TodoItem {
    id: String,
    title: String,
    notes: Option<String>,
    priority: i64,
    scheduled_date: Option<String>,
    deadline: Option<String>,
    reminder_date: Option<String>,
    is_today: bool,
    is_evening: bool,
    is_someday: bool,
    is_completed: bool,
    completed_at: Option<String>,
    is_cancelled: bool,
    cancelled_at: Option<String>,
    is_trashed: bool,
    sort_order: i64,
    heading_id: Option<String>,
    project_id: Option<String>,
    area_id: Option<String>,
    tag_ids: Vec<String>,
    checklist_items: Value,
    recurrence_rule: Option<Value>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Project {
    id: String,
    title: String,
    notes: Option<String>,
    status: String,
    scheduled_date: Option<String>,
    deadline: Option<String>,
    sort_order: i64,
    color_tag: Option<String>,
    area_id: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Area {
    id: String,
    title: String,
    sort_order: i64,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tag {
    id: String,
    title: String,
    color: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Heading {
    id: String,
    title: String,
    sort_order: i64,
    project_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadAllData {
    todos: Vec<TodoItem>,
    projects: Vec<Project>,
    areas: Vec<Area>,
    tags: Vec<Tag>,
    headings: Vec<Heading>,
}

// ---------------------------------------------------------------------------
// Request enum
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum Request {
    Init {
        #[serde(rename = "dbPath")]
        db_path: String,
    },
    LoadAll,
    UpsertTodo {
        todo: TodoItem,
    },
    DeleteTodo {
        id: String,
    },
    BatchUpsertTodos {
        todos: Vec<TodoItem>,
    },
    UpsertProject {
        project: Project,
    },
    DeleteProject {
        id: String,
    },
    UpsertArea {
        area: Area,
    },
    UpsertTag {
        tag: Tag,
    },
    UpsertHeading {
        heading: Heading,
    },
    DeleteHeading {
        id: String,
    },
    GetSyncKv {
        key: String,
    },
    SetSyncKv {
        key: String,
        value: String,
    },
    ClearAll,
    DeleteTrashed,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    if let Err(error) = serve() {
        eprintln!("delphi-db fatal: {error}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Serve loop
// ---------------------------------------------------------------------------

fn serve() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;

        if bytes_read == 0 {
            return Ok(());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(trimmed) {
            Ok(request) => match handle_request(request) {
                Ok(data) => json!({ "ok": true, "data": data }),
                Err(error) => json!({ "ok": false, "error": error }),
            },
            Err(error) => json!({ "ok": false, "error": error.to_string() }),
        };

        print_json_line(&response)?;
    }
}

fn print_json_line<T: Serialize>(value: &T) -> Result<(), String> {
    let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
    std::io::stdout()
        .write_all(format!("{json}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Request handler
// ---------------------------------------------------------------------------

fn handle_request(request: Request) -> Result<Value, String> {
    match request {
        Request::Init { db_path } => {
            let conn = open_db(&db_path)?;
            init_schema(&conn)?;
            let mut guard = DB.lock().unwrap();
            *guard = Some(conn);
            Ok(json!(true))
        }

        Request::LoadAll => {
            with_conn(|conn| {
                let todos = load_all_todos(conn)?;
                let projects = load_all_projects(conn)?;
                let areas = load_all_areas(conn)?;
                let tags = load_all_tags(conn)?;
                let headings = load_all_headings(conn)?;
                Ok(json!(LoadAllData { todos, projects, areas, tags, headings }))
            })
        }

        Request::UpsertTodo { todo } => {
            with_conn(|conn| {
                upsert_todo(conn, &todo)?;
                Ok(json!(true))
            })
        }

        Request::DeleteTodo { id } => {
            with_conn(|conn| {
                conn.execute("DELETE FROM todos WHERE id = ?1", params![id])
                    .map_err(|e| e.to_string())?;
                Ok(json!(true))
            })
        }

        Request::BatchUpsertTodos { todos } => {
            with_conn(|conn| {
                batch_upsert_todos(conn, &todos)?;
                Ok(json!(true))
            })
        }

        Request::UpsertProject { project } => {
            with_conn(|conn| {
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
                Ok(json!(true))
            })
        }

        Request::DeleteProject { id } => {
            with_conn(|conn| {
                conn.execute("DELETE FROM projects WHERE id = ?1", params![id])
                    .map_err(|e| e.to_string())?;
                Ok(json!(true))
            })
        }

        Request::UpsertArea { area } => {
            with_conn(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO areas (id, title, sort_order, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![area.id, area.title, area.sort_order, area.created_at],
                )
                .map_err(|e| e.to_string())?;
                Ok(json!(true))
            })
        }

        Request::UpsertTag { tag } => {
            with_conn(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO tags (id, title, color, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![tag.id, tag.title, tag.color, tag.created_at],
                )
                .map_err(|e| e.to_string())?;
                Ok(json!(true))
            })
        }

        Request::UpsertHeading { heading } => {
            with_conn(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO headings (id, title, sort_order, project_id)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![heading.id, heading.title, heading.sort_order, heading.project_id],
                )
                .map_err(|e| e.to_string())?;
                Ok(json!(true))
            })
        }

        Request::DeleteHeading { id } => {
            with_conn(|conn| {
                conn.execute("DELETE FROM headings WHERE id = ?1", params![id])
                    .map_err(|e| e.to_string())?;
                Ok(json!(true))
            })
        }

        Request::GetSyncKv { key } => {
            with_conn(|conn| {
                let value: Option<String> = conn
                    .query_row(
                        "SELECT value FROM sync_kv WHERE key = ?1",
                        params![key],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(|e: rusqlite::Error| e.to_string())?;
                Ok(json!(value))
            })
        }

        Request::SetSyncKv { key, value } => {
            with_conn(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO sync_kv (key, value) VALUES (?1, ?2)",
                    params![key, value],
                )
                .map_err(|e| e.to_string())?;
                Ok(json!(true))
            })
        }

        Request::ClearAll => {
            with_conn(|conn| {
                conn.execute_batch(
                    "DELETE FROM todos; DELETE FROM projects; DELETE FROM areas; DELETE FROM tags; DELETE FROM headings; DELETE FROM sync_kv;"
                ).map_err(|e| e.to_string())?;
                Ok(json!(null))
            })
        }

        Request::DeleteTrashed => {
            with_conn(|conn| {
                let count = conn.execute(
                    "DELETE FROM todos WHERE is_trashed = 1",
                    [],
                ).map_err(|e| e.to_string())?;
                Ok(json!(count))
            })
        }
    }
}

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

fn with_conn<F>(f: F) -> Result<Value, String>
where
    F: FnOnce(&Connection) -> Result<Value, String>,
{
    let guard = DB.lock().unwrap();
    match guard.as_ref() {
        Some(conn) => f(conn),
        None => Err("Database not initialized. Call Init first.".to_string()),
    }
}

fn open_db(path: &str) -> Result<Connection, String> {
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

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS todos (
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

        CREATE TABLE IF NOT EXISTS projects (
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

        CREATE TABLE IF NOT EXISTS areas (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            color TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS headings (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            project_id TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_kv (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Todo upsert
// ---------------------------------------------------------------------------

fn upsert_todo(conn: &Connection, todo: &TodoItem) -> Result<(), String> {
    let tag_ids_json =
        serde_json::to_string(&todo.tag_ids).map_err(|e| e.to_string())?;
    let checklist_json =
        serde_json::to_string(&todo.checklist_items).map_err(|e| e.to_string())?;
    let recurrence_json: Option<String> = todo
        .recurrence_rule
        .as_ref()
        .map(|v| serde_json::to_string(v))
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

fn batch_upsert_todos(conn: &Connection, todos: &[TodoItem]) -> Result<(), String> {
    // rusqlite Connection doesn't impl DerefMut so we use execute inside the loop;
    // we simulate a transaction via execute_batch for BEGIN/COMMIT.
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

// ---------------------------------------------------------------------------
// Load helpers
// ---------------------------------------------------------------------------

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

            let tag_ids: Vec<String> =
                serde_json::from_str(&tag_ids_str).unwrap_or_default();
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

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
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

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
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

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
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

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
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

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
