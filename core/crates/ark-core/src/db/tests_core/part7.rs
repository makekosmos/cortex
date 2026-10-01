    // KOS-287: canonical app identity, top-apps merge, active-time sort.

    fn top_app_entry(
        id: &str,
        path: &str,
        display_name: &str,
        foreground_ms: i64,
        last_seen_at: Option<&str>,
    ) -> TopAppEntry {
        TopAppEntry {
            id: id.to_string(),
            display_name: display_name.to_string(),
            process_name: path.rsplit('\\').next().unwrap_or(path).to_string(),
            normalized_path: path.to_string(),
            icon_ref: None,
            runtime_ms: foreground_ms * 3,
            foreground_ms,
            idle_ms: 0,
            sessions: 1,
            last_seen_at: last_seen_at.map(str::to_string),
            is_system: false,
        }
    }

    #[test]
    fn canonical_key_strips_version_dirs_and_case() {
        assert_eq!(
            canonical_app_key(
                "c:\\users\\k\\appdata\\local\\discord\\app-1.0.9164\\discord.exe",
                "discord.exe"
            ),
            "c:\\users\\k\\appdata\\local\\discord\\discord.exe"
        );
        assert_eq!(
            canonical_app_key("C:/Users/K/AppData/Local/Discord/app-1.0.9200/Discord.EXE", "x"),
            "c:\\users\\k\\appdata\\local\\discord\\discord.exe"
        );
        assert_eq!(
            canonical_app_key(
                "c:\\browsers\\chrome\\120.0.6099.71\\chrome.exe",
                "chrome.exe"
            ),
            "c:\\browsers\\chrome\\chrome.exe"
        );
        // Plain numeric dirs and non-version segments are preserved.
        assert_eq!(
            canonical_app_key("c:\\games\\2024\\game.exe", "game.exe"),
            "c:\\games\\2024\\game.exe"
        );
        assert_eq!(canonical_app_key("", "App.EXE"), "proc:app.exe");
    }

    #[test]
    fn merge_dedups_updates_and_keeps_freshest_row() {
        let rows = vec![
            top_app_entry(
                "old",
                "c:\\apps\\discord\\app-1.0.1\\discord.exe",
                "Discord — chatting",
                100,
                Some("2026-01-01T00:00:00Z"),
            ),
            top_app_entry(
                "new",
                "c:\\apps\\discord\\app-1.0.2\\discord.exe",
                "Discord",
                50,
                Some("2026-02-01T00:00:00Z"),
            ),
        ];
        let merged = merge_top_apps(rows, 10, "c:\\windows");
        assert_eq!(merged.len(), 1);
        let app = &merged[0];
        assert_eq!(app.id, "new");
        assert_eq!(app.display_name, "Discord");
        assert_eq!(
            app.normalized_path,
            "c:\\apps\\discord\\app-1.0.2\\discord.exe"
        );
        assert_eq!(app.foreground_ms, 150);
        assert_eq!(app.runtime_ms, 450);
        assert_eq!(app.sessions, 2);
    }

    #[test]
    fn merge_groups_browser_rows_regardless_of_title_names() {
        // Legacy rows stored the last window title as display_name — same exe
        // path must still collapse to one row.
        let rows = vec![
            top_app_entry(
                "a",
                "c:\\edge\\msedge.exe",
                "YouTube — Microsoft Edge",
                10,
                Some("2026-01-01T00:00:00Z"),
            ),
            top_app_entry(
                "b",
                "c:\\edge\\msedge.exe",
                "Microsoft Edge",
                20,
                Some("2026-01-02T00:00:00Z"),
            ),
        ];
        let merged = merge_top_apps(rows, 10, "c:\\windows");
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].display_name, "Microsoft Edge");
        assert_eq!(merged[0].foreground_ms, 30);
    }

    #[test]
    fn merge_sorts_by_active_time_and_marks_system() {
        let rows = vec![
            top_app_entry("x", "c:\\apps\\x.exe", "X", 10, Some("2026-01-01T00:00:00Z")),
            top_app_entry(
                "explorer",
                "c:\\windows\\explorer.exe",
                "Explorateur",
                5,
                Some("2026-01-02T00:00:00Z"),
            ),
            top_app_entry("y", "c:\\apps\\y.exe", "Y", 99, Some("2026-01-01T00:00:00Z")),
        ];
        let merged = merge_top_apps(rows, 10, "c:\\windows");
        assert_eq!(
            merged.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["y", "x", "explorer"]
        );
        assert!(merged[2].is_system);
        assert!(!merged[0].is_system);
    }

    #[test]
    fn system_path_matches_windows_dir_only() {
        assert!(is_system_path(
            "c:\\windows\\systemapps\\shell\\sihost.exe",
            "C:\\Windows"
        ));
        assert!(!is_system_path("c:\\apps\\explorer.exe", "c:\\windows"));
        assert!(!is_system_path("c:\\windowsx\\evil.exe", "c:\\windows"));
    }

    #[test]
    fn load_usage_analytics_dedups_version_dirs_on_stored_data() {
        // End-to-end over real fixtures: two tracked_apps rows that differ
        // only by a Squirrel version dir must collapse to one top_apps row,
        // sorted by active (foreground) time — no stored records rewritten.
        let conn = setup_db();

        let mut old_app = make_tracked_app("app-old");
        old_app.normalized_exe_path = "c:\\apps\\discord\\app-1.0.1\\discord.exe".to_string();
        old_app.exe_path = "C:\\Apps\\Discord\\app-1.0.1\\Discord.exe".to_string();
        old_app.display_name = Some("Discord — Chat".to_string());

        let mut new_app = make_tracked_app("app-new");
        new_app.normalized_exe_path = "c:\\apps\\discord\\app-1.0.2\\discord.exe".to_string();
        new_app.exe_path = "C:\\Apps\\Discord\\app-1.0.2\\Discord.exe".to_string();
        new_app.display_name = Some("Discord".to_string());
        new_app.last_seen_at = "2026-02-01T00:00:00.000Z".to_string();

        let mut explorer = make_tracked_app("app-explorer");
        explorer.normalized_exe_path = "c:\\windows\\explorer.exe".to_string();
        explorer.exe_path = "C:\\Windows\\explorer.exe".to_string();
        explorer.process_name = "explorer.exe".to_string();

        upsert_tracked_app(&conn, &old_app).unwrap();
        upsert_tracked_app(&conn, &new_app).unwrap();
        upsert_tracked_app(&conn, &explorer).unwrap();

        let mut s_old = make_usage_session("s-old", "app-old");
        s_old.foreground_ms = 600_000;
        let mut s_new = make_usage_session("s-new", "app-new");
        s_new.foreground_ms = 300_000;
        s_new.ended_at = Some("2026-02-01T00:10:00.000Z".to_string());
        let mut s_exp = make_usage_session("s-exp", "app-explorer");
        s_exp.foreground_ms = 1_000;

        upsert_usage_session(&conn, &s_old).unwrap();
        upsert_usage_session(&conn, &s_new).unwrap();
        upsert_usage_session(&conn, &s_exp).unwrap();

        let snapshot = load_usage_analytics(&conn, 21, 10, 24).unwrap();
        assert_eq!(snapshot.top_apps.len(), 2);
        let discord = &snapshot.top_apps[0];
        assert_eq!(discord.display_name, "Discord");
        assert_eq!(discord.foreground_ms, 900_000);
        assert_eq!(discord.sessions, 2);
        assert!(!discord.is_system);
        assert_eq!(snapshot.top_apps[1].process_name, "explorer.exe");
        assert!(snapshot.top_apps[1].is_system);
    }
