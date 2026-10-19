//! Pure compatibility mapping for the one-time legacy task migration.

use chrono::NaiveDate;
use serde_json::{json, Map, Value};

use super::definitions::canonical_type_registrations;
use super::validation::validate_canonical;
use super::TypeRegistration;

pub const LEGACY_TASK_TYPE: &str = crate::canonical_type::LEGACY_TASK_TYPE;
pub const LEGACY_TASK_VERSION: &str = "1.1.0";
pub const CORE_TASK_TYPE: &str = "core.task";
pub const CORE_TASK_VERSION: &str = "0.1.0";

static NULL: Value = Value::Null;

/// Поля `extensions`, которые переходят в первые поля `core.task`. Остальные
/// (price, billable, significance, sort_order, system_kind, …) остаются в
/// `extensions` как есть, кроме `REMOVED_EXTENSION_KEYS`.
const PROMOTED_EXTENSION_KEYS: [&str; 18] = [
    "is_today",
    "is_someday",
    "is_evening",
    "is_trashed",
    "is_completed",
    "is_cancelled",
    "status",
    "completed_at",
    "cancelled_at",
    "scheduled_date",
    "deadline",
    "checklist_items",
    "recurrence_rule",
    "reminder_date",
    "project_id",
    "tag_ids",
    "heading_id",
    "area_id",
];

/// Связи и напоминания, которые в живой базе должны быть пустыми: перенос
/// этих полей в контракт `core.task` сейчас не реализован.
const UNSUPPORTED_EXTENSION_KEYS: [&str; 7] = [
    "checklist_items",
    "recurrence_rule",
    "reminder_date",
    "project_id",
    "tag_ids",
    "heading_id",
    "area_id",
];

/// Поля, которые перезапись отбрасывает намеренно: топливо (`fuel_cost`) удалено
/// из продукта. Это не «неподдерживаемые» поля, их значение не останавливает
/// перезапись. Исходные значения остаются в бэкапе `<db>.pre-core-task.bak`.
const REMOVED_EXTENSION_KEYS: [&str; 1] = ["fuel_cost"];
#[derive(Debug, Clone)]
pub struct LegacyTask {
    pub id: String,
    pub type_version: String,
    pub content_json: String,
    pub props_json: String,
    pub deleted_at: Option<String>,
}

/// Convert a stored legacy task to validated `core.task` properties.
pub fn map_legacy_task(row: &LegacyTask) -> Result<Value, String> {
    let registration = canonical_type_registrations()?
        .into_iter()
        .find(|r| r.type_id == CORE_TASK_TYPE && r.version == CORE_TASK_VERSION)
        .ok_or_else(|| format!("{CORE_TASK_TYPE} {CORE_TASK_VERSION} is not registered"))?;
    map_row(row, &registration)
}

/// Одна задача 1.1.0 → props `core.task` 0.1.0. Ошибка называет id задачи и
/// причину, но не заголовок и не текст.
fn map_row(row: &LegacyTask, registration: &TypeRegistration) -> Result<Value, String> {
    let bad = |why: &str| format!("{LEGACY_TASK_TYPE} {}: {why}", row.id);
    if row.type_version != LEGACY_TASK_VERSION {
        return Err(bad("unexpected type version"));
    }
    let props: Map<String, Value> =
        serde_json::from_str(&row.props_json).map_err(|_| bad("props are not an object"))?;
    let ext = props
        .get("extensions")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("extensions are not an object"))?;
    let content: Value =
        serde_json::from_str(&row.content_json).map_err(|_| bad("content is not JSON"))?;

    for key in [
        "dueAt",
        "reminderAt",
        "scheduledAt",
        "recurrence",
        "checklist",
    ] {
        if !is_blank(props.get(key)) {
            return Err(bad(&format!("top-level {key} is not migrated")));
        }
    }
    for key in UNSUPPORTED_EXTENSION_KEYS {
        if !is_blank(ext.get(key)) {
            return Err(bad(&format!("{key} is not migrated")));
        }
    }
    for (top, nested) in [
        ("canceledAt", "cancelled_at"),
        ("completedAt", "completed_at"),
    ] {
        if value_of(&props, top) != value_of(ext, nested) {
            return Err(bad(&format!("{top} disagrees with {nested}")));
        }
    }

    let status = text(ext, "status").ok_or_else(|| bad("status is missing"))?;
    if text(&props, "status") != Some(status) {
        return Err(bad("top-level status disagrees with extensions"));
    }
    let completed = flag(ext, "is_completed").ok_or_else(|| bad("is_completed is missing"))?;
    let cancelled = flag(ext, "is_cancelled").ok_or_else(|| bad("is_cancelled is missing"))?;
    let today = flag(ext, "is_today").ok_or_else(|| bad("is_today is missing"))?;
    let someday = flag(ext, "is_someday").ok_or_else(|| bad("is_someday is missing"))?;
    let evening = flag(ext, "is_evening").ok_or_else(|| bad("is_evening is missing"))?;
    let trashed = flag(ext, "is_trashed").ok_or_else(|| bad("is_trashed is missing"))?;
    if trashed != row.deleted_at.is_some() {
        return Err(bad("trash flag disagrees with deleted_at"));
    }
    if completed && cancelled {
        return Err(bad("task is both completed and cancelled"));
    }
    match (status, completed) {
        ("done", true) | ("inbox" | "todo", false) => {}
        _ => return Err(bad("status disagrees with is_completed")),
    }
    let priority = text(&props, "priority").ok_or_else(|| bad("priority is missing"))?;
    if !["none", "low", "medium", "high", "urgent"].contains(&priority) {
        return Err(bad("priority is outside the contract"));
    }

    let state = if cancelled {
        let at = text(ext, "cancelled_at").ok_or_else(|| bad("cancelled_at is missing"))?;
        json!({"kind": "canceled", "at": at})
    } else if completed {
        let at = text(ext, "completed_at").ok_or_else(|| bad("completed_at is missing"))?;
        json!({"kind": "done", "at": at})
    } else {
        json!({"kind": "open", "at": null})
    };

    let scheduled = match ext.get("scheduled_date") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => {
            Some(calendar_date(s).ok_or_else(|| bad("scheduled_date is not YYYY-MM-DD"))?)
        }
        Some(_) => return Err(bad("scheduled_date has an unexpected type")),
    };
    let (planning_kind, planning_date) = if today && someday {
        return Err(bad("task is both today and someday"));
    } else if someday || today {
        if scheduled.is_some() {
            return Err(bad("scheduled date with today or someday is not migrated"));
        }
        (if someday { "someday" } else { "today" }, None)
    } else if let Some(date) = scheduled {
        ("date", Some(date))
    } else if status == "inbox" {
        ("inbox", None)
    } else {
        ("anytime", None)
    };

    let deadline = match ext.get("deadline") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => {
            Some(calendar_date(s).ok_or_else(|| bad("deadline is not YYYY-MM-DD"))?)
        }
        Some(_) => return Err(bad("deadline has an unexpected type")),
    };

    let mut extensions = ext.clone();
    for key in PROMOTED_EXTENSION_KEYS
        .into_iter()
        .chain(REMOVED_EXTENSION_KEYS)
    {
        extensions.remove(key);
    }
    let migrated = json!({
        "state": state,
        "planning": {
            "kind": planning_kind,
            "period": if evening { "evening" } else { "any" },
            "date": planning_date,
        },
        "deadline": deadline,
        "reminders": [],
        "recurrence": null,
        "priority": priority,
        "checklist": [],
        "extensions": extensions,
    });
    validate_canonical(registration, &migrated, &content)
        .map_err(|e| bad(&format!("does not match core.task 0.1.0 at {}", e.pointer)))?;
    Ok(migrated)
}

fn value_of<'a>(map: &'a Map<String, Value>, key: &str) -> &'a Value {
    map.get(key).unwrap_or(&NULL)
}

fn text<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key).and_then(Value::as_str)
}

fn flag(map: &Map<String, Value>, key: &str) -> Option<bool> {
    map.get(key).and_then(Value::as_bool)
}

fn is_blank(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::Array(items)) => items.is_empty(),
        Some(_) => false,
    }
}

/// Дата контракта `format: date`. Длинная строка с временем не подходит: её
/// день нельзя взять без выбора часового пояса, поэтому перезапись стоп.
fn calendar_date(value: &str) -> Option<String> {
    (value.len() == 10 && NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok())
        .then(|| value.to_owned())
}
