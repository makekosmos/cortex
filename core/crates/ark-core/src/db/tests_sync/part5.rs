    #[tokio::test]
    async fn storage_backend_versioned_object_matrix_holds_unknown_payload_and_replays_exactly() {
        let backend = make_backend();
        let conn = backend.conn.clone();
        {
            let guard = conn.lock().unwrap();
            upsert_object_type(&guard, &make_object_type("remote-type", "Remote")).unwrap();
        }
        let mut old = sync_object("remote-old", "remote-type", "old-wire");
        old.data.remove("typeVersion");
        backend.apply_entity(&old).await.unwrap();
        assert_eq!(
            get_object(&conn.lock().unwrap(), "remote-old")
                .unwrap()
                .unwrap()
                .type_version,
            type_registry::LEGACY_VERSION
        );
        let mut unknown = sync_object("remote-unknown", "remote-type", "unknown-wire");
        unknown.data.insert("typeVersion".into(), json!("9.9.9"));
        unknown
            .data
            .insert("futurePayload".into(), json!({"keep":true}));
        backend.apply_entity(&unknown).await.unwrap();
        let guard = conn.lock().unwrap();
        assert!(get_object(&guard, "remote-unknown").unwrap().is_none());
        let payload: String = guard
            .query_row(
                "SELECT payload FROM sync_pending_objects WHERE id='remote-unknown'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(payload.contains("futurePayload"));
        drop(guard);
        let replayed = {
            let guard = conn.lock().unwrap();
            replay_pending_for_type(&guard, "remote-type", type_registry::LEGACY_VERSION).unwrap()
        };
        assert_eq!(replayed, 0);
        assert!(get_object(&conn.lock().unwrap(), "remote-unknown")
            .unwrap()
            .is_none());
    }

    // -------------------------------------------------------------------
    // KOS-177: usage sync-log residue / paging drift regressions
    // -------------------------------------------------------------------

    fn sync_usage_session_with_origin(
        id: &str,
        tracked_app_id: &str,
        origin: &str,
        seq: u64,
    ) -> SyncEntity {
        let mut entity = sync_usage_session(id, tracked_app_id);
        entity.hlc = format!("2026-01-01T00:00:00.000Z:{seq:06}:{origin}");
        entity.origin_device_id = Some(origin.to_string());
        entity.origin_seq = Some(seq);
        entity
    }

    #[tokio::test]
    async fn usage_sync_page_does_not_duplicate_entities_behind_dead_refs() {
        // A usage_sync_log ref whose entity row is gone must not corrupt the
        // page offsets: callers advance `offset` by entities returned, while
        // the SQL OFFSET counts refs. A skipped ref made the next page
        // re-emit entities it had already returned.
        let backend = make_backend();
        let conn = backend.conn.clone();
        backend
            .apply_entity(&sync_tracked_app("app-drift"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_usage_session_with_origin(
                "session-dead",
                "app-drift",
                "peer-a",
                2,
            ))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_usage_session_with_origin(
                "session-live",
                "app-drift",
                "peer-a",
                3,
            ))
            .await
            .unwrap();

        // Simulate a dead ref: row gone with no tombstone (e.g. clear_all
        // residue or a delete path that bypassed sync bookkeeping).
        {
            let guard = conn.lock().unwrap();
            guard
                .execute("DELETE FROM usage_sessions WHERE id = 'session-dead'", [])
                .unwrap();
        }

        let loaded = backend.load_entities(&VersionVector::new()).await;
        let live_count = loaded
            .iter()
            .filter(|entity| entity.id == "session-live")
            .count();
        assert_eq!(
            live_count, 1,
            "dead ref must not cause session-live to be emitted twice: {loaded:?}"
        );
        // The dead ref must still surface as a tombstone so the receiver's
        // contiguous usage cursor can advance past its seq.
        let tombstones = loaded
            .iter()
            .filter(|entity| entity.id == "session-dead" && entity.deleted == Some(true))
            .count();
        assert_eq!(
            tombstones, 1,
            "dead ref should surface exactly one tombstone entity: {loaded:?}"
        );
    }

    #[tokio::test]
    async fn clear_all_clears_usage_sync_log_residue() {
        // clear_all wiped rows + tombstones but left usage_sync_log /
        // usage_sync_versions behind: dead refs then re-emitted stale
        // tombstones / skipped rows to every fresh peer, and the leftover
        // versions blocked peers from re-sending resurrected entities.
        let backend = make_backend();
        let conn = backend.conn.clone();
        backend
            .apply_entity(&sync_tracked_app("app-clear"))
            .await
            .unwrap();
        backend
            .apply_entity(&sync_usage_session_with_origin(
                "session-clear",
                "app-clear",
                "peer-a",
                7,
            ))
            .await
            .unwrap();
        {
            let guard = conn.lock().unwrap();
            assert_eq!(
                guard.query_row(
                    "SELECT COUNT(*) FROM usage_sync_log",
                    [],
                    |row| row.get::<_, i64>(0),
                ),
                Ok(1),
                "applied usage entity should have a sync-log ref"
            );
            clear_all(&guard).unwrap();
            for table in ["usage_sync_log", "usage_sync_versions"] {
                let count: i64 = guard
                    .query_row(
                        &format!("SELECT COUNT(*) FROM {table}"),
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(count, 0, "clear_all must clear {table}");
            }
        }
    }

    #[test]
    fn delete_trashed_records_sync_tombstones() {
        // delete_trashed hard-deleted todo rows without writing sync
        // tombstones, so peers kept resurrecting them on the next sync.
        let conn = setup_db();
        let mut todo = make_todo("trashed-1", "gone");
        todo.is_trashed = true;
        upsert_todo(&conn, &todo).unwrap();

        let deleted = delete_trashed(&conn, "device-under-test").unwrap();
        assert_eq!(deleted, 1);
        let tombstone_hlc: Option<String> = conn
            .query_row(
                "SELECT hlc FROM sync_tombstones WHERE id = 'trashed-1' AND entity_type = 'todo'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap();
        assert!(
            tombstone_hlc.is_some(),
            "delete_trashed must record a sync tombstone per deleted todo"
        );
        let vector = get_sync_kv(&conn, "lan_sync.version_vector").unwrap();
        assert!(
            vector
                .as_deref()
                .is_some_and(|raw| raw.contains("trashed-1")),
            "delete_trashed must bump the version vector; got {vector:?}"
        );
    }
