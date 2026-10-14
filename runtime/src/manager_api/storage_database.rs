use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

fn table_group(table: &str) -> (&'static str, String) {
    let (id, label) = if table.starts_with("object_search_fts") {
        ("search_index", "Поиск по содержимому")
    } else if table.starts_with("usage_") || table == "tracked_apps" {
        ("usage_tracker", "История активности")
    } else if matches!(table, "objects" | "todos" | "projects" | "tags") {
        ("objects", "Задачи, проекты и другие записи")
    } else if table.starts_with("object_type") {
        ("types", "Форматы сохранённых записей")
    } else if table.starts_with("sync_") || table == "object_sync_versions" {
        ("sync", "Данные синхронизации")
    } else if table.contains("migration") || table.starts_with("legacy_type") {
        ("migrations", "История обновлений базы")
    } else if matches!(table, "object_links" | "external_refs") {
        ("links", "Связи между записями")
    } else if table == "object_local_state" {
        ("local_state", "Состояние записей на этом компьютере")
    } else if table.starts_with("integration_") || table == "authorized_nodes" {
        ("integrations", "Настройки и разрешения интеграций")
    } else if table == "sqlite_schema" {
        ("schema", "Структура базы данных")
    } else {
        return ("table", format!("Таблица «{table}»"));
    };
    (id, label.into())
}

fn read_detail(db_path: &Path, immutable: bool) -> Result<Value, String> {
    let mut flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY;
    let location = if immutable {
        flags |= rusqlite::OpenFlags::SQLITE_OPEN_URI;
        format!(
            "file:{}?mode=ro&immutable=1",
            db_path
                .to_string_lossy()
                .replace('%', "%25")
                .replace('?', "%3F")
                .replace('#', "%23")
        )
    } else {
        db_path.to_string_lossy().into_owned()
    };
    let mut conn =
        rusqlite::Connection::open_with_flags(location, flags).map_err(|e| e.to_string())?;
    let transaction = conn.transaction().map_err(|e| e.to_string())?;
    let mut stmt = transaction
        .prepare(
            "SELECT COALESCE(m.tbl_name,s.name), m.type = 'index', SUM(s.pgsize)
         FROM dbstat s LEFT JOIN sqlite_schema m ON m.name=s.name
         GROUP BY COALESCE(m.tbl_name,s.name), m.type = 'index'",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<bool>>(1)?.unwrap_or(false),
                row.get::<_, u64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut groups: BTreeMap<String, (String, u64)> = BTreeMap::new();
    let mut record_bytes = [0_u64; 2];
    for (table, is_index, bytes) in rows {
        let (id, label) = table_group(&table);
        if id == "objects" {
            record_bytes[usize::from(is_index)] += bytes;
        }
        let key = if id == "table" {
            format!("table_{table}")
        } else {
            id.into()
        };
        let entry = groups.entry(key).or_insert((label, 0));
        entry.1 += bytes;
    }
    let mut parts: Vec<Value> = groups
        .into_iter()
        .map(|(id, (label, bytes))| json!({"id":id,"label":label,"bytes":bytes}))
        .collect();
    records::split(&transaction, &mut parts, record_bytes)?;
    let pages: u64 = transaction
        .query_row("PRAGMA freelist_count", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let page_size: u64 = transaction
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if pages > 0 {
        parts.push(json!({"id":"free_pages","label":"Свободное место внутри базы","bytes":pages*page_size}));
    }
    Ok(json!(parts))
}

pub(super) fn database_detail(db_path: &Path) -> Result<Value, String> {
    if !db_path.is_file() {
        return Err("ark.db does not exist".into());
    }
    let before = std::fs::metadata(db_path).map_err(|e| e.to_string())?;
    let result = read_detail(db_path, false);
    if result.is_ok() {
        return result;
    }
    let has_sidecar = || {
        ["-wal", "-shm", "-journal"]
            .iter()
            .any(|suffix| Path::new(&format!("{}{suffix}", db_path.display())).exists())
    };
    if has_sidecar() {
        return result;
    }
    let snapshot = read_detail(db_path, true)?;
    let after = std::fs::metadata(db_path).map_err(|e| e.to_string())?;
    if has_sidecar()
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err("database changed during read-only inspection".into());
    }
    Ok(snapshot)
}

#[path = "storage_records.rs"]
mod records;
