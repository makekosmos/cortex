use ark_domain::types::ArkObjectWrite;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

pub const GAME_TYPE_ID: &str = "core.game";
pub const GAME_VERSION: &str = "0.1.0";
pub const GAME_QUARANTINE_CONTRACT: &str = "game-rpc-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameLocalState {
    pub source: Option<String>,
    pub source_app_id: Option<String>,
    pub install_dir: Option<String>,
    pub exe_path: Option<String>,
    pub save_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameProviderRef {
    pub provider: String,
    pub provider_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameQuarantine {
    pub provider_refs: Vec<GameProviderRef>,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GameUsage {
    pub seconds: i64,
    pub session_count: u64,
    pub last_played_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameLink {
    pub id: String,
    pub link_type: String,
    pub target_object_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameRecord {
    pub id: String,
    pub title: String,
    pub play_status: Option<String>,
    pub user_rating: Option<f64>,
    pub genres: Vec<String>,
    pub platforms: Vec<String>,
    pub released: Option<String>,
    pub description: Option<String>,
    pub extensions: Value,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub links: Vec<GameLink>,
    pub local: GameLocalState,
    pub quarantine: GameQuarantine,
    pub usage: GameUsage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameProperties {
    pub play_status: Option<String>,
    pub user_rating: Option<f64>,
    pub genres: Vec<String>,
    pub platforms: Vec<String>,
    pub released: Option<String>,
    pub description: Option<String>,
    pub extensions: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameMutationResult {
    pub record: GameRecord,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameUpsertCommand {
    pub id: String,
    pub title: String,
    pub play_status: Option<String>,
    pub user_rating: Option<f64>,
    pub genres: Vec<String>,
    pub platforms: Vec<String>,
    pub released: Option<String>,
    pub description: Option<String>,
    pub extensions: Value,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub links: Vec<GameLink>,
    pub local: GameLocalState,
    pub quarantine: GameQuarantine,
    #[serde(default)]
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameUpsertError {
    InvalidRating,
    DefinitionUnavailable,
    CanonicalField(String),
    CanonicalDefinition(String),
    InvalidLinkType(String),
    LinkTargetNotFound,
    LinkTargetWrongType,
    LinkCardinalityViolation,
}

impl fmt::Display for GameUpsertError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRating => {
                formatter.write_str("invalid_request:canonical_field:/userRating")
            }
            Self::DefinitionUnavailable => formatter.write_str("definition_invariant:game"),
            Self::CanonicalField(pointer) => {
                write!(formatter, "invalid_request:canonical_field:{pointer}")
            }
            Self::CanonicalDefinition(pointer) => {
                write!(formatter, "invalid_request:canonical_definition:{pointer}")
            }
            Self::InvalidLinkType(link_type) => write!(formatter, "invalid_link_type:{link_type}"),
            Self::LinkTargetNotFound => formatter.write_str("link_target_not_found"),
            Self::LinkTargetWrongType => formatter.write_str("link_target_wrong_type"),
            Self::LinkCardinalityViolation => formatter.write_str("link_cardinality_violation"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameLinkTarget {
    pub type_id: String,
    pub deleted: bool,
}

#[derive(Debug, Clone)]
pub struct GameUpsertFacts {
    pub current: Option<GameRecord>,
    pub link_targets: HashMap<String, GameLinkTarget>,
}

#[derive(Debug, Clone)]
pub struct GameUpsertPlan {
    pub object: ArkObjectWrite,
    pub effective_device_id: String,
    pub local: GameLocalState,
    pub quarantine: GameQuarantine,
    pub remove_link_ids: Vec<String>,
    pub links: Vec<GameLink>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub enum GameUpsertDecision {
    Unchanged(GameRecord),
    Apply(GameUpsertPlan),
}
