use ark_core::canonical_types::migration_objects::{apply_plan, plan_objects};
use ark_core::db::init_schema_prerequisites_for_phase3;
use rusqlite::{params, Connection};
use serde_json::json;

fn db() -> Connection {
    let conn = Connection::open_in_memory().expect("sqlite");
    init_schema_prerequisites_for_phase3(&conn).expect("schema");
    for id in [
        "com.kosmos.note",
        "com.kosmos.task",
        "com.kosmos.project",
        "com.kosmos.tag",
        "com.kosmos.person",
        "com.kosmos.image",
        "com.kosmos.time-entry",
        "com.kosmos.game",
        "com.kosmos.book",
    ] {
        conn.execute(concat!("INSERT OR IGNORE INTO object_types(id,name,schema_json,ui_schema_json,","created_at,updated_at) VALUES(?1,'canonical','{}','{}','c','u')"), [id]).expect("canonical type");
    }
    conn
}

fn insert_generic(
    conn: &Connection,
    id: &str,
    alias: &str,
    props: serde_json::Value,
    deleted: Option<&str>,
) {
    conn.execute(concat!("INSERT OR IGNORE INTO object_types(id,name,schema_json,ui_schema_json,","created_at,updated_at) VALUES(?1,'legacy','{}','{}','c','u')"), [alias]).expect("legacy type");
    let content = if matches!(alias, "image_obj" | "game_obj") {
        json!({"type":"image"})
    } else {
        json!({"type":"doc","content":[{"type":"paragraph"}]})
    };
    conn.execute(concat!("INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,","created_at,updated_at,deleted_at) VALUES(?1,?2,'0.0.0-legacy',?3,?4,?5,","'2026-01-01T00:00:00Z','2026-01-02T00:00:00Z',?6)"), params![id, alias, alias, serde_json::to_string(&content).unwrap(), serde_json::to_string(&props).unwrap(), deleted]).expect("insert legacy object");
}

#[test]
fn populated_generic_fixture_maps_all_nine_aliases_and_applies_exact_envelope() {
    let conn = db();
    let fixtures = [
        ("note_obj", json!({"description":"memo"})),
        (
            "task_obj",
            json!({"status":"done","priority":3,"is_today":true}),
        ),
        ("project_obj", json!({"status":"completed","color":"blue"})),
        ("tag_obj", json!({"color":"red"})),
        (
            "person_obj",
            json!({"first_name":"Ada","last_name":"Lovelace"}),
        ),
        (
            "image_obj",
            json!({"file_name":"a.png","mime_type":"image/png","size_bytes":7,"alt_text":"a"}),
        ),
        (
            "time_entry_obj",
            json!({"started_at":"2026-01-01T00:00:00Z","source":"manual"}),
        ),
        (
            "game_obj",
            json!({"play_status":"completed","genres":["rpg"],"total_playtime_seconds":42}),
        ),
        (
            "book_obj",
            json!({"author":"A","page_count":10,"cover_image":"https://example.invalid/cover"}),
        ),
    ];
    for (index, (alias, props)) in fixtures.iter().enumerate() {
        insert_generic(
            &conn,
            &format!("g-{index}"),
            alias,
            props.clone(),
            (index == 8).then_some("2026-01-03T00:00:00Z"),
        );
    }
    let plan = plan_objects(&conn, "wall-clock-must-not-leak").expect("plan");
    assert_eq!(plan.items.len(), 9);
    // The URL cover is intentionally quarantine-only, not a plan blocker.
    assert_eq!(plan.blocked.len(), 0);
    apply_plan(&conn, &plan, "unused").expect("apply");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM objects",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 9);
    let expected_types = [
        "com.kosmos.note",
        "com.kosmos.task",
        "com.kosmos.project",
        "com.kosmos.tag",
        "com.kosmos.person",
        "com.kosmos.image",
        "com.kosmos.time-entry",
        "com.kosmos.game",
        "com.kosmos.book",
    ];
    for (index, expected_type) in expected_types.iter().enumerate() {
        let id = format!("g-{index}");
        let (type_id, version, created, updated, deleted): (String, String, String, String, Option<String>) = conn.query_row(concat!("SELECT type_id,type_version,created_at,updated_at,deleted_at FROM objects ","WHERE id=?1"), [&id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).unwrap();
        assert_eq!(type_id, *expected_type);
        // Mapped objects claim the newest registered version of their type.
        assert_eq!(
            version,
            if matches!(*expected_type, "com.kosmos.task" | "com.kosmos.project") {
                "1.1.0"
            } else {
                "1.0.0"
            }
        );
        assert_eq!(created, "2026-01-01T00:00:00Z");
        assert_eq!(updated, "2026-01-02T00:00:00Z");
        assert_eq!(
            deleted,
            (index == 8).then_some("2026-01-03T00:00:00Z".into())
        );
    }
}

#[test]
fn native_adapters_migrate_area_heading_identity_and_hierarchy() {
    let conn = db();
    conn.execute_batch(
        "CREATE TABLE areas (id TEXT PRIMARY KEY, title TEXT NOT NULL, sort_order INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL);
         CREATE TABLE headings (id TEXT PRIMARY KEY, title TEXT NOT NULL, sort_order INTEGER NOT NULL DEFAULT 0, project_id TEXT NOT NULL);",
    )
    .unwrap();
    conn.execute(concat!("INSERT INTO todos(id,title,notes,priority,project_id,tag_ids,","checklist_items,created_at) VALUES('todo-1','Todo','note',2,NULL,'[]','[]',","'2026-01-01T00:00:00Z')"), []).unwrap();
    conn.execute(concat!("INSERT INTO projects(id,title,notes,status,color_tag,created_at) VALUES(","'project-1','Project','notes','completed','green','2026-01-01T00:00:00Z')"), []).unwrap();
    conn.execute(
        "INSERT INTO areas(id,title,created_at) VALUES('area-1','Area','2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO headings(id,title,project_id) VALUES('heading-1','Heading','project-1')",
        [],
    )
    .unwrap();
    conn.execute(concat!("INSERT INTO tags(id,title,color,created_at) VALUES('tag-1','Tag','blue',","'2026-01-01T00:00:00Z')"), []).unwrap();
    let plan = plan_objects(&conn, "now").unwrap();
    assert_eq!(plan.items.len(), 5);
    assert!(plan.items.iter().any(|item| item.source_kind == "todos"
        && item.source_kind_variant
            == ark_core::canonical_types::preflight::SourceKind::Native("todos".into())));
    apply_plan(&conn, &plan, "now").unwrap();
    let objects: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM objects",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(objects, 5);
    assert_eq!(
        conn.query_row::<i64, _, _>(
            "SELECT COUNT(*) FROM objects WHERE id IN ('area-1','heading-1')",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        conn.query_row::<i64, _, _>(concat!("SELECT COUNT(*) FROM object_links WHERE source_object_id='heading-1' AND ","link_type='related' AND target_object_id='project-1'"), [], |r| r.get(0)).unwrap(),
        1
    );
}

#[test]
fn malformed_source_is_collected_and_apply_is_globally_refused() {
    let conn = db();
    conn.execute(concat!("INSERT OR IGNORE INTO object_types(id,name,schema_json,ui_schema_json,","created_at,updated_at) VALUES('note_obj','legacy','{}','{}','c','u')"), []).unwrap();
    conn.execute(concat!("INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,","created_at,updated_at) VALUES('bad','note_obj','0.0.0-legacy','bad',","'not-json','{}','c','u')"), []).unwrap();
    let plan = plan_objects(&conn, "now").unwrap();
    assert_eq!(plan.blocked.len(), 1);
    assert_eq!(plan.blocked[0].code, "MALFORMED_JSON");
    assert_eq!(plan.blocked[0].raw_source, b"not-json");
    assert!(apply_plan(&conn, &plan, "now").is_err());
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM canonical_migration_runs", [], |r| r
            .get(0))
            .unwrap_or(0),
        0
    );
}

#[test]
fn rerunning_identical_plan_is_idempotent_and_normalizes_json_order() {
    let conn = db();
    insert_generic(&conn, "n1", "note_obj", json!({"description":"x"}), None);
    let plan = plan_objects(&conn, "now").unwrap();
    apply_plan(&conn, &plan, "now").unwrap();
    let before: (i64,i64,String) = conn.query_row(concat!("SELECT (SELECT COUNT(*) FROM objects),(SELECT COUNT(*) FROM ","object_sync_versions),updated_at FROM objects WHERE id='n1'"), [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    apply_plan(&conn, &plan, "later").unwrap();
    let after: (i64,i64,String) = conn.query_row(concat!("SELECT (SELECT COUNT(*) FROM objects),(SELECT COUNT(*) FROM ","object_sync_versions),updated_at FROM objects WHERE id='n1'"), [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(before, after);
}

#[test]
fn link_id_collision_is_a_canonical_conflict_without_partial_object() {
    let conn = db();
    conn.execute(concat!("INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,","updated_at) VALUES('other','other','{}','{}','c','u')"), []).unwrap();
    conn.execute(concat!("INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,","created_at,updated_at) VALUES('other','other','1.0.0','other','{}','{}','c',","'u')"), []).unwrap();
    insert_generic(&conn, "project-1", "project_obj", json!({}), None);
    insert_generic(
        &conn,
        "task-1",
        "task_obj",
        json!({"project_id":"project-1"}),
        None,
    );
    let collision = ark_core::canonical_types::migration_objects::stable_link_id(
        "task-1",
        "project",
        "project-1",
    );
    conn.execute(concat!("INSERT INTO object_links(id,source_object_id,target_object_id,link_type,","created_at) VALUES(?1,'other','project-1','project','x')"), [&collision]).unwrap();
    let plan = plan_objects(&conn, "now").unwrap();
    assert!(apply_plan(&conn, &plan, "now").is_err());
    assert_eq!(
        conn.query_row::<i64, _, _>(
            concat!("SELECT COUNT(*) FROM objects WHERE id IN ('project-1','task-1') AND type_id ","LIKE 'com.kosmos.%'"),
            [],
            |r| r.get(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn typed_missing_relation_is_blocked_without_any_candidate_mutation() {
    let conn = db();
    insert_generic(
        &conn,
        "task-missing",
        "task_obj",
        json!({"project_id":"does-not-exist"}),
        None,
    );
    let plan = plan_objects(&conn, "now").unwrap();
    assert_eq!(plan.items.len(), 0);
    assert_eq!(plan.blocked.len(), 1);
    assert_eq!(plan.blocked[0].code, "INVALID_FIELD");
    assert_eq!(
        conn.query_row::<i64, _, _>(
            "SELECT COUNT(*) FROM objects WHERE id='task-missing' AND type_id='com.kosmos.task'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        0
    );
    assert!(apply_plan(&conn, &plan, "now").is_err());
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM canonical_migration_runs", [], |r| r
            .get(0))
            .unwrap_or(0),
        0
    );
}
