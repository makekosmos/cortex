#![allow(clippy::unwrap_used)]

use crate::canonical_game::{apply_upsert, get_game, list_games};
use ark_application::canonical_game::{
    GameLink, GameLocalState, GameQuarantine, GameUpsertCommand,
};
use ark_domain::types::{ArkObject, ObjectLink, TrackedApp, UsageSession};
use rusqlite::Connection;
use serde_json::json;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&conn).unwrap();
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

fn tracked_app(id: &str, path: &str) -> TrackedApp {
    TrackedApp {
        id: id.into(),
        platform: "windows".into(),
        exe_path: path.into(),
        normalized_exe_path: path.into(),
        process_name: "game.exe".into(),
        display_name: None,
        publisher: None,
        icon_ref: None,
        first_seen_at: "2026-01-01T00:00:00.000Z".into(),
        last_seen_at: "2026-01-01T00:00:00.000Z".into(),
    }
}

fn usage_session(id: &str, app_id: &str, foreground_ms: i64, idle_ms: i64) -> UsageSession {
    UsageSession {
        id: id.into(),
        tracked_app_id: app_id.into(),
        device_id: "d".into(),
        device_name: "D".into(),
        platform: "windows".into(),
        started_at: "2026-01-01T00:00:00.000Z".into(),
        ended_at: Some("2026-01-01T01:00:00.000Z".into()),
        runtime_ms: 3_600_000,
        foreground_ms,
        idle_ms,
        window_title: None,
        process_name: "game.exe".into(),
        exe_path: String::new(),
        pid_start: None,
        pid_end: None,
        meta_json: json!({}),
    }
}

fn target(id: &str, type_id: &str) -> ArkObject {
    ArkObject {
        id: id.into(),
        type_id: type_id.into(),
        type_version: "0.1.0".into(),
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
    let mut conn = setup();
    let record = apply_upsert(&mut conn, command("g", "a"), "fallback")
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
    let mut conn = setup();
    apply_upsert(&mut conn, command("g", "a"), "fallback").unwrap();
    let mut other = command("g", "b");
    other.local.source = Some("gog".into());
    apply_upsert(&mut conn, other, "fallback").unwrap();
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
    let mut conn = setup();
    apply_upsert(&mut conn, command("g", "a"), "fallback").unwrap();
    conn.execute(
        concat!(
            "INSERT INTO object_migration_quarantine(object_id,contract_version,",
            "source_type_id,fields_json,source_hash,updated_at) VALUES('g','other','x',",
            "'{\"fields\":[\"wrong\"]}','h','z')"
        ),
        [],
    )
    .unwrap();
    assert_eq!(
        get_game(&conn, "g", "a").unwrap().quarantine.fields,
        vec!["legacy"]
    );
}

#[test]
fn invalid_game_rejects_without_object_hlc_or_vector_mutation() {
    let mut conn = setup();
    let mut invalid = command("bad", "a");
    invalid.user_rating = Some(f64::NAN);
    assert!(apply_upsert(&mut conn, invalid, "fallback").is_err());
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
    let mut conn = setup();
    crate::objects::upsert_object_compat(&conn, &target("note", "core.task")).unwrap();
    let mut value = command("g", "a");
    value.links = vec![GameLink {
        id: "l".into(),
        link_type: "note".into(),
        target_object_id: "note".into(),
    }];
    assert!(apply_upsert(&mut conn, value, "fallback").is_err());
    assert!(crate::objects::get_object(&conn, "g").unwrap().is_none());
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM object_local_state", [], |r| r.get(0))
            .unwrap(),
        0
    );
}

#[test]
fn late_sync_version_failure_rolls_back_game_local_quarantine_and_links() {
    let mut conn = setup();
    crate::objects::upsert_object_compat(&conn, &target("note", "core.note")).unwrap();
    conn.execute_batch(
        "CREATE TRIGGER reject_game_sync_version
         BEFORE INSERT ON object_sync_versions
         BEGIN SELECT RAISE(ABORT, 'injected game sync failure'); END;",
    )
    .unwrap();
    let mut game = command("g", "a");
    game.links = vec![GameLink {
        id: "game-note".into(),
        link_type: "note".into(),
        target_object_id: "note".into(),
    }];

    assert!(apply_upsert(&mut conn, game, "fallback").is_err());
    assert!(crate::objects::get_object(&conn, "g").unwrap().is_none());
    let local_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM object_local_state WHERE object_id='g'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let quarantine_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM object_migration_quarantine WHERE object_id='g'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let link_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM object_links WHERE source_object_id='g'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(local_rows, 0);
    assert_eq!(quarantine_rows, 0);
    assert_eq!(link_rows, 0);
    let vectors: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sync_kv WHERE key='lan_sync.version_vector'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(vectors, 0);
}

#[test]
fn usage_projection_merges_game_executable_version_splits() {
    let mut conn = setup();
    apply_upsert(&mut conn, command("game-1", "a"), "fallback").unwrap();
    for (id, path) in [
        ("app-v1", "c:\\games\\demo\\app-1.0.1\\game.exe"),
        ("app-v2", "c:\\games\\demo\\app-1.0.2\\game.exe"),
    ] {
        crate::usage::upsert_tracked_app(&conn, &tracked_app(id, path)).unwrap();
        crate::objects::upsert_object_compat(&conn, &target(id, "core.image")).unwrap();
    }
    crate::usage::upsert_usage_session(&conn, &usage_session("s1", "app-v1", 1_800_000, 600_000))
        .unwrap();
    crate::usage::upsert_usage_session(&conn, &usage_session("s2", "app-v2", 600_000, 0)).unwrap();
    crate::objects::upsert_object_link(
        &conn,
        &ObjectLink {
            id: "game-app".into(),
            source_object_id: "game-1".into(),
            target_object_id: "app-v2".into(),
            link_type: "game".into(),
            created_at: "2026-01-01T00:00:00.000Z".into(),
        },
    )
    .unwrap();

    let usage = get_game(&conn, "game-1", "a").unwrap().usage;
    assert_eq!(usage.seconds, 2_400);
    assert_eq!(usage.session_count, 2);
    assert!(usage.last_played_at.is_some());
}

#[test]
fn replace_removes_stale_owned_links() {
    let mut conn = setup();
    crate::objects::upsert_object_compat(&conn, &target("n1", "core.note")).unwrap();
    crate::objects::upsert_object_compat(&conn, &target("n2", "core.note")).unwrap();
    let mut first = command("g", "a");
    first.links = vec![GameLink {
        id: "l1".into(),
        link_type: "note".into(),
        target_object_id: "n1".into(),
    }];
    apply_upsert(&mut conn, first, "fallback").unwrap();
    let mut second = command("g", "a");
    second.links = vec![GameLink {
        id: "l2".into(),
        link_type: "note".into(),
        target_object_id: "n2".into(),
    }];
    let record = apply_upsert(&mut conn, second, "fallback").unwrap().record;
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
    let mut conn = setup();
    apply_upsert(&mut conn, command("g", "device-a"), "fallback").unwrap();
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
    let mut conn = setup();
    let first = apply_upsert(&mut conn, command("g", "a"), "fallback").unwrap();
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
    let second = apply_upsert(&mut conn, command("g", "a"), "fallback").unwrap();
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
