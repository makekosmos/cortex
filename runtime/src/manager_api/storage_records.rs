//! Split shared SQLite pages into actual field data by type and shared overhead.
use rusqlite::Transaction;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn label(name: &str) -> String {
    match name {
        "Задача" => "Задачи",
        "Проект" => "Проекты",
        "Заметка" => "Заметки",
        "Тег" | "Метка" => "Метки",
        other => other,
    }
    .into()
}

pub(super) fn split(
    tx: &Transaction<'_>,
    parts: &mut Vec<Value>,
    pages: [u64; 2],
) -> Result<(), String> {
    // Old schemas without typed records retain their table-level inventory.
    let mut columns = tx
        .prepare("PRAGMA table_info(objects)")
        .map_err(|e| e.to_string())?;
    let columns = columns
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if !columns.iter().any(|column| column == "type_id") {
        return Ok(());
    }
    let fields = columns
        .iter()
        .map(|column| {
            format!(
                "coalesce(length(CAST(o.\"{}\" AS BLOB)),0)",
                column.replace('"', "\"\"")
            )
        })
        .collect::<Vec<_>>()
        .join("+");
    let query = format!(
        "SELECT o.type_id, coalesce(t.name,o.type_id), o.deleted_at IS NOT NULL,
        count(*), sum({fields}) FROM objects o LEFT JOIN object_types t ON t.id=o.type_id
        GROUP BY o.type_id,t.name,o.deleted_at IS NOT NULL"
    );
    let mut stmt = tx.prepare(&query).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, u64>(3)?,
                row.get::<_, u64>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut types: BTreeMap<String, Value> = BTreeMap::new();
    let mut payload = 0_u64;
    for (id, name, deleted, count, bytes) in rows {
        payload += bytes;
        let noun = match name.as_str() {
            "Задача" | "Проект" | "Заметка" | "Тег" | "Метка" => {
                label(&name).to_lowercase()
            }
            _ => "записи".into(),
        };
        let entry = types.entry(id.clone()).or_insert_with(|| {
            json!({
                "id":format!("record_type_{id}"), "label":label(&name), "bytes":0,
                "measurement":"field_bytes", "detail":[]
            })
        });
        entry["bytes"] = json!(entry["bytes"].as_u64().unwrap_or(0) + bytes);
        if let Some(details) = entry["detail"].as_array_mut() {
            details.push(json!({"id":if deleted {"deleted"} else {"active"},
                "label":format!("{} {noun}", if deleted {"Удалённые"} else {"Текущие"}),
                "bytes":bytes, "count":count}));
        }
    }
    if payload > pages[0] {
        return Err("record fields exceed allocated table pages".into());
    }
    parts.retain(|part| part["id"] != "objects");
    parts.extend(types.into_values());
    if pages[0] > payload {
        parts.push(json!({"id":"record_overhead", "label":"Служебные данные и запас места записей", "bytes":pages[0]-payload}));
    }
    if pages[1] > 0 {
        parts.push(json!({"id":"record_indexes", "label":"Индексы для поиска и сортировки записей", "bytes":pages[1]}));
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn types_and_deleted_records_are_measured_in_utf8_bytes_without_page_estimates() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ark.db");
        let conn = rusqlite::Connection::open(&file).unwrap();
        conn.execute_batch("CREATE TABLE object_types(id TEXT PRIMARY KEY,name TEXT);
            CREATE TABLE objects(id TEXT PRIMARY KEY,type_id TEXT,content_json TEXT,deleted_at TEXT);
            INSERT INTO object_types VALUES('fixture.task','Задача'),('fixture.note','Заметка');
            INSERT INTO objects VALUES('1','fixture.task','Привет',NULL),
                ('2','fixture.task','deleted','2026'),('3','fixture.note','note',NULL);
            CREATE INDEX record_type_index ON objects(type_id);").unwrap();
        let value = super::super::database_detail(&file).unwrap();
        let parts = value.as_array().unwrap();
        let tasks = parts.iter().find(|part| part["label"] == "Задачи").unwrap();
        assert_eq!(tasks["bytes"], 1 + 12 + 12 + 1 + 12 + 7 + 4);
        assert_eq!(tasks["detail"][0]["bytes"], 25);
        assert_eq!(tasks["detail"][1]["bytes"], 24);
        let notes = parts
            .iter()
            .find(|part| part["label"] == "Заметки")
            .unwrap();
        assert_eq!(notes["bytes"], 1 + 12 + 4);
        assert!(parts.iter().any(|part| part["id"] == "record_indexes"));
        assert!(parts.iter().any(|part| part["id"] == "record_overhead"));
        let total: u64 = parts
            .iter()
            .map(|part| part["bytes"].as_u64().unwrap())
            .sum();
        assert_eq!(total, std::fs::metadata(file).unwrap().len());
    }
}
