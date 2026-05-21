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
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct ArkObject {
    pub id: String,
    pub type_id: String,
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
pub struct ObjectType {
    pub id: String,
    pub name: String,
    pub schema_json: String,
    pub ui_schema_json: String,
    pub created_at: String,
    pub updated_at: String,
    pub system_locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct ObjectLink {
    pub id: String,
    pub source_object_id: String,
    pub target_object_id: String,
    pub link_type: String,
    pub created_at: String,
}

fn default_meta_json() -> Value {
    json!({})
}

fn default_content_json() -> Value {
    json!({
        "type": "doc",
        "content": [{ "type": "paragraph" }]
    })
}

fn default_props_json() -> Value {
    json!({})
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct LoadAllData {
    pub todos: Vec<TodoItem>,
    pub projects: Vec<Project>,
    pub areas: Vec<Area>,
    pub tags: Vec<Tag>,
    pub headings: Vec<Heading>,
    pub tracked_apps: Vec<TrackedApp>,
    pub usage_sessions: Vec<UsageSession>,
    pub usage_events: Vec<UsageEvent>,
    pub objects: Vec<ArkObject>,
    pub object_types: Vec<ObjectType>,
    pub object_links: Vec<ObjectLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct UsageSummary {
    pub tracked_app_count: i64,
    pub session_count: i64,
    pub event_count: i64,
    pub total_foreground_ms: i64,
    pub total_idle_ms: i64,
    pub first_recorded_at: Option<String>,
    pub last_recorded_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct DailyTrendPoint {
    pub date: String,
    pub foreground_ms: i64,
    pub idle_ms: i64,
    pub sessions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct HourlyHeatmapCell {
    pub weekday: i64,
    pub hour: i64,
    pub foreground_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct TopAppEntry {
    pub id: String,
    pub display_name: String,
    pub process_name: String,
    pub normalized_path: String,
    pub foreground_ms: i64,
    pub idle_ms: i64,
    pub sessions: i64,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct RecentSessionEntry {
    pub id: String,
    pub tracked_app_id: String,
    pub display_name: String,
    pub process_name: String,
    pub platform: String,
    pub device_name: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub foreground_ms: i64,
    pub idle_ms: i64,
    pub window_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct UsageAnalyticsSnapshot {
    pub generated_at: String,
    pub summary: UsageSummary,
    pub daily_trend: Vec<DailyTrendPoint>,
    pub hourly_heatmap: Vec<HourlyHeatmapCell>,
    pub top_apps: Vec<TopAppEntry>,
    pub recent_sessions: Vec<RecentSessionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct UsageProcessCandidate {
    pub tracked_app_id: String,
    pub display_name: String,
    pub exe_path: Option<String>,
    pub process_name: Option<String>,
    pub last_seen_at: Option<String>,
    pub session_count: i64,
    pub binding_match_type: String,
    pub binding_match_value: String,
    pub binding_normalized_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct UsageGamePlaytimeBinding {
    pub game_id: String,
    pub game_name: String,
    pub match_type: String,
    pub match_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct UsageGamePlaytimeAggregate {
    pub game_id: String,
    pub game_name: String,
    pub total_seconds: i64,
    pub session_count: i64,
    pub last_played: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct UsageGameDailyTotal {
    pub date: String,
    pub seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct UsageGameRangeTotal {
    pub game_id: String,
    pub game_name: String,
    pub seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct UsageGamePlaytimeSummary {
    pub aggregates: Vec<UsageGamePlaytimeAggregate>,
    pub daily_totals: Vec<UsageGameDailyTotal>,
    pub per_game_totals: Vec<UsageGameRangeTotal>,
}

// ---------------------------------------------------------------------------
// Sync entity (snake_case JSON for protocol compatibility)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct SyncEntity {
    /// Entity type: "todo", "project", "area", "tag", "heading",
    /// "tracked_app", "usage_session", or "usage_event".
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts-rs", ts(rename = "type"))]
    pub entity_type: String,
    pub id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "Record<string, unknown>"))]
    pub data: serde_json::Map<String, Value>,
    pub hlc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
}

// ---------------------------------------------------------------------------
// Version vector
// ---------------------------------------------------------------------------

pub type VersionVector = std::collections::HashMap<String, String>;

// ---------------------------------------------------------------------------
// Peer record (snake_case JSON for protocol compatibility)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "../../../../packages/ark/src/generated/")
)]
pub struct PeerRecord {
    pub device_id: String,
    pub device_name: String,
    pub addresses: Vec<String>,
    pub last_seen: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_address: Option<String>,
}
