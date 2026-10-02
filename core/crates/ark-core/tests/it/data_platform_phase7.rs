#![allow(clippy::unwrap_used)]
use ark_core::{data_platform, db};
use rusqlite::Connection;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[test]
fn external_refs_are_additive_and_unique() {
    let conn = Connection::open_in_memory().unwrap();
    db::init_schema(&conn).unwrap();
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,created_at,updated_at) ",
            "VALUES('note-1','com.kosmos.note','1.0.0','Note','now','now')"
        ),
        [],
    )
    .unwrap();
    data_platform::upsert_external_ref(
        &conn,
        &data_platform::ExternalRefUpsert {
            connector_id: "obsidian".into(),
            account_id: "vault".into(),
            external_type: "markdown".into(),
            external_id: "notes/a.md".into(),
            object_id: "note-1".into(),
            revision: Some("1".into()),
            hash: Some("a".repeat(64)),
            state: "clean".into(),
        },
    )
    .unwrap();
    assert_eq!(
        conn.query_row("SELECT object_id FROM external_refs", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "note-1"
    );
    assert!(data_platform::upsert_external_ref(
        &conn,
        &data_platform::ExternalRefUpsert {
            connector_id: String::new(),
            account_id: "vault".into(),
            external_type: "markdown".into(),
            external_id: "x".into(),
            object_id: "note-1".into(),
            revision: None,
            hash: None,
            state: "clean".into(),
        },
    )
    .is_err());
}

#[test]
fn frozen_filter_v1_is_canonical_and_bounded() {
    let canonical = data_platform::validate_filter_json(
        r#"{"where":{"eq":{"value":null,"field":"title"}},"version":1}"#,
    )
    .unwrap();
    assert_eq!(
        canonical,
        r#"{"version":1,"where":{"eq":{"field":"title","value":null}}}"#
    );
    assert!(data_platform::validate_filter_json(
        r#"{"version":1,"where":{"eq":{"field":"body","value":"x"},"bad":true}}"#
    )
    .is_err());
    assert!(data_platform::validate_filter_json(
        r#"{"version":1,"where":{"in":{"field":"title","values":[]}}}"#
    )
    .is_err());
    assert_eq!(
        data_platform::validate_filter_json("{}").unwrap(),
        r#"{"version":1,"where":{"and":[]}}"#
    );
    assert_eq!(data_platform::filter_digest("{}").unwrap().len(), 32);
}

#[test]
fn sync_profiles_store_frozen_modes_before_objects_and_cascade_rules() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    data_platform::ensure_schema(&conn).unwrap();
    conn.execute(
        concat!(
            "INSERT INTO sync_profiles(profile_id,device_id,revision,name,active) VALUES(",
            "'p1','d',1,'one',1)"
        ),
        [],
    )
    .unwrap();
    for (kind, id, mode) in [
        ("type", "com.kosmos.note", "full"),
        ("dataset", "usage", "metadata"),
        ("blob", "attachments", "none"),
    ] {
        conn.execute(
            concat!(
                "INSERT INTO sync_profile_rules(profile_id,resource_kind,resource_id,mode) ",
                "VALUES(?1,?2,?3,?4)"
            ),
            rusqlite::params!["p1", kind, id, mode],
        )
        .unwrap();
    }
    let modes: Vec<String> = conn
        .prepare("SELECT mode FROM sync_profile_rules WHERE profile_id='p1' ORDER BY resource_kind")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(modes, ["none", "metadata", "full"]);
    assert!(conn
        .execute(
            concat!(
                "INSERT INTO sync_profile_rules(profile_id,resource_kind,resource_id,mode) ",
                "VALUES('p1','type','bad','secret')"
            ),
            []
        )
        .is_err());
    conn.execute("DELETE FROM sync_profiles WHERE profile_id='p1'", [])
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sync_profile_rules WHERE profile_id='p1'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn selective_profile_projects_closure_in_order_and_keeps_local_copy_on_disable() {
    use ark_core::types::SyncEntity;
    let entity = |kind: &str, id: &str, data: serde_json::Value| SyncEntity {
        entity_type: kind.into(),
        id: id.into(),
        data: data.as_object().unwrap().clone(),
        hlc: format!("{kind}-{id}"),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    };
    let entities = vec![
        entity(
            "object",
            "game",
            json!(
                {"typeId":"com.kosmos.game",
                "typeVersion":"1.0.0",
                "title":"Game",
                "contentJson":{"body":"heavy"},
                "propsJson":{"localState":{"path":"C:/game"}}}),
        ),
        entity(
            "object",
            "note",
            json!(
                {"typeId":"com.kosmos.note",
                "typeVersion":"1.0.0",
                "title":"Note",
                "contentJson":{"body":"full"},
                "propsJson":{}}),
        ),
        entity(
            "object_link",
            "link",
            json!({"sourceObjectId":"note","targetObjectId":"game","linkType":"related"}),
        ),
        entity(
            "usage_event",
            "usage-1",
            json!({"exePath":"C:/secret.exe","secret":"no"}),
        ),
        entity("object_type", "com.kosmos.note", json!({"name":"Note"})),
    ];
    let mut profile = data_platform::SelectiveSyncProfile::default();
    profile.set_rule("type", "com.kosmos.note", data_platform::SyncMode::Full);
    profile.set_rule("type", "com.kosmos.game", data_platform::SyncMode::Metadata);
    profile.set_rule("dataset", "usage", data_platform::SyncMode::None);
    let projected = data_platform::filter_entities_for_profile(&profile, &entities);
    assert_eq!(
        projected
            .iter()
            .map(|e| e.entity_type.as_str())
            .collect::<Vec<_>>(),
        ["object_type", "object", "object", "object_link"]
    );
    let game = projected.iter().find(|e| e.id == "game").unwrap();
    assert!(!game.data.contains_key("contentJson"));
    assert!(!game.data.contains_key("localState"));
    assert_eq!(game.data["propsJson"], json!({}));
    assert!(projected
        .iter()
        .all(|e| !e.data.contains_key("secret") && !e.data.contains_key("exePath")));
    let disabled = data_platform::filter_entities_for_profile(&Default::default(), &projected);
    assert!(disabled.is_empty());
    assert_eq!(projected.len(), 4);
}

#[tokio::test]
async fn selective_profile_is_used_by_backend_pages_and_replay_is_idempotent() {
    use ark_core::db::SqliteStorageBackend;
    use ark_core::sync_server::StorageBackend;
    let conn = Connection::open_in_memory().unwrap();
    db::init_schema(&conn).unwrap();
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
    let backend = Arc::new(SqliteStorageBackend::new(Arc::new(Mutex::new(conn))));
    let mut profile = data_platform::SelectiveSyncProfile::default();
    profile.set_rule("type", "com.example.note", data_platform::SyncMode::Full);
    profile.set_rule(
        "type",
        "com.example.game",
        data_platform::SyncMode::Metadata,
    );
    backend.set_selective_sync_profile(Some(profile));
    let loaded = backend.load_entities(&HashMap::new()).await;
    assert_eq!(
        loaded
            .iter()
            .map(|entity| entity.entity_type.as_str())
            .collect::<Vec<_>>(),
        [
            "object_type",
            "object_type",
            "object",
            "object",
            "object_link"
        ]
    );
    let game = loaded.iter().find(|entity| entity.id == "game").unwrap();
    assert!(!game.data.contains_key("contentJson"));
    assert!(!game.data.contains_key("localState"));
    assert_eq!(
        backend
            .load_entities_page(&HashMap::new(), 0, 2)
            .await
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

#[test]
fn persisted_profile_reloads_owner_revision_and_rejects_stale_or_foreign_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profiles.db");
    let rules = [data_platform::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "com.kosmos.note".into(),
        mode: data_platform::SyncMode::Full,
        filter_json: "{}".into(),
    }];
    {
        let conn = Connection::open(&path).unwrap();
        data_platform::ensure_schema(&conn).unwrap();
        let created = data_platform::upsert_sync_profile(
            &conn, "device-a", "phone", "Phone", true, None, &rules,
        )
        .unwrap();
        assert_eq!(created.revision, 1);
    }
    {
        let conn = Connection::open(&path).unwrap();
        let loaded = data_platform::load_sync_profile(&conn, "device-a", "phone").unwrap();
        assert_eq!(loaded.revision, 1);
        assert_eq!(loaded.device_id, "device-a");
        assert_eq!(
            loaded.rules[0].filter_json,
            r#"{"version":1,"where":{"and":[]}}"#
        );
        assert_eq!(
            data_platform::upsert_sync_profile(
                &conn,
                "device-b",
                "phone",
                "Other",
                true,
                Some(1),
                &rules
            )
            .unwrap_err(),
            "PROFILE_OWNER_DENIED"
        );
        assert_eq!(
            data_platform::upsert_sync_profile(
                &conn,
                "device-a",
                "phone",
                "Phone",
                true,
                Some(0),
                &rules
            )
            .unwrap_err(),
            "PROFILE_REVISION_STALE"
        );
        let updated = data_platform::upsert_sync_profile(
            &conn,
            "device-a",
            "phone",
            "Phone",
            true,
            Some(1),
            &rules,
        )
        .unwrap();
        assert_eq!(updated.revision, 2);
    }
    let reopened = Connection::open(&path).unwrap();
    let loaded = data_platform::load_sync_profile(&reopened, "device-a", "phone").unwrap();
    assert_eq!(
        (loaded.device_id, loaded.revision, loaded.name),
        ("device-a".into(), 2, "Phone".into())
    );
}
