use ark_application::canonical_game::{
    plan_game_upsert, read_game_props, GameLink, GameLinkTarget, GameLocalState,
    GameMutationResult, GameQuarantine, GameRecord, GameUpsertCommand, GameUpsertDecision,
    GameUpsertError, GameUpsertFacts, GameUsage, GAME_QUARANTINE_CONTRACT, GAME_TYPE_ID,
    GAME_VERSION,
};
use ark_domain::types::{ArkObject, ObjectLink};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Debug)]
pub enum CanonicalGameOperationError {
    Storage(String),
    Invalid(GameUpsertError),
}

impl fmt::Display for CanonicalGameOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => formatter.write_str(error),
            Self::Invalid(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl From<String> for CanonicalGameOperationError {
    fn from(error: String) -> Self {
        Self::Storage(error)
    }
}

fn links_for(conn: &Connection, id: &str) -> Result<Vec<GameLink>, String> {
    Ok(crate::objects::list_object_links_for_source(conn, id)?
        .into_iter()
        .map(|link| GameLink {
            id: link.id,
            link_type: link.link_type,
            target_object_id: link.target_object_id,
        })
        .collect())
}

fn usage_for(conn: &Connection, id: &str) -> Result<GameUsage, String> {
    let summary =
        crate::usage_queries::summarize_linked_app_usage(conn, id, &["game", "game-usage"])?;
    Ok(GameUsage {
        seconds: summary.seconds.max(0),
        session_count: summary.session_count.max(0) as u64,
        last_played_at: summary.last_played_at,
    })
}

fn quarantine_for(conn: &Connection, id: &str) -> Result<GameQuarantine, String> {
    let raw =
        crate::objects::get_migration_quarantine_fields_json(conn, id, GAME_QUARANTINE_CONTRACT)?;
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
    let properties = read_game_props(&object)?;
    Ok(GameRecord {
        id: object.id.clone(),
        title: object.title,
        play_status: properties.play_status,
        user_rating: properties.user_rating,
        genres: properties.genres,
        platforms: properties.platforms,
        released: properties.released,
        description: properties.description,
        extensions: properties.extensions,
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
    let raw = crate::objects::get_object_local_state_json(conn, id, device_id)?;
    Ok(raw
        .map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_default())
}

pub fn list_games(conn: &Connection, device_id: &str) -> Result<Vec<GameRecord>, String> {
    crate::objects::list_objects_by_type(conn, GAME_TYPE_ID).and_then(|objects| {
        objects
            .into_iter()
            .map(|o| project_game(conn, o, device_id))
            .collect()
    })
}

pub fn get_game(conn: &Connection, id: &str, device_id: &str) -> Result<GameRecord, String> {
    let object =
        crate::objects::get_object(conn, id)?.ok_or_else(|| "game_not_found".to_string())?;
    project_game(conn, object, device_id)
}

pub fn apply_upsert(
    connection: &mut Connection,
    command: GameUpsertCommand,
    fallback_device_id: &str,
) -> Result<GameMutationResult, crate::writer::TransactionFailure<CanonicalGameOperationError>> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(crate::writer::TransactionFailure::Begin)?;
    let result = upsert_game_in(&transaction, command, fallback_device_id)
        .map_err(crate::writer::TransactionFailure::Operation)?;
    transaction
        .commit()
        .map_err(crate::writer::TransactionFailure::Commit)?;
    Ok(result)
}

fn upsert_game_in(
    conn: &Transaction<'_>,
    command: GameUpsertCommand,
    fallback_device_id: &str,
) -> Result<GameMutationResult, CanonicalGameOperationError> {
    let current = crate::objects::get_object(conn, &command.id)?
        .map(|object| {
            let effective_device_id = command.device_id.as_deref().unwrap_or(fallback_device_id);
            project_game(conn, object, effective_device_id)
        })
        .transpose()?;
    let mut link_targets = std::collections::HashMap::new();
    for link in &command.links {
        if link_targets.contains_key(&link.target_object_id) {
            continue;
        }
        if let Some(object) = crate::objects::get_object(conn, &link.target_object_id)? {
            link_targets.insert(
                link.target_object_id.clone(),
                GameLinkTarget {
                    type_id: object.type_id,
                    deleted: object.deleted_at.is_some(),
                },
            );
        }
    }
    let decision = plan_game_upsert(
        command,
        fallback_device_id,
        GameUpsertFacts {
            current,
            link_targets,
        },
    )
    .map_err(CanonicalGameOperationError::Invalid)?;
    let plan = match decision {
        GameUpsertDecision::Unchanged(record) => {
            return Ok(GameMutationResult {
                record,
                changed: false,
            });
        }
        GameUpsertDecision::Apply(plan) => plan,
    };

    let prepared = crate::object_write::prepare_object(conn, plan.object)
        .map_err(|error| error.to_string())?;
    crate::objects::upsert_object_compat(conn, &prepared)?;
    crate::objects::upsert_object_local_state_json(
        conn,
        &prepared.id,
        &plan.effective_device_id,
        &serde_json::to_string(&plan.local).map_err(|error| error.to_string())?,
        &plan.updated_at,
    )?;
    crate::objects::delete_migration_quarantine(conn, &prepared.id, GAME_QUARANTINE_CONTRACT)?;
    if !plan.quarantine.provider_refs.is_empty() || !plan.quarantine.fields.is_empty() {
        let fields = serde_json::to_string(&json!({
            "providerRefs": plan.quarantine.provider_refs,
            "fields": plan.quarantine.fields
        }))
        .map_err(|error| error.to_string())?;
        let source_hash = format!("{:x}", Sha256::digest(fields.as_bytes()));
        crate::objects::upsert_migration_quarantine(
            conn,
            &prepared.id,
            GAME_QUARANTINE_CONTRACT,
            GAME_TYPE_ID,
            &fields,
            &source_hash,
            &plan.updated_at,
        )?;
    }

    for link_id in &plan.remove_link_ids {
        crate::objects::delete_object_link(conn, link_id)?;
    }
    for link in &plan.links {
        crate::objects::upsert_object_link_preserving_created_at(
            conn,
            &ObjectLink {
                id: link.id.clone(),
                source_object_id: prepared.id.clone(),
                target_object_id: link.target_object_id.clone(),
                link_type: link.link_type.clone(),
                created_at: plan.created_at.clone(),
            },
        )?;
    }
    let hlc = crate::sync_versions::bump_sync_version_vector(
        conn,
        "object",
        &prepared.id,
        &plan.effective_device_id,
        false,
    )?;
    crate::sync_versions::set_object_sync_version_state(conn, &prepared.id, &hlc, false)?;
    Ok(GameMutationResult {
        record: get_game(conn, &prepared.id, &plan.effective_device_id)?,
        changed: true,
    })
}
