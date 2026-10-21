use serde::{Deserialize, Serialize};

// Read models for usage analytics responses.

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageGameRangeTotal {
    pub game_id: String,
    pub game_name: String,
    pub seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageGamePlaytimeSummary {
    pub aggregates: Vec<UsageGamePlaytimeAggregate>,
    pub daily_totals: Vec<UsageGameDailyTotal>,
    pub per_game_totals: Vec<UsageGameRangeTotal>,
}
