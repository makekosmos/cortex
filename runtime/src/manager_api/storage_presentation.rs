//! Explain the files from the inventory without exposing paths as UI labels.
use serde_json::{json, Value};
use std::path::Path;

fn expand_details(category: &mut Value, id: &str) {
    let Some(details) = category["detail"].as_array_mut() else {
        return;
    };
    let mut expanded = Vec::new();
    for detail in details.drain(..) {
        let path = detail["path"].as_str().map(Path::new);
        let split = path.is_some_and(|path| {
            (id == "dictation_models"
                && matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("dictation" | "whisper")
                ))
                || (id == "installations" && path.file_name().is_some_and(|name| name == "Apps"))
        });
        if split {
            if let Some(entries) = path.and_then(|path| std::fs::read_dir(path).ok()) {
                for entry in entries.flatten() {
                    expanded.push(json!({"path":entry.path(), "bytes":super::entry_bytes(&entry)}));
                }
                continue;
            }
        }
        expanded.push(detail);
    }
    *details = expanded;
}

fn purpose(id: &str) -> &'static str {
    match id {
        "database" => "Задачи, проекты и другие объекты ARK, история изменений и журнал SQLite.",
        "backups" => "Снимки базы для восстановления данных после сбоя или миграции.",
        "dictation_models" => "Локальные ИИ-модели преобразуют речь в текст на этом компьютере.",
        "tools" => "Исполняемые движки и библиотеки для локального распознавания речи.",
        "packages" => "Код, ресурсы и сохранённые версии интеграций Mundus.",
        "updates" => "Скачанные установщики и архивы для обновления приложений.",
        "file_index" => "Индекс имён и расположений файлов ускоряет поиск на компьютере.",
        "app_index" => "Список установленных приложений для поиска и запуска.",
        "legacy_quarantine" => {
            "Данные предыдущих версий, отложенные во время перехода на новый формат."
        }
        "installations" => "Программы Mundus и установленные через него приложения.",
        "cache" => "Повторно используемые значки и временные ресурсы интерфейса.",
        "logs" => "Диагностика работы, отчёты о сбоях и материалы для поддержки.",
        "settings" => "Выбранная тема, параметры диктовки, браузера и интерфейса.",
        "runtime_state" => "Идентификатор устройства, контроль запуска Engine и совместимость API.",
        "objects" => "Сохранённые объекты, их свойства и индексы SQLite.",
        "usage_tracker" => "История активности приложений и её индексы.",
        "search_index" => "Полнотекстовый индекс для поиска по сохранённым объектам.",
        "schema" => "Служебные таблицы SQLite, определения типов и данные синхронизации.",
        "recordings" => "Аудио и результаты записей диктовки, сохранённые локально.",
        _ => "Файл присутствует в каталоге Mundus, но его назначение не зарегистрировано в Engine.",
    }
}

fn file_label(path: &Path, category: &str) -> String {
    if let Some(label) = provenance::sqlite_label(path) {
        return label;
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if category == "dictation_models" {
        if let Some(model) = crate::dictation::local_models::MODEL_CATALOG
            .iter()
            .find(|model| model.filename == name)
        {
            return model.name.into();
        }
    }
    if category == "installations" {
        if let Some(app) = crate::native_apps::NATIVE_APPS
            .iter()
            .find(|app| app.id == name)
        {
            return app.name.into();
        }
    }
    let label = match name.as_ref() {
        "ark.db" => "Основная база ARK",
        "ark.db-wal" | "ark.db-journal" => "Журнал незавершённых изменений базы",
        "ark.db-shm" => "Координация доступа к базе",
        "backups" => "Снимки основной базы",
        "Apps" => "Установленные приложения Mundus",
        "Engine" => "Mundus Engine",
        "Mundus Manager.app" => "Mundus Manager",
        "Mundus Manager" | "manager.json" => "Параметры интерфейса Manager",
        "packages" => "Хранилище интеграций",
        "updates" => "Архивы обновлений",
        "dictation" if category == "dictation_models" => "Модели распознавания речи",
        "dictation" if category == "tools" => "Движки локальной диктовки",
        "dictation" => "История записей диктовки",
        "logs" => "Журналы работы Engine и приложений",
        "crashes" => "Отчёты о сбоях",
        "support-bundles" => "Диагностические архивы для поддержки",
        "app-icons" | "icon-cache" | "icons" => "Значки установленных приложений",
        "cache" | "Mundus" if category == "cache" => "Кеш ресурсов Mundus",
        "appearance-settings.json" => "Тема и оформление приложений",
        "dictation-config.json" => "Параметры распознавания речи",
        "dictation-config.json.bak" => "Предыдущие параметры диктовки",
        "dictation-stats.json" => "Статистика диктовки",
        "browser.json" => "Параметры встроенного браузера",
        "mundus-device-id.txt" => "Идентификатор устройства для синхронизации",
        "protocol-usage.json" => "Сведения о версиях API клиентов",
        "engine-supervisor-state.json" => "Состояние фонового Engine",
        "engine-supervisor-control.json" => "Команды управления фоновым Engine",
        "engine.lock.json" => "Подключение приложений к Engine",
        "mundus-migration.json" => "Результат перехода на новую версию",
        "legacy-quarantine" => "Файлы предыдущей версии Mundus",
        _ if name.contains("singleton") => "Защита от одновременного запуска Engine",
        _ if name.contains("supervisor.lock") => "Координация фонового процесса Engine",
        _ if name.starts_with("file-index.db") => "Поисковый индекс файлов",
        _ if name.starts_with("app-index.db") => "Поисковый индекс приложений",
        _ => return format!("Файл «{name}»"),
    };
    label.into()
}

pub(super) fn describe(value: &mut Value) {
    provenance::database_breakdown(value);
    let Some(categories) = value["categories"].as_array_mut() else {
        return;
    };
    for category in categories.iter_mut() {
        let id = category["id"].as_str().unwrap_or_default().to_owned();
        category["description"] = json!(purpose(&id));
        expand_details(category, &id);
        if let Some(details) = category["detail"].as_array_mut() {
            for detail in details {
                if let Some(path) = detail["path"].as_str().map(std::path::PathBuf::from) {
                    detail["label"] = json!(file_label(&path, &id));
                    let model = (id == "dictation_models")
                        .then(|| {
                            crate::dictation::local_models::MODEL_CATALOG
                                .iter()
                                .find(|model| {
                                    path.file_name().is_some_and(|name| name == model.filename)
                                })
                        })
                        .flatten();
                    detail["description"] =
                        json!(model.map_or_else(|| purpose(&id), |model| model.description));
                } else if detail.get("description").is_none() {
                    detail["description"] =
                        json!(purpose(detail["id"].as_str().unwrap_or_default()));
                }
            }
        }
    }
    // Unregistered files are named individually instead of hiding behind "Other".
    if let Some(index) = categories
        .iter()
        .position(|category| category["id"] == "other")
    {
        let other = categories.remove(index);
        if let Some(details) = other["detail"].as_array() {
            for (index, detail) in details.iter().enumerate() {
                categories.push(json!({
                    "id": format!("additional_{index}"), "label": detail["label"],
                    "description": detail["description"], "bytes": detail["bytes"],
                    "detail": [detail]
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_use_catalogue_names_and_service_files_explain_their_purpose() {
        let mut value = json!({"categories":[
            {"id":"dictation_models", "detail":[{"path":"/models/parakeet-tdt-0.6b-v3-ultra-int8", "bytes":637}]},
            {"id":"runtime_state", "detail":[{"path":"/data/mundus-device-id.txt", "bytes":16}]}
        ]});
        describe(&mut value);
        assert_eq!(
            value["categories"][0]["detail"][0]["label"],
            "Parakeet Ultra"
        );
        assert_eq!(
            value["categories"][1]["detail"][0]["label"],
            "Идентификатор устройства для синхронизации"
        );
    }

    #[test]
    fn visible_fields_never_use_paths_or_other_as_a_placeholder() {
        let mut value = json!({"categories":[
            {"id":"other", "bytes":7, "detail":[{"path":"/private/data/extra.bin", "bytes":7}]},
            {"id":"settings", "detail":[{"path":"/private/data/appearance-settings.json", "bytes":12}]},
            {"id":"database", "detail":[{"id":"schema", "label":"Структура и служебные таблицы", "bytes":123}]}
        ]});
        describe(&mut value);
        assert!(!value["categories"]
            .as_array()
            .unwrap()
            .iter()
            .any(|category| category["id"] == "other"));
        fn visible(value: &Value) {
            match value {
                Value::Object(fields) => {
                    for (key, value) in fields {
                        if key == "label" || key == "description" {
                            let text = value.as_str().unwrap();
                            assert!(!text.contains("/private"));
                            assert!(!text.contains("Прочее"));
                            assert!(!text.is_empty());
                        } else if key != "path" {
                            visible(value);
                        }
                    }
                }
                Value::Array(values) => {
                    for value in values {
                        visible(value);
                    }
                }
                _ => {}
            }
        }
        visible(&value);
    }
}

#[path = "storage_provenance.rs"]
mod provenance;
