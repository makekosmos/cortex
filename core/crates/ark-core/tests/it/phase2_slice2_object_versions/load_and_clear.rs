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
    drop(reopened);
    std::fs::remove_file(&path).expect("backup db must be released after close");
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
