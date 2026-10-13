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
    // KOS-302: 0.9.x leftover with no writer in cortex — surfaced as its own
    // row so the «Данные» page can offer the «Очистить» action.
    LegacyQuarantine,
    Installations,
    Cache,
    Logs,
    Settings,
    RuntimeState,
    Recordings,
    Tools,
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
            Category::LegacyQuarantine => "Устаревшие данные",
            Category::Installations => "Установленные приложения",
            Category::Cache => "Кеш",
            Category::Logs => "Журналы",
            Category::Settings => "Настройки приложений",
            Category::RuntimeState => "Состояние Engine",
            Category::Recordings => "Записи диктовки",
            Category::Tools => "Движки распознавания",
            Category::Other => "Дополнительные файлы",
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
            Category::LegacyQuarantine => "legacy_quarantine",
            Category::Installations => "installations",
            Category::Cache => "cache",
            Category::Logs => "logs",
            Category::Settings => "settings",
            Category::RuntimeState => "runtime_state",
            Category::Recordings => "recordings",
            Category::Tools => "tools",
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
    Category::LegacyQuarantine,
    Category::Installations,
    Category::Cache,
    Category::Logs,
    Category::Settings,
    Category::RuntimeState,
    Category::Recordings,
    Category::Tools,
    Category::Other,
];

/// Classify one top-level entry of the data dir by its file name.
/// `is_package_root` covers the case where the store does not sit at
/// `<data_dir>/packages`.
fn classify(name: &str, is_package_root: bool) -> Category {
    match name {
        "ark.db" | "ark.db-wal" | "ark.db-shm" | "ark.db-journal" => Category::Database,
        "backups" => Category::Backups,
        "updates" => Category::Updates,
        "Apps" | "Engine" => Category::Installations,
        "cache" | "icon-cache" | "app-icons" | "icons" => Category::Cache,
        "logs" => Category::Logs,
        "dictation" => Category::Recordings,
        "tools" | "runtimes" => Category::Tools,
        "crashes" | "support-bundles" => Category::Logs,
        _ if name.contains("config")
            || name.contains("settings")
            || name == "browser.json"
            || name == "manager.json" =>
        {
            Category::Settings
        }
        _ if name.contains("lock")
            || name.starts_with("engine-")
            || name == "mundus-device-id.txt"
            || name == "protocol-usage.json"
            || name == "mundus-migration.json" =>
        {
            Category::RuntimeState
        }
        "packages" => Category::Packages,
        "legacy-quarantine" => Category::LegacyQuarantine,
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

/// Bytes attributed to one directory entry. `DirEntry::file_type` /
/// `DirEntry::metadata` never follow links: a symlink or junction counts
/// only its own (tiny) record and is never descended into — a junction
/// inside the data dir could otherwise loop the walk or count bytes twice.
fn entry_bytes(entry: &std::fs::DirEntry) -> u64 {
    match entry.file_type() {
        Ok(ft) if ft.is_symlink() => entry.metadata().map(|m| m.len()).unwrap_or(0),
        Ok(ft) if ft.is_dir() => directory_bytes(&entry.path()),
        Ok(_) => entry.metadata().map(|m| m.len()).unwrap_or(0),
        Err(_) => 0,
    }
}

pub(super) fn directory_bytes(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries.flatten().map(|entry| entry_bytes(&entry)).sum()
}

/// Accumulate `bytes` into the per-category bucket keyed by discriminant —
/// `Category` is not `Ord`, and a HashMap<Category> needs `Hash`.
fn add_path(acc: &mut BTreeMap<usize, u64>, category: Category, bytes: u64) {
    *acc.entry(category as usize).or_default() += bytes;
}

fn storage_breakdown(data_dir: &Path, package_root: &Path) -> Value {
    let package_root = std::fs::canonicalize(package_root).ok();
    let mut acc: BTreeMap<usize, u64> = BTreeMap::new();
    let mut details: BTreeMap<usize, Vec<Value>> = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_package_root = package_root
                .as_deref()
                .is_some_and(|root| std::fs::canonicalize(&path).is_ok_and(|p| p == *root));
            let is_dir = entry.file_type().is_ok_and(|ft| ft.is_dir());
            if name == "models" && is_dir {
                if let Ok(models) = std::fs::read_dir(&path) {
                    for model_entry in models.flatten() {
                        let category =
                            classify_models_entry(&model_entry.file_name().to_string_lossy());
                        let bytes = entry_bytes(&model_entry);
                        add_path(&mut acc, category, bytes);
                        details
                            .entry(category as usize)
                            .or_default()
                            .push(json!({"path": model_entry.path(), "bytes": bytes}));
                    }
                }
                continue;
            }
            let category = classify(&name, is_package_root);
            let bytes = entry_bytes(&entry);
            add_path(&mut acc, category, bytes);
            if !matches!(category, Category::Database) && bytes > 0 {
                details
                    .entry(category as usize)
                    .or_default()
                    .push(json!({"path": path, "bytes": bytes}));
            }
        }
    }
    let mut value = finish_breakdown(data_dir, acc);
    for category in ALL_CATEGORIES {
        if let Some(detail) = details.remove(&(*category as usize)) {
            if let Some(entry) = value["categories"].as_array_mut().and_then(|entries| {
                entries
                    .iter_mut()
                    .find(|entry| entry["id"] == category.id())
            }) {
                entry["detail"] = json!(detail);
            }
        }
    }
    value
}

fn finish_breakdown(data_dir: &Path, acc: BTreeMap<usize, u64>) -> Value {
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
            let db_path = data_dir.join("ark.db");
            match database::database_detail(&db_path) {
                Ok(detail) => value["detail"] = detail,
                Err(error) => {
                    tracing::warn!(
                        target: "manager_api",
                        path = %db_path.display(),
                        %error,
                        "ark.db page breakdown skipped"
                    );
                }
            }
        }
        categories.push(value);
    }
    json!({"total_bytes": total, "categories": categories, "data_dir": data_dir})
}

impl ManagerState {
    /// Directory walk and the dbstat scan run on a blocking thread — the
    /// request loop must not stall on a multi-GB data dir.
    pub async fn data_storage(&self, package_root: &Path) -> Result<Value, String> {
        let data_dir = self.data_dir().to_path_buf();
        let package_root = package_root.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let mut value = storage_breakdown(&data_dir, &package_root);
            external::add_external_storage(&mut value, &data_dir, &package_root);
            presentation::describe(&mut value);
            value
        })
        .await
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;

#[path = "storage_external.rs"]
mod external;

#[path = "storage_database.rs"]
mod database;
#[path = "storage_presentation.rs"]
mod presentation;
