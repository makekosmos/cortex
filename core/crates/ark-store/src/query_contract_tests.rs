#![allow(clippy::unwrap_used)]

use ark_domain::types::{ArkObject, ObjectType, TodoItem, TrackedApp, UsageEvent, UsageSession};
use rusqlite::{params, Connection};
use serde_json::json;

use crate::legacy_entities::upsert_todo;
use crate::objects::{
    list_object_summaries, list_object_summaries_by_type, list_objects_by_type,
    list_running_time_entries, search_objects,
};
use crate::schema::CREATE_TABLES;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .unwrap();
    crate::open::init_schema(&conn).unwrap();
    conn
}

fn make_object(id: &str, type_id: &str, title: &str) -> ArkObject {
    ArkObject {
        id: id.to_owned(),
        type_id: type_id.to_owned(),
        type_version: "0.0.0-legacy".to_owned(),
        title: title.to_owned(),
        content_json: json!({"type":"doc","content":[{"type":"paragraph"}]}),
        props_json: json!({"description":format!("Description for {title}")}),
        created_at: "2026-01-01T00:00:00.000Z".to_owned(),
        updated_at: "2026-01-01T00:00:00.000Z".to_owned(),
        deleted_at: None,
    }
}

fn make_object_type(id: &str, name: &str) -> ObjectType {
    ObjectType {
        id: id.to_owned(),
        name: name.to_owned(),
        schema_json: "{}".to_owned(),
        ui_schema_json: "{}".to_owned(),
        created_at: "2026-01-01T00:00:00.000Z".to_owned(),
        updated_at: "2026-01-01T00:00:00.000Z".to_owned(),
        system_locked: false,
    }
}

fn make_todo(id: &str, title: &str) -> TodoItem {
    TodoItem {
        id: id.to_owned(),
        title: title.to_owned(),
        notes: None,
        priority: 0,
        scheduled_date: None,
        deadline: None,
        reminder_date: None,
        is_today: false,
        is_evening: false,
        is_someday: false,
        is_completed: false,
        completed_at: None,
        is_cancelled: false,
        cancelled_at: None,
        is_trashed: false,
        sort_order: 0,
        heading_id: None,
        project_id: None,
        area_id: None,
        tag_ids: vec![],
        checklist_items: json!([]),
        recurrence_rule: None,
        created_at: "2026-01-01T00:00:00.000Z".to_owned(),
    }
}

fn make_tracked_app(id: &str) -> TrackedApp {
    TrackedApp {
        id: id.to_owned(),
        platform: "windows".to_owned(),
        exe_path: String::new(),
        normalized_exe_path: String::new(),
        process_name: "demo.exe".to_owned(),
        display_name: None,
        publisher: None,
        icon_ref: None,
        first_seen_at: "2026-01-01T00:00:00.000Z".to_owned(),
        last_seen_at: "2026-01-01T00:00:00.000Z".to_owned(),
    }
}

fn make_usage_session(id: &str, tracked_app_id: &str) -> UsageSession {
    UsageSession {
        id: id.to_owned(),
        tracked_app_id: tracked_app_id.to_owned(),
        device_id: "device-1".to_owned(),
        device_name: "Test Device".to_owned(),
        platform: "windows".to_owned(),
        started_at: "2026-01-01T00:00:00.000Z".to_owned(),
        ended_at: Some("2026-01-01T00:10:00.000Z".to_owned()),
        runtime_ms: 600_000,
        foreground_ms: 600_000,
        idle_ms: 0,
        window_title: None,
        process_name: "demo.exe".to_owned(),
        exe_path: String::new(),
        pid_start: None,
        pid_end: None,
        meta_json: json!({}),
    }
}

fn make_usage_event(id: &str, tracked_app_id: &str, session_id: Option<&str>) -> UsageEvent {
    UsageEvent {
        id: id.to_owned(),
        tracked_app_id: tracked_app_id.to_owned(),
        usage_session_id: session_id.map(str::to_owned),
        device_id: "device-1".to_owned(),
        device_name: "Test Device".to_owned(),
        platform: "windows".to_owned(),
        occurred_at: "2026-01-01T00:05:00.000Z".to_owned(),
        kind: "foreground".to_owned(),
        window_title: None,
        process_name: "demo.exe".to_owned(),
        exe_path: String::new(),
        pid: None,
        is_foreground: true,
        is_idle: false,
        meta_json: json!({}),
    }
}

#[test]
fn object_summary_queries_skip_body_and_filter_by_type() {
    let conn = setup_db();
    let object_type = make_object_type("summary_fixture_book_type", "Book");
    crate::type_registry::upsert_object_type(&conn, &object_type).unwrap();
    crate::type_registry::upsert_object_type(
        &conn,
        &make_object_type("summary_fixture_type", "Journal"),
    )
    .unwrap();

    let mut older = make_object("obj-1", "summary_fixture_type", "Journal");
    older.content_json = json!({ "text": "large body that must not be selected by summaries" });
    older.props_json = json!({ "kind": "note" });
    older.created_at = "2026-01-01T00:00:00.000Z".to_string();
    older.updated_at = "2026-01-01T00:00:00.000Z".to_string();
    crate::objects::upsert_object_compat(&conn, &older).unwrap();

    let mut newer = make_object("obj-2", "summary_fixture_book_type", "Clean Code");
    newer.content_json = json!({ "text": "another body" });
    newer.props_json = json!({ "kind": "book" });
    newer.created_at = "2026-01-02T00:00:00.000Z".to_string();
    newer.updated_at = "2026-01-02T00:00:00.000Z".to_string();
    crate::objects::upsert_object_compat(&conn, &newer).unwrap();

    let summaries = list_object_summaries(&conn).unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].id, "obj-2");
    assert_eq!(summaries[0].props_json, json!({ "kind": "book" }));
    assert_eq!(summaries[1].id, "obj-1");

    let book_summaries = list_object_summaries_by_type(&conn, "summary_fixture_book_type").unwrap();
    assert_eq!(book_summaries.len(), 1);
    assert_eq!(book_summaries[0].id, "obj-2");
    assert_eq!(book_summaries[0].type_id, "summary_fixture_book_type");
}

fn make_time_entry(id: &str, started_at: &str, ended_at: Option<&str>, source: &str) -> ArkObject {
    ArkObject {
        id: id.to_string(),
        type_id: "time_entry_obj".to_string(),
        type_version: "0.0.0-legacy".to_string(),
        title: format!("entry {id}"),
        content_json: json!({}),
        props_json: json!({
            "startedAt": started_at,
            "endedAt": ended_at,
            "source": source,
        }),
        created_at: started_at.to_string(),
        updated_at: started_at.to_string(),
        deleted_at: None,
    }
}

#[test]
fn list_running_time_entries_empty_db_returns_empty() {
    let conn = setup_db();
    let result = list_running_time_entries(&conn, None).unwrap();
    assert!(result.is_empty());
    let result_filtered = list_running_time_entries(&conn, Some("manual")).unwrap();
    assert!(result_filtered.is_empty());
}

#[test]
fn list_running_time_entries_returns_only_running() {
    let conn = setup_db();
    crate::objects::upsert_object_compat(
        &conn,
        &make_time_entry("te-running", "2026-05-20T10:00:00.000Z", None, "manual"),
    )
    .unwrap();
    crate::objects::upsert_object_compat(
        &conn,
        &make_time_entry(
            "te-done",
            "2026-05-20T08:00:00.000Z",
            Some("2026-05-20T09:00:00.000Z"),
            "manual",
        ),
    )
    .unwrap();

    let result = list_running_time_entries(&conn, None).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "te-running");
}

#[test]
fn list_running_time_entries_filter_by_source() {
    let conn = setup_db();
    crate::objects::upsert_object_compat(
        &conn,
        &make_time_entry("te-manual", "2026-05-20T10:00:00.000Z", None, "manual"),
    )
    .unwrap();
    crate::objects::upsert_object_compat(
        &conn,
        &make_time_entry("te-pomo", "2026-05-20T11:00:00.000Z", None, "pomodoro"),
    )
    .unwrap();
    crate::objects::upsert_object_compat(
        &conn,
        &make_time_entry(
            "te-break",
            "2026-05-20T12:00:00.000Z",
            None,
            "pomodoro_break",
        ),
    )
    .unwrap();

    let manual = list_running_time_entries(&conn, Some("manual")).unwrap();
    assert_eq!(manual.len(), 1);
    assert_eq!(manual[0].id, "te-manual");

    let pomo = list_running_time_entries(&conn, Some("pomodoro")).unwrap();
    assert_eq!(pomo.len(), 1);
    assert_eq!(pomo[0].id, "te-pomo");

    let breaks = list_running_time_entries(&conn, Some("pomodoro_break")).unwrap();
    assert_eq!(breaks.len(), 1);
    assert_eq!(breaks[0].id, "te-break");

    // Р‘РµР· С„РёР»СЊС‚СЂР° вЂ” РІСЃРµ С‚СЂРё, РѕС‚СЃРѕСЂС‚РёСЂРѕРІР°РЅС‹ DESC РїРѕ startedAt.
    let all = list_running_time_entries(&conn, None).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].id, "te-break");
    assert_eq!(all[1].id, "te-pomo");
    assert_eq!(all[2].id, "te-manual");
}

#[test]
fn list_running_time_entries_excludes_deleted() {
    let conn = setup_db();
    let mut deleted = make_time_entry("te-del", "2026-05-20T10:00:00.000Z", None, "manual");
    deleted.deleted_at = Some("2026-05-20T10:30:00.000Z".to_string());
    crate::objects::upsert_object_compat(&conn, &deleted).unwrap();

    let result = list_running_time_entries(&conn, None).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_object_search_fts_rebuilds_existing_objects_on_init() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(CREATE_TABLES).unwrap();

    let object_type = make_object_type("search_obj", "Searchable");
    crate::type_registry::upsert_object_type(&conn, &object_type).unwrap();
    let mut object = make_object("obj-existing-search", "search_obj", "Existing Object");
    object.props_json = json!({
        "description": "preexisting nebula archive"
    });
    crate::objects::upsert_object_compat(&conn, &object).unwrap();

    assert!(
        !crate::objects::object_search_fts_exists(&conn).unwrap(),
        "legacy DB setup should not have the FTS table yet"
    );

    crate::open::init_schema(&conn).unwrap();

    let results = search_objects(&conn, "nebula").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry_id, "obj-existing-search");
}

#[test]
fn test_object_search_fts_updates_and_removes_index_entries() {
    let conn = setup_db();
    let object_type = make_object_type("search_obj", "Searchable");
    crate::type_registry::upsert_object_type(&conn, &object_type).unwrap();

    let mut object = make_object("obj-search-update", "search_obj", "Search Target");
    object.props_json = json!({ "description": "alpha marker" });
    crate::objects::upsert_object_compat(&conn, &object).unwrap();
    assert_eq!(
        search_objects(&conn, "alpha").unwrap()[0].entry_id,
        object.id
    );

    object.props_json = json!({ "description": "beta marker" });
    object.updated_at = "2026-01-02T00:00:00.000Z".to_string();
    crate::objects::upsert_object_compat(&conn, &object).unwrap();
    assert!(
        search_objects(&conn, "alpha").unwrap().is_empty(),
        "old FTS text should be removed on object update"
    );
    assert_eq!(
        search_objects(&conn, "beta").unwrap()[0].entry_id,
        object.id
    );

    crate::objects::delete_object(&conn, &object.id).unwrap();
    assert!(
        search_objects(&conn, "beta").unwrap().is_empty(),
        "deleted objects should be removed from the FTS index"
    );
}

#[test]
fn test_object_search_is_punctuation_safe_and_falls_back_without_fts() {
    let conn = setup_db();
    let object_type = make_object_type("search_obj", "Searchable");
    crate::type_registry::upsert_object_type(&conn, &object_type).unwrap();
    let mut object = make_object("obj-punctuation-search", "search_obj", "C++ Primer");
    object.props_json = json!({
        "description": "boss-fight co-op notes"
    });
    crate::objects::upsert_object_compat(&conn, &object).unwrap();

    let punctuation_results = search_objects(&conn, "boss-fight??").unwrap();
    assert_eq!(punctuation_results.len(), 1);
    assert_eq!(punctuation_results[0].entry_id, "obj-punctuation-search");

    conn.execute_batch("DROP TABLE object_search_fts").unwrap();
    let fallback_results = search_objects(&conn, "co-op").unwrap();
    assert_eq!(fallback_results.len(), 1);
    assert_eq!(fallback_results[0].entry_id, "obj-punctuation-search");
}

#[test]
fn test_sync_kv() {
    let conn = setup_db();
    assert_eq!(crate::sync_kv::get_sync_kv(&conn, "foo").unwrap(), None);
    crate::sync_kv::set_sync_kv(&conn, "foo", "bar").unwrap();
    assert_eq!(
        crate::sync_kv::get_sync_kv(&conn, "foo").unwrap(),
        Some("bar".to_string())
    );
}

#[test]
fn test_clear_all() {
    let conn = setup_db();
    upsert_todo(&conn, &make_todo("t1", "X")).unwrap();
    crate::usage::upsert_tracked_app(&conn, &make_tracked_app("app-clear")).unwrap();
    crate::usage::upsert_usage_session(&conn, &make_usage_session("session-clear", "app-clear"))
        .unwrap();
    crate::usage::upsert_usage_event(
        &conn,
        &make_usage_event("event-clear", "app-clear", Some("session-clear")),
    )
    .unwrap();
    crate::sync_kv::set_sync_kv(&conn, "k", "v").unwrap();
    crate::load_all::clear_all(&conn).unwrap();
    let data = crate::load_all::load_all(&conn).unwrap();
    assert_eq!(data.todos.len(), 0);
    assert_eq!(data.tracked_apps.len(), 0);
    assert_eq!(data.usage_sessions.len(), 0);
    assert_eq!(data.usage_events.len(), 0);
    assert_eq!(data.objects.len(), 0);
    assert_eq!(data.object_links.len(), 0);
    assert!(data
        .object_types
        .iter()
        .any(|item| item.id == "com.kosmos.note"));
    assert!(data
        .object_types
        .iter()
        .any(|item| item.id == "com.kosmos.game"));
    assert_eq!(crate::sync_kv::get_sync_kv(&conn, "k").unwrap(), None);
}

#[test]
fn test_delete_trashed() {
    let conn = setup_db();
    let mut todo = make_todo("t1", "Keep");
    upsert_todo(&conn, &todo).unwrap();

    todo.id = "t2".to_string();
    todo.title = "Trash me".to_string();
    todo.is_trashed = true;
    upsert_todo(&conn, &todo).unwrap();

    let count = crate::load_all::delete_trashed(&conn, "device-under-test").unwrap();
    assert_eq!(count, 1);
    let data = crate::load_all::load_all(&conn).unwrap();
    assert_eq!(data.todos.len(), 1);
    assert_eq!(data.todos[0].id, "t1");
}

#[test]
fn by_type_queries_match_objects_stored_under_legacy_alias() {
    let conn = setup_db();
    // Rows written before the type registry existed keep `task_obj` in
    // objects.type_id, which never was a row in object_types. Seeding the
    // row directly avoids the compatibility writer canonicalising the ID,
    // so this connection disables FK checks for the fixture.
    conn.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
    for (id, deleted) in [
        ("legacy-task-1", None),
        ("legacy-task-2", None),
        ("legacy-task-gone", Some("2026-02-01T00:00:00.000Z")),
    ] {
        conn.execute(
            "INSERT INTO objects (id,type_id,type_version,title,content_json,props_json,
                 created_at,updated_at,deleted_at)
                 VALUES (?1,'task_obj','0.0.0-legacy',?1,'{}','{}','2026-01-01T00:00:00.000Z',
                 '2026-01-02T00:00:00.000Z',?2)",
            params![id, deleted],
        )
        .unwrap();
    }
    for queried in ["com.kosmos.task", "task_obj"] {
        let summaries = list_object_summaries_by_type(&conn, queried).unwrap();
        assert_eq!(
            summaries.len(),
            3,
            "{queried} must see alias-stored objects"
        );
        let objects = list_objects_by_type(&conn, queried).unwrap();
        assert_eq!(objects.len(), 3);
    }

    // Same contract for the running-time-entry hot path.
    let running = make_time_entry("entry-legacy", "2026-03-01T10:00:00.000Z", None, "manual");
    conn.execute(
        "INSERT INTO objects (id,type_id,type_version,title,content_json,props_json,created_at,
             updated_at,deleted_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL)",
        params![
            running.id,
            running.type_id,
            running.type_version,
            running.title,
            serde_json::to_string(&running.content_json).unwrap(),
            serde_json::to_string(&running.props_json).unwrap(),
            running.created_at,
            running.updated_at,
        ],
    )
    .unwrap();
    let running_entries = list_running_time_entries(&conn, Some("manual")).unwrap();
    assert_eq!(running_entries.len(), 1);
    assert_eq!(running_entries[0].id, "entry-legacy");
}
