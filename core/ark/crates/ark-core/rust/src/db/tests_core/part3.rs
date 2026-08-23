
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
