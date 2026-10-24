use super::*;

async fn fixture() -> (
    tempfile::TempDir,
    Arc<ArkHost>,
    ManagerState,
) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("ark.db");
    let ark = Arc::new(ArkHost::open(db_path.to_str().unwrap()).await.unwrap());
    let state = ManagerState::new(dir.path().to_path_buf());
    (dir, ark, state)
}

async fn shutdown_ark(ark: Arc<ArkHost>) {
    let ark = Arc::try_unwrap(ark).unwrap_or_else(|_| panic!("manager test released ARK handles"));
    ark.shutdown(std::time::Instant::now() + std::time::Duration::from_secs(30))
        .await
        .unwrap();
}

#[tokio::test]
async fn data_page_counts_core_objects() {
    let (_dir, ark, state) = fixture().await;
    for (id, type_id, props) in [
        (
            "task-1",
            "core.task",
            json!({"state":{"kind":"open","at":null},"planning":{"kind":"inbox","period":"any","date":null},"deadline":null,"reminders":[],"recurrence":null,"priority":"none","checklist":[],"extensions":{}}),
        ),
        (
            "task-2",
            "core.task",
            json!({"state":{"kind":"open","at":null},"planning":{"kind":"inbox","period":"any","date":null},"deadline":null,"reminders":[],"recurrence":null,"priority":"none","checklist":[],"extensions":{}}),
        ),
        (
            "note-1",
            "core.note",
            json!({"description":null,"extensions":{}}),
        ),
    ] {
        let result = ark
            .request(
                "upsert_object",
                json!({"object": {
                    "id": id,
                    "typeId": type_id,
                    "typeVersion": "0.1.0",
                    "title": id,
                    "contentJson": {"type":"doc","content":[{"type":"paragraph"}]},
                    "propsJson": props,
                    "createdAt": "2026-01-01T00:00:00.000Z",
                    "updatedAt": "2026-01-02T00:00:00.000Z",
                    "deletedAt": null
                }}),
            )
            .await
            .unwrap();
        assert!(result.ok, "{result:?}");
    }

    let types = state.data_types(&ark).await.unwrap();
    let types = types.as_array().unwrap();
    let count_of = |id: &str| {
        types
            .iter()
            .find(|t| t["type_id"] == id)
            .map(|t| t["count"].as_u64().unwrap())
    };
    assert_eq!(count_of("core.task"), Some(2), "deleted task excluded");
    assert_eq!(count_of("core.note"), Some(1));

    for queried in ["core.task", "core.note"] {
        let list = state
            .data_list(&ark, &json!({"type_id": queried, "limit": 50}))
            .await
            .unwrap();
        let items = list["items"].as_array().unwrap();
        let expected = if queried == "core.task" { 2 } else { 1 };
        assert_eq!(items.len(), expected, "{queried} list count");
        assert!(items.iter().all(|item| item["type_id"] == queried));
    }

    let all = state.data_list(&ark, &json!({})).await.unwrap();
    assert_eq!(all["items"].as_array().unwrap().len(), 3);

    let summary = state.data_summary(&ark).await.unwrap();
    let task = summary["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["type_id"] == "core.task")
        .unwrap()
        .clone();
    assert_eq!(task["name"], "Задача");
    assert_eq!(task["count"], 2);
    shutdown_ark(ark).await;
}

/// `search_objects` returns `{entryId, line, text}` hits, not object
/// rows — the page used to discard all of them.
#[tokio::test]
async fn data_search_resolves_hits_back_to_objects() {
    let (_dir, ark, state) = fixture().await;
    let created = ark
        .request(
            "upsert_object",
            json!({"object": {
                "id": "note-found",
                "typeId": "core.note",
                "typeVersion": "0.1.0",
                "title": "unsichtbare orchidee",
                "contentJson": {"type": "doc", "content": [
                    {"type": "paragraph", "content": [
                        {"type": "text", "text": "a note body mentioning orchidee"},
                    ]},
                ]},
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
    assert_eq!(items[0]["type_id"], "core.note");
    assert!(!items[0]["excerpt"].as_str().unwrap().is_empty());
    assert!(state
        .data_search(&ark, &json!({"query": "   "}))
        .await
        .is_err());
    shutdown_ark(ark).await;
}

#[test]
fn manager_rows_are_bounded_redacted_and_body_free() {
    let row = json!({
        "id": "n1",
        "type_id": "core.note",
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
        "typeId": "core.note",
        "typeVersion": "0.1.0",
        "title": "Заметка",
        "createdAt": "2026-02-01T00:00:00Z",
        "updatedAt": "2026-02-02T00:00:00Z",
        "deletedAt": Value::Null,
    });
    assert!(is_live_public_row(&camel));
    let camel_safe = safe_row(&camel, None).expect("camelCase boundary projection");
    assert_eq!(camel_safe["type_id"], "core.note");
    assert_eq!(camel_safe["type_version"], "0.1.0");
    assert_eq!(camel_safe["created_at"], "2026-02-01T00:00:00Z");

    let task = json!({
        "id": "task-1", "type_id": "core.task", "type_version": "0.1.0",
        "title": "Task",
        "props_json": {
            "state": {"kind": "done", "at": null},
            "planning": {"kind": "inbox", "period": "any", "date": null},
            "api_token": "secret",
            "local_path": "/private"
        }
    });
    let task_safe = safe_row(&task, None).expect("core task row projects");
    assert_eq!(task_safe["type_id"], "core.task");
    assert_eq!(task_safe["type_version"], "0.1.0");
    assert_eq!(task_safe["fields"]["state"]["kind"], "done");
    assert!(task_safe["fields"].get("api_token").is_none());
    assert!(task_safe["fields"].get("local_path").is_none());

    let game = json!({
        "id": "game-1", "type_id": "core.game", "type_version": "0.1.0", "title": "Game",
        "props_json": {"playStatus": "completed", "genres": ["rpg"], "secret": "drop"},
        "links": [{"id": "l1", "targetObjectId": "note-1", "linkType": "note"}]
    });
    let game_safe = safe_row(&game, None).expect("game projection");
    assert_eq!(game_safe["type_id"], "core.game");
    assert_eq!(game_safe["fields"]["playStatus"], "completed");
    assert!(game_safe["fields"].get("secret").is_none());
    assert_eq!(game_safe["links"][0]["target_object_id"], "note-1");

    // Package-owned types project to identity only — no field allow-list.
    let custom = json!({
        "id": "w1", "type_id": "workout-custom", "title": "Run",
        "props_json": {"distance": 5}
    });
    let custom_safe = safe_row(&custom, None).expect("custom type identity row");
    assert_eq!(custom_safe["type_id"], "workout-custom");
    assert!(custom_safe["fields"].as_object().unwrap().is_empty());

    let internal = json!({"id": "x", "type_id": "ark_internal", "title": "x"});
    assert!(safe_row(&internal, None).is_none());
    let tombstone = json!({"typeId": "core.note", "deletedAt": "2026-02-03T00:00:00Z"});
    assert!(!is_live_public_row(&tombstone));
}

#[test]
fn browse_cursor_advances_by_scanned_rows_not_returned_items() {
    let note = |id: &str| json!({"id": id, "type_id": "core.note", "title": id});
    // Rows the projection drops (no id here) must still move the cursor:
    // a page of entirely dropped rows has to advance, or the client
    // re-requests the same offset forever.
    let dropped = |key: &str| json!({"type_id": "core.note", "title": key});
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
