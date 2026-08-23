//! Core-owned typed Game RPC contract and persistence facade.
//!
//! The generic object tables remain an implementation detail here.  Consumers
//! receive this projection and never construct or inspect `ArkObject`/props.

use crate::canonical_types::ingress::prepare_object;
use crate::db;

use crate::types::{ArkObject, ArkObjectWrite};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

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

fn validate_game_command(_conn: &Connection, command: &GameUpsertCommand) -> Result<(), String> {
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

fn validate_game_links(conn: &Connection, game_id: &str, links: &[GameLink]) -> Result<(), String> {
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

fn object_write(command: &GameUpsertCommand) -> ArkObjectWrite {
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

fn read_game_props(
    object: &ArkObject,
) -> Result<
    (
        Option<String>,
        Option<f64>,
        Vec<String>,
        Vec<String>,
        Option<String>,
        Option<String>,
        Value,
    ),
    String,
> {
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

fn links_for(conn: &Connection, id: &str) -> Result<Vec<GameLink>, String> {
    Ok(db::list_object_links(conn)?
        .into_iter()
        .filter(|link| link.source_object_id == id)
        .map(|link| GameLink {
            id: link.id,
            link_type: link.link_type,
            target_object_id: link.target_object_id,
        })
        .collect())
}

fn usage_for(conn: &Connection, id: &str) -> Result<GameUsage, String> {
    let row: Option<(i64, i64, Option<String>)> = conn.query_row(
        "SELECT COALESCE(SUM(CAST((julianday(COALESCE(s.ended_at,s.started_at))-julianday(s.started_at))*86400 AS INTEGER)),0), COUNT(s.id), MAX(COALESCE(s.ended_at,s.started_at)) FROM usage_sessions s JOIN object_links l ON l.target_object_id=s.tracked_app_id WHERE l.source_object_id=?1 AND l.link_type IN ('game','game-usage')",
        params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().map_err(|e| e.to_string())?;
    let (seconds, count, last) = row.unwrap_or((0, 0, None));
    Ok(GameUsage {
        seconds: seconds.max(0),
        session_count: count.max(0) as u64,
        last_played_at: last,
    })
}

fn quarantine_for(conn: &Connection, id: &str) -> Result<GameQuarantine, String> {
    let raw: Option<String> = conn.query_row("SELECT fields_json FROM object_migration_quarantine WHERE object_id=?1 AND contract_version=?2", params![id, GAME_QUARANTINE_CONTRACT], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    let value = raw
        .map(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_else(|| json!({}));
    let provider_refs = value
        .get("providerRefs")
        .or_else(|| value.get("provider_refs"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    let fields = value
        .get("fields")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok(GameQuarantine {
        provider_refs,
        fields,
    })
}

pub fn project_game(
    conn: &Connection,
    object: ArkObject,
    device_id: &str,
) -> Result<GameRecord, String> {
    if object.type_id != GAME_TYPE_ID {
        return Err("wrong_type".into());
    }
    if object.type_version != GAME_VERSION {
        return Err("wrong_version".into());
    }
    let (play_status, user_rating, genres, platforms, released, description, extensions) =
        read_game_props(&object)?;
    Ok(GameRecord {
        id: object.id.clone(),
        title: object.title,
        play_status,
        user_rating,
        genres,
        platforms,
        released,
        description,
        extensions,
        created_at: object.created_at,
        updated_at: object.updated_at,
        deleted_at: object.deleted_at,
        links: links_for(conn, &object.id)?,
        local: local_for(conn, &object.id, device_id)?,
        quarantine: quarantine_for(conn, &object.id)?,
        usage: usage_for(conn, &object.id)?,
    })
}

fn local_for(conn: &Connection, id: &str, device_id: &str) -> Result<GameLocalState, String> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT data_json FROM object_local_state WHERE object_id=?1 AND device_id=?2",
            params![id, device_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(raw
        .map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_default())
}

pub fn list_games(conn: &Connection, device_id: &str) -> Result<Vec<GameRecord>, String> {
    db::list_objects_by_type(conn, GAME_TYPE_ID).and_then(|objects| {
        objects
            .into_iter()
            .map(|o| project_game(conn, o, device_id))
            .collect()
    })
}

pub fn get_game(conn: &Connection, id: &str, device_id: &str) -> Result<GameRecord, String> {
    let object = db::get_object(conn, id)?.ok_or_else(|| "game_not_found".to_string())?;
    project_game(conn, object, device_id)
}

pub fn upsert_game(
    conn: &Connection,
    command: GameUpsertCommand,
    device_id: &str,
) -> Result<GameMutationResult, String> {
    conn.execute_batch("SAVEPOINT canonical_game_upsert")
        .map_err(|e| e.to_string())?;
    let effective_device = command.device_id.as_deref().unwrap_or(device_id).to_owned();
    let result = (|| {
        validate_game_command(conn, &command)?;
        validate_game_links(conn, &command.id, &command.links)?;
        if let Some(existing) = db::get_object(conn, &command.id)? {
            let current = project_game(conn, existing, &effective_device)?;
            let same = current.title == command.title
                && current.play_status == command.play_status
                && current.user_rating == command.user_rating
                && current.genres == command.genres
                && current.platforms == command.platforms
                && current.released == command.released
                && current.description == command.description
                && current.extensions == command.extensions
                && current.created_at == command.created_at
                && current.updated_at == command.updated_at
                && current.deleted_at == command.deleted_at
                && current.links == command.links
                && current.local == command.local
                && current.quarantine == command.quarantine;
            if same {
                return Ok(GameMutationResult {
                    record: current,
                    changed: false,
                });
            }
        }
        let prepared = prepare_object(conn, object_write(&command)).map_err(|e| e.to_string())?;
        db::upsert_object(conn, &prepared)?;
        conn.execute("INSERT INTO object_local_state(object_id,device_id,data_json,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(object_id,device_id) DO UPDATE SET data_json=excluded.data_json,updated_at=excluded.updated_at", params![command.id, effective_device, serde_json::to_string(&command.local).map_err(|e| e.to_string())?, command.updated_at]).map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM object_migration_quarantine WHERE object_id=?1 AND contract_version=?2",
            params![command.id, GAME_QUARANTINE_CONTRACT],
        )
        .map_err(|e| e.to_string())?;
        if !command.quarantine.provider_refs.is_empty() || !command.quarantine.fields.is_empty() {
            let fields = serde_json::to_string(&json!({"providerRefs": command.quarantine.provider_refs, "fields": command.quarantine.fields})).map_err(|e| e.to_string())?;
            let source_hash = format!("{:x}", Sha256::digest(fields.as_bytes()));
            conn.execute("INSERT INTO object_migration_quarantine(object_id,contract_version,source_type_id,fields_json,source_hash,updated_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(object_id,contract_version) DO UPDATE SET fields_json=excluded.fields_json,source_hash=excluded.source_hash,updated_at=excluded.updated_at", params![command.id, GAME_QUARANTINE_CONTRACT, GAME_TYPE_ID, fields, source_hash, command.updated_at]).map_err(|e| e.to_string())?;
        }
        let desired: std::collections::HashSet<String> =
            command.links.iter().map(|l| l.id.clone()).collect();
        for existing in db::list_object_links(conn)?.into_iter().filter(|l| {
            l.source_object_id == command.id
                && ["cover-image", "background-image", "note", "task", "tag"]
                    .contains(&l.link_type.as_str())
        }) {
            if !desired.contains(&existing.id) {
                db::delete_object_link(conn, &existing.id)?;
            }
        }
        for link in &command.links {
            conn.execute("INSERT INTO object_links(id,source_object_id,target_object_id,link_type,created_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET target_object_id=excluded.target_object_id,link_type=excluded.link_type", params![link.id, command.id, link.target_object_id, link.link_type, command.created_at]).map_err(|e| e.to_string())?;
        }
        let hlc =
            db::bump_sync_version_vector(conn, "object", &command.id, &effective_device, false)?;
        conn.execute("INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0", params![command.id, hlc]).map_err(|e| e.to_string())?;
        Ok(GameMutationResult {
            record: get_game(conn, &command.id, &effective_device)?,
            changed: true,
        })
    })();
    match result {
        Ok(record) => {
            conn.execute_batch("RELEASE SAVEPOINT canonical_game_upsert")
                .map_err(|e| e.to_string())?;
            Ok(record)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT canonical_game_upsert; RELEASE SAVEPOINT canonical_game_upsert");
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_contract_is_typed_and_camel_case() {
        let command = GameUpsertCommand {
            id: "g".into(),
            title: "Game".into(),
            play_status: None,
            user_rating: None,
            genres: vec![],
            platforms: vec![],
            released: None,
            description: None,
            extensions: json!({}),
            created_at: "c".into(),
            updated_at: "u".into(),
            deleted_at: None,
            links: vec![],
            local: Default::default(),
            quarantine: Default::default(),
            device_id: None,
        };
        let value = serde_json::to_value(command).unwrap();
        assert!(value.get("playStatus").is_some());
        assert!(value.get("play_status").is_none());
        assert!(value.get("propsJson").is_none());
    }
}
