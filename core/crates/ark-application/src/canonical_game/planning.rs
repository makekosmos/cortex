use super::types::*;
use ark_domain::types::ArkObjectWrite;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub fn game_props(command: &GameUpsertCommand) -> Value {
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

pub fn object_write(command: &GameUpsertCommand) -> ArkObjectWrite {
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

pub fn validate_game_command(command: &GameUpsertCommand) -> Result<(), GameUpsertError> {
    use ark_domain::canonical_type::validation::{validate_canonical, CanonicalValidationCode};

    if !command.user_rating.map(f64::is_finite).unwrap_or(true) {
        return Err(GameUpsertError::InvalidRating);
    }
    let registration = ark_domain::canonical_type::definitions::canonical_type_registrations()
        .map_err(|_| GameUpsertError::DefinitionUnavailable)?
        .into_iter()
        .find(|registration| {
            registration.type_id == GAME_TYPE_ID && registration.version == GAME_VERSION
        })
        .ok_or(GameUpsertError::DefinitionUnavailable)?;
    validate_canonical(&registration, &game_props(command), &json!({})).map_err(|error| match error
        .code
    {
        CanonicalValidationCode::InvalidField => GameUpsertError::CanonicalField(error.pointer),
        CanonicalValidationCode::InvariantViolation => {
            GameUpsertError::CanonicalDefinition(error.pointer)
        }
    })
}

pub fn plan_game_upsert(
    command: GameUpsertCommand,
    fallback_device_id: &str,
    facts: GameUpsertFacts,
) -> Result<GameUpsertDecision, GameUpsertError> {
    validate_game_command(&command)?;
    validate_game_links(&command.links, &facts.link_targets)?;
    let effective_device_id = command
        .device_id
        .as_deref()
        .unwrap_or(fallback_device_id)
        .to_owned();
    if let Some(current) = facts.current.as_ref() {
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
            return Ok(GameUpsertDecision::Unchanged(current.clone()));
        }
    }
    let desired: HashSet<_> = command.links.iter().map(|link| link.id.as_str()).collect();
    let remove_link_ids = facts
        .current
        .into_iter()
        .flat_map(|record| record.links)
        .filter(|link| {
            ["cover-image", "background-image", "note", "task", "tag"]
                .contains(&link.link_type.as_str())
                && !desired.contains(link.id.as_str())
        })
        .map(|link| link.id)
        .collect();
    Ok(GameUpsertDecision::Apply(GameUpsertPlan {
        object: object_write(&command),
        effective_device_id,
        local: command.local,
        quarantine: command.quarantine,
        remove_link_ids,
        links: command.links,
        created_at: command.created_at,
        updated_at: command.updated_at,
    }))
}

fn validate_game_links(
    links: &[GameLink],
    targets: &HashMap<String, GameLinkTarget>,
) -> Result<(), GameUpsertError> {
    let definitions = [
        ("cover-image", "com.kosmos.image", 1usize),
        ("background-image", "com.kosmos.image", 1usize),
        ("note", "com.kosmos.note", usize::MAX),
        ("task", "core.task", usize::MAX),
        ("tag", "com.kosmos.tag", usize::MAX),
    ];
    let mut counts = HashMap::<&str, usize>::new();
    for link in links {
        let (_, target_type, max) = definitions
            .iter()
            .find(|(kind, _, _)| *kind == link.link_type)
            .ok_or_else(|| GameUpsertError::InvalidLinkType(link.link_type.clone()))?;
        let target = targets
            .get(&link.target_object_id)
            .ok_or(GameUpsertError::LinkTargetNotFound)?;
        if target.type_id != *target_type || target.deleted {
            return Err(GameUpsertError::LinkTargetWrongType);
        }
        let count = counts.entry(link.link_type.as_str()).or_default();
        *count += 1;
        if *count > *max {
            return Err(GameUpsertError::LinkCardinalityViolation);
        }
    }
    Ok(())
}
