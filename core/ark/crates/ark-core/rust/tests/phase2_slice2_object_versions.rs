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

#[test]
fn load_all_clear_and_raw_backup_preserve_phase2_registry_contract() {
    let conn = setup();
    let user = ObjectType {
        id: "user-type".into(),
        name: "User".into(),
        schema_json: "{\"type\":\"object\"}".into(),
        ui_schema_json: "{}".into(),
        created_at: "c".into(),
        updated_at: "u".into(),
        system_locked: false,
    };
    let builtin = ObjectType {
        id: "builtin-type".into(),
        name: "Builtin".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "c".into(),
        updated_at: "u".into(),
        system_locked: true,
    };
    upsert_object_type(&conn, &user).unwrap();
    upsert_object_type(&conn, &builtin).unwrap();
    ark_core::type_registry::register_alias(
        &conn,
        &ark_core::type_registry::AliasRecord {
            alias: "user.alias".into(),
            canonical_type_id: user.id.clone(),
            created_at: "a".into(),
        },
    )
    .unwrap();
    upsert_object(
        &conn,
        &ArkObject {
            id: "user-object".into(),
            type_id: user.id.clone(),
            type_version: "0.0.0-legacy".into(),
            title: "x".into(),
            content_json: json!({"x":1}),
            props_json: json!({}),
            created_at: "c".into(),
            updated_at: "u".into(),
            deleted_at: None,
        },
    )
    .unwrap();
    let exported = load_all(&conn).unwrap();
    assert!(exported
        .object_type_summaries
        .iter()
        .any(|x| x.type_id == "user-type"));
    assert_eq!(
        exported
            .object_type_versions
            .iter()
            .filter(|x| x.type_id == "user-type")
            .count(),
        2
    );
    assert_eq!(
        exported
            .object_type_aliases
            .iter()
            .find(|alias| alias.alias == "user.alias")
            .map(|alias| alias.alias.as_str()),
        Some("user.alias")
    );
    assert_eq!(exported.objects[0].type_version, "0.0.0-legacy");
    let path = std::env::temp_dir().join(format!("ark-phase2-backup-{}.db", std::process::id()));
    backup_to_file(&conn, path.to_str().unwrap()).unwrap();
    let reopened = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        reopened
            .query_row(
                "SELECT COUNT(*) FROM object_type_versions WHERE type_id='user-type'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
    assert_eq!(
        reopened
            .query_row(
                "SELECT type_version FROM objects WHERE id='user-object'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "0.0.0-legacy"
    );
    assert_eq!(
        reopened
            .query_row(
                "PRAGMA foreign_key_check",
                [],
                |_| Ok::<_, rusqlite::Error>(())
            )
            .unwrap_or(()),
        ()
    );
    std::fs::remove_file(path).ok();
    clear_all(&conn).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM objects", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT name FROM object_types WHERE id='com.kosmos.note'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "Заметка"
    );
    assert_eq!(
        conn.query_row(
            "SELECT canonical_type_id FROM object_type_aliases WHERE alias='note_obj'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "com.kosmos.note"
    );
    assert_eq!(
        conn.query_row(
            "SELECT canonical_type_id FROM object_type_aliases WHERE alias='user.alias'",
            [],
            |r| r.get::<_, String>(0)
        )
        .optional()
        .unwrap(),
        None
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM object_types WHERE id='builtin-type'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM object_type_versions WHERE type_id='builtin-type'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
}

#[test]
fn legacy_object_type_reads_hide_deprecated_but_registry_keeps_definition() {
    let conn = setup();
    let object_type = ObjectType {
        id: "legacy-visible".into(),
        name: "Legacy".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "c".into(),
        updated_at: "u".into(),
        system_locked: false,
    };
    upsert_object_type(&conn, &object_type).unwrap();
    assert_eq!(
        ark_core::db::list_object_types(&conn)
            .unwrap()
            .iter()
            .filter(|x| x.id == object_type.id)
            .count(),
        1
    );
    ark_core::db::delete_object_type(&conn, &object_type.id).unwrap();
    assert!(!ark_core::db::list_object_types(&conn)
        .unwrap()
        .iter()
        .any(|x| x.id == object_type.id));
    assert!(ark_core::type_registry::list_type_summaries(&conn)
        .unwrap()
        .iter()
        .any(|x| x.type_id == object_type.id && x.status == "deprecated"));
    upsert_object_type(&conn, &object_type).unwrap();
    assert_eq!(
        ark_core::db::list_object_types(&conn)
            .unwrap()
            .iter()
            .filter(|x| x.id == object_type.id)
            .count(),
        1
    );
    let version: String = conn
        .query_row(
            "SELECT current_version FROM object_types WHERE id=?1",
            [&object_type.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(version.starts_with("0.0.0+legacy."));
    let listed = ark_core::db::list_object_types(&conn).unwrap();
    let listed_json = serde_json::to_value(&listed[0]).unwrap();
    assert_eq!(
        listed_json
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        [
            "createdAt",
            "id",
            "name",
            "schemaJson",
            "systemLocked",
            "uiSchemaJson",
            "updatedAt"
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>()
    );
    assert_eq!(
        serde_json::to_value(ark_core::db::get_object_type(&conn, &object_type.id).unwrap())
            .unwrap(),
        serde_json::to_value(Some(object_type.clone())).unwrap()
    );
    ark_core::db::delete_object_type(&conn, &object_type.id).unwrap();
    assert!(ark_core::db::get_object_type(&conn, &object_type.id)
        .unwrap()
        .is_none());
    assert!(ark_core::type_registry::list_type_summaries(&conn)
        .unwrap()
        .iter()
        .any(|x| x.type_id == object_type.id && x.status == "deprecated"));
    let original_hash: String = conn
        .query_row(
            "SELECT schema_hash FROM object_type_versions WHERE type_id=?1 AND version=?2",
            [&object_type.id, &version],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute(
        "UPDATE object_type_versions SET schema_hash=?1 WHERE type_id=?2 AND version=?3",
        rusqlite::params!["f".repeat(64), object_type.id, version],
    )
    .unwrap();
    assert!(upsert_object_type(&conn, &object_type).is_err());
    conn.execute(
        "UPDATE object_type_versions SET schema_hash=?1 WHERE type_id=?2 AND version=?3",
        rusqlite::params![original_hash, object_type.id, version],
    )
    .unwrap();
}

#[test]
fn load_all_json_is_additive_deterministic_and_hides_deprecated_legacy_types() {
    let conn = setup();
    for id in ["z-type", "a-type"] {
        upsert_object_type(
            &conn,
            &ObjectType {
                id: id.into(),
                name: id.into(),
                schema_json: "{}".into(),
                ui_schema_json: "{}".into(),
                created_at: "c".into(),
                updated_at: "u".into(),
                system_locked: false,
            },
        )
        .unwrap();
    }
    ark_core::db::delete_object_type(&conn, "z-type").unwrap();
    let value = serde_json::to_value(load_all(&conn).unwrap()).unwrap();
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert!(keys.iter().any(|k| k == "objectTypes"));
    assert!(keys.iter().any(|k| k == "objectTypeSummaries"));
    assert!(keys.iter().any(|k| k == "objectTypeVersions"));
    assert!(keys.iter().any(|k| k == "objectTypeAliases"));
    let legacy_ids = value["objectTypes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(!legacy_ids.contains(&"z-type"));
    let registry_ids = value["objectTypeSummaries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["typeId"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(registry_ids.contains(&"z-type"));
    assert!(registry_ids.windows(2).all(|w| w[0] <= w[1]));
}
