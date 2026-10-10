#![allow(clippy::unwrap_used)]
use ark_core::canonical_types::definitions::canonical_type_registrations;
use ark_core::db::{delete_object_type, init_schema, upsert_object_type};
use ark_core::type_registry::{canonical_schema_hash, resolve_alias};
use ark_core::types::ObjectType;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;

const EXPECTED: [(&str, &str, &str); 9] = [
    (
        "com.kosmos.note",
        "note_obj",
        "8dae6b0a41de279ab6176bfc2115ec88c08b40551bb5ed566a3773b0cac2be53",
    ),
    (
        "com.kosmos.task",
        "task_obj",
        "7e4d010591eb8356224b390725f5eb8ac4347c005d66550d389e9acd08f86a4d",
    ),
    (
        "com.kosmos.project",
        "project_obj",
        "55ea05493991deda6093dc86c73e30d05f5db3b4b1d42c9b675d6b176a633bac",
    ),
    (
        "com.kosmos.tag",
        "tag_obj",
        "de364bb69edcad14f474b018e65c2ccc8be53426133d5107ac9cbd9492d5b18f",
    ),
    (
        "com.kosmos.person",
        "person_obj",
        "664f5f1858db443e53c0d304c29822a00fa862c48e361fca455350adc64372c3",
    ),
    (
        "com.kosmos.image",
        "image_obj",
        "83e9bc320b20236b48b29294c084ca2adaecb83ddd8702217ee220641ca193ff",
    ),
    (
        "com.kosmos.time-entry",
        "time_entry_obj",
        "e455f5ef3e0b60bcd5097488c2e8ccdbf8c58ace92d02a22d94f8496ff5ed4b5",
    ),
    (
        "com.kosmos.game",
        "game_obj",
        "de41b108ef21958b901851a92561dfb046cffcedf29e6cb7c355d97396f1ba99",
    ),
    (
        "com.kosmos.book",
        "book_obj",
        "8ce61b253c590aa2b54b00bac4854c8b0e92469bb916e58ebc9fccd7070b12a0",
    ),
];

// Newer versions registered alongside their predecessors; stored objects keep
// the version they were written with, so every listed version stays valid.
const SUPERSEDED: &[(&str, &str, &str)] = &[
    (
        "com.kosmos.task",
        "task_obj",
        "a435d0ce5b85ad859400d9899a74a8ef4b8b2f91e479724f4c34e8ddb1401485",
    ),
    (
        "com.kosmos.project",
        "project_obj",
        "4e301956f8119dd69d8a79f38a651da50b26d0798d9e62f04324b0110dbf26c5",
    ),
];

#[test]
fn canonical_definitions_match_frozen_order_and_metadata() {
    let registrations = canonical_type_registrations().expect("canonical definitions");
    assert_eq!(registrations.len(), EXPECTED.len() + SUPERSEDED.len());
    for (registration, (type_id, alias, hash)) in registrations.iter().zip(EXPECTED) {
        assert_eq!(registration.type_id, type_id);
        assert_eq!(registration.version, "1.0.0");
        assert_eq!(registration.owner_kind, "core");
        assert_eq!(registration.owner_id.as_deref(), Some("com.kosmos.core"));
        assert_eq!(registration.status, "active");
        assert_eq!(registration.base_type_id, None);
        assert_eq!(registration.created_at, "1970-01-01T00:00:00.000Z");
        assert_eq!(registration.aliases.len(), 1);
        assert_eq!(registration.aliases[0].alias, alias);
        assert_eq!(registration.aliases[0].canonical_type_id, type_id);
        assert_eq!(registration.schema_hash, hash);
    }
    for (registration, (type_id, alias, hash)) in
        registrations[EXPECTED.len()..].iter().zip(SUPERSEDED)
    {
        assert_eq!(registration.type_id, *type_id);
        assert_eq!(registration.version, "1.1.0");
        assert_eq!(registration.aliases[0].alias, *alias);
        assert_eq!(registration.schema_hash, *hash);
    }
}

#[test]
fn canonical_definition_json_is_parseable_and_hashes_exactly() {
    let registrations = canonical_type_registrations().expect("canonical definitions");
    let expected: Vec<(&str, &str, &str)> =
        EXPECTED.iter().chain(SUPERSEDED.iter()).copied().collect();
    for (registration, (_, _, expected_hash)) in registrations.iter().zip(expected) {
        let schema: Value = serde_json::from_str(&registration.schema_json).unwrap();
        let ui_schema: Value = serde_json::from_str(&registration.ui_schema_json).unwrap();
        let content: Value = serde_json::from_str(&registration.content_contract_json).unwrap();
        let relations: Value = serde_json::from_str(&registration.relations_json).unwrap();
        let sync_policy: Value = serde_json::from_str(&registration.sync_policy_json).unwrap();
        assert_eq!(
            canonical_schema_hash(&schema, &ui_schema, &content, &relations, &sync_policy).unwrap(),
            expected_hash
        );
    }
}

#[test]
fn canonical_ids_and_aliases_do_not_collide() {
    let registrations = canonical_type_registrations().expect("canonical definitions");
    let ids: HashSet<_> = registrations.iter().map(|r| r.type_id.as_str()).collect();
    let aliases: HashSet<_> = registrations
        .iter()
        .flat_map(|r| r.aliases.iter().map(|a| a.alias.as_str()))
        .collect();
    assert_eq!(ids.len(), EXPECTED.len());
    assert_eq!(aliases.len(), EXPECTED.len());
    assert!(ids.is_disjoint(&aliases));
}

#[test]
fn db_does_not_own_legacy_registry_or_compatibility_authority() {
    let db =
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/db.rs")).expect("db source");
    for forbidden in [
        "fn builtin_note_object_type",
        "fn builtin_game_object_type",
        "fn builtin_time_entry_object_type",
        "fn builtin_tag_object_type",
        "fn seed_builtin_object_types",
        "fn apply_legacy_compat_entity",
        "map_legacy_source(",
        "let canonical_alias = match input_type_id.as_str()",
    ] {
        assert!(
            !db.contains(forbidden),
            "db.rs retains forbidden canonical/legacy authority: {forbidden}"
        );
    }
}

#[test]
fn fresh_init_registers_exact_canonical_authority_and_routes_legacy_aliases() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();

    let rows: Vec<(String, String)> = conn
        .prepare("SELECT id, current_version FROM object_types ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut expected: Vec<(String, String)> = EXPECTED
        .iter()
        .map(|(id, _, _)| {
            // The superseded contract versions registered in SUPERSEDED become
            // current_version; older registrations stay readable.
            let current = if SUPERSEDED.iter().any(|(s, _, _)| s == id) {
                "1.1.0"
            } else {
                "1.0.0"
            };
            ((*id).to_owned(), current.to_owned())
        })
        .collect();
    expected.sort();
    assert_eq!(rows, expected);

    for (canonical, alias, _) in EXPECTED {
        assert_eq!(
            resolve_alias(&conn, alias).unwrap().as_deref(),
            Some(canonical)
        );
    }
    let summaries: Vec<String> = conn
        .prepare("SELECT schema_json || ui_schema_json FROM object_types")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(summaries
        .iter()
        .all(|summary| { !summary.contains("cover_image") && !summary.contains("rawg_id") }));

    let legacy = ObjectType {
        id: "note_obj".into(),
        name: "legacy authority".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        system_locked: false,
    };
    let error = upsert_object_type(&conn, &legacy).unwrap_err();
    assert!(error.contains("alias collides with canonical type"));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM object_types WHERE id='note_obj'",
            [],
            |row| { row.get::<_, i64>(0) }
        )
        .unwrap(),
        0
    );
}

#[test]
fn inbound_legacy_object_type_entity_cannot_mutate_canonical_row() {
    // KOS-370: sync exports every object_types row as a legacy `object_type`
    // entity. Applying a peer's canonical definition on this side must not
    // downgrade `current_version` to `0.0.0+legacy.*` or deprecate the row —
    // either mutation fails the phase3 registry preflight at next open and
    // bricks the database.
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();

    let synced = ObjectType {
        id: "com.kosmos.task".into(),
        name: "Peer Task".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        system_locked: false,
    };
    upsert_object_type(&conn, &synced).unwrap();
    delete_object_type(&conn, "com.kosmos.task").unwrap();

    let (current_version, status): (String, String) = conn
        .query_row(
            "SELECT current_version, status FROM object_types WHERE id='com.kosmos.task'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(current_version, "1.1.0");
    assert_eq!(status, "active");
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM object_type_versions
             WHERE type_id='com.kosmos.task' AND version LIKE '0.0.0+legacy.%'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
