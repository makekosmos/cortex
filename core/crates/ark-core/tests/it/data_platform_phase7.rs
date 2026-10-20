#![allow(clippy::unwrap_used)]
use ark_application::sync_storage::CommittedFeedPort;
use ark_core::db;
use ark_store::test_support::TestDatabase;
use rusqlite::Connection;
use serde_json::json;

#[test]
fn external_refs_are_additive_and_unique() {
    let conn = Connection::open_in_memory().unwrap();
    ark_store::open::init_schema(&conn).unwrap();
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,created_at,updated_at) ",
            "VALUES('note-1','com.kosmos.note','1.0.0','Note','now','now')"
        ),
        [],
    )
    .unwrap();
    ark_store::data_platform::upsert_external_ref(
        &conn,
        &ark_application::external_ref::ExternalRefUpsert {
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
    assert!(ark_store::data_platform::upsert_external_ref(
        &conn,
        &ark_application::external_ref::ExternalRefUpsert {
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
    let typed = ark_domain::selective_sync::FilterV1::parse(
        r#"{"where":{"eq":{"value":null,"field":"title"}},"version":1}"#,
    )
    .unwrap();
    let reordered = ark_domain::selective_sync::FilterV1::parse(
        r#"{"version":1,"where":{"eq":{"field":"title","value":null}}}"#,
    )
    .unwrap();
    assert_eq!(typed, reordered);
    assert_eq!(typed.digest(), reordered.digest());
    assert_eq!(typed.digest().len(), 32);

    let canonical = ark_domain::selective_sync::validate_filter_json(
        r#"{"where":{"eq":{"value":null,"field":"title"}},"version":1}"#,
    )
    .unwrap();
    assert_eq!(
        canonical,
        r#"{"version":1,"where":{"eq":{"field":"title","value":null}}}"#
    );
    assert!(ark_domain::selective_sync::validate_filter_json(
        r#"{"version":1,"where":{"eq":{"field":"body","value":"x"},"bad":true}}"#
    )
    .is_err());
    assert!(ark_domain::selective_sync::validate_filter_json(
        r#"{"version":1,"where":{"in":{"field":"title","values":[]}}}"#
    )
    .is_err());
    assert_eq!(
        ark_domain::selective_sync::validate_filter_json("{}").unwrap(),
        r#"{"version":1,"where":{"and":[]}}"#
    );
    assert_eq!(
        ark_domain::selective_sync::filter_digest("{}").unwrap(),
        ark_domain::selective_sync::FilterV1::parse("{}")
            .unwrap()
            .digest()
    );
}

#[test]
fn filter_v1_evaluates_predicates_against_exact_entity_fields() {
    let fields = json!({"title":"Note", "priority":2, "enabled":true})
        .as_object()
        .unwrap()
        .clone();
    for predicate in [
        json!({"eq":{"field":"title","value":"Note"}}),
        json!({"in":{"field":"priority","values":[1,2,3]}}),
        json!({"exists":{"field":"enabled","value":true}}),
        json!({
            "and": [
                {"eq": {"field": "title", "value": "Note"}},
                {"not": {"eq": {"field": "priority", "value": 1}}}
            ]
        }),
        json!({
            "or": [
                {"eq": {"field": "title", "value": "Other"}},
                {"eq": {"field": "enabled", "value": true}}
            ]
        }),
    ] {
        let filter = serde_json::to_string(&json!({"version":1,"where":predicate})).unwrap();
        assert!(ark_domain::selective_sync::FilterV1::parse(&filter)
            .unwrap()
            .matches(&fields));
    }
    assert!(!ark_domain::selective_sync::FilterV1::parse(
        r#"{"version":1,"where":{"eq":{"field":"missing","value":null}}}"#
    )
    .unwrap()
    .matches(&fields));
    assert!(ark_domain::selective_sync::FilterV1::match_all().matches(&fields));
}

#[tokio::test]
async fn sync_policy_generation_changes_when_only_filter_changes() {
    let database = TestDatabase::create().unwrap();
    let backend = db::SqliteStorageBackend::open_for_test(
        database.path().to_string_lossy().into_owned(),
        "test-device",
    )
    .await
    .unwrap();
    let mut profile = ark_domain::selective_sync::SelectiveSyncProfile::default();
    profile.set_rule(
        "type",
        "com.kosmos.note",
        ark_domain::selective_sync::SyncMode::Full,
    );
    backend.set_selective_sync_profile(Some(profile.clone()));
    let unfiltered = backend.selective_sync_policy_generation();
    profile.set_rule_with_filter(
        "type",
        "com.kosmos.note",
        ark_domain::selective_sync::SyncMode::Full,
        ark_domain::selective_sync::FilterV1::parse(
            r#"{"version":1,"where":{"eq":{"field":"title","value":"Note"}}}"#,
        )
        .unwrap(),
    );
    backend.set_selective_sync_profile(Some(profile));
    assert_ne!(unfiltered, backend.selective_sync_policy_generation());
}

#[test]
fn sync_profiles_store_frozen_modes_before_objects_and_cascade_rules() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    ark_store::data_platform::ensure_schema(&conn).unwrap();
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
    let mut profile = ark_domain::selective_sync::SelectiveSyncProfile::default();
    profile.set_rule(
        "type",
        "com.kosmos.note",
        ark_domain::selective_sync::SyncMode::Full,
    );
    profile.set_rule(
        "type",
        "com.kosmos.game",
        ark_domain::selective_sync::SyncMode::Metadata,
    );
    profile.set_rule(
        "dataset",
        "usage",
        ark_domain::selective_sync::SyncMode::None,
    );
    let projected = ark_domain::selective_sync::filter_entities_for_profile(&profile, &entities);
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
    profile.set_rule_with_filter(
        "type",
        "com.kosmos.game",
        ark_domain::selective_sync::SyncMode::Metadata,
        ark_domain::selective_sync::FilterV1::parse(
            r#"{"version":1,"where":{"eq":{"field":"title","value":"Other"}}}"#,
        )
        .unwrap(),
    );
    let filtered = ark_domain::selective_sync::filter_entities_for_profile(&profile, &entities);
    assert_eq!(
        filtered
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<Vec<_>>(),
        ["com.kosmos.note", "note"]
    );
    let disabled =
        ark_domain::selective_sync::filter_entities_for_profile(&Default::default(), &projected);
    assert!(disabled.is_empty());
    assert_eq!(projected.len(), 4);
}

#[test]
fn persisted_profile_reloads_owner_revision_and_rejects_stale_or_foreign_writes() {
    let dir = crate::support::temp_dir();
    let path = dir.path().join("profiles.db");
    let rules = [ark_application::sync_policy::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "com.kosmos.note".into(),
        mode: ark_domain::selective_sync::SyncMode::Full,
        filter: ark_domain::selective_sync::FilterV1::parse(
            r#"{"where":{"eq":{"field":"title","value":"Note"}},"version":1}"#,
        )
        .unwrap(),
    }];
    {
        let conn = Connection::open(&path).unwrap();
        ark_store::data_platform::ensure_schema(&conn).unwrap();
        let created = ark_store::data_platform::upsert_sync_profile(
            &conn, "device-a", "phone", "Phone", true, None, &rules,
        )
        .unwrap();
        assert_eq!(created.revision, 1);
    }
    {
        let conn = Connection::open(&path).unwrap();
        let loaded =
            ark_store::data_platform::load_sync_profile(&conn, "device-a", "phone").unwrap();
        assert_eq!(loaded.revision, 1);
        assert_eq!(loaded.device_id, "device-a");
        assert_eq!(
            loaded.rules[0].filter.canonical_json(),
            r#"{"version":1,"where":{"eq":{"field":"title","value":"Note"}}}"#
        );
        let profile = loaded.selective_profile();
        let entity = |id: &str, title: &str| ark_core::types::SyncEntity {
            entity_type: "object".into(),
            id: id.into(),
            data: json!({"typeId":"com.kosmos.note", "title":title})
                .as_object()
                .unwrap()
                .clone(),
            hlc: "1@device-a".into(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };
        let matches = entity("note-match", "Note");
        let rejected = entity("note-other", "Other");
        assert_eq!(
            ark_domain::selective_sync::filter_entities_for_profile(&profile, &[matches, rejected])
                .iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
            ["note-match"]
        );
        assert_eq!(
            ark_store::data_platform::upsert_sync_profile(
                &conn,
                "device-b",
                "phone",
                "Other",
                true,
                Some(1),
                &rules
            )
            .unwrap_err()
            .to_string(),
            "PROFILE_OWNER_DENIED"
        );
        assert_eq!(
            ark_store::data_platform::upsert_sync_profile(
                &conn,
                "device-a",
                "phone",
                "Phone",
                true,
                Some(0),
                &rules
            )
            .unwrap_err()
            .to_string(),
            "PROFILE_REVISION_STALE"
        );
        let updated = ark_store::data_platform::upsert_sync_profile(
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
    let loaded =
        ark_store::data_platform::load_sync_profile(&reopened, "device-a", "phone").unwrap();
    assert_eq!(
        (loaded.device_id, loaded.revision, loaded.name),
        ("device-a".into(), 2, "Phone".into())
    );
}
