use serde_json::{json, Value};
use std::path::Path;

pub(super) fn database_detail(db_path: &Path) -> Result<Value, String> {
    if !db_path.is_file() {
        return Err("ark.db does not exist".into());
    }
    let conn =
        rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT name, SUM(pgsize) FROM dbstat GROUP BY name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut objects = 0_i64;
    let mut usage = 0_i64;
    let mut search = 0_i64;
    let mut other = 0_i64;
    for (name, bytes) in rows {
        if name.starts_with("object_search_fts") {
            search += bytes;
        } else if name.starts_with("usage_") || name.starts_with("idx_usage") {
            usage += bytes;
        } else if name == "objects"
            || name.starts_with("object_")
            || name.starts_with("idx_objects")
        {
            objects += bytes;
        } else {
            other += bytes;
        }
    }
    Ok(json!([
        {"id": "objects", "label": "Объекты", "bytes": objects.max(0)},
        {"id": "usage_tracker", "label": "Трекер использования", "bytes": usage.max(0)},
        {"id": "search_index", "label": "Поисковый индекс", "bytes": search.max(0)},
        {"id": "schema", "label": "Структура и служебные таблицы", "bytes": other.max(0)},
    ]))
}
