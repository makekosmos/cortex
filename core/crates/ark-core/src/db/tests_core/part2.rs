
    #[test]
    fn test_schema_creation() {
        let conn = setup_db();
        // Verify tables exist
        let count: i64 = conn
            .query_row(
                concat!(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('todos',",
                    "'projects','tags','tracked_apps','usage_sessions','usage_events','sync_kv',",
                    "'sync_tombstones')"
                ),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 8);
        let object_count: i64 = conn
            .query_row(
                concat!(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN (",
                    "'object_types', 'objects', 'object_links')"
                ),
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
        // Migrated objects claim the newest registered version of their type.
        assert_eq!(
            migrated,
            ("com.kosmos.task".to_string(), "1.1.0".to_string()),
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
                (id, platform, exe_path, normalized_exe_path, process_name, display_name, \
                first_seen_at, last_seen_at)
            VALUES
                ('app-legacy', 'windows', 'C:\\Games\\Legacy\\legacy.exe', \
                'c:\\games\\legacy\\legacy.exe',
                 'legacy.exe', 'Legacy', '2026-01-01T00:00:00.000Z', \
                 '2026-01-01T01:00:00.000Z');
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
