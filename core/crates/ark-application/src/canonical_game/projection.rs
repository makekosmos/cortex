use super::types::*;
use serde_json::{json, Value};

pub fn read_game_props(object: &ark_domain::types::ArkObject) -> Result<GameProperties, String> {
    use ark_domain::canonical_type::validation::{validate_canonical, CanonicalValidationCode};

    let props = object.props_json.as_object().ok_or("invalid_game_props")?;
    let registration = ark_domain::canonical_type::definitions::canonical_type_registrations()
        .map_err(|_| "definition_invariant:game".to_owned())?
        .into_iter()
        .find(|registration| {
            registration.type_id == GAME_TYPE_ID && registration.version == GAME_VERSION
        })
        .ok_or("definition_invariant:game")?;
    validate_canonical(&registration, &object.props_json, &object.content_json).map_err(
        |error| {
            format!(
                "invalid_request:{}:{}",
                match error.code {
                    CanonicalValidationCode::InvalidField => "canonical_field",
                    CanonicalValidationCode::InvariantViolation => "canonical_definition",
                },
                error.pointer
            )
        },
    )?;
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
    Ok(GameProperties {
        play_status: props
            .get("playStatus")
            .and_then(Value::as_str)
            .map(str::to_owned),
        user_rating: props.get("userRating").and_then(Value::as_f64),
        genres: strings("genres"),
        platforms: strings("platforms"),
        released: props
            .get("released")
            .and_then(Value::as_str)
            .map(str::to_owned),
        description: props
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned),
        extensions: props
            .get("extensions")
            .cloned()
            .unwrap_or_else(|| json!({})),
    })
}
