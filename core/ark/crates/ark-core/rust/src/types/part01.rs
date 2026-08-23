use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// DB entity types (camelCase JSON for sidecar compatibility)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct TodoItem {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub priority: i64,
    pub scheduled_date: Option<String>,
    pub deadline: Option<String>,
    pub reminder_date: Option<String>,
    pub is_today: bool,
    pub is_evening: bool,
    pub is_someday: bool,
    pub is_completed: bool,
    pub completed_at: Option<String>,
    pub is_cancelled: bool,
    pub cancelled_at: Option<String>,
    pub is_trashed: bool,
    pub sort_order: i64,
    pub heading_id: Option<String>,
    pub project_id: Option<String>,
    pub area_id: Option<String>,
    pub tag_ids: Vec<String>,
    #[cfg_attr(feature = "ts-rs", ts(type = "unknown"))]
    pub checklist_items: Value,
    #[cfg_attr(feature = "ts-rs", ts(type = "unknown"))]
    pub recurrence_rule: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub status: String,
    pub scheduled_date: Option<String>,
    pub deadline: Option<String>,
    pub sort_order: i64,
    pub color_tag: Option<String>,
    pub area_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct Area {
    pub id: String,
    pub title: String,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct Tag {
    pub id: String,
    pub title: String,
    pub color: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct Heading {
    pub id: String,
    pub title: String,
    pub sort_order: i64,
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct TrackedApp {
    pub id: String,
    pub platform: String,
    pub exe_path: String,
    pub normalized_exe_path: String,
    pub process_name: String,
    pub display_name: Option<String>,
    pub publisher: Option<String>,
    pub icon_ref: Option<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct UsageSession {
    pub id: String,
    pub tracked_app_id: String,
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    #[serde(default)]
    pub runtime_ms: i64,
    pub foreground_ms: i64,
    pub idle_ms: i64,
    pub window_title: Option<String>,
    pub process_name: String,
    pub exe_path: String,
    pub pid_start: Option<i64>,
    pub pid_end: Option<i64>,
    #[serde(default = "default_meta_json")]
    #[cfg_attr(feature = "ts-rs", ts(type = "Record<string, unknown>"))]
    pub meta_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct UsageEvent {
    pub id: String,
    pub tracked_app_id: String,
    pub usage_session_id: Option<String>,
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub occurred_at: String,
    pub kind: String,
    pub window_title: Option<String>,
    pub process_name: String,
    pub exe_path: String,
    pub pid: Option<i64>,
    pub is_foreground: bool,
    pub is_idle: bool,
    #[serde(default = "default_meta_json")]
    #[cfg_attr(feature = "ts-rs", ts(type = "Record<string, unknown>"))]
    pub meta_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageDay {
    pub id: String,
    pub device_id: String,
    pub day: String,
    #[serde(default = "default_usage_day_payload")]
    pub payload_json: Value,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSpanWrite {
    pub device_id: String,
    pub started_at_unix: i64,
    pub ended_at_unix: i64,
    pub tracked_app_id: String,
    pub window_title: Option<String>,
    #[serde(default)]
    pub flags: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageTitleTotal {
    pub active_seconds: i64,
    pub idle_seconds: i64,
}

fn default_usage_day_payload() -> Value {
    serde_json::json!({ "a": [], "t": [], "s": [] })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct ArkObject {
    pub id: String,
    pub type_id: String,
    pub type_version: String,
    pub title: String,
    #[serde(default = "default_content_json")]
    #[cfg_attr(feature = "ts-rs", ts(type = "unknown"))]
    pub content_json: Value,
    #[serde(default = "default_props_json")]
    #[cfg_attr(feature = "ts-rs", ts(type = "Record<string, unknown>"))]
    pub props_json: Value,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct ArkObjectSummary {
    pub id: String,
    pub type_id: String,
    pub type_version: String,
    pub title: String,
    #[serde(default = "default_props_json")]
    #[cfg_attr(feature = "ts-rs", ts(type = "Record<string, unknown>"))]
    pub props_json: Value,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}
