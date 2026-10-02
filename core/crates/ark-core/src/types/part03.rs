
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

// ---------------------------------------------------------------------------
// Sync entity (snake_case JSON for protocol compatibility)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncEntity {
    /// Entity type: "todo", "project", "area", "tag", "heading",
    /// "tracked_app", "usage_session", or "usage_event".
    #[serde(rename = "type")]
    pub entity_type: String,
    pub id: String,
    pub data: serde_json::Map<String, Value>,
    pub hlc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_seq: Option<u64>,
}

// ---------------------------------------------------------------------------
// Version vector
// ---------------------------------------------------------------------------

pub type VersionVector = std::collections::HashMap<String, String>;

// ---------------------------------------------------------------------------
// Peer record (snake_case JSON for protocol compatibility)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerRecord {
    pub device_id: String,
    pub device_name: String,
    pub addresses: Vec<String>,
    pub last_seen: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_address: Option<String>,
}
