    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    fn make_todo(id: &str, title: &str) -> TodoItem {
        TodoItem {
            id: id.to_string(),
            title: title.to_string(),
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
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        }
    }

    fn make_tracked_app(id: &str) -> TrackedApp {
        TrackedApp {
            id: id.to_string(),
            platform: "windows".to_string(),
            exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
            normalized_exe_path: r"c:\\apps\\demo\\demo.exe".to_string(),
            process_name: "demo.exe".to_string(),
            display_name: Some("Demo App".to_string()),
            publisher: Some("Demo Corp".to_string()),
            icon_ref: None,
            first_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
            last_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
        }
    }

    fn make_usage_session(id: &str, tracked_app_id: &str) -> UsageSession {
        UsageSession {
            id: id.to_string(),
            tracked_app_id: tracked_app_id.to_string(),
            device_id: "device-1".to_string(),
            device_name: "Test Device".to_string(),
            platform: "windows".to_string(),
            started_at: "2026-01-01T00:00:00.000Z".to_string(),
            ended_at: Some("2026-01-01T00:10:00.000Z".to_string()),
            runtime_ms: 600_000,
            foreground_ms: 600_000,
            idle_ms: 0,
            window_title: Some("Demo Window".to_string()),
            process_name: "demo.exe".to_string(),
            exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
            pid_start: Some(1234),
            pid_end: Some(1234),
            meta_json: json!({"note": "session"}),
        }
    }

    fn make_usage_event(id: &str, tracked_app_id: &str, session_id: Option<&str>) -> UsageEvent {
        UsageEvent {
            id: id.to_string(),
            tracked_app_id: tracked_app_id.to_string(),
            usage_session_id: session_id.map(|value| value.to_string()),
            device_id: "device-1".to_string(),
            device_name: "Test Device".to_string(),
            platform: "windows".to_string(),
            occurred_at: "2026-01-01T00:05:00.000Z".to_string(),
            kind: "foreground".to_string(),
            window_title: Some("Demo Window".to_string()),
            process_name: "demo.exe".to_string(),
            exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
            pid: Some(1234),
            is_foreground: true,
            is_idle: false,
            meta_json: json!({"note": "event"}),
        }
    }

    fn make_object(id: &str, type_id: &str, title: &str) -> ArkObject {
        ArkObject {
            id: id.to_string(),
            type_id: type_id.to_string(),
            type_version: "0.0.0-legacy".to_string(),
            title: title.to_string(),
            content_json: json!({
                "type": "doc",
                "content": [{ "type": "paragraph" }]
            }),
            props_json: json!({
                "description": format!("Description for {title}")
            }),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            deleted_at: None,
        }
    }

    fn make_object_type(id: &str, name: &str) -> ObjectType {
        ObjectType {
            id: id.to_string(),
            name: name.to_string(),
            schema_json: json!({
                "fields": [
                    { "id": "description", "label": "Описание", "kind": "long_text", "required": false, "visible": true, "read_only": false }
                ]
            })
            .to_string(),
            ui_schema_json: json!({
                "visible_fields": ["description"],
                "hidden_fields": ["created_at", "updated_at", "deleted_at"],
                "read_only_fields": [],
            })
            .to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            system_locked: false,
        }
    }

    fn make_object_link(id: &str, source_object_id: &str, target_object_id: &str) -> ObjectLink {
        ObjectLink {
            id: id.to_string(),
            source_object_id: source_object_id.to_string(),
            target_object_id: target_object_id.to_string(),
            link_type: "related".to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn test_schema_creation() {
        let conn = setup_db();
        // Verify tables exist
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('todos','projects','areas','tags','headings','tracked_apps','usage_sessions','usage_events','sync_kv','sync_tombstones')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 10);
        let object_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('object_types', 'objects', 'object_links')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(object_count, 3);
    }

    #[test]
    fn test_init_schema_migrates_existing_db_without_destroying_data() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE todos (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                notes TEXT,
                priority INTEGER NOT NULL DEFAULT 0,
                scheduled_date TEXT,
                deadline TEXT,
                reminder_date TEXT,
                is_today INTEGER NOT NULL DEFAULT 0,
                is_evening INTEGER NOT NULL DEFAULT 0,
                is_someday INTEGER NOT NULL DEFAULT 0,
                is_completed INTEGER NOT NULL DEFAULT 0,
                completed_at TEXT,
                is_cancelled INTEGER NOT NULL DEFAULT 0,
                cancelled_at TEXT,
                is_trashed INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                heading_id TEXT,
                project_id TEXT,
                area_id TEXT,
                tag_ids TEXT NOT NULL DEFAULT '[]',
                checklist_items TEXT NOT NULL DEFAULT '[]',
                recurrence_rule TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE projects (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                notes TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                scheduled_date TEXT,
                deadline TEXT,
                sort_order INTEGER NOT NULL DEFAULT 0,
                color_tag TEXT,
                area_id TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE areas (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE tags (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                color TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE headings (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                project_id TEXT NOT NULL
            );
            CREATE TABLE sync_kv (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )
        .unwrap();

        let todo = make_todo("legacy-todo", "Keep me");
        upsert_todo(&conn, &todo).unwrap();
        set_sync_kv(&conn, "legacy-key", "legacy-value").unwrap();

        init_schema(&conn).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 1);
        assert_eq!(data.todos[0].id, "legacy-todo");
        assert_eq!(data.todos[0].title, "Keep me");
        assert_eq!(
            get_sync_kv(&conn, "legacy-key").unwrap(),
            Some("legacy-value".to_string())
        );

        let usage_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table'
                   AND name IN ('tracked_apps', 'usage_sessions', 'usage_events')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(usage_table_count, 3);
        let object_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table'
                   AND name IN ('object_types', 'objects', 'object_links')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(object_table_count, 3);
        let tombstone_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table' AND name = 'sync_tombstones'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tombstone_table_count, 1);
        let object_types = list_object_types(&conn).unwrap();
        assert!(object_types.iter().any(|item| item.id == "com.kosmos.note"));
        assert!(object_types.iter().any(|item| item.id == "com.kosmos.task"));
        let migrated: (String, String) = conn
            .query_row(
                "SELECT type_id, type_version FROM objects WHERE id = 'legacy-todo'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            migrated,
            ("com.kosmos.task".to_string(), "1.0.0".to_string()),
        );
        let canonical_type_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM object_types WHERE id = 'com.kosmos.task'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(canonical_type_exists, 1);
    }

    #[test]
    fn test_init_schema_adds_usage_runtime_ms_to_existing_sessions() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE tracked_apps (
                id TEXT PRIMARY KEY,
                platform TEXT NOT NULL,
                exe_path TEXT NOT NULL,
                normalized_exe_path TEXT NOT NULL,
                process_name TEXT NOT NULL,
                display_name TEXT,
                publisher TEXT,
                icon_ref TEXT,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL
            );
            CREATE TABLE usage_sessions (
                id TEXT PRIMARY KEY,
                tracked_app_id TEXT NOT NULL,
                device_id TEXT NOT NULL,
                device_name TEXT NOT NULL,
                platform TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                foreground_ms INTEGER NOT NULL DEFAULT 0,
                idle_ms INTEGER NOT NULL DEFAULT 0,
                window_title TEXT,
                process_name TEXT NOT NULL,
                exe_path TEXT NOT NULL,
                pid_start INTEGER,
                pid_end INTEGER,
                meta_json TEXT NOT NULL DEFAULT '{}'
            );
            INSERT INTO tracked_apps
                (id, platform, exe_path, normalized_exe_path, process_name, display_name, first_seen_at, last_seen_at)
            VALUES
                ('app-legacy', 'windows', 'C:\\Games\\Legacy\\legacy.exe', 'c:\\games\\legacy\\legacy.exe',
                 'legacy.exe', 'Legacy', '2026-01-01T00:00:00.000Z', '2026-01-01T01:00:00.000Z');
            INSERT INTO usage_sessions
                (id, tracked_app_id, device_id, device_name, platform, started_at, ended_at,
                 foreground_ms, idle_ms, window_title, process_name, exe_path)
            VALUES
                ('session-legacy', 'app-legacy', 'device-1', 'Device', 'windows',
                 '2026-01-01T00:00:00.000Z', '2026-01-01T01:00:00.000Z',
                 2400000, 300000, 'Legacy', 'legacy.exe', 'C:\\Games\\Legacy\\legacy.exe');
            ",
        )
        .unwrap();

        init_schema(&conn).unwrap();

        let runtime_ms: i64 = conn
            .query_row(
                "SELECT runtime_ms FROM usage_sessions WHERE id = 'session-legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(runtime_ms, 2_700_000);
        let data = load_all(&conn).unwrap();
        assert_eq!(data.usage_sessions[0].runtime_ms, 2_700_000);
    }

    #[test]
    fn test_todo_crud() {
        let conn = setup_db();
        let todo = make_todo("t1", "Buy milk");
        upsert_todo(&conn, &todo).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 1);
        assert_eq!(data.todos[0].title, "Buy milk");

        delete_todo(&conn, "t1").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 0);
    }

    #[test]
    fn test_batch_upsert() {
        let conn = setup_db();
        let todos = vec![make_todo("t1", "A"), make_todo("t2", "B")];
        batch_upsert_todos(&conn, &todos).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 2);
    }

    #[test]
    fn test_project_crud() {
        let conn = setup_db();
        let project = Project {
            id: "p1".to_string(),
            title: "My Project".to_string(),
            notes: None,
            status: "active".to_string(),
            scheduled_date: None,
            deadline: None,
            sort_order: 0,
            color_tag: None,
            area_id: None,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        };
        upsert_project(&conn, &project).unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].title, "My Project");

        delete_project(&conn, "p1").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.projects.len(), 0);
    }

    #[test]
    fn test_area_crud() {
        let conn = setup_db();
        let area = Area {
            id: "a1".to_string(),
            title: "Work".to_string(),
            sort_order: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        };
        upsert_area(&conn, &area).unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.areas.len(), 1);
    }

    #[test]
    fn test_tag_crud() {
        let conn = setup_db();
        let tag = Tag {
            id: "tg1".to_string(),
            title: "urgent".to_string(),
            color: Some("red".to_string()),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        };
        upsert_tag(&conn, &tag).unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.tags.len(), 1);
        assert_eq!(data.tags[0].color, Some("red".to_string()));
    }

    #[test]
    fn test_heading_crud() {
        let conn = setup_db();
        let heading = Heading {
            id: "h1".to_string(),
            title: "Section 1".to_string(),
            sort_order: 0,
            project_id: "p1".to_string(),
        };
        upsert_heading(&conn, &heading).unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.headings.len(), 1);

        delete_heading(&conn, "h1").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.headings.len(), 0);
    }

    #[test]
    fn test_usage_tracking_crud() {
        let conn = setup_db();
        let tracked_app = make_tracked_app("app-1");
        let session = make_usage_session("session-1", &tracked_app.id);
        let event = make_usage_event("event-1", &tracked_app.id, Some(&session.id));

        upsert_tracked_app(&conn, &tracked_app).unwrap();
        upsert_usage_session(&conn, &session).unwrap();
        upsert_usage_event(&conn, &event).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.tracked_apps.len(), 1);
        assert_eq!(data.usage_sessions.len(), 1);
        assert_eq!(data.usage_events.len(), 1);
        assert_eq!(data.tracked_apps[0].platform, "windows");
        assert_eq!(data.usage_sessions[0].tracked_app_id, "app-1");
        assert_eq!(data.usage_events[0].kind, "foreground");

        delete_usage_event(&conn, "event-1").unwrap();
        delete_usage_session(&conn, "session-1").unwrap();
        delete_tracked_app(&conn, "app-1").unwrap();
        let data = load_all(&conn).unwrap();
        assert_eq!(data.tracked_apps.len(), 0);
        assert_eq!(data.usage_sessions.len(), 0);
        assert_eq!(data.usage_events.len(), 0);
    }

    #[test]
    fn usage_analytics_snapshot_includes_summary_and_zero_filled_trend() {
        let conn = setup_db();
        let mut tracked_app = make_tracked_app("app-analytics");
        tracked_app.display_name = None;
        upsert_tracked_app(&conn, &tracked_app).unwrap();

        let today = Utc::now().date_naive();
        let today_start = format!("{}T10:00:00.000Z", today.format("%Y-%m-%d"));
        let today_end = format!("{}T10:30:00.000Z", today.format("%Y-%m-%d"));
        let mut session = make_usage_session("session-analytics", &tracked_app.id);
        session.started_at = today_start.clone();
        session.ended_at = Some(today_end);
        session.runtime_ms = 1_500;
        session.foreground_ms = 1_200;
        session.idle_ms = 300;
        upsert_usage_session(&conn, &session).unwrap();

        let mut event = make_usage_event("event-analytics", &tracked_app.id, Some(&session.id));
        event.occurred_at = today_start;
        upsert_usage_event(&conn, &event).unwrap();

        let snapshot = load_usage_analytics(&conn, 3, 5, 5).unwrap();

        assert_eq!(snapshot.summary.tracked_app_count, 1);
        assert_eq!(snapshot.summary.session_count, 1);
        assert_eq!(snapshot.summary.event_count, 1);
        assert_eq!(snapshot.summary.total_runtime_ms, 1_500);
        assert_eq!(snapshot.summary.total_foreground_ms, 1_200);
        assert_eq!(snapshot.summary.total_idle_ms, 300);
        assert_eq!(snapshot.daily_trend.len(), 3);
        assert_eq!(
            snapshot
                .daily_trend
                .iter()
                .filter(|point| point.sessions == 0)
                .count(),
            2,
            "range should include zero-filled days without sessions"
        );
        assert_eq!(snapshot.daily_trend.last().unwrap().sessions, 1);
        assert_eq!(snapshot.top_apps[0].display_name, "demo.exe");
        assert_eq!(snapshot.top_apps[0].icon_ref, None);
        assert_eq!(snapshot.top_apps[0].runtime_ms, 1_500);
        assert_eq!(snapshot.top_apps[0].foreground_ms, 1_200);
        assert_eq!(snapshot.recent_sessions[0].id, "session-analytics");
        assert_eq!(snapshot.recent_sessions[0].runtime_ms, 1_500);
        assert_eq!(snapshot.hourly_heatmap[0].foreground_ms, 1_200);
    }

    #[test]
    fn usage_process_queries_return_recent_and_search_candidates() {
        let conn = setup_db();
        let mut app = make_tracked_app("app-process");
        app.display_name = Some("Nebula Game".to_string());
        app.exe_path = r"C:/Games/Nebula/nebula.exe".to_string();
        app.normalized_exe_path = r"c:\games\nebula\nebula.exe".to_string();
        app.process_name = "Nebula.exe".to_string();
        upsert_tracked_app(&conn, &app).unwrap();
        let mut session = make_usage_session("session-process", &app.id);
        session.started_at = "2026-04-26T10:00:00.000Z".to_string();
        session.ended_at = Some("2026-04-26T11:00:00.000Z".to_string());
        session.runtime_ms = 3_600_000;
        session.foreground_ms = 3_600_000;
        upsert_usage_session(&conn, &session).unwrap();

        let recent = list_recent_usage_processes(&conn, 10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].tracked_app_id, "app-process");
        assert_eq!(recent[0].binding_match_type, "exe_path");
        assert_eq!(
            recent[0].binding_normalized_value,
            r"c:\games\nebula\nebula.exe"
        );
        assert_eq!(recent[0].session_count, 1);

        let search_results = search_usage_processes(&conn, "nebula", 10).unwrap();
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].display_name, "Nebula Game");
        assert!(search_usage_processes(&conn, "missing", 10)
            .unwrap()
            .is_empty());
    }

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
        let object_type = make_object_type("crud_fixture_book_type", "Книга");
        upsert_object_type(&conn, &object_type).unwrap();
        upsert_object_type(&conn, &make_object_type("crud_fixture_type", "Заметка")).unwrap();

        let note = make_object("obj-1", "crud_fixture_type", "Первая заметка");
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
        let note = make_object("obj-note", "note_obj", "Первая заметка");
        let task = make_object("obj-task", "note_obj", "Задача");
        let tag = make_object("obj-tag", "note_obj", "Тег");
        upsert_object(&conn, &note).unwrap();
        upsert_object(&conn, &task).unwrap();
        upsert_object(&conn, &tag).unwrap();

        upsert_object_link(&conn, &make_object_link("link-out", "obj-note", "obj-task")).unwrap();
        upsert_object_link(&conn, &make_object_link("link-in", "obj-tag", "obj-note")).unwrap();

        let mut updated_note = make_object("obj-note", "note_obj", "Обновленная заметка");
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
            "Обновленная заметка"
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

        // Без фильтра — все три, отсортированы DESC по startedAt.
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

        let count = delete_trashed(&conn).unwrap();
        assert_eq!(count, 1);
        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 1);
        assert_eq!(data.todos[0].id, "t1");
    }

    #[test]
    fn test_todo_with_tags_and_checklist() {
        let conn = setup_db();
        let todo = TodoItem {
            tag_ids: vec!["tag1".to_string(), "tag2".to_string()],
            checklist_items: json!([{"id": "c1", "title": "Step 1", "isCompleted": false}]),
            recurrence_rule: Some(json!({"frequency": "daily", "interval": 1})),
            ..make_todo("t1", "Complex")
        };
        upsert_todo(&conn, &todo).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos[0].tag_ids, vec!["tag1", "tag2"]);
        assert!(data.todos[0].checklist_items.is_array());
        assert!(data.todos[0].recurrence_rule.is_some());
    }

