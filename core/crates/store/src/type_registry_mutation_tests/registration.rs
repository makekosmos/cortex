use crate::type_registry::register_type;
use domain::r#type::{AliasRecord, TypeRegistration};
use rusqlite::{Connection, OptionalExtension};

type ObjectTypeRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
);

#[test]
fn production_registration_rejects_mismatched_alias_target_before_mutation() {
    let conn = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&conn).unwrap();
    register_type(
        &conn,
        &TypeRegistration {
            type_id: "other-type".into(),
            name: "Other".into(),
            schema_json: "{}".into(),
            ui_schema_json: "{}".into(),
            content_contract_json: "{}".into(),
            relations_json: "[]".into(),
            sync_policy_json: "{}".into(),
            version: "1.0.0".into(),
            schema_hash: String::new(),
            owner_kind: "extension".into(),
            owner_id: Some("other-owner".into()),
            status: "active".into(),
            base_type_id: None,
            aliases: Vec::new(),
            created_at: "other-time".into(),
        },
    )
    .unwrap();

    let before: (i64, i64, i64, String, String, String) = conn
        .query_row(
            "SELECT
                (SELECT count(*) FROM object_types),
                (SELECT count(*) FROM object_type_versions),
                (SELECT count(*) FROM object_type_aliases),
                (SELECT owner_id FROM object_types WHERE id='other-type'),
                (SELECT current_version FROM object_types WHERE id='other-type'),
                (SELECT status FROM object_types WHERE id='other-type')",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();

    let error = register_type(
        &conn,
        &TypeRegistration {
            type_id: "new-type".into(),
            name: "New".into(),
            schema_json: "{}".into(),
            ui_schema_json: "{}".into(),
            content_contract_json: "{}".into(),
            relations_json: "[]".into(),
            sync_policy_json: "{}".into(),
            version: "1.0.0".into(),
            schema_hash: String::new(),
            owner_kind: "extension".into(),
            owner_id: Some("owner".into()),
            status: "active".into(),
            base_type_id: None,
            aliases: vec![AliasRecord {
                alias: "new-alias".into(),
                canonical_type_id: "other-type".into(),
                created_at: "now".into(),
            }],
            created_at: "now".into(),
        },
    )
    .unwrap_err();
    assert_eq!(error, "alias canonical type mismatch");

    let after: (i64, i64, i64, String, String, String) = conn
        .query_row(
            "SELECT
                (SELECT count(*) FROM object_types),
                (SELECT count(*) FROM object_type_versions),
                (SELECT count(*) FROM object_type_aliases),
                (SELECT owner_id FROM object_types WHERE id='other-type'),
                (SELECT current_version FROM object_types WHERE id='other-type'),
                (SELECT status FROM object_types WHERE id='other-type')",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(before, after);
    assert!(conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id='new-type'",
            [],
            |_| Ok(())
        )
        .optional()
        .unwrap()
        .is_none());
}

#[test]
fn production_registration_rejects_existing_canonical_without_any_changes() {
    let conn = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&conn).unwrap();
    let before: Vec<ObjectTypeRow> = conn
        .prepare(concat!(
            "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, ",
            "system_locked, owner_kind, owner_id, current_version, status, base_type_id ",
            "FROM object_types ORDER BY id"
        ))
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let before_versions: Vec<(String, String)> = conn
        .prepare("SELECT type_id, version FROM object_type_versions ORDER BY type_id, version")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let before_aliases: Vec<(String, String)> = conn
        .prepare("SELECT alias, canonical_type_id FROM object_type_aliases ORDER BY alias")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    let error = register_type(
        &conn,
        &TypeRegistration {
            type_id: "core.note".into(),
            name: "Must Not Replace".into(),
            schema_json: "{\"changed\":true}".into(),
            ui_schema_json: "{\"changed\":true}".into(),
            content_contract_json: "{}".into(),
            relations_json: "[]".into(),
            sync_policy_json: "{}".into(),
            version: "99.0.0".into(),
            schema_hash: String::new(),
            owner_kind: "replacement".into(),
            owner_id: Some("replacement-owner".into()),
            status: "deprecated".into(),
            base_type_id: None,
            aliases: vec![AliasRecord {
                alias: "must-not-exist".into(),
                canonical_type_id: "core.note".into(),
                created_at: "later".into(),
            }],
            created_at: "replacement-time".into(),
        },
    )
    .unwrap_err();
    assert_eq!(error, "canonical type already exists");

    let after: Vec<ObjectTypeRow> = conn
        .prepare(concat!(
            "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, ",
            "system_locked, owner_kind, owner_id, current_version, status, base_type_id ",
            "FROM object_types ORDER BY id"
        ))
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let after_versions: Vec<(String, String)> = conn
        .prepare("SELECT type_id, version FROM object_type_versions ORDER BY type_id, version")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let after_aliases: Vec<(String, String)> = conn
        .prepare("SELECT alias, canonical_type_id FROM object_type_aliases ORDER BY alias")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(before_versions, after_versions);
    assert_eq!(before_aliases, after_aliases);
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM object_type_aliases WHERE alias='must-not-exist'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

fn application_registration(type_id: &str, version: &str) -> TypeRegistration {
    TypeRegistration {
        type_id: type_id.into(),
        name: "Application type".into(),
        schema_json: r#"{"type":"object","properties":{"title":{"type":"string"}}}"#.into(),
        ui_schema_json: "{}".into(),
        content_contract_json: "{}".into(),
        relations_json: "[]".into(),
        sync_policy_json: "{}".into(),
        version: version.into(),
        schema_hash: String::new(),
        owner_kind: "application".into(),
        owner_id: Some("com.example".into()),
        status: "active".into(),
        base_type_id: None,
        aliases: Vec::new(),
        created_at: "registered-at".into(),
    }
}

#[test]
fn the_builtin_migration_and_register_type_write_the_same_definition_rows() {
    let conn = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&conn).unwrap();
    register_type(
        &conn,
        &application_registration("com.example.note", "1.0.0"),
    )
    .unwrap();

    for (type_id, version, locked) in [("core.note", "0.1.0", 1), ("com.example.note", "1.0.0", 0)]
    {
        let (schema, hash, current_version, system_locked): (String, String, String, i64) = conn
            .query_row(
                concat!(
                    "SELECT versions.schema_json, versions.schema_hash, types.current_version,",
                    " types.system_locked FROM object_type_versions versions JOIN object_types",
                    " types ON types.id=versions.type_id",
                    " WHERE versions.type_id=?1 AND versions.version=?2"
                ),
                rusqlite::params![type_id, version],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&schema).is_ok(),
            "{type_id}: the version row stores a definition"
        );
        assert_eq!(
            hash.len(),
            64,
            "{type_id}: the version row carries its hash"
        );
        assert_eq!(
            current_version, version,
            "{type_id}: current_version follows"
        );
        assert_eq!(system_locked, locked, "{type_id}: locking stays per caller");
    }

    // Replaying the builtin install is idempotent: one row per version, same hash.
    crate::canonical_migrations::registry::ensure_canonical_type_versions(&conn).unwrap();
    let (version_rows, hash_after): (i64, String) = conn
        .query_row(
            concat!(
                "SELECT count(*), MAX(schema_hash) FROM object_type_versions",
                " WHERE type_id='core.note' AND version='0.1.0'"
            ),
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        version_rows, 1,
        "reinstalling does not duplicate the version row"
    );
    assert_eq!(hash_after.len(), 64);
}
