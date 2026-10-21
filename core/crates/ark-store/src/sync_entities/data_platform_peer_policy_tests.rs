#![allow(clippy::unwrap_used)]

use ark_application::sync_policy::{digest_sync_policy, SyncProfile, SyncProfileRule};
use ark_domain::selective_sync::{FilterV1, SyncMode};
use ark_domain::types::VersionVector;
use rusqlite::Connection;

use crate::test_support::TestDatabase;

fn policy(device_id: &str, active: bool, resource_id: &str, mode: SyncMode) -> SyncProfile {
    SyncProfile {
        profile_id: format!("profile-{device_id}"),
        device_id: device_id.into(),
        revision: 1,
        name: "test policy".into(),
        active,
        rules: vec![SyncProfileRule {
            resource_kind: "type".into(),
            resource_id: resource_id.into(),
            mode,
            filter: FilterV1::match_all(),
        }],
    }
}

fn insert_policy(conn: &Connection, peer_id: &str, policy: &SyncProfile) {
    crate::data_platform::accept_peer_sync_policy(
        conn,
        peer_id,
        policy.revision,
        &digest_sync_policy(&policy.rules),
        policy.active,
        &policy.rules,
    )
    .unwrap();
}

#[test]
fn accepted_peer_policies_project_snapshot_pages_per_peer() {
    let database = TestDatabase::create().unwrap();
    let conn = database.open_connection().unwrap();
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

    let note_policy = policy("peer-a", true, "com.example.note", SyncMode::Full);
    let game_policy = policy("peer-b", false, "com.example.game", SyncMode::Metadata);
    insert_policy(&conn, "peer-a", &note_policy);
    insert_policy(&conn, "peer-b", &game_policy);

    let note_accepted = crate::data_platform::load_accepted_peer_sync_policy(&conn, "peer-a")
        .unwrap()
        .unwrap();
    let peer_a = crate::sync_entities::read_policy_bound_page(
        &conn,
        "peer-a",
        &note_accepted.identity(),
        &VersionVector::default(),
        "test-device",
        0,
        100,
    )
    .unwrap();
    assert!(peer_a.entities.iter().any(|entity| entity.id == "note"));
    assert!(!peer_a.entities.iter().any(|entity| entity.id == "game"));

    let game_accepted = crate::data_platform::load_accepted_peer_sync_policy(&conn, "peer-b")
        .unwrap()
        .unwrap();
    let peer_b = crate::sync_entities::read_policy_bound_page(
        &conn,
        "peer-b",
        &game_accepted.identity(),
        &VersionVector::default(),
        "test-device",
        0,
        100,
    )
    .unwrap();
    assert!(peer_b.entities.iter().any(|entity| entity.id == "note"));
    assert!(peer_b.entities.iter().any(|entity| entity.id == "game"));
    let game = peer_b
        .entities
        .iter()
        .find(|entity| entity.id == "game")
        .unwrap();
    assert!(game.data.contains_key("contentJson"));
    assert!(!game.data.contains_key("localState"));

    let loaded =
        crate::sync_entities::collect_entities(&conn, &VersionVector::default(), "test-device")
            .unwrap();
    assert!(loaded.iter().any(|entity| entity.id == "note"));
    assert!(loaded.iter().any(|entity| entity.id == "game"));
    assert!(loaded
        .iter()
        .any(|entity| entity.entity_type == "object_link"));
    assert_eq!(
        crate::sync_entities::collect_entities_page(
            &conn,
            &VersionVector::default(),
            "test-device",
            0,
            2,
        )
        .unwrap()
        .len(),
        2
    );
    let note = loaded.iter().find(|entity| entity.id == "note").unwrap();
    let first_apply = conn.unchecked_transaction().unwrap();
    crate::sync_entities::apply_inbound_entity(&first_apply, note).unwrap();
    first_apply.commit().unwrap();
    let repeated_apply = conn.unchecked_transaction().unwrap();
    crate::sync_entities::apply_inbound_entity(&repeated_apply, note).unwrap();
    repeated_apply.commit().unwrap();
}
