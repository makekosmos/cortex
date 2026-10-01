//! `manager.data.storage` — честная разбивка каталога данных по категориям.
//!
//! Раньше страница показывала только «управляемое хранилище»
//! (ark.db + packages ≈ 0.5 ГБ), а реальный каталог данных занимал ~4 ГБ —
//! бэкапы, модели диктовки, скачанные установщики и индексы не входили в
//! сумму. Теперь Engine обходит весь data_dir и отдаёт по категориям.

use super::ManagerState;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// Fixed category ids — the Manager renders them in this order.
#[derive(Clone, Copy)]
enum Category {
    Database,
    Backups,
    DictationModels,
    Packages,
    Updates,
    FileIndex,
    AppIndex,
    Other,
}

impl Category {
    fn label(self) -> &'static str {
        match self {
            Category::Database => "База данных",
            Category::Backups => "Резервные копии",
            Category::DictationModels => "Модели диктовки",
            Category::Packages => "Пакеты",
            Category::Updates => "Загруженные обновления",
            Category::FileIndex => "Индекс файлов",
            Category::AppIndex => "Индекс приложений",
            Category::Other => "Прочее",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Category::Database => "database",
            Category::Backups => "backups",
            Category::DictationModels => "dictation_models",
            Category::Packages => "packages",
            Category::Updates => "updates",
            Category::FileIndex => "file_index",
            Category::AppIndex => "app_index",
            Category::Other => "other",
        }
    }
}

const ALL_CATEGORIES: &[Category] = &[
    Category::Database,
    Category::Backups,
    Category::DictationModels,
    Category::Packages,
    Category::Updates,
    Category::FileIndex,
    Category::AppIndex,
    Category::Other,
];

/// Classify one top-level entry of the data dir by its file name.
/// `is_package_root` covers the case where the store does not sit at
/// `<data_dir>/packages`.
fn classify(name: &str, is_package_root: bool) -> Category {
    match name {
        "ark.db" | "ark.db-wal" | "ark.db-shm" => Category::Database,
        "backups" => Category::Backups,
        "updates" => Category::Updates,
        "packages" => Category::Packages,
        _ if is_package_root => Category::Packages,
        _ if name.starts_with("ark.db.") => Category::Backups,
        _ if name.starts_with("file-index.db") => Category::FileIndex,
        _ if name.starts_with("app-index.db") => Category::AppIndex,
        _ => Category::Other,
    }
}

/// `models/` holds dictation assets (`dictation/`, legacy `whisper/`);
/// anything else in there is still reported, under «Прочее».
fn classify_models_entry(name: &str) -> Category {
    match name {
        "dictation" | "whisper" => Category::DictationModels,
        _ => Category::Other,
    }
}

fn file_bytes(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn directory_bytes(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let p = entry.path();
            if p.is_dir() {
                directory_bytes(&p)
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

/// Split `models/` per subdirectory; everything outside it is one class.
fn add_path(acc: &mut BTreeMap<usize, u64>, category: Category, bytes: u64) {
    *acc.entry(category as usize).or_default() += bytes;
}

fn database_detail(db_path: &Path) -> Option<Value> {
    if !db_path.is_file() {
        return None;
    }
    // Read-only path through SQLite itself: `dbstat` reports on-disk pages
    // per btree, so «трекер использования» includes usage_sync_log *and* its
    // indexes instead of just the raw column bytes.
    let conn = rusqlite::Connection::open(db_path).ok()?;
    let mut stmt = conn
        .prepare("SELECT name, SUM(pgsize) FROM dbstat GROUP BY name")
        .ok()?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .ok()?
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
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
    Some(json!([
        {"id": "objects", "label": "Объекты", "bytes": objects.max(0)},
        {"id": "usage_tracker", "label": "Трекер использования", "bytes": usage.max(0)},
        {"id": "search_index", "label": "Поисковый индекс", "bytes": search.max(0)},
        {"id": "other", "label": "Прочее в базе", "bytes": other.max(0)},
    ]))
}

fn storage_breakdown(data_dir: &Path, package_root: &Path) -> Value {
    let package_root = std::fs::canonicalize(package_root).ok();
    let mut acc: BTreeMap<usize, u64> = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_package_root = package_root
                .as_deref()
                .is_some_and(|root| std::fs::canonicalize(&path).is_ok_and(|p| p == *root));
            if name == "models" && path.is_dir() {
                if let Ok(models) = std::fs::read_dir(&path) {
                    for model_entry in models.flatten() {
                        let model_path = model_entry.path();
                        let bytes = if model_path.is_dir() {
                            directory_bytes(&model_path)
                        } else {
                            model_entry.metadata().map(|m| m.len()).unwrap_or(0)
                        };
                        add_path(
                            &mut acc,
                            classify_models_entry(&model_entry.file_name().to_string_lossy()),
                            bytes,
                        );
                    }
                }
                continue;
            }
            let bytes = if path.is_dir() {
                directory_bytes(&path)
            } else {
                file_bytes(&path)
            };
            let category = classify(&name, is_package_root);
            add_path(&mut acc, category, bytes);
        }
    }
    let mut categories = Vec::new();
    let mut total = 0_u64;
    for category in ALL_CATEGORIES {
        let bytes = acc.get(&(*category as usize)).copied().unwrap_or(0);
        total = total.saturating_add(bytes);
        if bytes == 0 && !matches!(category, Category::Database | Category::Other) {
            continue;
        }
        let mut value = json!({
            "id": category.id(),
            "label": category.label(),
            "bytes": bytes,
        });
        if matches!(category, Category::Database) {
            if let Some(detail) = database_detail(&data_dir.join("ark.db")) {
                value["detail"] = detail;
            }
        }
        categories.push(value);
    }
    json!({"total_bytes": total, "categories": categories})
}

impl ManagerState {
    /// Directory walk and the dbstat scan run on a blocking thread — the
    /// request loop must not stall on a multi-GB data dir.
    pub async fn data_storage(&self, package_root: &Path) -> Result<Value, String> {
        let data_dir = self.data_dir().to_path_buf();
        let package_root = package_root.to_path_buf();
        tokio::task::spawn_blocking(move || storage_breakdown(&data_dir, &package_root))
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
