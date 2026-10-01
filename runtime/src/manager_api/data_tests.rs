use super::*;

/// Rows written before canonicalisation keep `task_obj`/`note_obj` in
/// `objects.type_id`. The objects FK points at `object_types.id`, which
/// never held those ids, so the fixture seeds through a raw connection
/// with FK checks off — exactly how the legacy writer produced them.
fn seed_object(conn: &rusqlite::Connection, id: &str, type_id: &str, deleted_at: Option<&str>) {
    conn.execute(
            "INSERT INTO objects (id,type_id,type_version,title,content_json,props_json,created_at,updated_at,deleted_at)
             VALUES (?1,?2,'0.0.0-legacy',?1,'{}','{}','2026-01-01T00:00:00.000Z','2026-01-02T00:00:00.000Z',?3)",
            rusqlite::params![id, type_id, deleted_at],
        )
        .unwrap();
}

async fn fixture() -> (
    tempfile::TempDir,
    Arc<ArkHost>,
    ManagerState,
    rusqlite::Connection,
) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("ark.db");
    let ark = Arc::new(ArkHost::open(db_path.to_str().unwrap()).await.unwrap());
    let seed = rusqlite::Connection::open(&db_path).unwrap();
    seed.execute_batch("PRAGMA foreign_keys = OFF; PRAGMA busy_timeout = 5000;")
        .unwrap();
    let state = ManagerState::new(dir.path().to_path_buf());
    (dir, ark, state, seed)
}

/// Regression for KOS-288: the page reported zero objects while ark.db
/// held tasks/notes stored under legacy alias ids.
#[tokio::test]
async fn data_page_counts_objects_stored_under_legacy_alias_ids() {
    let (_dir, ark, state, seed) = fixture().await;
    seed_object(&seed, "task-1", "task_obj", None);
    seed_object(&seed, "task-2", "task_obj", None);
    seed_object(
        &seed,
        "task-gone",
        "task_obj",
        Some("2026-02-01T00:00:00.000Z"),
    );
    seed_object(&seed, "note-1", "note_obj", None);
    seed_object(&seed, "workout-1", "workout_obj", None);
    seed_object(&seed, "workout-2", "workout_obj", None);

    let types = state.data_types(&ark).await.unwrap();
    let types = types.as_array().unwrap();
    let count_of = |id: &str| {
        types
            .iter()
            .find(|t| t["type_id"] == id)
            .map(|t| t["count"].as_u64().unwrap())
    };
    assert_eq!(
        count_of("com.kosmos.task"),
        Some(2),
        "deleted task excluded"
    );
    assert_eq!(count_of("com.kosmos.note"), Some(1));
    assert_eq!(count_of("workout_obj"), Some(2));

    for queried in ["com.kosmos.task", "task_obj"] {
        let list = state
            .data_list(&ark, &json!({"type_id": queried, "limit": 50}))
            .await
            .unwrap();
        let items = list["items"].as_array().unwrap();
        assert_eq!(items.len(), 2, "{queried} must list alias-stored rows");
        assert_eq!(items[0]["type_id"], "com.kosmos.task");
    }

    let all = state.data_list(&ark, &json!({})).await.unwrap();
    assert_eq!(all["items"].as_array().unwrap().len(), 5);

    let summary = state.data_summary(&ark).await.unwrap();
    let task = summary["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["type_id"] == "com.kosmos.task")
        .unwrap()
        .clone();
    assert_eq!(task["name"], "Задача");
    assert_eq!(task["count"], 2);
}

/// `search_objects` returns `{entryId, line, text}` hits, not object
/// rows — the page used to discard all of them.
#[tokio::test]
async fn data_search_resolves_hits_back_to_objects() {
    let (_dir, ark, state, _seed) = fixture().await;
    let created = ark
        .request(
            "upsert_object",
            json!({"object": {
                "id": "note-found",
                "typeId": "com.kosmos.note",
                "typeVersion": "1.0.0",
                "title": "unsichtbare orchidee",
                "contentJson": {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "a note body mentioning orchidee"}]}]},
                "propsJson": {"description": null, "extensions": {}},
                "createdAt": "2026-01-01T00:00:00.000Z",
                "updatedAt": "2026-01-02T00:00:00.000Z",
                "deletedAt": null
            }}),
        )
        .await
        .unwrap();
    assert!(created.ok, "{created:?}");
    let found = state
        .data_search(&ark, &json!({"query": "orchidee"}))
        .await
        .unwrap();
    let items = found["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "search hit must resolve to its object");
    assert_eq!(items[0]["id"], "note-found");
    assert_eq!(items[0]["type_id"], "com.kosmos.note");
    assert!(!items[0]["excerpt"].as_str().unwrap().is_empty());
    assert!(state
        .data_search(&ark, &json!({"query": "   "}))
        .await
        .is_err());
}

#[test]
fn manager_rows_are_bounded_redacted_and_body_free() {
    let row = json!({
        "id": "n1",
        "type_id": "note_obj",
        "title": r"C:\Users\alice\private note",
        "content_json": {"secret": "body"},
        "props_json": {"token": "abc"},
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-02T00:00:00Z",
        "deleted_at": Value::Null,
    });
    let safe = safe_row(&row, None).expect("canonical projection");
    assert!(safe.get("content_json").is_none());
    assert!(safe.get("props_json").is_none());
    assert!(!safe.to_string().contains("alice"));
    assert!(safe.get("excerpt").is_some());
    assert_eq!(bounded_limit(Some(&json!(200)), MAX_BROWSE_PAGE), Ok(200));
    assert!(bounded_limit(Some(&json!(201)), MAX_BROWSE_PAGE).is_err());
    assert!(parse_cursor(Some(&json!("nope"))).is_err());

    let camel = json!({
        "id": "n2",
        "typeId": "note_obj",
        "title": "Заметка",
        "createdAt": "2026-02-01T00:00:00Z",
        "updatedAt": "2026-02-02T00:00:00Z",
        "deletedAt": Value::Null,
    });
    assert!(is_live_public_row(&camel));
    let camel_safe = safe_row(&camel, None).expect("legacy boundary projection");
    assert_eq!(camel_safe["type_id"], "com.kosmos.note");
    assert_eq!(camel_safe["type_version"], Value::Null);
    assert_eq!(camel_safe["created_at"], "2026-02-01T00:00:00Z");

    // Legacy-version rows are the norm for pre-registry data: they must
    // stay visible, with only the allow-listed fields projected.
    let task = json!({
        "id": "task-1", "type_id": "task_obj", "type_version": "0.0.0-legacy",
        "title": "Task", "props_json": {"status": "done", "api_token": "secret", "local_path": "/private"}
    });
    let task_safe = safe_row(&task, None).expect("legacy task stays visible");
    assert_eq!(task_safe["type_id"], "com.kosmos.task");
    assert_eq!(task_safe["type_version"], "0.0.0-legacy");
    assert_eq!(task_safe["fields"]["status"], "done");
    assert!(task_safe["fields"].get("api_token").is_none());
    assert!(task_safe["fields"].get("local_path").is_none());

    let game = json!({
        "id": "game-1", "type_id": "game_obj", "title": "Game",
        "props_json": {"playStatus": "completed", "genres": ["rpg"], "secret": "drop"},
        "links": [{"id": "l1", "targetObjectId": "note-1", "linkType": "note"}]
    });
    let game_safe = safe_row(&game, None).expect("game projection");
    assert_eq!(game_safe["type_id"], "com.kosmos.game");
    assert_eq!(game_safe["fields"]["playStatus"], "completed");
    assert!(game_safe["fields"].get("secret").is_none());
    assert_eq!(game_safe["links"][0]["target_object_id"], "note-1");

    // Package-owned types project to identity only — no field allow-list.
    let custom = json!({
        "id": "w1", "type_id": "workout_obj", "title": "Run",
        "props_json": {"distance": 5}
    });
    let custom_safe = safe_row(&custom, None).expect("custom type identity row");
    assert_eq!(custom_safe["type_id"], "workout_obj");
    assert!(custom_safe["fields"].as_object().unwrap().is_empty());

    let internal = json!({"id": "x", "type_id": "ark_internal", "title": "x"});
    assert!(safe_row(&internal, None).is_none());
    let tombstone = json!({"typeId": "note_obj", "deletedAt": "2026-02-03T00:00:00Z"});
    assert!(!is_live_public_row(&tombstone));
}

#[test]
fn browse_cursor_advances_by_scanned_rows_not_returned_items() {
    let note = |id: &str| json!({"id": id, "type_id": "note_obj", "title": id});
    // Rows the projection drops (no id here) must still move the cursor:
    // a page of entirely dropped rows has to advance, or the client
    // re-requests the same offset forever.
    let dropped = |key: &str| json!({"type_id": "note_obj", "title": key});
    let rows: Vec<Value> = (0..3).map(|i| dropped(&format!("d{i}"))).collect();
    let all: Vec<&Value> = rows.iter().collect();
    let (items, next_cursor) = browse_page(&all, 0, 2);
    assert!(items.is_empty());
    assert_eq!(
        next_cursor.as_deref(),
        Some("2"),
        "dropped rows still advance the cursor"
    );

    // Mixed page: the cursor must land past every scanned row, otherwise
    // the next page re-scans rows that were already covered.
    let rows: Vec<Value> = vec![note("n1"), dropped("d0"), note("n2"), note("n3")];
    let all: Vec<&Value> = rows.iter().collect();
    let (items, next_cursor) = browse_page(&all, 0, 3);
    assert_eq!(items.len(), 2);
    assert_eq!(next_cursor.as_deref(), Some("3"));
    let (items, next_cursor) = browse_page(&all, 3, 3);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "n3");
    assert_eq!(next_cursor, None);
}
