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
            canonical_app_key(
                "C:/Users/K/AppData/Local/Discord/app-1.0.9200/Discord.EXE",
                "x"
            ),
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
        let merged = merge_top_apps(rows, 10, Some("c:\\windows"));
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
        let merged = merge_top_apps(rows, 10, Some("c:\\windows"));
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].display_name, "Microsoft Edge");
        assert_eq!(merged[0].foreground_ms, 30);
    }

    #[test]
    fn merge_sorts_by_active_time_and_marks_system() {
        let rows = vec![
            top_app_entry(
                "x",
                "c:\\apps\\x.exe",
                "X",
                10,
                Some("2026-01-01T00:00:00Z"),
            ),
            top_app_entry(
                "explorer",
                "c:\\windows\\explorer.exe",
                "Explorateur",
                5,
                Some("2026-01-02T00:00:00Z"),
            ),
            top_app_entry(
                "y",
                "c:\\apps\\y.exe",
                "Y",
                99,
                Some("2026-01-01T00:00:00Z"),
            ),
        ];
        let merged = merge_top_apps(rows, 10, Some("c:\\windows"));
        assert_eq!(
            merged.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["y", "x", "explorer"]
        );
        assert!(merged[2].is_system);
        assert!(!merged[0].is_system);
    }

    #[test]
    fn merge_marks_nothing_system_without_windows_dir() {
        // Engine could not resolve %SystemRoot% (or non-Windows host): rows
        // stay unmarked rather than guessing a hard-coded path.
        let rows = vec![top_app_entry(
            "explorer",
            "c:\\windows\\explorer.exe",
            "Explorer",
            5,
            Some("2026-01-02T00:00:00Z"),
        )];
        let merged = merge_top_apps(rows, 10, None);
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

        let snapshot = load_usage_analytics(&conn, 21, 10, 24, Some("c:\\windows")).unwrap();
        assert_eq!(snapshot.top_apps.len(), 2);
        let discord = &snapshot.top_apps[0];
        assert_eq!(discord.display_name, "Discord");
        assert_eq!(discord.foreground_ms, 900_000);
        assert_eq!(discord.sessions, 2);
        assert!(!discord.is_system);
        assert_eq!(snapshot.top_apps[1].process_name, "explorer.exe");
        assert!(snapshot.top_apps[1].is_system);
    }

    #[test]
    fn summary_headline_is_active_time_not_visible_runtime() {
        // KOS-287 regression: the summary card's total must be active
        // (foreground && !idle) time — the removed `totalRuntimeMs` carried
        // visible wall-clock time and inflated the headline.
        let conn = setup_db();
        let app = make_tracked_app("app-1");
        upsert_tracked_app(&conn, &app).unwrap();
        let mut session = make_usage_session("s-1", "app-1");
        session.runtime_ms = 10_000; // visible
        session.foreground_ms = 4_000; // active
        session.idle_ms = 1_000;
        upsert_usage_session(&conn, &session).unwrap();

        let snapshot = load_usage_analytics(&conn, 21, 10, 24, None).unwrap();
        assert_eq!(snapshot.summary.total_foreground_ms, 4_000);
        assert_eq!(snapshot.summary.total_idle_ms, 1_000);
        // totalRuntimeMs stays on the wire as raw visible-time data (SDK
        // contract) — the headline reads totalForegroundMs.
        assert_eq!(snapshot.summary.total_runtime_ms, 10_000);
    }

    #[test]
    fn usage_process_candidates_dedup_across_version_dirs() {
        // The game-binding picker must not list the same app twice when its
        // exe moved between version dirs (pre-KOS-287 ids hashed the path).
        let conn = setup_db();
        let mut old_app = make_tracked_app("cand-old");
        old_app.exe_path = "C:\\Games\\Nebula\\app-1.0.1\\nebula.exe".to_string();
        old_app.normalized_exe_path = "c:\\games\\nebula\\app-1.0.1\\nebula.exe".to_string();
        let mut new_app = make_tracked_app("cand-new");
        new_app.exe_path = "C:\\Games\\Nebula\\app-1.0.2\\nebula.exe".to_string();
        new_app.normalized_exe_path = "c:\\games\\nebula\\app-1.0.2\\nebula.exe".to_string();
        upsert_tracked_app(&conn, &old_app).unwrap();
        upsert_tracked_app(&conn, &new_app).unwrap();
        upsert_usage_session(&conn, &make_usage_session("s-old", "cand-old")).unwrap();
        let mut s_new = make_usage_session("s-new", "cand-new");
        s_new.ended_at = Some("2026-02-01T00:10:00.000Z".to_string());
        upsert_usage_session(&conn, &s_new).unwrap();

        let recent = list_recent_usage_processes(&conn, 10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].tracked_app_id, "cand-new");
    }
