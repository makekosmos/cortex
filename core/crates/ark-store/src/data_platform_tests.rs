#![allow(clippy::unwrap_used)]

use rusqlite::Connection;
use serde_json::json;

#[test]
fn external_refs_are_additive_and_unique() {
    let conn = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&conn).unwrap();
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,created_at,updated_at) ",
            "VALUES('note-1','com.kosmos.note','1.0.0','Note','now','now')"
        ),
        [],
    )
    .unwrap();
    crate::data_platform::upsert_external_ref(
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
    assert!(crate::data_platform::upsert_external_ref(
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
fn sync_profiles_store_frozen_modes_before_objects_and_cascade_rules() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    crate::data_platform::ensure_schema(&conn).unwrap();
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
fn persisted_profile_reloads_owner_revision_and_rejects_stale_or_foreign_writes() {
    let dir = crate::test_support::test_temp_dir().expect("profile fixture directory");
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
        crate::data_platform::ensure_schema(&conn).unwrap();
        let created = crate::data_platform::upsert_sync_profile(
            &conn, "device-a", "phone", "Phone", true, None, &rules,
        )
        .unwrap();
        assert_eq!(created.revision, 1);
    }
    {
        let conn = Connection::open(&path).unwrap();
        let loaded = crate::data_platform::load_sync_profile(&conn, "device-a", "phone").unwrap();
        assert_eq!(loaded.revision, 1);
        assert_eq!(loaded.device_id, "device-a");
        assert_eq!(
            loaded.rules[0].filter.canonical_json(),
            r#"{"version":1,"where":{"eq":{"field":"title","value":"Note"}}}"#
        );
        let profile = loaded.selective_profile();
        let entity = |id: &str, title: &str| ark_domain::types::SyncEntity {
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
            crate::data_platform::upsert_sync_profile(
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
            crate::data_platform::upsert_sync_profile(
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
        let updated = crate::data_platform::upsert_sync_profile(
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
    let loaded = crate::data_platform::load_sync_profile(&reopened, "device-a", "phone").unwrap();
    assert_eq!(
        (loaded.device_id, loaded.revision, loaded.name),
        ("device-a".into(), 2, "Phone".into())
    );
}
