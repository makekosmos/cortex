#![allow(clippy::unwrap_used)]

use ark_domain::hlc::HLC;
use ark_domain::types::{
    ArkObject, ObjectType, SyncEntity, UsageDay, UsageSpanWrite, UsageTitleTotal,
    VersionVector,
};
use rusqlite::Connection;
use serde_json::json;


fn setup_db() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .unwrap();
    crate::open::init_schema(&connection).unwrap();
    connection
}

fn make_object(id: &str, type_id: &str, title: &str) -> ArkObject {
    ArkObject {
        id: id.into(),
        type_id: type_id.into(),
        type_version: "0.0.0-legacy".into(),
        title: title.into(),
        content_json: json!({"type":"doc","content":[{"type":"paragraph"}]}),
        props_json: json!({"description":format!("Description for {title}")}),
        created_at: "2026-01-01T00:00:00.000Z".into(),
        updated_at: "2026-01-01T00:00:00.000Z".into(),
        deleted_at: None,
    }
}

fn make_object_type(id: &str, name: &str) -> ObjectType {
    ObjectType {
        id: id.into(),
        name: name.into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "2026-01-01T00:00:00.000Z".into(),
        updated_at: "2026-01-01T00:00:00.000Z".into(),
        system_locked: false,
    }
}
#[test]
fn upsert_object_type_nests_in_outer_transaction() {
    let conn = setup_db();
    let ot = make_object_type("ot-nested", "РўРёРї РІР»РѕР¶РµРЅРЅС‹Р№");
    // РћС‚РєСЂС‹РІР°РµРј РІРЅРµС€РЅСЋСЋ С‚СЂР°РЅР·Р°РєС†РёСЋ вЂ” РёРјРёС‚РёСЂСѓРµРј РІС‹Р·РѕРІ
    // РёР· with_write_tx.
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    // Р”Рѕ Р¤Р°Р·С‹ B РїР°РґР°РµС‚ С‡РµСЂРµР· replay_pending_for_type в†’ BEGIN IMMEDIATE:
    // "cannot start a transaction within a transaction".
    // РџРѕСЃР»Рµ Р¤Р°Р·С‹ B (SAVEPOINT) РґРѕР»Р¶РЅРѕ РїСЂРѕР№С‚Рё Р±РµР· РѕС€РёР±РєРё.
    crate::type_registry::upsert_object_type(&conn, &ot).expect(concat!(
        "upsert_object_type РґРѕР»Р¶РµРЅ СЂР°Р±РѕС‚Р°С‚СЊ РІРЅСѓС‚СЂРё ",
        "РІРЅРµС€РЅРµР№ С‚СЂР°РЅР·Р°РєС†РёРё"
    ));
    conn.execute_batch("COMMIT").unwrap();
    // РџСЂРѕРІРµСЂСЏРµРј, С‡С‚Рѕ С‚РёРї Р·Р°РїРёСЃР°РЅ.
    let types = crate::object_types::list(&conn).unwrap();
    assert!(
        types.iter().any(|t| t.id == "ot-nested"),
        "object_type РґРѕР»Р¶РµРЅ Р±С‹С‚СЊ Р·Р°РїРёСЃР°РЅ"
    );
}

#[test]
fn compact_usage_span_is_idempotent_and_dictionary_encoded() {
    let conn = setup_db();
    let mut write = UsageSpanWrite {
        device_id: "device-compact".to_string(),
        started_at_unix: 1_767_225_600,
        ended_at_unix: 1_767_225_630,
        tracked_app_id: "browser".to_string(),
        window_title: Some("Facebook".to_string()),
        flags: 0,
        updated_at: "2026-01-01T00:00:30.000Z".to_string(),
    };
    crate::usage_queries::upsert_usage_span(&conn, &write).unwrap();
    write.ended_at_unix += 30;
    crate::usage_queries::upsert_usage_span(&conn, &write).unwrap();

    let day = crate::usage::load_usage_day(&conn, "usage-day:device-compact:2026-01-01")
        .unwrap()
        .unwrap();
    assert_eq!(day.payload_json["a"], json!(["browser"]));
    assert_eq!(day.payload_json["t"], json!(["Facebook"]));
    assert_eq!(day.payload_json["s"], json!([[0, 60, 0, 0, 0]]));
    assert_eq!(
        crate::usage_queries::get_usage_title_total(&conn, "face").unwrap(),
        UsageTitleTotal {
            active_seconds: 60,
            idle_seconds: 0,
        }
    );
}

#[test]
fn compact_usage_span_splits_at_utc_midnight_and_syncs() {
    let source = setup_db();
    let write = UsageSpanWrite {
        device_id: "device-midnight".to_string(),
        started_at_unix: 1_767_311_990,
        ended_at_unix: 1_767_312_010,
        tracked_app_id: "reader".to_string(),
        window_title: Some("Book".to_string()),
        flags: 1,
        updated_at: "2026-01-02T00:00:10.000Z".to_string(),
    };
    let days = crate::usage_queries::upsert_usage_span(&source, &write).unwrap();
    assert_eq!(days.len(), 2);
    crate::usage_sync_log::ensure_usage_sequence_migrated(&source, "source-device").unwrap();

    let entities =
        crate::sync_entities::collect_entities(&source, &VersionVector::new(), "source-device")
            .unwrap();
    let usage_days = entities
        .iter()
        .filter(|entity| entity.entity_type == "usage_day")
        .collect::<Vec<_>>();
    assert_eq!(usage_days.len(), 2);

    let peer = setup_db();
    for entity in usage_days {
        crate::sync_entities::apply_inbound_entity(&peer, entity).unwrap();
    }
    assert_eq!(
        peer.query_row("SELECT COUNT(*) FROM usage_days", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn ac8_local_state_is_device_scoped_and_never_exported_or_synced() {
    let source = setup_db();
    let mut object = make_object("ac8-object", "note_obj", "AC8 object");
    object.props_json = json!({"description": "SHARED_MARKER", "extensions": {}});
    crate::objects::upsert_object_compat(&source, &object).unwrap();
    source
        .execute(
            "INSERT INTO object_local_state(object_id,device_id,data_json,updated_at) VALUES
                 ('ac8-object','device-a','{\"localMarker\":\"LOCAL_A\"}','now'),
                 ('ac8-object','device-b','{\"localMarker\":\"LOCAL_B\"}','now')",
            [],
        )
        .unwrap();

    assert_eq!(
        source
            .query_row(
                "SELECT COUNT(*) FROM object_local_state WHERE object_id='ac8-object'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        2
    );
    let exported = serde_json::to_string(&crate::load_all::load_all(&source).unwrap()).unwrap();
    assert!(exported.contains("SHARED_MARKER"));
    assert!(!exported.contains("LOCAL_A"));
    assert!(!exported.contains("LOCAL_B"));

    let entity =
        crate::sync_entities::collect_entities(&source, &VersionVector::new(), "source-device")
            .unwrap()
            .into_iter()
            .find(|entity| entity.entity_type == "object" && entity.id == "ac8-object")
            .expect("canonical object should be exported for sync");
    let sync_payload = serde_json::to_string(&entity).unwrap();
    assert!(sync_payload.contains("SHARED_MARKER"));
    assert!(!sync_payload.contains("LOCAL_A"));
    assert!(!sync_payload.contains("LOCAL_B"));

    let peer = setup_db();
    crate::sync_entities::apply_inbound_entity(&peer, &entity).unwrap();
    assert_eq!(
        peer.query_row("SELECT COUNT(*) FROM object_local_state", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let peer_props: String = peer
        .query_row(
            "SELECT props_json FROM objects WHERE id='ac8-object'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(peer_props.contains("SHARED_MARKER"));
    assert!(!peer_props.contains("LOCAL_A"));
    assert!(!peer_props.contains("LOCAL_B"));
}

#[test]
fn usage_sequence_cursor_waits_for_missing_entries() {
    let conn = setup_db();

    crate::usage_sync_log::record_usage_sequence(
        &conn,
        "usage_day",
        "day-2",
        "peer-device",
        2,
        "2026-01-02T00:00:00.000Z:000002:peer-device",
        false,
    )
    .unwrap();
    let vector: VersionVector = serde_json::from_str(
        &crate::sync_kv::get_sync_kv(&conn, crate::VERSION_VECTOR_KEY)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(vector.get("@usage:peer-device"), Some(&"0".to_string()));

    crate::usage_sync_log::record_usage_sequence(
        &conn,
        "usage_day",
        "day-1",
        "peer-device",
        1,
        "2026-01-01T00:00:00.000Z:000001:peer-device",
        false,
    )
    .unwrap();
    let vector: VersionVector = serde_json::from_str(
        &crate::sync_kv::get_sync_kv(&conn, crate::VERSION_VECTOR_KEY)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(vector.get("@usage:peer-device"), Some(&"2".to_string()));
}

#[test]
fn local_usage_versions_use_one_device_cursor() {
    let conn = setup_db();
    let first = crate::sync_versions::bump_sync_version_vector(
        &conn,
        "usage_day",
        "day-a",
        "local-device",
        false,
    )
    .unwrap();
    let second = crate::sync_versions::bump_sync_version_vector(
        &conn,
        "usage_day",
        "day-b",
        "local-device",
        false,
    )
    .unwrap();

    let vector: VersionVector = serde_json::from_str(
        &crate::sync_kv::get_sync_kv(&conn, crate::VERSION_VECTOR_KEY)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(vector.get("@usage:local-device"), Some(&"2".to_string()));
    assert!(!vector.contains_key("day-a"));
    assert!(!vector.contains_key("day-b"));
    assert!(HLC::is_newer(&second, &first));
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM usage_sync_log", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap(),
        2
    );
}

#[test]
fn legacy_usage_replay_keeps_one_synthetic_sequence() {
    let conn = setup_db();
    let day = UsageDay {
        id: "usage-day:legacy:2026-01-01".to_string(),
        device_id: "legacy".to_string(),
        day: "2026-01-01".to_string(),
        payload_json: json!({"a": ["browser"], "t": ["Example"], "s": [[0, 30, 0, 0, 0]]}),
        updated_at: "2026-01-01T00:00:30.000Z".to_string(),
    };
    let mut data = serde_json::to_value(&day)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    data.remove("id");
    let entity = SyncEntity {
        entity_type: "usage_day".to_string(),
        id: day.id.clone(),
        data,
        hlc: "2026-01-01T00:00:30.000Z:000007:old-peer".to_string(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    };

    crate::sync_entities::apply_inbound_entity(&conn, &entity).unwrap();
    crate::sync_entities::apply_inbound_entity(&conn, &entity).unwrap();

    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM usage_sync_log", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap(),
        1
    );
    let vector: VersionVector = serde_json::from_str(
        &crate::sync_kv::get_sync_kv(&conn, crate::VERSION_VECTOR_KEY)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        vector.get("@usage:legacy:old-peer").map(String::as_str),
        Some("1")
    );
    assert!(!vector.contains_key(&day.id));
}
