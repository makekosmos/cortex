#![allow(clippy::unwrap_used)]
use ark_core::canonical_types::game::{GameLocalState, GameQuarantine, GameUpsertCommand};
use serde_json::json;

fn command(id: &str, device: &str) -> GameUpsertCommand {
    GameUpsertCommand {
        id: id.into(),
        title: "Game".into(),
        play_status: Some("notStarted".into()),
        user_rating: Some(8.5),
        genres: vec!["RPG".into()],
        platforms: vec!["PC".into()],
        released: None,
        description: Some("desc".into()),
        extensions: json!({"rawg":{"id":42}}),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
        deleted_at: None,
        links: vec![],
        local: GameLocalState {
            source: Some("steam".into()),
            ..Default::default()
        },
        quarantine: GameQuarantine {
            fields: vec!["legacy".into()],
            ..Default::default()
        },
        device_id: Some(device.into()),
    }
}

#[tokio::test]
async fn game_upsert_rpc_uses_actor_and_publishes_only_changed_events() {
    use std::time::Duration;

    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("game-rpc.db");
    let service = ark_core::service::ArkService::open(db_path.to_string_lossy())
        .await
        .unwrap();
    let mut events = service.subscribe();
    let params = json!({
        "game": command("rpc-game", "device-a"),
        "deviceId": "fallback-device"
    });

    let created = service
        .call("canonical.game.upsert", params.clone())
        .await
        .unwrap();
    assert_eq!(created["changed"], true);
    let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .expect("change event arrives")
        .expect("event receiver remains open");
    assert_eq!(event, json!({"event":"arrancador.changed"}));

    let unchanged = service.call("canonical.game.upsert", params).await.unwrap();
    assert_eq!(unchanged["changed"], false);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), events.recv())
            .await
            .is_err()
    );
}
