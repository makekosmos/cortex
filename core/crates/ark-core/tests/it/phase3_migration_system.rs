#![allow(clippy::unwrap_used)]
use ark_core::canonical_types::migration::{migrate_phase3, retire_legacy_planning_tables};
use ark_core::db::{
    backup_to_file, clear_all, init_schema, init_schema_prerequisites_for_phase3, load_all, open_db,
};
use rusqlite::{params, Connection};
use serde_json::json;

use crate::phase3_legacy_fixtures;

// Pre-migration retired-planning shape plus shared seed row; three tests use it.
const LEGACY_AREAS_HEADINGS_SCHEMA: &str =
    "CREATE TABLE areas (id TEXT PRIMARY KEY, title TEXT NOT NULL, sort_order INTEGER NOT NULL
     DEFAULT 0, created_at TEXT NOT NULL);
     CREATE TABLE headings (id TEXT PRIMARY KEY, title TEXT NOT NULL, sort_order INTEGER NOT NULL
     DEFAULT 0, project_id TEXT NOT NULL);
     INSERT INTO areas(id,title,sort_order,created_at) VALUES('area-1','Work',7,
     '2026-01-01T00:00:00Z');";

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [name],
        |row| row.get(0),
    )
    .unwrap()
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

#[test]
fn phase9_retires_planning_tables_after_lossless_canonical_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("planning-retirement.sqlite");
    let conn = open_db(path.to_str().unwrap()).unwrap();
    init_schema_prerequisites_for_phase3(&conn).unwrap();
    conn.execute_batch(LEGACY_AREAS_HEADINGS_SCHEMA).unwrap();
    conn.execute_batch(
        "INSERT INTO projects(id,title,notes,status,sort_order,color_tag,area_id,created_at)
         VALUES('project-1','Project','Keep','active',2,'blue','area-1','2026-01-02T00:00:00Z');
         INSERT INTO headings(id,title,sort_order,project_id) VALUES('heading-1','Section',3,
         'project-1');
         INSERT INTO todos(id,title,priority,heading_id,project_id,area_id,tag_ids,checklist_items,
         created_at) VALUES('todo-1','Task',2,'heading-1','project-1','area-1','[]','[]',
         '2026-01-03T00:00:00Z');",
    )
    .unwrap();

    let first = migrate_phase3(&conn).unwrap();
    assert_eq!(first.status, "completed");
    assert_eq!(count(&conn, "canonical_migration_source_archive"), 4);
    assert_eq!(count(&conn, "objects"), 4);
    assert_eq!(count(&conn, "object_links"), 3);
    let (areas, headings) = retire_legacy_planning_tables(&conn).unwrap();
    assert_eq!((areas, headings), (1, 1));
    assert!(!table_exists(&conn, "areas"));
    assert!(!table_exists(&conn, "headings"));
    let data = load_all(&conn).unwrap();
    assert_eq!(
        data.areas.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
        ["area-1"]
    );
    assert_eq!(data.areas[0].sort_order, 7);
    assert_eq!(
        data.headings
            .iter()
            .map(|x| x.id.as_str())
            .collect::<Vec<_>>(),
        ["heading-1"]
    );
    assert_eq!(data.headings[0].project_id, "project-1");
    let area_object = data.objects.iter().find(|x| x.id == "area-1").unwrap();
    assert_eq!(
        area_object.props_json["extensions"]["compatibility"]["legacyKind"],
        "area"
    );
    assert_eq!(
        area_object.props_json["extensions"]["compatibility"]["sortOrder"],
        7
    );
    let heading_object = data.objects.iter().find(|x| x.id == "heading-1").unwrap();
    assert_eq!(
        heading_object.props_json["extensions"]["compatibility"]["legacyParentProjectId"],
        "project-1"
    );
    let task = data.objects.iter().find(|x| x.id == "todo-1").unwrap();
    assert_eq!(
        task.props_json["extensions"]["compatibility"]["areaId"],
        "area-1"
    );
    assert_eq!(
        task.props_json["extensions"]["compatibility"]["headingId"],
        "heading-1"
    );
    assert_eq!(count(&conn, "canonical_migration_source_archive"), 4);

    drop(conn);
    let reopened = open_db(path.to_str().unwrap()).unwrap();
    init_schema(&reopened).unwrap();
    assert!(!table_exists(&reopened, "areas"));
    assert!(!table_exists(&reopened, "headings"));
    assert_eq!(load_all(&reopened).unwrap().areas.len(), 1);
    assert_eq!(load_all(&reopened).unwrap().headings.len(), 1);
    assert_eq!(retire_legacy_planning_tables(&reopened).unwrap(), (0, 0));
}

#[test]
fn phase9_retirement_refuses_archive_or_semantic_mismatch_and_rolls_back_drop_batch() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema_prerequisites_for_phase3(&conn).unwrap();
    conn.execute_batch(LEGACY_AREAS_HEADINGS_SCHEMA).unwrap();
    conn.execute_batch(
        "INSERT INTO projects(id,title,status,area_id,created_at) VALUES('project-1','Project',
         'active','area-1','2026-01-02T00:00:00Z');
         INSERT INTO headings(id,title,sort_order,project_id) VALUES('heading-1','Section',3,
         'project-1');",
    )
    .unwrap();
    migrate_phase3(&conn).unwrap();

    let original_raw: Vec<u8> = conn
        .query_row(
            "SELECT raw_source FROM canonical_migration_source_archive WHERE
             source_kind='native:areas' AND source_id='area-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    conn.execute(
        "UPDATE canonical_migration_source_archive SET raw_source=?1 WHERE
         source_kind='native:areas' AND source_id='area-1'",
        [b"tampered".as_slice()],
    )
    .unwrap();
    assert!(retire_legacy_planning_tables(&conn).is_err());
    assert!(table_exists(&conn, "areas"));
    assert!(table_exists(&conn, "headings"));
    conn.execute(
        "UPDATE canonical_migration_source_archive SET raw_source=?1 WHERE
         source_kind='native:areas' AND source_id='area-1'",
        [original_raw.as_slice()],
    )
    .unwrap();

    let original_props: String = conn
        .query_row(
            "SELECT props_json FROM objects WHERE id='area-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    conn.execute("UPDATE objects SET props_json='{}' WHERE id='area-1'", [])
        .unwrap();
    assert!(retire_legacy_planning_tables(&conn).is_err());
    assert!(table_exists(&conn, "areas"));
    conn.execute(
        "UPDATE objects SET props_json=?1 WHERE id='area-1'",
        [original_props.as_str()],
    )
    .unwrap();

    // A view with the legacy name makes the second DROP fail; the savepoint
    // restores the areas table.
    conn.execute_batch(
        "DROP TABLE headings;
         CREATE VIEW headings AS SELECT 'heading-1' AS id, 'Section' AS title, 3 AS sort_order,\
 'project-1' AS project_id;",
    )
    .unwrap();
    assert!(retire_legacy_planning_tables(&conn).is_err());
    assert!(table_exists(&conn, "areas"));
    assert!(!table_exists(&conn, "headings"));
    assert!(conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='view' AND name='headings')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .unwrap());
}

#[test]
fn init_schema_tolerates_blocked_phase3_plan_and_keeps_legacy_tables() {
    // KOS-89: pre-phase3 DB where a legacy source fails canonical mapping →
    // plan reports "blocked" and migrate_phase3 returns early. init_schema
    // must not retire legacy tables unconditionally: the engine has to start,
    // legacy tables stay readable via the compatibility view, and retirement
    // stays armed for the next successful migration.
    let conn = Connection::open_in_memory().unwrap();
    init_schema_prerequisites_for_phase3(&conn).unwrap();
    phase3_legacy_fixtures::seed_historical_legacy_authorities(&conn);
    conn.execute_batch(LEGACY_AREAS_HEADINGS_SCHEMA).unwrap();
    conn.execute_batch(
        "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,created_at,
         updated_at) VALUES('dangling-note','note_obj','0.0.0-legacy','n','{}',
         '{\"relatedNotes\":[\"missing-target\"]}','2026-01-01T00:00:00.000Z',
         '2026-01-01T00:00:00.000Z');",
    )
    .unwrap();

    init_schema(&conn).unwrap();
    init_schema(&conn).unwrap();
    assert!(table_exists(&conn, "areas"));
    assert!(table_exists(&conn, "headings"));
    assert!(retire_legacy_planning_tables(&conn).is_err());
    assert_eq!(count(&conn, "areas"), 1);
}

#[test]
fn production_init_runs_phase3_once_and_reopen_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    for table in [
        "object_local_state",
        "object_sync_versions",
        "object_migration_quarantine",
        "legacy_type_definition_archive",
        "canonical_migration_runs",
        "canonical_migration_items",
    ] {
        assert!(table_exists(&conn, table), "missing {table}");
    }
    assert_eq!(count(&conn, "object_types"), 9);
    assert_eq!(count(&conn, "object_type_versions"), 11);
    assert_eq!(count(&conn, "object_type_aliases"), 9);
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM object_types WHERE system_locked=1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap(),
        9
    );
    let before: (i64, String) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(completed_at), '') FROM canonical_migration_runs",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    init_schema(&conn).unwrap();
    let after: (i64, String) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(completed_at), '') FROM canonical_migration_runs",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn historical_malformed_registry_fails_closed_before_phase3_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys=ON;
         CREATE TABLE object_types(id TEXT PRIMARY KEY, name TEXT NOT NULL, schema_json TEXT \
         NOT NULL, ui_schema_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, \
         system_locked INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE object_type_versions(type_id TEXT NOT NULL, version TEXT NOT NULL, \
         schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL, content_contract_json TEXT NOT NULL, \
         relations_json TEXT NOT NULL, sync_policy_json TEXT NOT NULL, schema_hash TEXT NOT NULL, \
         created_at TEXT NOT NULL, PRIMARY KEY(type_id,version));
         CREATE TABLE object_type_aliases(alias TEXT PRIMARY KEY, canonical_type_id TEXT NOT \
         NULL, created_at TEXT NOT NULL);
         INSERT INTO object_types VALUES('com.kosmos.note','wrong','{}','{}','old','old',1);
         INSERT INTO object_type_aliases VALUES('note_obj','com.kosmos.note','old');",
    )
    .unwrap();
    let original: (String, String) = conn
        .query_row(
            "SELECT name, canonical_type_id FROM object_types JOIN object_type_aliases ON 1=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(init_schema(&conn).is_err());
    assert_eq!(
        conn.query_row(
            "SELECT name, canonical_type_id FROM object_types JOIN object_type_aliases ON 1=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap(),
        original
    );
    if table_exists(&conn, "canonical_migration_runs") {
        assert_eq!(count(&conn, "canonical_migration_runs"), 0);
    }
    drop(conn);
    let reopened = Connection::open(path).unwrap();
    assert_eq!(
        reopened
            .query_row("SELECT name FROM object_types", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "wrong"
    );
}

#[test]
fn clear_all_removes_ephemeral_rows_but_keeps_locked_registry_and_archive() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    conn.execute_batch(
        "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,created_at,
         updated_at) VALUES('obj-marker','com.kosmos.note','1.0.0','marker','{}','{}','now',
         'now');INSERT INTO object_links(id,source_object_id,target_object_id,link_type,created_at)
         VALUES('link-marker','obj-marker','obj-marker','marker','now');INSERT INTO
         object_local_state(object_id,device_id,data_json,updated_at) VALUES('obj-marker','dev',
         'local-marker','now');INSERT INTO object_sync_versions(object_id,hlc,deleted)
         VALUES('obj-marker','marker-hlc',0);INSERT INTO object_migration_quarantine(object_id,
         contract_version,source_type_id,fields_json,source_hash,updated_at) VALUES('obj-marker',
         'phase3-canonical-v1','com.kosmos.note','quarantine-marker','hash','now')",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO canonical_migration_items(contract_version,source_kind,source_id,source_hash,
         raw_source,status,updated_at) VALUES(?1,?2,?3,?4,?5,'unchanged','now')",
        params!["phase3-canonical-v1", "marker", "marker", "hash", b"marker"],
    )
    .unwrap();
    let locked_before = count(&conn, "object_types");
    let archive_before = count(&conn, "legacy_type_definition_archive");
    clear_all(&conn).unwrap();
    for table in [
        "objects",
        "object_links",
        "object_local_state",
        "object_sync_versions",
        "object_migration_quarantine",
        "canonical_migration_items",
        "canonical_migration_runs",
    ] {
        assert_eq!(count(&conn, table), 0, "{table} not cleared");
    }
    assert_eq!(count(&conn, "object_types"), locked_before);
    assert_eq!(
        count(&conn, "legacy_type_definition_archive"),
        archive_before
    );
}

#[test]
fn internal_markers_are_not_exposed_by_load_all() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    conn.execute(
        "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,created_at,
         updated_at) VALUES('obj-public','com.kosmos.note','1.0.0','Public object','{}','{}','now',
         'now')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO object_local_state(object_id,device_id,data_json,updated_at)
         VALUES('obj-public','dev','LOCAL_INTERNAL_MARKER','now')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES('obj-public',
         'SYNC_INTERNAL_MARKER',0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO object_migration_quarantine(object_id,contract_version,source_type_id,
         fields_json,source_hash,updated_at) VALUES('obj-public','phase3-canonical-v1',
         'com.kosmos.note','QUARANTINE_INTERNAL_MARKER','hash','now')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO canonical_migration_items(contract_version,source_kind,source_id,source_hash,
         raw_source,status,updated_at) VALUES('phase3-canonical-v1','marker','marker','hash',
         X'4c45444745525f494e5445524e414c5f4d41524b4552','unchanged','now')",
        [],
    )
    .unwrap();
    let exported = serde_json::to_string(&load_all(&conn).unwrap()).unwrap();
    assert!(exported.contains("Public object"));
    for marker in [
        "LOCAL_INTERNAL_MARKER",
        "SYNC_INTERNAL_MARKER",
        "QUARANTINE_INTERNAL_MARKER",
        "LEDGER_INTERNAL_MARKER",
    ] {
        assert!(
            !exported.contains(marker),
            "internal marker leaked: {marker}"
        );
    }
}

#[test]
fn migration_init_emits_no_search_or_sync_events() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    assert_eq!(count(&conn, "object_sync_versions"), 0);
    assert_eq!(count(&conn, "canonical_migration_items"), 0);
    assert_eq!(count(&conn, "canonical_migration_runs"), 1);
    init_schema(&conn).unwrap();
    assert_eq!(count(&conn, "canonical_migration_runs"), 1);
    assert_eq!(count(&conn, "object_sync_versions"), 0);
    if table_exists(&conn, "object_search_fts") {
        assert_eq!(count(&conn, "object_search_fts"), 0);
    }
}

#[test]
fn raw_backup_preserves_nonempty_internal_blobs_and_schema() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.sqlite");
    let dest = dir.path().join("backup.sqlite");
    let conn = open_db(source.to_str().unwrap()).unwrap();
    init_schema(&conn).unwrap();
    conn.execute(
        "INSERT INTO canonical_migration_items(contract_version,source_kind,source_id,source_hash,
         raw_source,status,updated_at) VALUES(?1,?2,?3,?4,?5,'unchanged','now')",
        params![
            "phase3-canonical-v1",
            "marker",
            "blob",
            "hash",
            b"NONEMPTY_BACKUP_BLOB"
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO object_local_state(object_id,device_id,data_json,updated_at) SELECT id,
         'backup-device','BACKUP_LOCAL_MARKER','now' FROM objects LIMIT 1",
        [],
    )
    .unwrap();
    let source_item: (Vec<u8>, String) = conn
        .query_row(
            "SELECT raw_source, result_json FROM canonical_migration_items WHERE source_id='blob'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    backup_to_file(&conn, dest.to_str().unwrap()).unwrap();
    let reopened = open_db(dest.to_str().unwrap()).unwrap();
    let backup_item: (Vec<u8>, String) = reopened
        .query_row(
            "SELECT raw_source, result_json FROM canonical_migration_items WHERE source_id='blob'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(source_item, backup_item);
    assert_eq!(
        count(&reopened, "canonical_migration_items"),
        count(&conn, "canonical_migration_items")
    );
    let source_schema: Vec<String> = conn
        .prepare("SELECT name, sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
        .unwrap()
        .query_map([], |r| {
            Ok(format!(
                "{}:{}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let backup_schema: Vec<String> = reopened
        .prepare("SELECT name, sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
        .unwrap()
        .query_map([], |r| {
            Ok(format!(
                "{}:{}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(source_schema, backup_schema);
}

#[allow(dead_code)]
fn _json_marker() -> serde_json::Value {
    json!({"marker": "internal"})
}
