use ark_core::db::{
    backup_to_file, clear_all, get_object, get_objects_by_ids, init_schema, insert_pending_object,
    list_object_summaries, list_object_summaries_by_type, list_objects, list_objects_by_type,
    list_running_time_entries, load_all, upsert_object, upsert_object_link, upsert_object_type,
};
use ark_core::type_registry::{register_alias, AliasRecord};
use ark_core::types::{ArkObject, ObjectType, SyncEntity};
use rusqlite::{Connection, OptionalExtension};
use serde_json::json;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    conn
}

fn phase2_state_snapshot(conn: &Connection) -> serde_json::Value {
    let object_types: Vec<_> = conn
        .prepare("SELECT id,name,schema_json,ui_schema_json,created_at,updated_at,system_locked,COALESCE(current_version,''),COALESCE(status,'') FROM object_types ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok(json!([row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, i64>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?]))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let versions: Vec<_> = conn
        .prepare("SELECT type_id,version,schema_json,ui_schema_json,schema_hash,created_at FROM object_type_versions ORDER BY type_id,version")
        .unwrap()
        .query_map([], |row| Ok(json!([row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?])))
        .unwrap().collect::<Result<_, _>>().unwrap();
    let aliases: Vec<_> = conn
        .prepare(
            "SELECT alias,canonical_type_id,created_at FROM object_type_aliases ORDER BY alias",
        )
        .unwrap()
        .query_map([], |row| {
            Ok(json!([
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?
            ]))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let objects: Vec<_> = conn
        .prepare("SELECT id,type_id,type_version,title,content_json,props_json,created_at,updated_at,deleted_at FROM objects ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok(json!([row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, String>(6)?, row.get::<_, String>(7)?, row.get::<_, Option<String>>(8)?])))
        .unwrap().collect::<Result<_, _>>().unwrap();
    let links: Vec<_> = conn
        .prepare("SELECT id,source_object_id,target_object_id,link_type,created_at FROM object_links ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok(json!([row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?])))
        .unwrap().collect::<Result<_, _>>().unwrap();
    let sync_kv: Vec<_> = conn
        .prepare("SELECT key,value FROM sync_kv ORDER BY key")
        .unwrap()
        .query_map([], |row| {
            Ok(json!([row.get::<_, String>(0)?, row.get::<_, String>(1)?]))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    json!({"objectTypes": object_types, "versions": versions, "aliases": aliases, "objects": objects, "links": links, "syncKv": sync_kv})
}

#[test]
fn legacy_upsert_rejects_alias_canonical_collision_atomically() {
    let conn = setup();
    upsert_object_type(
        &conn,
        &ObjectType {
            id: "type-a".into(),
            name: "A".into(),
            schema_json: "{}".into(),
            ui_schema_json: "{}".into(),
            created_at: "a-created".into(),
            updated_at: "a-updated".into(),
            system_locked: false,
        },
    )
    .unwrap();
    register_alias(
        &conn,
        &AliasRecord {
            alias: "foo".into(),
            canonical_type_id: "type-a".into(),
            created_at: "alias-created".into(),
        },
    )
    .unwrap();
    let type_version: String = conn
        .query_row(
            "SELECT current_version FROM object_types WHERE id='type-a'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    for id in ["object-a", "object-b"] {
        upsert_object(
            &conn,
            &ArkObject {
                id: id.into(),
                type_id: "type-a".into(),
                type_version: type_version.clone(),
                title: id.into(),
                content_json: json!({"body": id}),
                props_json: json!({"source": "collision-test"}),
                created_at: "object-created".into(),
                updated_at: "object-updated".into(),
                deleted_at: None,
            },
        )
        .unwrap();
    }
    upsert_object_link(
        &conn,
        &ark_core::types::ObjectLink {
            id: "link-a-b".into(),
            source_object_id: "object-a".into(),
            target_object_id: "object-b".into(),
            link_type: "related".into(),
            created_at: "link-created".into(),
        },
    )
    .unwrap();
    ark_core::db::bump_sync_version_vector(
        &conn,
        "object",
        "object-a",
        "collision-test-device",
        false,
    )
    .unwrap();
    let before = phase2_state_snapshot(&conn);
    let error = upsert_object_type(
        &conn,
        &ObjectType {
            id: "foo".into(),
            name: "Collision".into(),
            schema_json: "{\"collision\":true}".into(),
            ui_schema_json: "{}".into(),
            created_at: "collision-created".into(),
            updated_at: "collision-updated".into(),
            system_locked: false,
        },
    )
    .unwrap_err();
    assert!(error.contains("alias"), "unexpected error: {error}");
    assert_eq!(phase2_state_snapshot(&conn), before);
}

#[test]
fn persisted_object_and_summary_carry_exact_type_version() {
    let conn = setup();
    upsert_object_type(
        &conn,
        &ObjectType {
            id: "slice2_type".into(),
            name: "Slice 2".into(),
            schema_json: "{}".into(),
            ui_schema_json: "{}".into(),
            created_at: "created".into(),
            updated_at: "updated".into(),
            system_locked: false,
        },
    )
    .unwrap();

    upsert_object(
        &conn,
        &ArkObject {
            id: "object-1".into(),
            type_id: "slice2_type".into(),
            type_version: "0.0.0-legacy".into(),
            title: "Versioned".into(),
            content_json: json!({"body": "x"}),
            props_json: json!({}),
            created_at: "created".into(),
            updated_at: "updated".into(),
            deleted_at: None,
        },
    )
    .unwrap();

    assert_eq!(
        get_object(&conn, "object-1").unwrap().unwrap().type_version,
        "0.0.0-legacy"
    );
    assert_eq!(list_objects(&conn).unwrap()[0].type_version, "0.0.0-legacy");
    assert_eq!(
        list_object_summaries(&conn).unwrap()[0].type_version,
        "0.0.0-legacy"
    );
    assert_eq!(
        list_objects_by_type(&conn, "slice2_type").unwrap()[0].type_version,
        "0.0.0-legacy"
    );
    assert_eq!(
        list_object_summaries_by_type(&conn, "slice2_type").unwrap()[0].type_version,
        "0.0.0-legacy"
    );
    assert_eq!(
        get_objects_by_ids(&conn, &["object-1".into()]).unwrap()[0].type_version,
        "0.0.0-legacy"
    );
    assert!(list_running_time_entries(&conn, None).unwrap().is_empty());
}

#[test]
fn pending_same_id_keeps_newest_hlc_and_complete_payload() {
    let conn = setup();
    let newer = SyncEntity {
        entity_type: "object".into(),
        id: "pending-1".into(),
        data: serde_json::from_value(json!({
            "typeId": "missing",
            "typeVersion": "1.0.0",
            "title": "newer",
            "contentJson": {"v": 2},
            "propsJson": {},
            "createdAt": "c",
            "updatedAt": "u",
            "deletedAt": null
        }))
        .unwrap(),
        hlc: "2026-08-11T00:00:00.000Z:000002:new".into(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    };
    let older = SyncEntity {
        data: serde_json::from_value(json!({
            "typeId": "other-missing",
            "typeVersion": "2.0.0",
            "title": "older",
            "contentJson": {"v": 1},
            "propsJson": {},
            "createdAt": "c",
            "updatedAt": "u",
            "deletedAt": null
        }))
        .unwrap(),
        hlc: "2026-08-10T00:00:00.000Z:000099:old".into(),
        ..newer.clone()
    };
    insert_pending_object(&conn, &newer, "missing").unwrap();
    insert_pending_object(&conn, &older, "other-missing").unwrap();
    let row: (String, String, String) = conn
        .query_row(
            "SELECT payload, awaited_type_id, awaited_type_version FROM sync_pending_objects WHERE id='pending-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert!(row.0.contains("newer"));
    assert_eq!(row.1, "missing");
    assert_eq!(row.2, "1.0.0");
}