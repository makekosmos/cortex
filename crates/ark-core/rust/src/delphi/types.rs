//! TodoItem shape — мatch TS `extensions/delphi/src/types/task.ts:TodoItem`.
//!
//! Поля только те что нужны `filters` модулю; полная schema живёт в
//! task_obj propsJson и сериализуется TS-side adapter'ом (см.
//! `extensions/delphi/src/lib/electron-api-shim.ts:arkTaskObjectToTodo`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub scheduled_date: Option<String>,
    #[serde(default)]
    pub is_today: bool,
    #[serde(default)]
    pub is_someday: bool,
    #[serde(default)]
    pub is_completed: bool,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub is_cancelled: bool,
    #[serde(default)]
    pub cancelled_at: Option<String>,
    #[serde(default)]
    pub is_trashed: bool,
    #[serde(default)]
    pub sort_order: i64,
    pub created_at: String,
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmartList {
    Inbox,
    Today,
    Upcoming,
    Anytime,
    Someday,
    Logbook,
    Trash,
}

impl SmartList {
    pub const ALL: [SmartList; 7] = [
        SmartList::Inbox,
        SmartList::Today,
        SmartList::Upcoming,
        SmartList::Anytime,
        SmartList::Someday,
        SmartList::Logbook,
        SmartList::Trash,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            SmartList::Inbox => "inbox",
            SmartList::Today => "today",
            SmartList::Upcoming => "upcoming",
            SmartList::Anytime => "anytime",
            SmartList::Someday => "someday",
            SmartList::Logbook => "logbook",
            SmartList::Trash => "trash",
        }
    }
}
