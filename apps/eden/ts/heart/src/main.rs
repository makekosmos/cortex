use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value as TantivyValue, STORED,
    STRING,
};
use tantivy::tokenizer::{
    LowerCaser, NgramTokenizer, RemoveLongFilter, SimpleTokenizer, TextAnalyzer,
};
use tantivy::{doc, Index};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    id: String,
    title: String,
    content_json: String,
    created_at: i64,
    updated_at: i64,
    folder_id: Option<String>,
    type_id: Option<String>,
    header_layout: Option<String>,
    header_props_json: String,
    schema_version: i64,
    deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Folder {
    id: String,
    name: String,
    created_at: i64,
    parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NoteType {
    id: String,
    name: String,
    slug: String,
    icon: Option<String>,
    color: Option<String>,
    schema_json: String,
    header_template_json: String,
    ui_schema_json: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
struct SearchResult {
    file: String,
    line: usize,
    text: String,
    #[serde(rename = "entryId")]
    entry_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum Request {
    InitStore {
        #[serde(rename = "vaultPath")]
        vault_path: String,
    },
    LoadEntry {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        id: String,
    },
    ListEntries {
        #[serde(rename = "vaultPath")]
        vault_path: String,
    },
    SaveEntry {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        entry: Entry,
    },
    ExportMarkdownVault {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        #[serde(rename = "outputDir")]
        output_dir: String,
    },
    SearchEntries {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        query: String,
    },
    CreateFolder {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        id: String,
        name: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
    },
    ListFolders {
        #[serde(rename = "vaultPath")]
        vault_path: String,
    },
    ListNoteTypes {
        #[serde(rename = "vaultPath")]
        vault_path: String,
    },
    GetNoteTypeById {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        #[serde(rename = "noteTypeId")]
        note_type_id: String,
    },
    SaveNoteType {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        note_type: NoteType,
    },
    DeleteNoteType {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        #[serde(rename = "noteTypeId")]
        note_type_id: String,
    },
    MoveEntryToFolder {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        #[serde(rename = "entryId")]
        entry_id: String,
        #[serde(rename = "folderId")]
        folder_id: Option<String>,
    },
    DeleteEntry {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        #[serde(rename = "entryId")]
        entry_id: String,
    },
    DeleteFolder {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        #[serde(rename = "folderId")]
        folder_id: String,
    },
    MoveFolderToFolder {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        #[serde(rename = "folderId")]
        folder_id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
    },
    ListTrashEntries {
        #[serde(rename = "vaultPath")]
        vault_path: String,
    },
    RestoreEntry {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        #[serde(rename = "entryId")]
        entry_id: String,
    },
    PermanentDeleteEntry {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        #[serde(rename = "entryId")]
        entry_id: String,
    },
    PurgeExpiredTrash {
        #[serde(rename = "vaultPath")]
        vault_path: String,
        #[serde(rename = "maxAgeMs")]
        max_age_ms: i64,
    },
    GetVaultStorageInfo {
        #[serde(rename = "vaultPath")]
        vault_path: String,
    },
}

#[derive(Debug, Clone)]
struct StorePaths {
    db_path: PathBuf,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(|| "Missing command".to_string())?;

    match command.as_str() {
        "search" => Err("Legacy `search` command is removed. Use `request` mode.".to_string()),
        "request" => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .map_err(|error| error.to_string())?;
            let request =
                serde_json::from_str::<Request>(&input).map_err(|error| error.to_string())?;
            let response = handle_request(request)?;
            print_json(&response)
        }
        "serve" => serve_requests(),
        _ => Err(format!("Unsupported command: {command}")),
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    let json = serde_json::to_string(value).map_err(|error| error.to_string())?;
    std::io::stdout()
        .write_all(json.as_bytes())
        .map_err(|error| error.to_string())
}

fn print_json_line<T: Serialize>(value: &T) -> Result<(), String> {
    let json = serde_json::to_string(value).map_err(|error| error.to_string())?;
    std::io::stdout()
        .write_all(format!("{json}\n").as_bytes())
        .map_err(|error| error.to_string())?;
    std::io::stdout().flush().map_err(|error| error.to_string())
}

fn serve_requests() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;

        if bytes_read == 0 {
            return Ok(());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(trimmed) {
            Ok(request) => match handle_request(request) {
                Ok(data) => json!({
                    "ok": true,
                    "data": data,
                }),
                Err(error) => json!({
                    "ok": false,
                    "error": error,
                }),
            },
            Err(error) => json!({
                "ok": false,
                "error": error.to_string(),
            }),
        };

        print_json_line(&response)?;
    }
}

fn handle_request(request: Request) -> Result<Value, String> {
    match request {
        Request::InitStore { vault_path } => {
            let paths = store_paths(&vault_path);
            let _connection = open_store(&paths)?;
            Ok(json!(true))
        }
        Request::LoadEntry { vault_path, id } => {
            let paths = store_paths(&vault_path);
            let connection = open_store(&paths)?;
            Ok(json!(load_entry(&connection, &id)?))
        }
        Request::ListEntries { vault_path } => {
            let paths = store_paths(&vault_path);
            let connection = open_store(&paths)?;
            Ok(json!(list_entries(&connection)?))
        }
        Request::SaveEntry { vault_path, entry } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            save_entry(&mut connection, entry)
        }
        Request::ExportMarkdownVault {
            vault_path,
            output_dir,
        } => {
            let paths = store_paths(&vault_path);
            let connection = open_store(&paths)?;
            export_markdown_vault(&connection, Path::new(&output_dir))
        }
        Request::SearchEntries { vault_path, query } => {
            let paths = store_paths(&vault_path);
            let connection = open_store(&paths)?;
            Ok(json!(search_entries(&connection, &query)?))
        }
        Request::CreateFolder {
            vault_path,
            id,
            name,
            parent_id,
        } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            create_folder(&mut connection, &id, &name, parent_id.as_deref())
        }
        Request::ListFolders { vault_path } => {
            let paths = store_paths(&vault_path);
            let connection = open_store(&paths)?;
            Ok(json!(list_folders(&connection)?))
        }
        Request::ListNoteTypes { vault_path } => {
            let paths = store_paths(&vault_path);
            let connection = open_store(&paths)?;
            Ok(json!(list_note_types(&connection)?))
        }
        Request::GetNoteTypeById {
            vault_path,
            note_type_id,
        } => {
            let paths = store_paths(&vault_path);
            let connection = open_store(&paths)?;
            Ok(json!(get_note_type_by_id(&connection, &note_type_id)?))
        }
        Request::SaveNoteType {
            vault_path,
            note_type,
        } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            save_note_type(&mut connection, note_type)
        }
        Request::DeleteNoteType {
            vault_path,
            note_type_id,
        } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            delete_note_type(&mut connection, &note_type_id)
        }
        Request::MoveEntryToFolder {
            vault_path,
            entry_id,
            folder_id,
        } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            move_entry_to_folder(&mut connection, &entry_id, folder_id.as_deref())
        }
        Request::DeleteEntry {
            vault_path,
            entry_id,
        } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            delete_entry(&mut connection, &entry_id)
        }
        Request::DeleteFolder {
            vault_path,
            folder_id,
        } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            delete_folder(&mut connection, &folder_id)
        }
        Request::MoveFolderToFolder {
            vault_path,
            folder_id,
            parent_id,
        } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            move_folder_to_folder(&mut connection, &folder_id, parent_id.as_deref())
        }
        Request::ListTrashEntries { vault_path } => {
            let paths = store_paths(&vault_path);
            let connection = open_store(&paths)?;
            Ok(json!(list_trash_entries(&connection)?))
        }
        Request::RestoreEntry {
            vault_path,
            entry_id,
        } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            restore_entry(&mut connection, &entry_id)
        }
        Request::PermanentDeleteEntry {
            vault_path,
            entry_id,
        } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            permanent_delete_entry(&mut connection, &entry_id)
        }
        Request::PurgeExpiredTrash {
            vault_path,
            max_age_ms,
        } => {
            let paths = store_paths(&vault_path);
            let mut connection = open_store(&paths)?;
            purge_expired_trash(&mut connection, max_age_ms)
        }
        Request::GetVaultStorageInfo { vault_path } => {
            let paths = store_paths(&vault_path);
            let connection = open_store(&paths)?;
            get_vault_storage_info(&connection, &vault_path)
        }
    }
}

fn store_paths(vault_path: &str) -> StorePaths {
    let vault_path = PathBuf::from(vault_path);
    StorePaths {
        db_path: vault_path.join("eden.db"),
    }
}

fn open_store(paths: &StorePaths) -> Result<Connection, String> {
    if let Some(parent_dir) = paths.db_path.parent() {
        fs::create_dir_all(parent_dir).map_err(|error| error.to_string())?;
    }
    let connection = Connection::open(&paths.db_path).map_err(|error| error.to_string())?;
    apply_pragmas(&connection)?;
    bootstrap_schema(&connection)?;
    Ok(connection)
}

fn apply_pragmas(connection: &Connection) -> Result<(), String> {
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|error| error.to_string())?;
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(|error| error.to_string())?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| error.to_string())?;
    connection
        .pragma_update(None, "busy_timeout", "5000")
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn bootstrap_schema(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS folders (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              parent_id TEXT,
              FOREIGN KEY(parent_id) REFERENCES folders(id)
            );

            CREATE TABLE IF NOT EXISTS entries (
              id TEXT PRIMARY KEY,
              title TEXT,
              content_json TEXT,
              created_at INTEGER,
              updated_at INTEGER,
              folder_id TEXT,
              type_id TEXT,
              header_layout TEXT,
              header_props_json TEXT DEFAULT '{}',
              schema_version INTEGER DEFAULT 1,
              FOREIGN KEY(folder_id) REFERENCES folders(id)
            );

            CREATE TABLE IF NOT EXISTS note_types (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              slug TEXT NOT NULL UNIQUE,
              icon TEXT,
              color TEXT,
              schema_json TEXT NOT NULL,
              header_template_json TEXT NOT NULL,
              ui_schema_json TEXT,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL
            );
            "#,
        )
        .map_err(|error| error.to_string())?;

    add_column_if_missing(
        connection,
        "note_types",
        "ui_schema_json",
        "ALTER TABLE note_types ADD COLUMN ui_schema_json TEXT",
    )?;
    add_column_if_missing(
        connection,
        "entries",
        "folder_id",
        "ALTER TABLE entries ADD COLUMN folder_id TEXT REFERENCES folders(id)",
    )?;
    add_column_if_missing(
        connection,
        "entries",
        "type_id",
        "ALTER TABLE entries ADD COLUMN type_id TEXT",
    )?;
    add_column_if_missing(
        connection,
        "entries",
        "header_layout",
        "ALTER TABLE entries ADD COLUMN header_layout TEXT",
    )?;
    add_column_if_missing(
        connection,
        "entries",
        "header_props_json",
        "ALTER TABLE entries ADD COLUMN header_props_json TEXT DEFAULT '{}'",
    )?;
    add_column_if_missing(
        connection,
        "entries",
        "schema_version",
        "ALTER TABLE entries ADD COLUMN schema_version INTEGER DEFAULT 1",
    )?;
    add_column_if_missing(
        connection,
        "entries",
        "deleted_at",
        "ALTER TABLE entries ADD COLUMN deleted_at INTEGER",
    )?;
    add_column_if_missing(
        connection,
        "folders",
        "parent_id",
        "ALTER TABLE folders ADD COLUMN parent_id TEXT REFERENCES folders(id)",
    )?;

    // Indexes for common query patterns
    connection
        .execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_entries_deleted_at ON entries(deleted_at);
            CREATE INDEX IF NOT EXISTS idx_entries_folder_id ON entries(folder_id);
            CREATE INDEX IF NOT EXISTS idx_entries_type_id ON entries(type_id);
            CREATE INDEX IF NOT EXISTS idx_entries_updated_at ON entries(updated_at DESC) WHERE deleted_at IS NULL;
            CREATE INDEX IF NOT EXISTS idx_entries_title_folder ON entries(title, folder_id) WHERE deleted_at IS NULL;
            CREATE INDEX IF NOT EXISTS idx_folders_parent_id ON folders(parent_id);
            "#,
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn add_column_if_missing(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    alter_sql: &str,
) -> Result<(), String> {
    if column_exists(connection, table_name, column_name)? {
        return Ok(());
    }

    connection
        .execute(alter_sql, [])
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn column_exists(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table_name})"))
        .map_err(|error| error.to_string())?;
    let mut rows = statement.query([]).map_err(|error| error.to_string())?;

    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let name: String = row.get(1).map_err(|error| error.to_string())?;
        if name == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

fn normalize_entry(entry: Entry) -> Entry {
    Entry {
        type_id: entry.type_id,
        header_layout: entry.header_layout,
        header_props_json: if entry.header_props_json.is_empty() {
            "{}".to_string()
        } else {
            entry.header_props_json
        },
        schema_version: if entry.schema_version == 0 {
            1
        } else {
            entry.schema_version
        },
        deleted_at: entry.deleted_at,
        ..entry
    }
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> Result<Entry, rusqlite::Error> {
    Ok(normalize_entry(Entry {
        id: row.get(0)?,
        title: row.get(1)?,
        content_json: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        folder_id: row.get(5)?,
        type_id: row.get(6)?,
        header_layout: row.get(7)?,
        header_props_json: row
            .get::<_, Option<String>>(8)?
            .unwrap_or_else(|| "{}".to_string()),
        schema_version: row.get::<_, Option<i64>>(9)?.unwrap_or(1),
        deleted_at: row.get(10)?,
    }))
}

fn row_to_folder(row: &rusqlite::Row<'_>) -> Result<Folder, rusqlite::Error> {
    Ok(Folder {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at: row.get(2)?,
        parent_id: row.get(3)?,
    })
}

fn row_to_note_type(row: &rusqlite::Row<'_>) -> Result<NoteType, rusqlite::Error> {
    Ok(NoteType {
        id: row.get(0)?,
        name: row.get(1)?,
        slug: row.get(2)?,
        icon: row.get(3)?,
        color: row.get(4)?,
        schema_json: row.get(5)?,
        header_template_json: row.get(6)?,
        ui_schema_json: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn load_entry(connection: &Connection, id: &str) -> Result<Option<Entry>, String> {
    connection
        .query_row(
            "SELECT id, title, content_json, created_at, updated_at, folder_id, type_id, header_layout, header_props_json, schema_version, deleted_at FROM entries WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            row_to_entry,
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn list_entries(connection: &Connection) -> Result<Vec<Entry>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, title, content_json, created_at, updated_at, folder_id, type_id, header_layout, header_props_json, schema_version, deleted_at FROM entries WHERE deleted_at IS NULL ORDER BY updated_at DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], row_to_entry)
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn list_folders(connection: &Connection) -> Result<Vec<Folder>, String> {
    let mut statement = connection
        .prepare("SELECT id, name, created_at, parent_id FROM folders ORDER BY name ASC")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], row_to_folder)
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn get_folder(connection: &Connection, folder_id: &str) -> Result<Option<Folder>, String> {
    connection
        .query_row(
            "SELECT id, name, created_at, parent_id FROM folders WHERE id = ?1 LIMIT 1",
            [folder_id],
            row_to_folder,
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn get_note_type_by_id(
    connection: &Connection,
    note_type_id: &str,
) -> Result<Option<NoteType>, String> {
    connection
        .query_row(
            "SELECT id, name, slug, icon, color, schema_json, header_template_json, ui_schema_json, created_at, updated_at FROM note_types WHERE id = ?1 LIMIT 1",
            [note_type_id],
            row_to_note_type,
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn list_note_types(connection: &Connection) -> Result<Vec<NoteType>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, slug, icon, color, schema_json, header_template_json, ui_schema_json, created_at, updated_at FROM note_types ORDER BY name ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], row_to_note_type)
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn ensure_dir(dir_path: &Path) -> Result<(), String> {
    fs::create_dir_all(dir_path).map_err(|error| error.to_string())
}

fn duplicate_entry(
    connection: &Connection,
    entry_id: &str,
    title: &str,
    folder_id: Option<&str>,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            r#"
            SELECT id
            FROM entries
            WHERE id != ?1
              AND title = ?2
              AND ((?3 IS NULL AND folder_id IS NULL) OR folder_id = ?3)
              AND deleted_at IS NULL
            LIMIT 1
            "#,
            params![entry_id, title, folder_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn save_entry(connection: &mut Connection, entry: Entry) -> Result<Value, String> {
    let entry = normalize_entry(entry);

    let tx = connection
        .transaction()
        .map_err(|error| error.to_string())?;

    if let Some(conflicting_entry_id) = duplicate_entry(
        &tx,
        &entry.id,
        &entry.title,
        entry.folder_id.as_deref(),
    )? {
        return Ok(json!({
            "ok": false,
            "reason": "duplicate_title",
            "conflictingEntryId": conflicting_entry_id,
            "title": entry.title,
            "folder_id": entry.folder_id,
        }));
    }

    tx.execute(
        r#"
        INSERT INTO entries (id, title, content_json, created_at, updated_at, folder_id, type_id, header_layout, header_props_json, schema_version)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(id) DO UPDATE SET
          title = excluded.title,
          content_json = excluded.content_json,
          updated_at = excluded.updated_at,
          folder_id = excluded.folder_id,
          type_id = excluded.type_id,
          header_layout = excluded.header_layout,
          header_props_json = excluded.header_props_json,
          schema_version = excluded.schema_version
        "#,
        params![
            entry.id,
            entry.title,
            entry.content_json,
            entry.created_at,
            entry.updated_at,
            entry.folder_id,
            entry.type_id,
            entry.header_layout,
            entry.header_props_json,
            entry.schema_version,
        ],
    )
    .map_err(|error| error.to_string())?;

    tx.commit().map_err(|error| error.to_string())?;

    Ok(json!({
        "ok": true,
        "entryId": entry.id,
    }))
}

fn has_folder_name_conflict(
    connection: &Connection,
    name: &str,
    parent_id: Option<&str>,
    exclude_id: Option<&str>,
) -> Result<bool, String> {
    let conflict = connection
        .query_row(
            r#"
            SELECT id FROM folders
            WHERE LOWER(name) = LOWER(?1)
              AND (?2 IS NULL OR id != ?2)
              AND ((?3 IS NULL AND parent_id IS NULL) OR parent_id = ?3)
            LIMIT 1
            "#,
            params![name, exclude_id, parent_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    Ok(conflict.is_some())
}

fn create_folder(
    connection: &mut Connection,
    id: &str,
    name: &str,
    parent_id: Option<&str>,
) -> Result<Value, String> {
    let normalized_name = name.trim();
    if has_folder_name_conflict(connection, normalized_name, parent_id, None)? {
        return Ok(json!({
            "ok": false,
            "reason": "duplicate_folder_name",
            "name": normalized_name,
        }));
    }

    let folder = Folder {
        id: id.to_string(),
        name: normalized_name.to_string(),
        created_at: now_ms(),
        parent_id: parent_id.map(ToOwned::to_owned),
    };

    connection
        .execute(
            "INSERT INTO folders (id, name, created_at, parent_id) VALUES (?1, ?2, ?3, ?4)",
            params![folder.id, folder.name, folder.created_at, folder.parent_id],
        )
        .map_err(|error| error.to_string())?;

    Ok(json!({
        "ok": true,
        "folder": folder,
    }))
}

fn save_note_type(connection: &mut Connection, note_type: NoteType) -> Result<Value, String> {
    let duplicate = connection
        .query_row(
            "SELECT id FROM note_types WHERE slug = ?1 AND id != ?2 LIMIT 1",
            params![note_type.slug, note_type.id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    if duplicate.is_some() {
        return Ok(json!({
            "ok": false,
            "reason": "duplicate_slug",
            "message": "Тип с таким идентификатором уже существует",
        }));
    }

    connection
        .execute(
            r#"
            INSERT INTO note_types (id, name, slug, icon, color, schema_json, header_template_json, ui_schema_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
              name = excluded.name,
              slug = excluded.slug,
              icon = excluded.icon,
              color = excluded.color,
              schema_json = excluded.schema_json,
              header_template_json = excluded.header_template_json,
              ui_schema_json = excluded.ui_schema_json,
              updated_at = excluded.updated_at
            "#,
            params![
                note_type.id,
                note_type.name,
                note_type.slug,
                note_type.icon,
                note_type.color,
                note_type.schema_json,
                note_type.header_template_json,
                note_type.ui_schema_json,
                note_type.created_at,
                note_type.updated_at,
            ],
        )
        .map_err(|error| error.to_string())?;

    Ok(json!({
        "ok": true,
        "noteType": note_type,
    }))
}

fn delete_note_type(connection: &mut Connection, note_type_id: &str) -> Result<Value, String> {
    connection
        .execute("DELETE FROM note_types WHERE id = ?1", [note_type_id])
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "UPDATE entries SET type_id = NULL, header_layout = NULL, header_props_json = '{}', schema_version = 1 WHERE type_id = ?1",
            [note_type_id],
        )
        .map_err(|error| error.to_string())?;
    Ok(json!(true))
}

fn move_entry_to_folder(
    connection: &mut Connection,
    entry_id: &str,
    folder_id: Option<&str>,
) -> Result<Value, String> {
    let entry = match load_entry(connection, entry_id)? {
        Some(entry) => entry,
        None => {
            return Ok(json!({
                "ok": false,
                "reason": "entry_not_found",
                "message": "Entry not found",
            }));
        }
    };

    if let Some(folder_id) = folder_id {
        if get_folder(connection, folder_id)?.is_none() {
            return Ok(json!({
                "ok": false,
                "reason": "folder_not_found",
                "message": "Target folder not found",
            }));
        }
    }

    if duplicate_entry(connection, entry_id, &entry.title, folder_id)?.is_some() {
        return Ok(json!({
            "ok": false,
            "reason": "duplicate_title",
            "message": "Cannot move entry: duplicate title in target folder",
        }));
    }

    let result = connection.execute(
        "UPDATE entries SET folder_id = ?1 WHERE id = ?2",
        params![folder_id, entry_id],
    );

    if let Err(error) = result {
        return Err(error.to_string());
    }

    Ok(json!({
        "ok": true,
        "entryId": entry_id,
        "folder_id": folder_id,
    }))
}

fn delete_entry(connection: &mut Connection, entry_id: &str) -> Result<Value, String> {
    match load_entry(connection, entry_id)? {
        Some(_) => {}
        None => {
            return Ok(json!({
                "ok": false,
                "reason": "entry_not_found",
                "message": "Заметка не найдена",
            }));
        }
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64;
    connection
        .execute(
            "UPDATE entries SET deleted_at = ?2 WHERE id = ?1",
            rusqlite::params![entry_id, now],
        )
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "ok": true,
        "entryId": entry_id,
    }))
}

fn list_trash_entries(connection: &Connection) -> Result<Vec<Entry>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, title, content_json, created_at, updated_at, folder_id, type_id, header_layout, header_props_json, schema_version, deleted_at FROM entries WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], row_to_entry)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn restore_entry(connection: &mut Connection, entry_id: &str) -> Result<Value, String> {
    let affected = connection
        .execute(
            "UPDATE entries SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL",
            [entry_id],
        )
        .map_err(|e| e.to_string())?;
    if affected == 0 {
        return Ok(
            json!({ "ok": false, "reason": "entry_not_found", "message": "Заметка не найдена в корзине" }),
        );
    }
    Ok(json!({ "ok": true, "entryId": entry_id }))
}

fn permanent_delete_entry(connection: &mut Connection, entry_id: &str) -> Result<Value, String> {
    let affected = connection
        .execute(
            "DELETE FROM entries WHERE id = ?1 AND deleted_at IS NOT NULL",
            [entry_id],
        )
        .map_err(|e| e.to_string())?;
    if affected == 0 {
        return Ok(
            json!({ "ok": false, "reason": "entry_not_found", "message": "Заметка не найдена в корзине" }),
        );
    }
    Ok(json!({ "ok": true, "entryId": entry_id }))
}

fn purge_expired_trash(connection: &mut Connection, max_age_ms: i64) -> Result<Value, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64;
    let cutoff = now - max_age_ms;
    let purged = connection
        .execute(
            "DELETE FROM entries WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
            [cutoff],
        )
        .map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true, "purgedCount": purged }))
}

fn get_vault_storage_info(connection: &Connection, vault_path: &str) -> Result<Value, String> {
    let (text_bytes, trash_bytes, entry_count, trash_count) = connection
        .query_row(
            r#"
            SELECT
                COALESCE(SUM(CASE WHEN deleted_at IS NULL THEN LENGTH(content_json) END), 0),
                COALESCE(SUM(CASE WHEN deleted_at IS NOT NULL THEN LENGTH(content_json) END), 0),
                COALESCE(SUM(CASE WHEN deleted_at IS NULL THEN 1 END), 0),
                COALESCE(SUM(CASE WHEN deleted_at IS NOT NULL THEN 1 END), 0)
            FROM entries
            "#,
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?)),
        )
        .map_err(|e| e.to_string())?;

    let db_path = Path::new(vault_path).join("eden.db");
    let db_size = std::fs::metadata(&db_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    let vault_dir_size = dir_size(Path::new(vault_path));

    Ok(json!({
        "textBytes": text_bytes,
        "trashBytes": trash_bytes,
        "dbBytes": db_size,
        "vaultBytes": vault_dir_size,
        "entryCount": entry_count,
        "trashCount": trash_count,
    }))
}

fn dir_size(path: &Path) -> i64 {
    let mut total: i64 = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_file() {
                total += meta.len() as i64;
            } else if meta.is_dir() {
                total += dir_size(&entry.path());
            }
        }
    }
    total
}

fn is_folder_descendant(
    connection: &Connection,
    folder_id: &str,
    possible_parent_id: &str,
) -> Result<bool, String> {
    let mut current_folder = get_folder(connection, possible_parent_id)?;

    while let Some(folder) = current_folder {
        if folder.id == folder_id {
            return Ok(true);
        }

        current_folder = match folder.parent_id {
            Some(parent_id) => get_folder(connection, &parent_id)?,
            None => None,
        };
    }

    Ok(false)
}

fn move_folder_to_folder(
    connection: &mut Connection,
    folder_id: &str,
    parent_id: Option<&str>,
) -> Result<Value, String> {
    let folder = match get_folder(connection, folder_id)? {
        Some(folder) => folder,
        None => {
            return Ok(json!({
                "ok": false,
                "reason": "folder_not_found",
                "message": "Папка не найдена",
            }));
        }
    };

    if parent_id == Some(folder_id)
        || match parent_id {
            Some(parent_id) => is_folder_descendant(connection, folder_id, parent_id)?,
            None => false,
        }
    {
        return Ok(json!({
            "ok": false,
            "reason": "invalid_target",
            "message": "Нельзя переместить папку внутрь самой себя",
        }));
    }

    if let Some(parent_id) = parent_id {
        if get_folder(connection, parent_id)?.is_none() {
            return Ok(json!({
                "ok": false,
                "reason": "invalid_target",
                "message": "Целевая папка не найдена",
            }));
        }
    }

    if folder.parent_id.as_deref() == parent_id {
        return Ok(json!({
            "ok": true,
            "folderId": folder_id,
            "parent_id": parent_id,
        }));
    }

    if has_folder_name_conflict(connection, &folder.name, parent_id, Some(folder_id))? {
        return Ok(json!({
            "ok": false,
            "reason": "duplicate_folder_name",
            "message": "Папка с таким названием уже есть на этом уровне",
        }));
    }

    let result = connection.execute(
        "UPDATE folders SET parent_id = ?1 WHERE id = ?2",
        params![parent_id, folder_id],
    );

    if let Err(error) = result {
        return Err(error.to_string());
    }

    Ok(json!({
        "ok": true,
        "folderId": folder_id,
        "parent_id": parent_id,
    }))
}

fn delete_folder(connection: &mut Connection, folder_id: &str) -> Result<Value, String> {
    if get_folder(connection, folder_id)?.is_none() {
        return Ok(json!({
            "ok": false,
            "reason": "folder_not_found",
            "message": "Папка не найдена",
        }));
    }

    let entry_count: i64 = connection
        .query_row(
            "SELECT COUNT(1) FROM entries WHERE folder_id = ?1 AND deleted_at IS NULL",
            [folder_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if entry_count > 0 {
        return Ok(json!({
            "ok": false,
            "reason": "folder_not_empty",
            "message": "Нельзя удалить непустую папку. Сначала удалите или переместите заметки.",
            "entryCount": entry_count,
        }));
    }

    let child_folder_count: i64 = connection
        .query_row(
            "SELECT COUNT(1) FROM folders WHERE parent_id = ?1",
            [folder_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if child_folder_count > 0 {
        return Ok(json!({
            "ok": false,
            "reason": "folder_not_empty",
            "message": "Нельзя удалить папку, пока внутри есть другие папки.",
            "entryCount": child_folder_count,
        }));
    }

    let result = connection.execute("DELETE FROM folders WHERE id = ?1", [folder_id]);
    if let Err(error) = result {
        return Err(error.to_string());
    }

    Ok(json!({
        "ok": true,
        "folderId": folder_id,
    }))
}

fn search_entries(connection: &Connection, query: &str) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    search_entry_bodies(connection, query)
}

fn search_entry_bodies(connection: &Connection, query: &str) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut schema_builder = Schema::builder();
    let entry_id_field = schema_builder.add_text_field("entry_id", STRING | STORED);
    let title_field = schema_builder.add_text_field("title", STORED);
    let body_field = schema_builder.add_text_field("body", STORED);

    let default_indexing = TextFieldIndexing::default()
        .set_tokenizer("eden_default")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let ngram_indexing = TextFieldIndexing::default()
        .set_tokenizer("eden_ngram")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);

    let default_text = TextOptions::default().set_indexing_options(default_indexing.clone());
    let ngram_text = TextOptions::default().set_indexing_options(ngram_indexing.clone());

    let title_terms_field = schema_builder.add_text_field("title_terms", default_text.clone());
    let body_terms_field = schema_builder.add_text_field("body_terms", default_text);
    let title_ngrams_field = schema_builder.add_text_field("title_ngrams", ngram_text.clone());
    let body_ngrams_field = schema_builder.add_text_field("body_ngrams", ngram_text);
    let schema = schema_builder.build();

    let index = Index::create_in_ram(schema.clone());
    index.tokenizers().register(
        "eden_default",
        TextAnalyzer::builder(SimpleTokenizer::default())
            .filter(RemoveLongFilter::limit(80))
            .filter(LowerCaser)
            .filter(tantivy_stemmers::StemmerTokenizer::new(tantivy_stemmers::algorithms::russian))
            .build(),
    );
    index.tokenizers().register(
        "eden_ngram",
        TextAnalyzer::builder(
            NgramTokenizer::new(2, 12, false).map_err(|error| error.to_string())?,
        )
        .filter(LowerCaser)
        .build(),
    );

    let mut writer = index
        .writer(15_000_000)
        .map_err(|error| error.to_string())?;

    let mut statement = connection
        .prepare("SELECT id, title, content_json FROM entries WHERE deleted_at IS NULL")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let content_json: String = row.get(2)?;
            Ok((id, title, content_json))
        })
        .map_err(|error| error.to_string())?;

    for row in rows {
        let (entry_id, title, content_json) = row.map_err(|error| error.to_string())?;
        let body = extract_plain_text_from_content_json(&content_json);

        writer
            .add_document(doc!(
                entry_id_field => entry_id,
                title_field => title.clone(),
                body_field => body.clone(),
                title_terms_field => title.clone(),
                body_terms_field => body.clone(),
                title_ngrams_field => title,
                body_ngrams_field => body,
            ))
            .map_err(|error| error.to_string())?;
    }

    writer.commit().map_err(|error| error.to_string())?;

    let reader = index.reader().map_err(|error| error.to_string())?;
    let searcher = reader.searcher();
    let mut query_parser = QueryParser::for_index(
        &index,
        vec![
            title_terms_field,
            body_terms_field,
            title_ngrams_field,
            body_ngrams_field,
        ],
    );
    query_parser.set_field_boost(title_terms_field, 4.0);
    query_parser.set_field_boost(title_ngrams_field, 2.5);
    query_parser.set_field_boost(body_terms_field, 2.0);
    query_parser.set_field_boost(body_ngrams_field, 1.0);
    query_parser.set_conjunction_by_default();
    let safe_query = build_safe_query(query);
    let parsed_query = query_parser
        .parse_query(&safe_query)
        .map_err(|error| error.to_string())?;
    let top_docs = searcher
        .search(&parsed_query, &TopDocs::with_limit(30))
        .map_err(|error| error.to_string())?;

    let mut results = Vec::new();
    let mut seen = HashSet::new();
    for (_, address) in top_docs {
        let document = searcher
            .doc::<tantivy::schema::document::TantivyDocument>(address)
            .map_err(|error| error.to_string())?;
        let entry_id = document
            .get_first(entry_id_field)
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let title = document
            .get_first(title_field)
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let body = document
            .get_first(body_field)
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();

        let (line, text) = build_search_context(&title, &body, query);
        let result = SearchResult {
            file: String::new(),
            line,
            text,
            entry_id,
        };

        if seen.insert(format!(
            "{}-{}-{}",
            result.entry_id, result.line, result.text
        )) {
            results.push(result);
        }
    }

    Ok(results)
}

fn build_search_context(title: &str, body: &str, query: &str) -> (usize, String) {
    let query_terms = tokenize_search_text(query);

    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if line_matches_query(trimmed, query, &query_terms) {
            return (index + 1, build_snippet(trimmed, query, &query_terms));
        }
    }

    if line_matches_query(title, query, &query_terms) {
        return (
            0,
            format!(
                "Название: {}",
                build_snippet(title.trim(), query, &query_terms)
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

fn line_matches_query(line: &str, query: &str, query_terms: &[String]) -> bool {
    let line_lower = line.to_lowercase();
    if line_lower.contains(&query.to_lowercase()) {
        return true;
    }

    let line_terms = tokenize_search_text(line);
    query_terms.iter().any(|query_term| {
        line_terms
            .iter()
            .any(|line_term| line_term.starts_with(query_term))
    })
}

fn build_snippet(line: &str, query: &str, query_terms: &[String]) -> String {
    let line_lower = line.to_lowercase();
    let direct_match = line_lower.find(&query.to_lowercase());
    let term_match = query_terms.iter().find_map(|term| line_lower.find(term));
    let match_start = direct_match.or(term_match).unwrap_or(0);
    let snippet_radius = 56;

    let start = char_boundary_before(line, match_start.saturating_sub(snippet_radius));
    let end = char_boundary_after(
        line,
        (match_start + query.len() + snippet_radius).min(line.len()),
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

fn build_safe_query(query: &str) -> String {
    let tokens = tokenize_search_text(query);
    if tokens.is_empty() {
        query.trim().to_lowercase()
    } else {
        tokens.join(" ")
    }
}

fn extract_plain_text_from_content_json(content_json: &str) -> String {
    let parsed = match serde_json::from_str::<Value>(content_json) {
        Ok(value) => value,
        Err(_) => return String::new(),
    };

    let mut output = String::new();
    collect_plain_text(&parsed, &mut output);
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
            "hardBreak" => {
                output.push('\n');
            }
            _ => {}
        }
    }

    if let Some(children) = node.get("content").and_then(|value| value.as_array()) {
        for child in children {
            collect_plain_text(child, output);
        }
    }

    if matches!(
        node.get("type").and_then(|value| value.as_str()),
        Some("paragraph" | "heading" | "codeBlock" | "blockquote" | "listItem")
    ) {
        output.push('\n');
    }
}

fn export_markdown_vault(connection: &Connection, output_dir: &Path) -> Result<Value, String> {
    ensure_dir(output_dir)?;

    let entries = list_entries(connection)?;
    for entry in &entries {
        let target_dir = match entry.folder_id.as_deref() {
            Some(folder_id) => {
                output_dir.join(get_folder_export_relative_path(connection, folder_id)?)
            }
            None => output_dir.to_path_buf(),
        };
        ensure_dir(&target_dir)?;

        let file_name = format!("{}.md", sanitize_file_name(&entry.title, &entry.id));
        let file_path = target_dir.join(file_name);
        let markdown = render_entry_markdown(entry);
        fs::write(file_path, markdown).map_err(|error| error.to_string())?;
    }

    Ok(json!({
        "ok": true,
        "exportedCount": entries.len(),
        "outputDir": output_dir.to_string_lossy().to_string(),
    }))
}

fn get_folder_export_relative_path(
    connection: &Connection,
    folder_id: &str,
) -> Result<PathBuf, String> {
    let mut segments = Vec::new();
    let mut current_folder = get_folder(connection, folder_id)?;

    while let Some(folder) = current_folder {
        segments.insert(0, sanitize_folder_segment(&folder.name, &folder.id));
        current_folder = match folder.parent_id {
            Some(parent_id) => get_folder(connection, &parent_id)?,
            None => None,
        };
    }

    let mut relative = PathBuf::new();
    for segment in segments {
        relative.push(segment);
    }

    Ok(relative)
}

fn sanitize_folder_segment(name: &str, fallback_id: &str) -> String {
    let cleaned = name
        .trim()
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect::<String>();
    if cleaned.is_empty() {
        fallback_id.to_string()
    } else {
        cleaned
    }
}

fn sanitize_file_name(title: &str, fallback_id: &str) -> String {
    let sanitized = sanitize_folder_segment(title, fallback_id);
    if sanitized.is_empty() {
        fallback_id.to_string()
    } else {
        sanitized
    }
}

fn render_entry_markdown(entry: &Entry) -> String {
    let title = if entry.title.trim().is_empty() {
        "Без названия".to_string()
    } else {
        entry.title.trim().to_string()
    };
    let body = render_content_json_to_markdown(&entry.content_json);

    if body.trim().is_empty() {
        format!("# {title}\n")
    } else {
        format!("# {title}\n\n{}\n", body.trim())
    }
}

fn render_content_json_to_markdown(content_json: &str) -> String {
    let parsed = match serde_json::from_str::<Value>(content_json) {
        Ok(value) => value,
        Err(_) => return String::new(),
    };

    if let Some(blocks) = parsed.get("content").and_then(|value| value.as_array()) {
        let rendered_blocks = blocks
            .iter()
            .map(render_markdown_block)
            .filter(|block| !block.trim().is_empty())
            .collect::<Vec<_>>();
        return rendered_blocks.join("\n\n");
    }

    String::new()
}

fn render_markdown_block(node: &Value) -> String {
    let node_type = node
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    match node_type {
        "paragraph" => render_markdown_inline(node),
        "heading" => {
            let level = node
                .get("attrs")
                .and_then(|attrs| attrs.get("level"))
                .and_then(|value| value.as_u64())
                .unwrap_or(1)
                .clamp(1, 6) as usize;
            format!(
                "{} {}",
                "#".repeat(level),
                render_markdown_inline(node).trim()
            )
        }
        "codeBlock" => {
            let lang = node
                .get("attrs")
                .and_then(|attrs| attrs.get("language"))
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let code = render_markdown_inline(node);
            if lang.is_empty() {
                format!("```\n{}\n```", code)
            } else {
                format!("```{lang}\n{}\n```", code)
            }
        }
        "blockquote" => render_markdown_inline(node)
            .lines()
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        "bulletList" => render_markdown_list(node, false),
        "orderedList" => render_markdown_list(node, true),
        "horizontalRule" => "---".to_string(),
        _ => render_markdown_inline(node),
    }
}

fn render_markdown_list(node: &Value, ordered: bool) -> String {
    let mut lines = Vec::new();
    if let Some(items) = node.get("content").and_then(|value| value.as_array()) {
        for (index, item) in items.iter().enumerate() {
            let marker = if ordered {
                format!("{}. ", index + 1)
            } else {
                "- ".to_string()
            };
            let content = render_markdown_inline(item).trim().to_string();
            lines.push(format!("{marker}{content}"));
        }
    }
    lines.join("\n")
}

fn render_markdown_inline(node: &Value) -> String {
    let node_type = node
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    if node_type == "text" {
        let mut text = node
            .get("text")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();

        if let Some(marks) = node.get("marks").and_then(|value| value.as_array()) {
            for mark in marks {
                let mark_type = mark
                    .get("type")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                text = match mark_type {
                    "code" => format!("`{text}`"),
                    "bold" => format!("**{text}**"),
                    "italic" => format!("*{text}*"),
                    "strike" => format!("~~{text}~~"),
                    "link" => {
                        let href = mark
                            .get("attrs")
                            .and_then(|attrs| attrs.get("href"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default();
                        if href.is_empty() {
                            text
                        } else {
                            format!("[{text}]({href})")
                        }
                    }
                    _ => text,
                };
            }
        }

        return text;
    }

    if node_type == "hardBreak" {
        return "\n".to_string();
    }

    let mut output = String::new();
    if let Some(children) = node.get("content").and_then(|value| value.as_array()) {
        for child in children {
            output.push_str(&render_markdown_inline(child));
        }
    }

    output
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
