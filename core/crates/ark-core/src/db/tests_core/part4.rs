
    #[test]
    fn usage_game_playtime_summary_matches_bindings_and_range() {
        let conn = setup_db();
        let mut main_app = make_tracked_app("tracked-main");
        main_app.exe_path = r"C:\Games\Chronicle\Chronicle.exe".to_string();
        main_app.normalized_exe_path = r"c:\games\chronicle\chronicle.exe".to_string();
        main_app.process_name = "chronicle.exe".to_string();
        upsert_tracked_app(&conn, &main_app).unwrap();

        let mut alt_app = make_tracked_app("tracked-alt");
        alt_app.exe_path = r"C:\Games\Chronicle\Chronicle_DX12.exe".to_string();
        alt_app.normalized_exe_path = r"c:\games\chronicle\chronicle_dx12.exe".to_string();
        alt_app.process_name = "chronicle_dx12.exe".to_string();
        upsert_tracked_app(&conn, &alt_app).unwrap();

        let mut helper_app = make_tracked_app("tracked-helper");
        helper_app.exe_path = "".to_string();
        helper_app.normalized_exe_path = "".to_string();
        helper_app.process_name = "Chronicle Helper.exe".to_string();
        upsert_tracked_app(&conn, &helper_app).unwrap();

        let mut other_app = make_tracked_app("tracked-other");
        other_app.exe_path = r"C:\Other\Other.exe".to_string();
        other_app.normalized_exe_path = r"c:\other\other.exe".to_string();
        other_app.process_name = "other.exe".to_string();
        upsert_tracked_app(&conn, &other_app).unwrap();

        let sessions = [
            (
                "session-main",
                "tracked-main",
                "2026-04-20T10:00:00.000Z",
                1_800_000,
            ),
            (
                "session-alt",
                "tracked-alt",
                "2026-04-20T11:00:00.000Z",
                3_600_000,
            ),
            (
                "session-helper",
                "tracked-helper",
                "2026-04-21T12:00:00.000Z",
                600_000,
            ),
            (
                "session-other",
                "tracked-other",
                "2026-04-20T13:00:00.000Z",
                9_000_000,
            ),
        ];
        for (session_id, tracked_app_id, started_at, foreground_ms) in sessions {
            let mut session = make_usage_session(session_id, tracked_app_id);
            session.started_at = started_at.to_string();
            session.ended_at = Some(started_at.replace(":00.000Z", ":30.000Z"));
            session.runtime_ms = foreground_ms;
            session.foreground_ms = foreground_ms;
            upsert_usage_session(&conn, &session).unwrap();
        }

        let bindings = vec![
            UsageGamePlaytimeBinding {
                game_id: "game-1".to_string(),
                game_name: "Chronicle".to_string(),
                match_type: "exe_path".to_string(),
                match_value: r"C:\Games\Chronicle\Chronicle.exe".to_string(),
            },
            UsageGamePlaytimeBinding {
                game_id: "game-1".to_string(),
                game_name: "Chronicle".to_string(),
                match_type: "exe_path".to_string(),
                match_value: r"C:\Games\Chronicle\Chronicle_DX12.exe".to_string(),
            },
            UsageGamePlaytimeBinding {
                game_id: "game-1".to_string(),
                game_name: "Chronicle".to_string(),
                match_type: "process_name".to_string(),
                match_value: "chronicle helper.exe".to_string(),
            },
        ];

        let summary = load_usage_game_playtime_summary(
            &conn,
            &bindings,
            Some("2026-04-20"),
            Some("2026-04-21"),
        )
        .unwrap();

        assert_eq!(summary.aggregates.len(), 1);
        assert_eq!(summary.aggregates[0].game_id, "game-1");
        assert_eq!(summary.aggregates[0].total_seconds, 6_000);
        assert_eq!(summary.aggregates[0].session_count, 3);
        assert_eq!(
            summary.daily_totals,
            vec![
                UsageGameDailyTotal {
                    date: "2026-04-20".to_string(),
                    seconds: 5_400,
                },
                UsageGameDailyTotal {
                    date: "2026-04-21".to_string(),
                    seconds: 600,
                },
            ]
        );
        assert_eq!(summary.per_game_totals.len(), 1);
        assert_eq!(summary.per_game_totals[0].seconds, 6_000);
    }

    #[test]
    fn test_object_model_crud() {
        let conn = setup_db();
        let object_type = make_object_type("crud_fixture_book_type", "РљРЅРёРіР°");
        upsert_object_type(&conn, &object_type).unwrap();
        upsert_object_type(
            &conn,
            &make_object_type("crud_fixture_type", "Р—Р°РјРµС‚РєР°"),
        )
        .unwrap();

        let note = make_object("obj-1", "crud_fixture_type", "РџРµСЂРІР°СЏ Р·Р°РјРµС‚РєР°");
        let book = make_object("obj-2", "crud_fixture_book_type", "Clean Code");
        upsert_object(&conn, &note).unwrap();
        upsert_object(&conn, &book).unwrap();

        let link = make_object_link("link-1", "obj-1", "obj-2");
        upsert_object_link(&conn, &link).unwrap();

        let data = load_all(&conn).unwrap();
        assert!(data
            .object_types
            .iter()
            .any(|item| item.id == "com.kosmos.note"));
        assert!(data
            .object_types
            .iter()
            .any(|item| item.id == "com.kosmos.game"));
        assert!(data
            .object_types
            .iter()
            .any(|item| item.id == "crud_fixture_book_type"));
        assert_eq!(data.objects.len(), 2);
        assert_eq!(data.object_links.len(), 1);
        assert_eq!(data.object_links[0].source_object_id, "obj-1");

        delete_object_link(&conn, "link-1").unwrap();
        delete_object(&conn, "obj-2").unwrap();
        delete_object_type(&conn, "crud_fixture_book_type").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.object_links.len(), 0);
        assert_eq!(data.objects.len(), 1);
        assert!(!data
            .object_types
            .iter()
            .any(|item| item.id == "crud_fixture_book_type"));
    }

    #[test]
    fn upsert_object_preserves_existing_links() {
        // Regression: 2026-06-04. SQLite REPLACE deletes the old object row first.
        let conn = setup_db();
        let note = make_object("obj-note", "note_obj", "РџРµСЂРІР°СЏ Р·Р°РјРµС‚РєР°");
        let task = make_object("obj-task", "note_obj", "Р—Р°РґР°С‡Р°");
        let tag = make_object("obj-tag", "note_obj", "РўРµРі");
        upsert_object(&conn, &note).unwrap();
        upsert_object(&conn, &task).unwrap();
        upsert_object(&conn, &tag).unwrap();

        upsert_object_link(&conn, &make_object_link("link-out", "obj-note", "obj-task")).unwrap();
        upsert_object_link(&conn, &make_object_link("link-in", "obj-tag", "obj-note")).unwrap();

        let mut updated_note = make_object(
            "obj-note",
            "note_obj",
            "РћР±РЅРѕРІР»РµРЅРЅР°СЏ Р·Р°РјРµС‚РєР°",
        );
        updated_note.updated_at = "2026-01-02T00:00:00.000Z".to_string();
        upsert_object(&conn, &updated_note).unwrap();

        let links = list_object_links(&conn).unwrap();
        assert_eq!(links.len(), 2);
        assert!(links.iter().any(|link| {
            link.id == "link-out"
                && link.source_object_id == "obj-note"
                && link.target_object_id == "obj-task"
        }));
        assert!(links.iter().any(|link| {
            link.id == "link-in"
                && link.source_object_id == "obj-tag"
                && link.target_object_id == "obj-note"
        }));
        assert_eq!(
            get_object(&conn, "obj-note").unwrap().unwrap().title,
            "РћР±РЅРѕРІР»РµРЅРЅР°СЏ Р·Р°РјРµС‚РєР°"
        );
    }

    #[test]
    fn upsert_object_type_preserves_existing_objects_and_links() {
        // Regression: 2026-06-04. SQLite REPLACE cascades through object_types -> objects -> links.
        let conn = setup_db();
        let object_type = make_object_type("custom_note", "Custom Note");
        upsert_object_type(&conn, &object_type).unwrap();
        upsert_object(&conn, &make_object("obj-a", "custom_note", "A")).unwrap();
        upsert_object(&conn, &make_object("obj-b", "custom_note", "B")).unwrap();
        upsert_object_link(&conn, &make_object_link("link-custom", "obj-a", "obj-b")).unwrap();

        let mut updated_type = make_object_type("custom_note", "Custom Note Updated");
        updated_type.updated_at = "2026-01-02T00:00:00.000Z".to_string();
        upsert_object_type(&conn, &updated_type).unwrap();

        assert_eq!(list_objects_by_type(&conn, "custom_note").unwrap().len(), 2);
        let links = list_object_links(&conn).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].id, "link-custom");
        assert_eq!(
            get_object_type(&conn, "custom_note").unwrap().unwrap().name,
            "Custom Note Updated"
        );
    }

    #[test]
    fn upsert_object_link_updates_existing_row_in_place() {
        // Regression: 2026-06-04. Link upsert should update, not delete+insert.
        let conn = setup_db();
        upsert_object(&conn, &make_object("obj-a", "note_obj", "A")).unwrap();
        upsert_object(&conn, &make_object("obj-b", "note_obj", "B")).unwrap();
        upsert_object(&conn, &make_object("obj-c", "note_obj", "C")).unwrap();
        upsert_object_link(&conn, &make_object_link("link-1", "obj-a", "obj-b")).unwrap();

        let before_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM object_links WHERE id = ?1",
                params!["link-1"],
                |row| row.get(0),
            )
            .unwrap();

        let mut updated_link = make_object_link("link-1", "obj-a", "obj-c");
        updated_link.link_type = "tagged".to_string();
        upsert_object_link(&conn, &updated_link).unwrap();

        let after_rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM object_links WHERE id = ?1",
                params!["link-1"],
                |row| row.get(0),
            )
            .unwrap();
        let links = list_object_links(&conn).unwrap();
        assert_eq!(before_rowid, after_rowid);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target_object_id, "obj-c");
        assert_eq!(links[0].link_type, "tagged");
    }

    #[test]
    fn object_query_helpers_filter_by_type_and_ids() {
        let conn = setup_db();
        let object_type = make_object_type("query_fixture_book_type", "Book");
        upsert_object_type(&conn, &object_type).unwrap();
        upsert_object_type(&conn, &make_object_type("query_fixture_type", "Journal")).unwrap();
        upsert_object(
            &conn,
            &make_object("obj-1", "query_fixture_type", "Journal"),
        )
        .unwrap();
        upsert_object(
            &conn,
            &make_object("obj-2", "query_fixture_book_type", "Clean Code"),
        )
        .unwrap();
        upsert_object(
            &conn,
            &make_object("obj-3", "query_fixture_book_type", "Rust Book"),
        )
        .unwrap();

        let books = list_objects_by_type(&conn, "query_fixture_book_type").unwrap();
        assert_eq!(books.len(), 2);
        assert!(books
            .iter()
            .all(|object| object.type_id == "query_fixture_book_type"));

        let ids = vec![
            "obj-3".to_string(),
            "missing".to_string(),
            "obj-1".to_string(),
        ];
        let objects = get_objects_by_ids(&conn, &ids).unwrap();
        let returned_ids = objects
            .iter()
            .map(|object| object.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(returned_ids.len(), 2);
        assert!(returned_ids.contains(&"obj-3"));
        assert!(returned_ids.contains(&"obj-1"));
        assert!(get_objects_by_ids(&conn, &[]).unwrap().is_empty());
    }
