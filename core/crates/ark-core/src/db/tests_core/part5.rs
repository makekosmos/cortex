
    #[test]
    fn object_summary_queries_skip_body_and_filter_by_type() {
        let conn = setup_db();
        let object_type = make_object_type("summary_fixture_book_type", "Book");
        upsert_object_type(&conn, &object_type).unwrap();
        upsert_object_type(&conn, &make_object_type("summary_fixture_type", "Journal")).unwrap();

        let mut older = make_object("obj-1", "summary_fixture_type", "Journal");
        older.content_json = json!({ "text": "large body that must not be selected by summaries" });
        older.props_json = json!({ "kind": "note" });
        older.created_at = "2026-01-01T00:00:00.000Z".to_string();
        older.updated_at = "2026-01-01T00:00:00.000Z".to_string();
        upsert_object(&conn, &older).unwrap();

        let mut newer = make_object("obj-2", "summary_fixture_book_type", "Clean Code");
        newer.content_json = json!({ "text": "another body" });
        newer.props_json = json!({ "kind": "book" });
        newer.created_at = "2026-01-02T00:00:00.000Z".to_string();
        newer.updated_at = "2026-01-02T00:00:00.000Z".to_string();
        upsert_object(&conn, &newer).unwrap();

        let summaries = list_object_summaries(&conn).unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].id, "obj-2");
        assert_eq!(summaries[0].props_json, json!({ "kind": "book" }));
        assert_eq!(summaries[1].id, "obj-1");

        let book_summaries =
            list_object_summaries_by_type(&conn, "summary_fixture_book_type").unwrap();
        assert_eq!(book_summaries.len(), 1);
        assert_eq!(book_summaries[0].id, "obj-2");
        assert_eq!(book_summaries[0].type_id, "summary_fixture_book_type");
    }

    fn make_time_entry(
        id: &str,
        started_at: &str,
        ended_at: Option<&str>,
        source: &str,
    ) -> ArkObject {
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
        upsert_object(
            &conn,
            &make_time_entry("te-running", "2026-05-20T10:00:00.000Z", None, "manual"),
        )
        .unwrap();
        upsert_object(
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
        upsert_object(
            &conn,
            &make_time_entry("te-manual", "2026-05-20T10:00:00.000Z", None, "manual"),
        )
        .unwrap();
        upsert_object(
            &conn,
            &make_time_entry("te-pomo", "2026-05-20T11:00:00.000Z", None, "pomodoro"),
        )
        .unwrap();
        upsert_object(
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
        upsert_object(&conn, &deleted).unwrap();

        let result = list_running_time_entries(&conn, None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_object_search_fts_rebuilds_existing_objects_on_init() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_TABLES).unwrap();

        let object_type = make_object_type("search_obj", "Searchable");
        upsert_object_type(&conn, &object_type).unwrap();
        let mut object = make_object("obj-existing-search", "search_obj", "Existing Object");
        object.props_json = json!({
            "description": "preexisting nebula archive"
        });
        upsert_object(&conn, &object).unwrap();

        assert!(
            !object_search_fts_exists(&conn).unwrap(),
            "legacy DB setup should not have the FTS table yet"
        );

        init_schema(&conn).unwrap();

        let results = search_objects(&conn, "nebula").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry_id, "obj-existing-search");
    }

    #[test]
    fn test_object_search_fts_updates_and_removes_index_entries() {
        let conn = setup_db();
        let object_type = make_object_type("search_obj", "Searchable");
        upsert_object_type(&conn, &object_type).unwrap();

        let mut object = make_object("obj-search-update", "search_obj", "Search Target");
        object.props_json = json!({ "description": "alpha marker" });
        upsert_object(&conn, &object).unwrap();
        assert_eq!(
            search_objects(&conn, "alpha").unwrap()[0].entry_id,
            object.id
        );

        object.props_json = json!({ "description": "beta marker" });
        object.updated_at = "2026-01-02T00:00:00.000Z".to_string();
        upsert_object(&conn, &object).unwrap();
        assert!(
            search_objects(&conn, "alpha").unwrap().is_empty(),
            "old FTS text should be removed on object update"
        );
        assert_eq!(
            search_objects(&conn, "beta").unwrap()[0].entry_id,
            object.id
        );

        delete_object(&conn, &object.id).unwrap();
        assert!(
            search_objects(&conn, "beta").unwrap().is_empty(),
            "deleted objects should be removed from the FTS index"
        );
    }

    #[test]
    fn test_object_search_is_punctuation_safe_and_falls_back_without_fts() {
        let conn = setup_db();
        let object_type = make_object_type("search_obj", "Searchable");
        upsert_object_type(&conn, &object_type).unwrap();
        let mut object = make_object("obj-punctuation-search", "search_obj", "C++ Primer");
        object.props_json = json!({
            "description": "boss-fight co-op notes"
        });
        upsert_object(&conn, &object).unwrap();

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
        assert_eq!(get_sync_kv(&conn, "foo").unwrap(), None);
        set_sync_kv(&conn, "foo", "bar").unwrap();
        assert_eq!(get_sync_kv(&conn, "foo").unwrap(), Some("bar".to_string()));
    }

    #[test]
    fn test_clear_all() {
        let conn = setup_db();
        upsert_todo(&conn, &make_todo("t1", "X")).unwrap();
        upsert_tracked_app(&conn, &make_tracked_app("app-clear")).unwrap();
        upsert_usage_session(&conn, &make_usage_session("session-clear", "app-clear")).unwrap();
        upsert_usage_event(
            &conn,
            &make_usage_event("event-clear", "app-clear", Some("session-clear")),
        )
        .unwrap();
        set_sync_kv(&conn, "k", "v").unwrap();
        clear_all(&conn).unwrap();
        let data = load_all(&conn).unwrap();
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
        assert_eq!(get_sync_kv(&conn, "k").unwrap(), None);
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

        let count = delete_trashed(&conn, "device-under-test").unwrap();
        assert_eq!(count, 1);
        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 1);
        assert_eq!(data.todos[0].id, "t1");
    }
