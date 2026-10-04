use crate::db;
use crate::types::{ArkObject, ArkObjectWrite};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// Core-owned typed Game RPC contract and persistence facade.
//
// The generic object tables remain an implementation detail here.  Consumers
// receive this projection and never construct or inspect `ArkObject`/props.

pub const GAME_TYPE_ID: &str = "com.kosmos.game";
pub const GAME_VERSION: &str = "1.0.0";
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

fn game_validation_error(
    error: crate::canonical_types::validation::CanonicalValidationError,
) -> String {
    format!(
        "invalid_request:{}:{}",
        match error.code {
            crate::canonical_types::validation::CanonicalValidationCode::InvalidField =>
                "canonical_field",
            crate::canonical_types::validation::CanonicalValidationCode::InvariantViolation =>
                "canonical_definition",
        },
        error.pointer
    )
}

pub(crate) fn validate_game_command(
    _conn: &Connection,
    command: &GameUpsertCommand,
) -> Result<(), String> {
    if !command.user_rating.map(|v| v.is_finite()).unwrap_or(true) {
        return Err("invalid_request:canonical_field:/userRating".into());
    }
    let registration = crate::canonical_types::definitions::canonical_type_registrations()?
        .into_iter()
        .find(|r| r.type_id == GAME_TYPE_ID && r.version == GAME_VERSION)
        .ok_or_else(|| "definition_invariant:game".to_string())?;
    crate::canonical_types::validation::validate_canonical(
        &registration,
        &game_props(command),
        &json!({}),
    )
    .map_err(game_validation_error)
}

pub(crate) fn validate_game_links(
    conn: &Connection,
    game_id: &str,
    links: &[GameLink],
) -> Result<(), String> {
    let definitions = [
        ("cover-image", "com.kosmos.image", 1usize),
        ("background-image", "com.kosmos.image", 1usize),
        ("note", "com.kosmos.note", usize::MAX),
        ("task", "com.kosmos.task", usize::MAX),
        ("tag", "com.kosmos.tag", usize::MAX),
    ];
    let mut counts = std::collections::HashMap::<&str, usize>::new();
    for link in links {
        let (_, target_type, max) = definitions
            .iter()
            .find(|(kind, _, _)| *kind == link.link_type)
            .ok_or_else(|| format!("invalid_link_type:{}", link.link_type))?;
        let target = db::get_object(conn, &link.target_object_id)?
            .ok_or_else(|| "link_target_not_found".to_string())?;
        if target.type_id != *target_type || target.deleted_at.is_some() {
            return Err("link_target_wrong_type".into());
        }
        let count = counts.entry(link.link_type.as_str()).or_default();
        *count += 1;
        if *count > *max {
            return Err("link_cardinality_violation".into());
        }
    }
    let _ = game_id;
    Ok(())
}

fn game_props(command: &GameUpsertCommand) -> Value {
    json!({
        "playStatus": command.play_status,
        "userRating": command.user_rating,
        "genres": command.genres,
        "platforms": command.platforms,
        "released": command.released,
        "description": command.description,
        "extensions": command.extensions,
    })
}

pub(crate) fn object_write(command: &GameUpsertCommand) -> ArkObjectWrite {
    ArkObjectWrite {
        id: command.id.clone(),
        type_id: GAME_TYPE_ID.into(),
        type_version: Some(GAME_VERSION.into()),
        title: command.title.clone(),
        content_json: json!({}),
        props_json: game_props(command),
        created_at: command.created_at.clone(),
        updated_at: command.updated_at.clone(),
        deleted_at: command.deleted_at.clone(),
    }
}

type GameProps = (
    Option<String>,
    Option<f64>,
    Vec<String>,
    Vec<String>,
    Option<String>,
    Option<String>,
    Value,
);

pub(crate) fn read_game_props(object: &ArkObject) -> Result<GameProps, String> {
    let props = object.props_json.as_object().ok_or("invalid_game_props")?;
    let registration = crate::canonical_types::definitions::canonical_type_registrations()
        .map_err(|_| "definition_invariant:game".to_string())?
        .into_iter()
        .find(|r| r.type_id == GAME_TYPE_ID && r.version == GAME_VERSION)
        .ok_or("definition_invariant:game")?;
    crate::canonical_types::validation::validate_canonical(
        &registration,
        &object.props_json,
        &object.content_json,
    )
    .map_err(game_validation_error)?;
    let play_status = props
        .get("playStatus")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let user_rating = props.get("userRating").and_then(Value::as_f64);
    let strings = |key: &str| -> Vec<String> {
        props
            .get(key)
            .and_then(Value::as_array)
            .unwrap_or(&vec![])
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>()
            .ok_or(())
            .unwrap_or_default()
            .into_iter()
            .map(str::to_owned)
            .collect()
    };
    Ok((
        play_status,
        user_rating,
        strings("genres"),
        strings("platforms"),
        props
            .get("released")
            .and_then(Value::as_str)
            .map(str::to_owned),
        props
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned),
        props
            .get("extensions")
            .cloned()
            .unwrap_or_else(|| json!({})),
    ))
}
