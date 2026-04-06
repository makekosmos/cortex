use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::sync::Mutex;

use ark_core::db;
use ark_core::types::*;

// ---------------------------------------------------------------------------
// Global connection state
// ---------------------------------------------------------------------------

static DB: Mutex<Option<rusqlite::Connection>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Request enum (camelCase JSON-RPC for Electron sidecar compatibility)
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
        eprintln!("ark-core-rpc fatal: {error}");
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
        let bytes_read = reader.read_line(&mut line).map_err(|e| e.to_string())?;

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
            let conn = db::open_db(&db_path)?;
            db::init_schema(&conn)?;
            let mut guard = DB.lock().unwrap();
            *guard = Some(conn);
            Ok(json!(true))
        }

        Request::LoadAll => with_conn(|conn| {
            let data = db::load_all(conn)?;
            Ok(serde_json::to_value(data).map_err(|e| e.to_string())?)
        }),

        Request::UpsertTodo { todo } => with_conn(|conn| {
            db::upsert_todo(conn, &todo)?;
            Ok(json!(true))
        }),

        Request::DeleteTodo { id } => with_conn(|conn| {
            db::delete_todo(conn, &id)?;
            Ok(json!(true))
        }),

        Request::BatchUpsertTodos { todos } => with_conn(|conn| {
            db::batch_upsert_todos(conn, &todos)?;
            Ok(json!(true))
        }),

        Request::UpsertProject { project } => with_conn(|conn| {
            db::upsert_project(conn, &project)?;
            Ok(json!(true))
        }),

        Request::DeleteProject { id } => with_conn(|conn| {
            db::delete_project(conn, &id)?;
            Ok(json!(true))
        }),

        Request::UpsertArea { area } => with_conn(|conn| {
            db::upsert_area(conn, &area)?;
            Ok(json!(true))
        }),

        Request::UpsertTag { tag } => with_conn(|conn| {
            db::upsert_tag(conn, &tag)?;
            Ok(json!(true))
        }),

        Request::UpsertHeading { heading } => with_conn(|conn| {
            db::upsert_heading(conn, &heading)?;
            Ok(json!(true))
        }),

        Request::DeleteHeading { id } => with_conn(|conn| {
            db::delete_heading(conn, &id)?;
            Ok(json!(true))
        }),

        Request::GetSyncKv { key } => with_conn(|conn| {
            let value = db::get_sync_kv(conn, &key)?;
            Ok(json!(value))
        }),

        Request::SetSyncKv { key, value } => with_conn(|conn| {
            db::set_sync_kv(conn, &key, &value)?;
            Ok(json!(true))
        }),

        Request::ClearAll => with_conn(|conn| {
            db::clear_all(conn)?;
            Ok(json!(null))
        }),

        Request::DeleteTrashed => with_conn(|conn| {
            let count = db::delete_trashed(conn)?;
            Ok(json!(count))
        }),
    }
}

fn with_conn<F>(f: F) -> Result<Value, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<Value, String>,
{
    let guard = DB.lock().unwrap();
    match guard.as_ref() {
        Some(conn) => f(conn),
        None => Err("Database not initialized. Call Init first.".to_string()),
    }
}
