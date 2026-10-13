use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub(super) fn sqlite_label(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let base = match name.split_once(".db").map(|(stem, _)| stem) {
        Some("app-index") => "Каталог приложений",
        Some("file-index") => "Каталог файлов",
        Some("ark") => "Данные ARK",
        Some("engine-supervisor.lock") => "Реестр фонового Engine",
        Some("mundus-singleton.lock") => "Реестр запуска Engine",
        _ => return None,
    };
    Some(if name.ends_with("-wal") || name.ends_with("-journal") {
        format!("Журнал изменений: {base}")
    } else if name.ends_with("-shm") {
        format!("Координация доступа: {base}")
    } else {
        base.into()
    })
}

pub(super) fn database_breakdown(value: &mut Value) {
    let Some(root) = value["data_dir"].as_str().map(PathBuf::from) else {
        return;
    };
    let file = root.join("ark.db");
    if !file.is_file() {
        return;
    }
    let Some(database) = value["categories"].as_array_mut().and_then(|categories| {
        categories
            .iter_mut()
            .find(|entry| entry["id"] == "database")
    }) else {
        return;
    };
    let old_bytes = database["bytes"].as_u64().unwrap_or(0);
    let file_bytes = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
    let mut detail = match super::super::database::database_detail(&file) {
        Ok(Value::Array(parts)) => parts,
        _ => vec![
            json!({"id":"database_file", "label":"Файл данных ARK — разбивка недоступна", "bytes":file_bytes}),
        ],
    };
    let attributed: u64 = detail
        .iter()
        .filter_map(|part| part["bytes"].as_u64())
        .sum();
    if attributed > file_bytes {
        detail = vec![
            json!({"id":"database_file", "label":"Файл данных ARK — новые страницы ещё в журнале", "bytes":file_bytes}),
        ];
    }
    if file_bytes > attributed {
        detail.push(json!({"id":"file_reserve", "label":"Резерв файла SQLite", "bytes":file_bytes-attributed}));
    }
    let mut bytes = file_bytes;
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", file.display()));
        let size = std::fs::metadata(&sidecar).map(|m| m.len()).unwrap_or(0);
        if size > 0 {
            bytes += size;
            detail.push(json!({"id":suffix, "label":sqlite_label(&sidecar), "bytes":size}));
        }
    }
    database["detail"] = json!(detail);
    database["bytes"] = json!(bytes);
    value["total_bytes"] = json!(value["total_bytes"]
        .as_u64()
        .unwrap_or(0)
        .saturating_sub(old_bytes)
        .saturating_add(bytes));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn sqlite_components_have_distinct_names_without_paths() {
        let labels: BTreeSet<_> = ["app-index.db", "app-index.db-wal", "app-index.db-shm"]
            .map(|name| sqlite_label(Path::new(name)).unwrap())
            .into_iter()
            .collect();
        assert_eq!(labels.len(), 3);
        assert!(labels.iter().all(|label| !label.contains(".db")));
    }

    #[test]
    fn only_selected_database_is_split_into_flat_parts() {
        let dir = tempfile::tempdir().unwrap();
        let working = dir.path().join("working");
        let dev = dir.path().join("dev");
        for (root, count) in [(&working, 3), (&dev, 17)] {
            std::fs::create_dir(root).unwrap();
            let conn = rusqlite::Connection::open(root.join("ark.db")).unwrap();
            conn.execute_batch("CREATE TABLE object_types(id TEXT PRIMARY KEY,name TEXT); INSERT INTO object_types VALUES('fixture.task','Task'); CREATE TABLE objects(id TEXT PRIMARY KEY, type_id TEXT, deleted_at TEXT)")
                .unwrap();
            for index in 0..count {
                conn.execute(
                    "INSERT INTO objects VALUES(?1,'fixture.task',NULL)",
                    [index.to_string()],
                )
                .unwrap();
            }
        }
        let mut value = json!({"data_dir":working, "total_bytes":0, "categories":[
            {"id":"database", "bytes":0, "detail":[{"path":dev.join("ark.db")}]}]});
        database_breakdown(&mut value);
        let database = &value["categories"][0];
        let parts = database["detail"].as_array().unwrap();
        let records = parts
            .iter()
            .find(|part| part["id"] == "record_type_fixture.task")
            .unwrap();
        let children: u64 = records["detail"]
            .as_array()
            .unwrap()
            .iter()
            .map(|part| part["bytes"].as_u64().unwrap())
            .sum();
        assert_eq!(records["bytes"].as_u64().unwrap(), children);
        assert_eq!(records["detail"][0]["count"], 3);
        assert!(parts.iter().all(|part| part["id"] != "objects"));
        let total: u64 = parts
            .iter()
            .map(|part| part["bytes"].as_u64().unwrap())
            .sum();
        assert_eq!(database["bytes"].as_u64().unwrap(), total);
        assert_eq!(database["bytes"], value["total_bytes"]);
    }
}
