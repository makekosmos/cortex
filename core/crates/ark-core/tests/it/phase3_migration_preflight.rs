#![allow(clippy::unwrap_used)]
use ark_core::canonical_types::preflight::{inventory_sources, preflight_phase3, SourceKind};
use ark_core::db::init_schema_prerequisites_for_phase3;
use rusqlite::{params, Connection};

use crate::phase3_legacy_fixtures;

#[test]
fn preflight_inventories_native_rows_and_is_read_only() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema_prerequisites_for_phase3(&conn).unwrap();
    conn.execute_batch(
        "CREATE TABLE areas (id TEXT PRIMARY KEY, title TEXT NOT NULL, sort_order INTEGER NOT NULL
         DEFAULT 0, created_at TEXT NOT NULL);
         CREATE TABLE headings (id TEXT PRIMARY KEY, title TEXT NOT NULL, sort_order INTEGER NOT
         NULL DEFAULT 0, project_id TEXT NOT NULL);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO areas(id,title,sort_order,created_at) VALUES ('area-1','Area',7,'a')",
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO projects(id,title,notes,status,scheduled_date,deadline,",
            "sort_order,color_tag,area_id,created_at) VALUES ('project-1','Project','n',",
            "'active','2026-01-01',NULL,4,'blue','area-1','p')"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO headings(id,title,sort_order,project_id) VALUES ('heading-1',",
            "'Heading',8,'project-1')"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tags(id,title,color,created_at) VALUES ('tag-1','Tag','red','t')",
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO todos(id,title,notes,priority,scheduled_date,deadline,",
            "reminder_date,is_today,is_evening,is_someday,is_completed,completed_at,",
            "is_cancelled,cancelled_at,is_trashed,sort_order,heading_id,project_id,",
            "area_id,tag_ids,checklist_items,recurrence_rule,created_at) VALUES (",
            "'todo-1','Todo','n',3,'2026-01-02',NULL,NULL,1,0,0,0,NULL,0,NULL,0,9,",
            "'heading-1','project-1','area-1','[\"tag-1\"]','[]',NULL,'d')"
        ),
        [],
    )
    .unwrap();
    let before = conn
        .query_row(
            "SELECT COUNT(*), (SELECT COUNT(*) FROM sqlite_master)",
            [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .unwrap();
    let report = preflight_phase3(&conn).unwrap();
    assert_eq!(report.status, "ready");
    assert_eq!(
        report.types.iter().map(|t| t.before_count).sum::<usize>(),
        5
    );
    assert!(report
        .types
        .iter()
        .any(|t| t.source_kind == "todos" && t.before_ids == ["todo-1"]));
    let after = conn
        .query_row(
            "SELECT COUNT(*), (SELECT COUNT(*) FROM sqlite_master)",
            [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM objects", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn preflight_preserves_malformed_json_bytes_and_blocks_without_schema_mutation() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema_prerequisites_for_phase3(&conn).unwrap();
    phase3_legacy_fixtures::seed_historical_legacy_authorities(&conn);
    let raw = b"{";
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
            "created_at,updated_at,deleted_at) VALUES ('bad','note_obj','0.0.0-legacy',",
            "'Bad',?1,'{}','c','u',NULL)"
        ),
        params!["{"],
    )
    .unwrap();
    let before: String = conn
        .query_row(
            "SELECT group_concat(name, '|') FROM sqlite_master ORDER BY name",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let report = preflight_phase3(&conn).unwrap();
    assert_eq!(report.status, "blocked");
    let item = report.errors.iter().find(|e| e.source_id == "bad").unwrap();
    assert_eq!(item.code, "MalformedJson");
    assert_eq!(item.raw_source, raw);
    let after: String = conn
        .query_row(
            "SELECT group_concat(name, '|') FROM sqlite_master ORDER BY name",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name='canonical_migration_runs'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn inventory_hash_and_order_are_insertion_independent_for_generic_sources() {
    fn db(rows: &[(&str, &str)]) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema_prerequisites_for_phase3(&conn).unwrap();
        phase3_legacy_fixtures::seed_historical_legacy_authorities(&conn);
        for (id, props) in rows {
            conn.execute(
                concat!(
                    "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
                    "created_at,updated_at,deleted_at) VALUES (?1,'note_obj','0.0.0-legacy','n',",
                    "'{}',?2,'c','u',NULL)"
                ),
                params![id, props],
            )
            .unwrap();
        }
        conn
    }
    let a = inventory_sources(&db(&[("b", "{\"z\":2,\"a\":1}"), ("a", "{}")])).unwrap();
    let b = inventory_sources(&db(&[("a", "{}"), ("b", "{\"a\":1,\"z\":2}")])).unwrap();
    assert_eq!(
        a.iter()
            .map(|r| (&r.source_kind, &r.source_id, &r.source_hash))
            .collect::<Vec<_>>(),
        b.iter()
            .map(|r| (&r.source_kind, &r.source_id, &r.source_hash))
            .collect::<Vec<_>>(),
    );
    assert!(matches!(a[0].source_kind, SourceKind::Legacy(_)));
}
