#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArkObjectWrite {
    pub id: String,
    pub type_id: String,
    #[serde(default)]
    pub type_version: Option<String>,
    pub title: String,
    #[serde(default = "default_content_json")]
    pub content_json: Value,
    #[serde(default = "default_props_json")]
    pub props_json: Value,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

impl ArkObjectWrite {
    pub fn with_type_version(self, type_version: String) -> ArkObject {
        ArkObject {
            id: self.id,
            type_id: self.type_id,
            type_version,
            title: self.title,
            content_json: self.content_json,
            props_json: self.props_json,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub object_type_summaries: Vec<crate::type_registry::TypeSummary>,
    pub object_type_versions: Vec<crate::type_registry::TypeVersion>,
    pub object_type_aliases: Vec<crate::type_registry::AliasRecord>,
    pub object_links: Vec<ObjectLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub tracked_app_count: i64,
    pub session_count: i64,
    pub event_count: i64,
    /// Visible wall-clock time across all sessions — raw data, NOT "time
    /// used" (it counts background-visible windows and is what inflated
    /// headlines pre-KOS-287). Consumers presenting a total must read
    /// `total_foreground_ms`.
    pub total_runtime_ms: i64,
    /// The headline total: ACTIVE time — focused and not idle (KOS-287).
    pub total_foreground_ms: i64,
    pub total_idle_ms: i64,
    pub first_recorded_at: Option<String>,
    pub last_recorded_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DailyTrendPoint {
    pub date: String,
    pub foreground_ms: i64,
    pub idle_ms: i64,
    pub sessions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HourlyHeatmapCell {
    pub weekday: i64,
    pub hour: i64,
    pub foreground_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TopAppEntry {
    pub id: String,
    pub display_name: String,
    pub process_name: String,
    pub normalized_path: String,
    pub icon_ref: Option<String>,
    pub runtime_ms: i64,
    pub foreground_ms: i64,
    pub idle_ms: i64,
    pub sessions: i64,
    pub last_seen_at: Option<String>,
    /// Row's exe lives under %SystemRoot% — OS noise (explorer, sihost, …).
    /// UI hides these by default behind a toggle.
    pub is_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecentSessionEntry {
    pub id: String,
    pub tracked_app_id: String,
    pub display_name: String,
    pub process_name: String,
    pub platform: String,
    pub device_name: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub runtime_ms: i64,
    pub foreground_ms: i64,
    pub idle_ms: i64,
    pub window_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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
pub struct UsageGamePlaytimeBinding {
    pub game_id: String,
    pub game_name: String,
    pub match_type: String,
    pub match_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageGamePlaytimeAggregate {
    pub game_id: String,
    pub game_name: String,
    pub total_seconds: i64,
    pub session_count: i64,
    pub last_played: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageGameDailyTotal {
    pub date: String,
    pub seconds: i64,
}
