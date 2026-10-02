    // -------------------------------------------------------------------
    // KOS-302: usage_sync_log compaction — superseded-ref rule
    // -------------------------------------------------------------------

    fn usage_log_refs(conn: &Connection) -> Vec<(String, i64, String, String)> {
        conn.prepare(
            "SELECT device_id, seq, entity_type, entity_id
             FROM usage_sync_log ORDER BY device_id, seq",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    }

    fn served_usage_ids(conn: &Connection, device_id: &str) -> Vec<String> {
        let mut ids: Vec<String> =
            SqliteStorageBackend::collect_entities_blocking(conn, &VersionVector::new(), device_id)
                .into_iter()
                .filter(|entity| is_sequenced_usage_entity(&entity.entity_type))
                .map(|entity| entity.id)
                .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    #[test]
    fn compaction_deletes_only_superseded_old_refs() {
        let conn = setup_db();
        let old = |day: u8, counter: u64| {
            format!("2026-08-{day:02}T12:00:00.000Z:{counter:06}:tracker")
        };
        // Three writes to the same session — the first two refs are
        // superseded by the third.
        record_usage_sequence(&conn, "usage_session", "s-1", "dev", 1, &old(1, 1), false).unwrap();
        record_usage_sequence(&conn, "usage_session", "s-1", "dev", 2, &old(2, 2), false).unwrap();
        record_usage_sequence(&conn, "usage_session", "s-1", "dev", 3, &old(3, 3), false).unwrap();
        // Single ref — unsynced for any peer that never pulled: must stay.
        record_usage_sequence(&conn, "usage_day", "d-1", "dev", 4, &old(4, 4), false).unwrap();
        // Superseded, but the newer ref is itself old enough → s-2 drops to
        // its newest ref.
        record_usage_sequence(&conn, "usage_session", "s-2", "dev", 5, &old(5, 5), false).unwrap();
        record_usage_sequence(&conn, "usage_session", "s-2", "dev", 6, &old(6, 6), false).unwrap();
        // Superseded but too fresh — inside the retention horizon.
        record_usage_sequence(
            &conn,
            "usage_session",
            "s-3",
            "dev",
            7,
            "2026-10-01T00:00:00.000Z:000007:tracker",
            false,
        )
        .unwrap();
        record_usage_sequence(
            &conn,
            "usage_session",
            "s-3",
            "dev",
            8,
            "2026-10-01T00:01:00.000Z:000008:tracker",
            false,
        )
        .unwrap();
        // Tombstone ref for a deleted entity: never superseded, must stay so
        // peers learn the deletion.
        record_usage_sequence(&conn, "usage_session", "s-del", "dev", 9, &old(9, 9), true).unwrap();

        let deleted =
            compact_usage_sync_log(&conn, Some("dev"), "2026-09-01T00:00:00.000Z", 1_000).unwrap();
        assert_eq!(deleted, 3);
        assert_eq!(
            usage_log_refs(&conn),
            vec![
                ("dev".to_string(), 3, "usage_session".to_string(), "s-1".to_string()),
                ("dev".to_string(), 4, "usage_day".to_string(), "d-1".to_string()),
                ("dev".to_string(), 6, "usage_session".to_string(), "s-2".to_string()),
                ("dev".to_string(), 7, "usage_session".to_string(), "s-3".to_string()),
                ("dev".to_string(), 8, "usage_session".to_string(), "s-3".to_string()),
                ("dev".to_string(), 9, "usage_session".to_string(), "s-del".to_string()),
            ]
        );
        // Second pass is a no-op: nothing left to compact.
        assert_eq!(
            compact_usage_sync_log(&conn, Some("dev"), "2026-09-01T00:00:00.000Z", 1_000).unwrap(),
            0
        );
    }

    #[test]
    fn compaction_respects_batch_limit() {
        let conn = setup_db();
        for index in 0..5 {
            let seq = index + 1;
            record_usage_sequence(
                &conn,
                "usage_session",
                "s-batch",
                "dev",
                seq,
                &format!("2026-08-01T00:00:00.000Z:{seq:06}:tracker"),
                false,
            )
            .unwrap();
        }
        let deleted = compact_usage_sync_log(&conn, Some("dev"), "2026-09-01T00:00:00.000Z", 2).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(usage_log_refs(&conn).len(), 3);
        while compact_usage_sync_log(&conn, Some("dev"), "2026-09-01T00:00:00.000Z", 2).unwrap() > 0 {}
        assert_eq!(usage_log_refs(&conn).len(), 1);
    }

    #[test]
    fn compaction_keeps_usage_aggregates_and_served_set_identical() {
        let conn = setup_db();
        // Realistic fixture: several apps, active + idle spans, a session and
        // a span crossing the UTC day boundary.
        upsert_tracked_app(&conn, &make_tracked_app("app-browser")).unwrap();
        upsert_tracked_app(&conn, &make_tracked_app("app-editor")).unwrap();

        let mut sessions = Vec::new();
        for (id, app, started, ended, fg, idle) in [
            ("sa-1", "app-browser", "2026-01-01T23:55:00.000Z", "2026-01-02T00:10:00.000Z", 900_000, 0),
            ("sa-2", "app-editor", "2026-01-02T08:00:00.000Z", "2026-01-02T09:30:00.000Z", 3_000_000, 2_400_000),
            ("sa-3", "app-browser", "2026-01-03T12:00:00.000Z", "2026-01-03T12:30:00.000Z", 0, 1_800_000),
        ] {
            let mut session = make_usage_session(id, app);
            session.started_at = started.to_string();
            session.ended_at = Some(ended.to_string());
            session.foreground_ms = fg;
            session.idle_ms = idle;
            session.runtime_ms = fg + idle;
            upsert_usage_session(&conn, &session).unwrap();
            sessions.push(session);
        }
        upsert_usage_event(&conn, &make_usage_event("ev-1", "app-browser", Some("sa-1"))).unwrap();
        upsert_usage_span(
            &conn,
            &UsageSpanWrite {
                device_id: "dev".to_string(),
                started_at_unix: 1_767_311_990, // 2026-01-01T23:59:50Z
                ended_at_unix: 1_767_312_010,   // 2026-01-02T00:00:10Z
                tracked_app_id: "app-browser".to_string(),
                window_title: Some("News".to_string()),
                flags: 0,
                updated_at: "2026-01-02T00:00:10.000Z".to_string(),
            },
        )
        .unwrap();
        upsert_usage_span(
            &conn,
            &UsageSpanWrite {
                device_id: "dev".to_string(),
                started_at_unix: 1_767_396_000, // 2026-01-02T08:20:00Z
                ended_at_unix: 1_767_396_600,
                tracked_app_id: "app-editor".to_string(),
                window_title: Some("main.rs".to_string()),
                flags: 1, // idle span
                updated_at: "2026-01-02T08:30:00.000Z".to_string(),
            },
        )
        .unwrap();

        // Heartbeat churn: several seq refs per entity (stale hlcs) plus the
        // current refs the writers produce.
        ensure_usage_sequence_migrated(&conn, "dev").unwrap();
        let entity_ids: Vec<(&str, &str)> = sessions
            .iter()
            .map(|session| ("usage_session", session.id.as_str()))
            .chain([("usage_event", "ev-1")])
            .collect();
        // Churn refs must continue the migration's contiguous head — the
        // compaction rule keeps any ref above our contiguous cursor, so a
        // sequence jump here would (correctly) pin the churn in place.
        let mut seq = conn
            .query_row(
                "SELECT COALESCE(max_seq, 0) FROM usage_sync_heads WHERE device_id = 'dev'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
            .max(0) as u64;
        for (kind, id) in &entity_ids {
            for round in 0..3 {
                seq += 1;
                record_usage_sequence(
                    &conn,
                    kind,
                    id,
                    "dev",
                    seq,
                    &format!("2026-08-{round:02}T00:00:00.000Z:{seq:06}:dev"),
                    false,
                )
                .unwrap();
            }
        }
        let refs_before = usage_log_refs(&conn).len();
        assert!(refs_before > entity_ids.len(), "fixture must have churn");

        let analytics_before = load_usage_analytics(&conn, 21, 8, 24, None).unwrap();
        let title_before = get_usage_title_total(&conn, "news").unwrap();
        let served_before = served_usage_ids(&conn, "dev");

        // Full compaction loop, tiny batches like the Engine runs them.
        while compact_usage_sync_log(&conn, Some("dev"), "2026-09-01T00:00:00.000Z", 2).unwrap() > 0 {}

        let mut analytics_after = load_usage_analytics(&conn, 21, 8, 24, None).unwrap();
        analytics_after.generated_at = analytics_before.generated_at.clone();
        assert_eq!(
            serde_json::to_value(&analytics_before).unwrap(),
            serde_json::to_value(&analytics_after).unwrap(),
            "KOS-287 aggregates must not move under log compaction"
        );
        assert_eq!(get_usage_title_total(&conn, "news").unwrap(), title_before);
        assert_eq!(served_usage_ids(&conn, "dev"), served_before);

        // Every entity keeps its newest ref — nothing a peer could still
        // need is dropped.
        let refs = usage_log_refs(&conn);
        for (_, id) in &entity_ids {
            assert_eq!(
                refs.iter().filter(|(_, _, _, entity)| entity == id).count(),
                1,
                "entity {id} must keep exactly its newest ref"
            );
        }
    }
