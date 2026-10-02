#![allow(clippy::unwrap_used)]
use ark_core::canonical_types::definitions::canonical_type_registrations;
use ark_core::canonical_types::migration_registry::{
    apply_registry, apply_registry_with_failure, preflight_registry, RegistryError,
};
use ark_core::type_registry::legacy_compatibility_version;
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
    // Nine canonical types; task and project carry a second registered
    // version (1.1.0) next to the original 1.0.0.
    assert_eq!(report.installed, 11);
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
    conn.execute(
        concat!(
            "INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,",
            "updated_at,system_locked,owner_kind,owner_id,current_version,status,",
            "base_type_id) VALUES('com.kosmos.note','wrong','{}','{}','x','x',1,'core',",
            "'com.kosmos.core','1.0.0','active',NULL)"
        ),
        [],
    )
    .unwrap();
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
fn generated_legacy_definition_at_canonical_id_is_promoted_losslessly() {
    let conn = db();
    let (legacy_version, legacy_hash) = legacy_compatibility_version("{}", "{}").unwrap();
    conn.execute(
        concat!(
            "INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,",
            "updated_at,system_locked,owner_kind,owner_id,current_version,status,",
            "base_type_id) VALUES('com.kosmos.note','Заметка','{}','{}','legacy-created',",
            "'legacy-updated',1,'system',NULL,?1,'active',NULL)"
        ),
        [&legacy_version],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,",
            "content_contract_json,relations_json,sync_policy_json,schema_hash,",
            "created_at) VALUES('com.kosmos.note',?1,'{}','{}','{}','[]','{}',?2,",
            "'legacy-created')"
        ),
        [&legacy_version, &legacy_hash],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
            "created_at,updated_at,deleted_at) VALUES('note-1','com.kosmos.note',?1,",
            "'Legacy note','{}','{}','created','updated',NULL)"
        ),
        [&legacy_version],
    )
    .unwrap();

    let plan = preflight_registry(&conn).unwrap();
    apply_registry(&conn, &plan).unwrap();

    assert_eq!(
        conn.query_row(
            "SELECT current_version FROM object_types WHERE id='com.kosmos.note'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "1.0.0"
    );
    assert_eq!(
        conn.query_row(
            "SELECT type_version FROM objects WHERE id='note-1'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        legacy_version
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM object_type_versions WHERE type_id='com.kosmos.note'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        conn.query_row(
            concat!(
                "SELECT count(*) FROM legacy_type_definition_archive WHERE ",
                "legacy_type_id='com.kosmos.note'"
            ),
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn legacy_archive_is_lossless_and_late_failure_rolls_back() {
    let conn = db();
    conn.execute(
        concat!(
            "INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,",
            "updated_at,system_locked,owner_kind,owner_id,current_version,status,",
            "base_type_id) VALUES('note_obj','Legacy','{}','{}','created','updated',0,",
            "'package','p','2.0.0','active',NULL)"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,",
            "content_contract_json,relations_json,sync_policy_json,schema_hash,",
            "created_at) VALUES('note_obj','2.0.0','{}','{}','{}','[]','{}',",
            "'legacy-hash','v-created')"
        ),
        [],
    )
    .unwrap();
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
    let (summary, versions, hash): (String, String, String) = conn
        .query_row(
            concat!(
                "SELECT summary_json,versions_json,source_hash FROM ",
                "legacy_type_definition_archive WHERE legacy_type_id='note_obj'"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
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
    assert_eq!(canonical_type_registrations().unwrap().len(), 11);
}

#[test]
fn init_schema_installs_added_canonical_versions() {
    // A DB that completed the phase3 migration before the 1.1.0 bump: the new
    // version rows are missing and current_version still names 1.0.0.
    let conn = Connection::open_in_memory().unwrap();
    ark_core::db::init_schema(&conn).unwrap();
    conn.execute("DELETE FROM object_type_versions WHERE version='1.1.0'", [])
        .unwrap();
    conn.execute(
        concat!(
            "UPDATE object_types SET current_version='1.0.0' WHERE id IN (",
            "'com.kosmos.task','com.kosmos.project')"
        ),
        [],
    )
    .unwrap();
    // The next boot must reinstall the version rows and bump current_version
    // without touching stored objects.
    ark_core::db::init_schema(&conn).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM object_type_versions WHERE version='1.1.0'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    for id in ["com.kosmos.task", "com.kosmos.project"] {
        let current: String = conn
            .query_row(
                "SELECT current_version FROM object_types WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(current, "1.1.0", "{id}");
    }
    // Objects written under 1.0.0 keep their version and still resolve.
    let mut stmt = conn
        .prepare(concat!(
            "SELECT version FROM object_type_versions WHERE type_id='com.kosmos.task' ",
            "ORDER BY version"
        ))
        .unwrap();
    let versions: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(versions, ["1.0.0", "1.1.0"]);
}
