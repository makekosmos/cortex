use super::*;
use serde_json::json;
use std::collections::HashMap;

fn command() -> GameUpsertCommand {
    GameUpsertCommand {
        id: "game-1".into(),
        title: "Game".into(),
        play_status: Some("notStarted".into()),
        user_rating: Some(8.0),
        genres: vec!["RPG".into()],
        platforms: vec!["PC".into()],
        released: None,
        description: Some("Description".into()),
        extensions: json!({}),
        created_at: "created".into(),
        updated_at: "updated".into(),
        deleted_at: None,
        links: vec![],
        local: GameLocalState::default(),
        quarantine: GameQuarantine::default(),
        device_id: Some("command-device".into()),
    }
}

#[test]
fn command_wire_shape_remains_camel_case_and_typed() {
    let value = serde_json::to_value(command()).expect("game command serializes");

    assert!(value.get("playStatus").is_some());
    assert!(value.get("play_status").is_none());
    assert!(value.get("propsJson").is_none());
}

#[test]
fn planner_uses_command_device_and_returns_a_complete_write_plan() {
    let plan = plan_game_upsert(
        command(),
        "fallback-device",
        GameUpsertFacts {
            current: None,
            link_targets: HashMap::new(),
        },
    )
    .expect("valid game plans");

    let GameUpsertDecision::Apply(plan) = plan else {
        panic!("new game requires persistence");
    };
    assert_eq!(plan.effective_device_id, "command-device");
    assert_eq!(plan.object.type_id, GAME_TYPE_ID);
    assert_eq!(plan.local, GameLocalState::default());
    assert_eq!(plan.created_at, "created");
}

#[test]
fn planner_returns_unchanged_record_without_a_write_plan() {
    let command = command();
    let current = GameRecord {
        id: command.id.clone(),
        title: command.title.clone(),
        play_status: command.play_status.clone(),
        user_rating: command.user_rating,
        genres: command.genres.clone(),
        platforms: command.platforms.clone(),
        released: command.released.clone(),
        description: command.description.clone(),
        extensions: command.extensions.clone(),
        created_at: command.created_at.clone(),
        updated_at: command.updated_at.clone(),
        deleted_at: command.deleted_at.clone(),
        links: command.links.clone(),
        local: command.local.clone(),
        quarantine: command.quarantine.clone(),
        usage: GameUsage::default(),
    };
    let decision = plan_game_upsert(
        command,
        "fallback-device",
        GameUpsertFacts {
            current: Some(current.clone()),
            link_targets: HashMap::new(),
        },
    )
    .expect("valid no-op plan");

    assert!(matches!(decision, GameUpsertDecision::Unchanged(record) if record == current));
}

#[test]
fn planner_returns_typed_validation_errors_with_stable_wire_text() {
    let mut invalid_rating = command();
    invalid_rating.user_rating = Some(f64::NAN);
    let error = plan_game_upsert(
        invalid_rating,
        "fallback-device",
        GameUpsertFacts {
            current: None,
            link_targets: HashMap::new(),
        },
    )
    .expect_err("non-finite ratings are rejected");
    assert_eq!(error, GameUpsertError::InvalidRating);
    assert_eq!(
        error.to_string(),
        "invalid_request:canonical_field:/userRating"
    );

    let mut missing_target = command();
    missing_target.links.push(GameLink {
        id: "missing-link".into(),
        link_type: "note".into(),
        target_object_id: "missing-note".into(),
    });
    let error = plan_game_upsert(
        missing_target,
        "fallback-device",
        GameUpsertFacts {
            current: None,
            link_targets: HashMap::new(),
        },
    )
    .expect_err("links to missing objects are rejected");
    assert_eq!(error, GameUpsertError::LinkTargetNotFound);
    assert_eq!(error.to_string(), "link_target_not_found");
}
