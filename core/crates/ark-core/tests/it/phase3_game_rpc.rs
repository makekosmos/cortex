#![allow(clippy::unwrap_used)]
use ark_core::canonical_types::game::{
    get_game, list_games, upsert_game, GameLink, GameLocalState, GameQuarantine, GameUpsertCommand,
};
use ark_core::db;
use ark_core::types::ArkObject;
use rusqlite::Connection;
use serde_json::json;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init_schema(&conn).unwrap();
    conn
}

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

fn target(id: &str, type_id: &str) -> ArkObject {
    ArkObject {
        id: id.into(),
        type_id: type_id.into(),
        type_version: "1.0.0".into(),
        title: "target".into(),
        content_json: json!({}),
        props_json: json!({}),
        created_at: "c".into(),
        updated_at: "u".into(),
        deleted_at: None,
    }
}

#[test]
fn upsert_get_list_projection_contains_canonical_local_and_quarantine() {
    let conn = setup();
    let record = upsert_game(&conn, command("g", "a"), "fallback")
        .unwrap()
        .record;
    assert_eq!(record.id, "g");
    assert_eq!(record.local.source.as_deref(), Some("steam"));
    assert_eq!(record.quarantine.fields, vec!["legacy"]);
    assert_eq!(get_game(&conn, "g", "a").unwrap(), record);
    assert_eq!(list_games(&conn, "a").unwrap().len(), 1);
    assert!(list_games(&conn, "b").unwrap()[0].local.source.is_none());
}

#[test]
fn device_context_isolation_uses_exact_device_key() {
    let conn = setup();
    upsert_game(&conn, command("g", "a"), "fallback").unwrap();
    let mut other = command("g", "b");
    other.local.source = Some("gog".into());
    upsert_game(&conn, other, "fallback").unwrap();
    assert_eq!(
        get_game(&conn, "g", "a").unwrap().local.source.as_deref(),
        Some("steam")
    );
    assert_eq!(
        get_game(&conn, "g", "b").unwrap().local.source.as_deref(),
        Some("gog")
    );
    assert_eq!(
        conn.query_row::<i64, _, _>(
            "SELECT COUNT(*) FROM object_local_state WHERE object_id='g'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        2
    );
}

#[test]
fn quarantine_selection_is_exact_contract_not_arbitrary_row() {
    let conn = setup();
    upsert_game(&conn, command("g", "a"), "fallback").unwrap();
    conn.execute("INSERT INTO object_migration_quarantine(object_id,contract_version,source_type_id,fields_json,source_hash,updated_at) VALUES('g','other','x','{\"fields\":[\"wrong\"]}','h','z')", []).unwrap();
    assert_eq!(
        get_game(&conn, "g", "a").unwrap().quarantine.fields,
        vec!["legacy"]
    );
}

#[test]
fn invalid_game_rejects_without_object_hlc_or_vector_mutation() {
    let conn = setup();
    let mut invalid = command("bad", "a");
    invalid.user_rating = Some(f64::NAN);
    assert!(upsert_game(&conn, invalid, "fallback").is_err());
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM objects", [], |r| r.get(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row::<i64, _, _>(
            "SELECT COUNT(*) FROM sync_kv WHERE key='lan_sync.version_vector'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn wrong_link_target_rolls_back_object_and_local_state() {
    let conn = setup();
    db::upsert_object(&conn, &target("note", "com.kosmos.task")).unwrap();
    let mut value = command("g", "a");
    value.links = vec![GameLink {
        id: "l".into(),
        link_type: "note".into(),
        target_object_id: "note".into(),
    }];
    assert!(upsert_game(&conn, value, "fallback").is_err());
    assert!(db::get_object(&conn, "g").unwrap().is_none());
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM object_local_state", [], |r| r.get(0))
            .unwrap(),
        0
    );
}

#[test]
fn replace_removes_stale_owned_links() {
    let conn = setup();
    db::upsert_object(&conn, &target("n1", "com.kosmos.note")).unwrap();
    db::upsert_object(&conn, &target("n2", "com.kosmos.note")).unwrap();
    let mut first = command("g", "a");
    first.links = vec![GameLink {
        id: "l1".into(),
        link_type: "note".into(),
        target_object_id: "n1".into(),
    }];
    upsert_game(&conn, first, "fallback").unwrap();
    let mut second = command("g", "a");
    second.links = vec![GameLink {
        id: "l2".into(),
        link_type: "note".into(),
        target_object_id: "n2".into(),
    }];
    let record = upsert_game(&conn, second, "fallback").unwrap().record;
    assert_eq!(
        record
            .links
            .iter()
            .map(|l| l.id.as_str())
            .collect::<Vec<_>>(),
        vec!["l2"]
    );
    assert!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM object_links WHERE id='l1'", [], |r| r
            .get(0))
            .unwrap()
            == 0
    );
}

#[test]
fn stored_local_json_has_no_device_ambiguity() {
    let conn = setup();
    upsert_game(&conn, command("g", "device-a"), "fallback").unwrap();
    let raw: String = conn
        .query_row(
            "SELECT data_json FROM object_local_state WHERE object_id='g' AND device_id='device-a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!raw.contains("device"));
    assert!(serde_json::from_str::<GameLocalState>(&raw).is_ok());
}

#[test]
fn identical_mutation_is_noop_without_sync_state_changes() {
    let conn = setup();
    let first = upsert_game(&conn, command("g", "a"), "fallback").unwrap();
    let before_vector: String = conn
        .query_row(
            "SELECT value FROM sync_kv WHERE key='lan_sync.version_vector'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let before_hlc: String = conn
        .query_row(
            "SELECT hlc FROM object_sync_versions WHERE object_id='g'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let second = upsert_game(&conn, command("g", "a"), "fallback").unwrap();
    assert!(first.changed);
    assert!(!second.changed);
    assert_eq!(second.record, first.record);
    let after_vector: String = conn
        .query_row(
            "SELECT value FROM sync_kv WHERE key='lan_sync.version_vector'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let after_hlc: String = conn
        .query_row(
            "SELECT hlc FROM object_sync_versions WHERE object_id='g'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(before_vector, after_vector);
    assert_eq!(before_hlc, after_hlc);
}
