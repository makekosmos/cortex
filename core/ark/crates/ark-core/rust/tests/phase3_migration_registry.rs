use ark_core::canonical_types::definitions::canonical_type_registrations;
use ark_core::canonical_types::migration_registry::{
    apply_registry, apply_registry_with_failure, preflight_registry, RegistryError,
};
use rusqlite::Connection;

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys=ON;
         CREATE TABLE object_types (
           id TEXT PRIMARY KEY, name TEXT NOT NULL, schema_json TEXT NOT NULL,
           ui_schema_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
           system_locked INTEGER NOT NULL DEFAULT 0, owner_kind TEXT NOT NULL DEFAULT 'system',
           owner_id TEXT, current_version TEXT NOT NULL DEFAULT '0.0.0-legacy',
           status TEXT NOT NULL DEFAULT 'active', base_type_id TEXT REFERENCES object_types(id));
         CREATE TABLE object_type_versions (
           type_id TEXT NOT NULL REFERENCES object_types(id), version TEXT NOT NULL,
           schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL,
           content_contract_json TEXT NOT NULL DEFAULT '{}', relations_json TEXT NOT NULL DEFAULT '[]',
           sync_policy_json TEXT NOT NULL DEFAULT '{}', schema_hash TEXT NOT NULL, created_at TEXT NOT NULL,
           PRIMARY KEY(type_id, version));
         CREATE TABLE object_type_aliases (
           alias TEXT PRIMARY KEY, canonical_type_id TEXT NOT NULL REFERENCES object_types(id), created_at TEXT NOT NULL);
         CREATE TABLE objects (id TEXT PRIMARY KEY, type_id TEXT NOT NULL REFERENCES object_types(id), type_version TEXT NOT NULL,
           title TEXT NOT NULL, content_json TEXT NOT NULL, props_json TEXT NOT NULL,
           created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT);
         CREATE TABLE object_links (id TEXT PRIMARY KEY, source_object_id TEXT NOT NULL REFERENCES objects(id),
           target_object_id TEXT NOT NULL REFERENCES objects(id), link_type TEXT NOT NULL, created_at TEXT NOT NULL);
        ",
    )
    .unwrap();
    conn
}

#[test]
fn fresh_registry_installs_all_exact_definitions_and_aliases() {
    let conn = db();
    let plan = preflight_registry(&conn).unwrap();
    let report = apply_registry(&conn, &plan).unwrap();
    assert_eq!(report.installed, 9);
    assert_eq!(
        conn.query_row("SELECT count(*) FROM object_types", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        9
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM object_type_versions WHERE version='1.0.0'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        9
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM object_type_aliases", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        9
    );
    assert_eq!(
        conn.query_row(
            "SELECT min(system_locked),max(system_locked) FROM object_types",
            [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        )
        .unwrap(),
        (1, 1)
    );
    let rerun = apply_registry(&conn, &plan).unwrap();
    assert_eq!(rerun.archived, 0);
}

#[test]
fn mismatch_is_read_only_and_structured() {
    let conn = db();
    conn.execute("INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,updated_at,system_locked,owner_kind,owner_id,current_version,status,base_type_id) VALUES('com.kosmos.note','wrong','{}','{}','x','x',1,'core','com.kosmos.core','1.0.0','active',NULL)", []).unwrap();
    let err = preflight_registry(&conn).unwrap_err();
    assert!(
        matches!(err, RegistryError::CanonicalConflict { ref type_id } if type_id == "com.kosmos.note")
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM object_types", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn legacy_archive_is_lossless_and_late_failure_rolls_back() {
    let conn = db();
    conn.execute("INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,updated_at,system_locked,owner_kind,owner_id,current_version,status,base_type_id) VALUES('note_obj','Legacy','{}','{}','created','updated',0,'package','p','2.0.0','active',NULL)", []).unwrap();
    conn.execute("INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES('note_obj','2.0.0','{}','{}','{}','[]','{}','legacy-hash','v-created')", []).unwrap();
    let plan = preflight_registry(&conn).unwrap();
    let err = apply_registry_with_failure(&conn, &plan, Some(1)).unwrap_err();
    assert!(matches!(err, RegistryError::InjectedFailure));
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='legacy_type_definition_archive'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT name FROM object_types WHERE id='note_obj'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "Legacy"
    );
    let report = apply_registry(&conn, &plan).unwrap();
    assert_eq!(report.archived, 1);
    let (summary, versions, hash): (String, String, String) = conn.query_row("SELECT summary_json,versions_json,source_hash FROM legacy_type_definition_archive WHERE legacy_type_id='note_obj'", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert!(summary.contains("Legacy"));
    assert!(versions.contains("2.0.0"));
    assert_eq!(hash.len(), 64);
    assert_eq!(
        conn.query_row(
            "SELECT canonical_type_id FROM object_type_aliases WHERE alias='note_obj'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "com.kosmos.note"
    );
}

#[test]
fn definitions_are_the_registry_hash_authority() {
    assert_eq!(canonical_type_registrations().unwrap().len(), 9);
}
