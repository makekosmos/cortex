use ark_core::canonical_types::migration::{
    migrate_phase3, migrate_phase3_with_options, plan_phase3, MigrationOptions,
};
use ark_core::db::init_schema_prerequisites_for_phase3;
use rusqlite::{params, Connection};
use serde_json::json;

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    init_schema_prerequisites_for_phase3(&conn).unwrap();
    conn
}

fn legacy(conn: &Connection, id: &str, alias: &str, props: serde_json::Value) {
    conn.execute(
        concat!(
            "INSERT OR IGNORE INTO object_types(id,name,schema_json,ui_schema_json,",
            "created_at,updated_at) VALUES(?1,'legacy','{}','{}','c','u')"
        ),
        [alias],
    )
    .unwrap();
    conn.execute(
        concat!(
            "UPDATE object_types SET owner_kind='package',owner_id='fixture',",
            "current_version='0.0.0-legacy',status='active',system_locked=0 WHERE id=?1"
        ),
        [alias],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT OR IGNORE INTO object_type_versions(type_id,version,schema_json,",
            "ui_schema_json,content_contract_json,relations_json,sync_policy_json,",
            "schema_hash,created_at) VALUES(?1,'0.0.0-legacy','{}','{}','{}','[]','{}',",
            "'legacy-hash','c')"
        ),
        [alias],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
            "created_at,updated_at,deleted_at) VALUES(?1,?2,'0.0.0-legacy',?1,",
            "'{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}',?3,",
            "'2026-01-01T00:00:00Z','2026-01-02T00:00:00Z',NULL)"
        ),
        params![id, alias, serde_json::to_string(&props).unwrap()],
    )
    .unwrap();
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn populated_fixture_reaches_real_orchestrator_and_preserves_source_only_rows() {
    let conn = db();
    let aliases = [
        ("n", "note_obj", json!({"description":"memo"})),
        (
            "t",
            "task_obj",
            json!({"status":"done","priority":3,"is_today":true}),
        ),
        (
            "p",
            "project_obj",
            json!({"status":"completed","color":"blue"}),
        ),
        ("tag", "tag_obj", json!({"color":"red"})),
        (
            "person",
            "person_obj",
            json!({"first_name":"Ada","last_name":"Lovelace"}),
        ),
        (
            "image",
            "image_obj",
            json!({"file_name":"a.png","mime_type":"image/png","size_bytes":7}),
        ),
        (
            "time",
            "time_entry_obj",
            json!({"started_at":"2026-01-01T00:00:00Z","source":"manual"}),
        ),
        (
            "game",
            "game_obj",
            json!(
                {"play_status":"completed",
                "genres":["rpg"],
                "total_playtime_seconds":42,
                "exe_path":"/games/a"}),
        ),
        (
            "book",
            "book_obj",
            json!({"author":"A","page_count":10,"cover_image":"https://example.invalid/cover"}),
        ),
    ];
    for (id, alias, props) in aliases {
        legacy(&conn, id, alias, props);
    }
    conn.execute_batch(
        "CREATE TABLE areas (id TEXT PRIMARY KEY, title TEXT NOT NULL, sort_order INTEGER NOT NULL \
DEFAULT 0, created_at TEXT NOT NULL);
         CREATE TABLE headings (id TEXT PRIMARY KEY, title TEXT NOT NULL, sort_order INTEGER \
NOT NULL DEFAULT 0, project_id TEXT NOT NULL);",
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO todos(id,title,notes,priority,project_id,tag_ids,",
            "checklist_items,created_at) VALUES('todo-native','Todo','n',2,NULL,'[]',",
            "'[]','2026-01-01T00:00:00Z')"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO projects(id,title,notes,status,color_tag,created_at) VALUES(",
            "'project-native','Project','n','active','green','2026-01-01T00:00:00Z')"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO areas(id,title,created_at) VALUES('area-source','Area',",
            "'2026-01-01T00:00:00Z')"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO headings(id,title,project_id) VALUES('heading-source','Heading',",
            "'project-native')"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO tags(id,title,color,created_at) VALUES('tag-native','Tag',",
            "'blue','2026-01-01T00:00:00Z')"
        ),
        [],
    )
    .unwrap();
    let report = migrate_phase3(&conn).unwrap();
    assert_eq!(
        (report.migrated, report.quarantined, report.blocked.len()),
        (13, 1, 0)
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM objects", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        14
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM object_links", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM object_local_state", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM object_migration_quarantine",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM canonical_migration_items", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap(),
        14
    );
    let archive: Vec<(String, String, String, String, String, String, String)> = conn
        .prepare(concat!(
            "SELECT legacy_type_id,canonical_type_id,summary_json,versions_json,",
            "inbound_aliases_json,source_hash,archived_at FROM ",
            "legacy_type_definition_archive ORDER BY legacy_type_id"
        ))
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(archive.len(), 9);
    assert_eq!(
        archive.iter().map(|row| row.0.as_str()).collect::<Vec<_>>(),
        vec![
            "book_obj",
            "game_obj",
            "image_obj",
            "note_obj",
            "person_obj",
            "project_obj",
            "tag_obj",
            "task_obj",
            "time_entry_obj"
        ]
    );
    for (legacy_id, canonical_id, summary, versions, aliases, source_hash, archived_at) in &archive
    {
        assert!(!canonical_id.is_empty());
        assert!(summary.starts_with('{'));
        assert!(versions.starts_with('['));
        assert!(aliases.starts_with('['));
        assert_eq!(source_hash.len(), 64);
        assert!(matches!(
            archived_at.as_str(),
            "u" | "1970-01-01T00:00:00.000Z"
        ));
        assert!(legacy_id.ends_with("_obj"));
    }
    let rerun = migrate_phase3(&conn).unwrap();
    assert_eq!(rerun.unchanged, 14);
    let archive_after: Vec<(String, String, String, String, String, String, String)> = conn
        .prepare(concat!(
            "SELECT legacy_type_id,canonical_type_id,summary_json,versions_json,",
            "inbound_aliases_json,source_hash,archived_at FROM ",
            "legacy_type_definition_archive ORDER BY legacy_type_id"
        ))
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(archive, archive_after);
    assert_eq!(
        conn.query_row("SELECT title FROM areas WHERE id='area-source'", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "Area"
    );
    assert_eq!(
        conn.query_row(
            "SELECT type_id,type_version FROM objects WHERE id='n'",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        )
        .unwrap(),
        ("com.kosmos.note".into(), "1.0.0".into())
    );
}

#[test]
fn blocked_preflight_is_read_only_and_retains_exact_raw_evidence() {
    let conn = db();
    legacy(&conn, "bad", "note_obj", json!({}));
    conn.execute("UPDATE objects SET content_json='{' WHERE id='bad'", [])
        .unwrap();
    legacy(&conn, "missing", "task_obj", json!({"project_id":"absent"}));
    let before: String = conn
        .query_row(
            "SELECT group_concat(name,'|') FROM sqlite_master ORDER BY name",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let plan = plan_phase3(&conn).unwrap();
    assert_eq!(plan.status, "blocked");
    assert!(plan
        .blocked
        .iter()
        .any(|x| x.source_id == "bad" && x.raw_source == b"{" && x.code == "MalformedJson"));
    assert!(plan
        .blocked
        .iter()
        .any(|x| x.source_id == "missing" && x.code == "INVALID_FIELD"));
    assert!(!table_exists(&conn, "canonical_migration_runs"));
    let after: String = conn
        .query_row(
            "SELECT group_concat(name,'|') FROM sqlite_master ORDER BY name",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
    let report = migrate_phase3(&conn).unwrap();
    assert_eq!(report.status, "blocked");
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM objects WHERE type_id LIKE 'com.kosmos.%'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn registry_fault_rolls_back_archive_registry_and_schema_bytes() {
    let conn = db();
    legacy(&conn, "n", "note_obj", json!({"description":"x"}));
    let before: Vec<(String, String)> = conn
        .prepare("SELECT id,name FROM object_types ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let result = migrate_phase3_with_options(
        &conn,
        &MigrationOptions {
            fail_after_registry_archives: Some(1),
            ..Default::default()
        },
    );
    assert!(result.is_err());
    assert_eq!(
        before,
        conn.prepare("SELECT id,name FROM object_types ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<(String, String)>, _>>()
            .unwrap()
    );
    assert!(!table_exists(&conn, "legacy_type_definition_archive"));
    assert!(!table_exists(&conn, "canonical_migration_runs"));
}
