#![allow(clippy::unwrap_used)]
use ark_application::sync_storage::{SyncEntityMutationPort, SyncEntityReadPort, SyncReadClass};
use ark_store::test_support::TestDatabase;
use std::collections::HashMap;
use std::sync::Arc;

use super::SqliteStorageBackend;

#[tokio::test]
async fn accepted_peer_policies_project_snapshot_pages_per_peer() {
    let database = TestDatabase::create().unwrap();
    let conn = database.open_connection().unwrap();
    ark_store::open::init_schema(&conn).unwrap();
    for (id, name) in [("com.example.note", "Note"), ("com.example.game", "Game")] {
        conn.execute(
            concat!(
                "INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,",
                "updated_at) VALUES(?1,?2,'{}','{}','t','t')"
            ),
            rusqlite::params![id, name],
        )
        .unwrap();
    }
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
            "created_at,updated_at) VALUES('note','com.example.note','1.0.0','Note',",
            "'{\"body\":\"full\"}','{}','t','t')"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
            "created_at,updated_at) VALUES('game','com.example.game','1.0.0','Game',",
            "'{\"body\":\"heavy\"}','{\"localState\":{\"path\":\"C:/game\"}}','t','t')"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO object_links(id,source_object_id,target_object_id,link_type,",
            "created_at) VALUES('link','note','game','related','t')"
        ),
        [],
    )
    .unwrap();
    let note_profile = ark_application::sync_policy::SyncProfile {
        profile_id: "note-only".into(),
        device_id: "peer-a".into(),
        revision: 1,
        name: "Notes".into(),
        active: true,
        rules: vec![ark_application::sync_policy::SyncProfileRule {
            resource_kind: "type".into(),
            resource_id: "com.example.note".into(),
            mode: ark_domain::selective_sync::SyncMode::Full,
            filter: ark_domain::selective_sync::FilterV1::match_all(),
        }],
    };
    let game_profile = ark_application::sync_policy::SyncProfile {
        profile_id: "game-metadata".into(),
        device_id: "peer-b".into(),
        revision: 1,
        name: "Game metadata".into(),
        active: false,
        rules: vec![ark_application::sync_policy::SyncProfileRule {
            resource_kind: "type".into(),
            resource_id: "com.example.game".into(),
            mode: ark_domain::selective_sync::SyncMode::Metadata,
            filter: ark_domain::selective_sync::FilterV1::match_all(),
        }],
    };
    ark_store::data_platform::accept_peer_sync_policy(
        &conn,
        "peer-a",
        1,
        &note_profile.digest(),
        note_profile.active,
        &note_profile.rules,
    )
    .unwrap();
    ark_store::data_platform::accept_peer_sync_policy(
        &conn,
        "peer-b",
        1,
        &game_profile.digest(),
        game_profile.active,
        &game_profile.rules,
    )
    .unwrap();
    drop(conn);
    let backend = Arc::new(
        SqliteStorageBackend::open_for_test(
            database.path().to_string_lossy().into_owned(),
            "test-device",
        )
        .await
        .unwrap(),
    );
    let peer_a = backend
        .load_entities_page_for_peer("peer-a", SyncReadClass::SyncLive, &HashMap::new(), 0, 100)
        .await
        .unwrap();
    let peer_b = backend
        .load_entities_page_for_peer("peer-b", SyncReadClass::SyncLive, &HashMap::new(), 0, 100)
        .await
        .unwrap();
    assert!(peer_a.iter().any(|entity| entity.id == "note"));
    assert!(!peer_a.iter().any(|entity| entity.id == "game"));
    assert!(peer_b.iter().any(|entity| entity.id == "game"));
    assert!(peer_b.iter().any(|entity| entity.id == "note"));
    let game = peer_b.iter().find(|entity| entity.id == "game").unwrap();
    assert!(game.data.contains_key("contentJson"));
    assert!(!game.data.contains_key("localState"));

    let live_note = peer_a.iter().find(|entity| entity.id == "note").unwrap();
    assert!(backend
        .filter_outgoing_entity_for_peer("peer-a", live_note)
        .await
        .unwrap()
        .is_some());
    assert!(backend
        .filter_outgoing_entity_for_peer("peer-b", live_note)
        .await
        .unwrap()
        .is_some());

    let loaded = backend.load_entities(&HashMap::new()).await.unwrap();
    assert!(loaded.iter().any(|entity| entity.id == "note"));
    assert!(loaded.iter().any(|entity| entity.id == "game"));
    assert!(loaded
        .iter()
        .any(|entity| entity.entity_type == "object_link"));
    assert_eq!(
        backend
            .load_entities_page(&HashMap::new(), 0, 2)
            .await
            .unwrap()
            .len(),
        2
    );
    backend
        .apply_entity(loaded.iter().find(|entity| entity.id == "note").unwrap())
        .await
        .unwrap();
    backend
        .apply_entity(loaded.iter().find(|entity| entity.id == "note").unwrap())
        .await
        .unwrap();
}
