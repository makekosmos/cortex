    // -------------------------------------------------------------------
    // KOS-302 review: hole-tolerant usage cursors (usage_complete_through)
    // -------------------------------------------------------------------

    fn stored_vector(conn: &Connection) -> VersionVector {
        get_sync_kv(conn, VERSION_VECTOR_KEY)
            .unwrap_or(None)
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn save_stored_vector(conn: &Connection, vector: &VersionVector) {
        set_sync_kv(
            conn,
            VERSION_VECTOR_KEY,
            &serde_json::to_string(vector).unwrap(),
        )
        .unwrap();
    }

    fn usage_cursor(conn: &Connection, device_id: &str) -> u64 {
        stored_vector(conn)
            .get(&format!("@usage:{device_id}"))
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0)
    }

    fn count_usage_entities(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT (SELECT COUNT(*) FROM usage_sessions) +
                    (SELECT COUNT(*) FROM usage_events) +
                    (SELECT COUNT(*) FROM usage_days)",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }

    /// One VersionVector pull simulated at the db layer — the same calls the
    /// `SyncChanges` handlers make: the server pages
    /// `collect_entities_page_blocking` against the peer's vector, the
    /// receiver `apply_entity_blocking`s each entity, and on the final page
    /// the sender's `usage_complete_through` map raises the receiver's
    /// `@usage:` cursors. `claims = false` simulates an old-protocol peer
    /// that never sees the field. Returns how many entities were applied.
    fn pull_usage(
        server: &Connection,
        server_device: &str,
        client: &Connection,
        claims: bool,
    ) -> usize {
        // Claims snapshot BEFORE the stream, exactly like the fixed send
        // path: anything appended mid-pull is excluded from both the data
        // and the completeness claim.
        let through = claims.then(|| usage_log_complete_through(server, None).unwrap());
        let remote = stored_vector(client);
        let mut offset = 0;
        let mut applied = 0;
        loop {
            // Small pages on purpose: the pull must survive pagination, and
            // cursor claims are only valid once the stream is exhausted.
            let entities = SqliteStorageBackend::collect_entities_page_blocking(
                server,
                &remote,
                server_device,
                offset,
                3,
            );
            if entities.is_empty() {
                break;
            }
            offset += entities.len();
            for entity in entities {
                // Fixed (non-usage) entities like the tracked_app ride in
                // the same stream — count only journal-served usage refs.
                let is_usage = is_sequenced_usage_entity(&entity.entity_type);
                SqliteStorageBackend::apply_entity_blocking(client, &entity).unwrap();
                applied += usize::from(is_usage);
            }
        }
        if let Some(through) = through {
            let mut vector = stored_vector(client);
            crate::protocol::apply_usage_complete_through(&mut vector, &through);
            save_stored_vector(client, &vector);
        }
        applied
    }

    /// Server-side fixture: three sessions written by device `dev-a` with
    /// heartbeat churn — every ref's hlc is old enough for compaction.
    /// Returns the surviving refs: s-1 keeps seq 4, s-2 keeps seq 5,
    /// s-3 keeps seq 6 (its only ref — never dropped as unsynced).
    fn compacted_usage_server() -> Connection {
        let conn = setup_db();
        upsert_tracked_app(&conn, &make_tracked_app("app-1")).unwrap();
        for id in ["s-1", "s-2", "s-3"] {
            upsert_usage_session(&conn, &make_usage_session(id, "app-1")).unwrap();
        }
        // No ensure_usage_sequence_migrated here: it would backfill fresh-hlc
        // refs at seqs 1..N and swallow these churn seqs via INSERT OR IGNORE.
        let hlc = |seq: u64| format!("2026-08-01T00:00:{seq:02}.000Z:{seq:06}:dev-a");
        for (seq, id) in [
            (1, "s-1"),
            (2, "s-1"),
            (3, "s-2"),
            (4, "s-1"),
            (5, "s-2"),
            (6, "s-3"),
        ] {
            record_usage_sequence(&conn, "usage_session", id, "dev-a", seq, &hlc(seq), false)
                .unwrap();
        }
        let deleted =
            compact_usage_sync_log(&conn, Some("dev-a"), "2026-09-01T00:00:00.000Z", 1_000)
                .unwrap();
        assert_eq!(deleted, 3, "fixture must compact refs 1, 2 and 3");
        conn
    }

    #[test]
    fn compaction_never_deletes_above_foreign_contiguous_cursor() {
        // Relay safety: refs whose seq is above our contiguous coverage of
        // that origin are gaps we cannot vouch for — superseded or not,
        // they stay.
        let conn = setup_db();
        let insert = |seq: i64, entity: &str, hlc: &str| {
            conn.execute(
                "INSERT INTO usage_sync_log (device_id, seq, entity_type, entity_id, hlc, deleted)
                 VALUES ('remote', ?1, 'usage_session', ?2, ?3, 0)",
                params![seq, entity, hlc],
            )
            .unwrap();
        };
        insert(1, "s-1", "2026-08-01T00:00:01.000Z:000001:remote");
        insert(2, "s-1", "2026-08-01T00:00:02.000Z:000002:remote");
        insert(4, "s-2", "2026-08-01T00:00:04.000Z:000004:remote");
        insert(5, "s-2", "2026-08-01T00:00:05.000Z:000005:remote");
        conn.execute(
            "INSERT INTO usage_sync_heads (device_id, max_seq) VALUES ('remote', 5)",
            [],
        )
        .unwrap();
        // Our coverage of `remote` stopped at seq 2 — seqs 3+ were never
        // held contiguously, so we may not compact into that range.
        save_stored_vector(&conn, &{
            let mut vector = VersionVector::new();
            vector.insert("@usage:remote".to_string(), "2".to_string());
            vector
        });

        let deleted =
            compact_usage_sync_log(&conn, Some("dev-a"), "2026-09-01T00:00:00.000Z", 1_000)
                .unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(
            usage_log_refs(&conn)
                .iter()
                .map(|(_, seq, _, _)| *seq)
                .collect::<Vec<_>>(),
            vec![2, 4, 5],
            "seq 4 is superseded but above our contiguous cursor — it must stay"
        );
    }

    #[test]
    fn complete_through_reports_cursors_and_the_own_head() {
        let conn = compacted_usage_server();
        // Wire shape (`None`): every device is claimed by its contiguous
        // cursor — for our own device it already equals the head.
        let through = usage_log_complete_through(&conn, None).unwrap();
        assert_eq!(through["dev-a"], 6);
        // Compaction shape (`Some`): the own device is bounded by the log
        // head even if the vector cursor is gone.
        let conn2 = setup_db();
        upsert_tracked_app(&conn2, &make_tracked_app("app-1")).unwrap();
        upsert_usage_session(&conn2, &make_usage_session("s-1", "app-1")).unwrap();
        for seq in 1..=3 {
            record_usage_sequence(
                &conn2,
                "usage_session",
                "s-1",
                "dev",
                seq,
                &format!("2026-08-01T00:00:0{seq}.000Z:{seq:06}:dev"),
                false,
            )
            .unwrap();
        }
        let mut vector = stored_vector(&conn2);
        vector.remove("@usage:dev");
        save_stored_vector(&conn2, &vector);
        assert_eq!(usage_log_complete_through(&conn2, None).unwrap()["dev"], 0);
        assert_eq!(
            usage_log_complete_through(&conn2, Some("dev")).unwrap()["dev"],
            3
        );
        // A foreign origin we only partially received can only be claimed
        // up to our own contiguous coverage of it.
        let conn3 = setup_db();
        conn3
            .execute(
                "INSERT INTO usage_sync_heads (device_id, max_seq) VALUES ('remote', 9)",
                [],
            )
            .unwrap();
        save_stored_vector(&conn3, &{
            let mut vector = VersionVector::new();
            vector.insert("@usage:remote".to_string(), "4".to_string());
            vector
        });
        assert_eq!(
            usage_log_complete_through(&conn3, None).unwrap()["remote"],
            4
        );
        assert_eq!(
            usage_log_complete_through(&conn3, Some("me")).unwrap()["remote"],
            4
        );
    }

    #[test]
    fn fresh_device_converges_past_compacted_holes() {
        let server = compacted_usage_server();
        let client = setup_db();
        upsert_tracked_app(&client, &make_tracked_app("app-1")).unwrap();

        let applied = pull_usage(&server, "dev-a", &client, true);
        assert_eq!(applied, 3, "only the surviving refs are served");
        assert_eq!(count_usage_entities(&client), 3);
        // The contiguous cursor jumped the compacted holes via the
        // final-page claim — this is the fix under test.
        assert_eq!(usage_cursor(&client, "dev-a"), 6);

        // The second pull serves nothing: the cursor is at the head.
        assert_eq!(pull_usage(&server, "dev-a", &client, true), 0);
    }

    #[test]
    fn complete_through_does_not_jump_mid_page() {
        let server = compacted_usage_server();
        let client = setup_db();
        upsert_tracked_app(&client, &make_tracked_app("app-1")).unwrap();

        let remote = stored_vector(&client);
        // Apply exactly one small page, then stop — as a mid-pull batch
        // would. No claims yet, so no cursor jump past the hole.
        let first_page =
            SqliteStorageBackend::collect_entities_page_blocking(&server, &remote, "dev-a", 0, 2);
        assert_eq!(first_page.len(), 2);
        for entity in &first_page {
            SqliteStorageBackend::apply_entity_blocking(&client, entity).unwrap();
        }
        assert_eq!(
            usage_cursor(&client, "dev-a"),
            0,
            "cursor must not jump: seqs 1..3 are compacted holes on the receiver"
        );

        let through = usage_log_complete_through(&server, None).unwrap();
        let mut vector = stored_vector(&client);
        crate::protocol::apply_usage_complete_through(&mut vector, &through);
        save_stored_vector(&client, &vector);
        assert_eq!(usage_cursor(&client, "dev-a"), 6);
    }

    #[test]
    fn relay_converges_with_compaction_on_both_hops() {
        // A --(compacted)--> B --(compacted)--> C. B is a relay: it holds
        // A-origin refs plus its own writes, and must advertise
        // complete-through for both.
        let server_a = compacted_usage_server();
        let relay_b = setup_db();
        upsert_tracked_app(&relay_b, &make_tracked_app("app-1")).unwrap();
        pull_usage(&server_a, "dev-a", &relay_b, true);
        assert_eq!(usage_cursor(&relay_b, "dev-a"), 6);

        // B's own device churn: two refs for s-b1, the older one compacts.
        upsert_usage_session(&relay_b, &make_usage_session("s-b1", "app-1")).unwrap();
        record_usage_sequence(
            &relay_b,
            "usage_session",
            "s-b1",
            "dev-b",
            1,
            "2026-08-01T00:00:01.000Z:000001:dev-b",
            false,
        )
        .unwrap();
        record_usage_sequence(
            &relay_b,
            "usage_session",
            "s-b1",
            "dev-b",
            2,
            "2026-08-01T00:00:02.000Z:000002:dev-b",
            false,
        )
        .unwrap();
        assert_eq!(
            compact_usage_sync_log(&relay_b, Some("dev-b"), "2026-09-01T00:00:00.000Z", 1_000)
                .unwrap(),
            1
        );

        let client_c = setup_db();
        upsert_tracked_app(&client_c, &make_tracked_app("app-1")).unwrap();
        pull_usage(&relay_b, "dev-b", &client_c, true);
        assert_eq!(count_usage_entities(&client_c), 4);
        assert_eq!(usage_cursor(&client_c, "dev-a"), 6);
        assert_eq!(usage_cursor(&client_c, "dev-b"), 2);
        assert_eq!(pull_usage(&relay_b, "dev-b", &client_c, true), 0);
    }

    #[test]
    fn old_protocol_peer_still_converges_without_claims() {
        // Old receiver: `usage_complete_through` is absent from the wire, so
        // it keeps contiguous-only advancement — entities still arrive
        // (today's behaviour), the cursor just cannot pass the holes.
        let server = compacted_usage_server();
        let client = setup_db();
        upsert_tracked_app(&client, &make_tracked_app("app-1")).unwrap();

        let applied = pull_usage(&server, "dev-a", &client, false);
        assert_eq!(applied, 3);
        assert_eq!(count_usage_entities(&client), 3);
        assert_eq!(
            usage_cursor(&client, "dev-a"),
            0,
            "contiguous cursor stops at the first hole — pre-fix behaviour"
        );
        // Re-served tail: today's cost for an old peer.
        assert_eq!(pull_usage(&server, "dev-a", &client, false), 3);
    }

    #[test]
    fn own_refs_compact_by_journal_head_when_cursor_is_frozen() {
        // KOS-302 review round 2: own-device refs must be bounded by the
        // journal head, not the version-vector cursor — a wiped/reset
        // sync_kv (clear_all, corruption) can freeze `@usage:<own>` while
        // own seqs keep advancing past it, pinning every own ref forever.
        let conn = setup_db();
        upsert_tracked_app(&conn, &make_tracked_app("app-1")).unwrap();
        upsert_usage_session(&conn, &make_usage_session("s-1", "app-1")).unwrap();
        let hlc = |seq: u64| format!("2026-08-01T00:00:{seq:02}.000Z:{seq:06}:dev");
        for seq in 1..=3 {
            record_usage_sequence(&conn, "usage_session", "s-1", "dev", seq, &hlc(seq), false)
                .unwrap();
        }
        // Freeze the own cursor: the vector forgets it, the head stays 3.
        let mut vector = stored_vector(&conn);
        vector.remove("@usage:dev");
        save_stored_vector(&conn, &vector);

        let deleted =
            compact_usage_sync_log(&conn, Some("dev"), "2026-09-01T00:00:00.000Z", 100).unwrap();
        assert_eq!(deleted, 2, "own head bounds superseded own refs");
        assert_eq!(usage_log_refs(&conn).len(), 1, "the newest ref stays");
    }

    #[test]
    fn compaction_with_no_own_device_binds_everything_to_cursors() {
        // `None` means no origin gets the journal-head bound: the same refs
        // that `Some("dev")` compacts stay put once the vector cursor is
        // frozen — used by the wire claim path and by maintenance when the
        // tracker has never written.
        let conn = setup_db();
        upsert_tracked_app(&conn, &make_tracked_app("app-1")).unwrap();
        upsert_usage_session(&conn, &make_usage_session("s-1", "app-1")).unwrap();
        let hlc = |seq: u64| format!("2026-08-01T00:00:{seq:02}.000Z:{seq:06}:dev");
        for seq in 1..=3 {
            record_usage_sequence(&conn, "usage_session", "s-1", "dev", seq, &hlc(seq), false)
                .unwrap();
        }
        let mut vector = stored_vector(&conn);
        vector.remove("@usage:dev");
        save_stored_vector(&conn, &vector);

        assert_eq!(
            compact_usage_sync_log(&conn, None, "2026-09-01T00:00:00.000Z", 100).unwrap(),
            0
        );
        assert_eq!(usage_log_refs(&conn).len(), 3);
    }

    #[test]
    fn complete_through_snapshot_excludes_mid_stream_appends() {
        // KOS-302 review round 2: the claim map is snapshotted BEFORE the
        // first page is produced. An entry appended mid-stream must not be
        // covered by it — otherwise the receiver's cursor jumps past a seq
        // it never received.
        let server = compacted_usage_server(); // surviving refs {4,5,6}, head 6
        let client = setup_db();
        upsert_tracked_app(&client, &make_tracked_app("app-1")).unwrap();

        let through = usage_log_complete_through(&server, None).unwrap();
        assert_eq!(through["dev-a"], 6);

        let remote = stored_vector(&client);
        let mut offset = 0;
        let mut appended = false;
        loop {
            let entities = SqliteStorageBackend::collect_entities_page_blocking(
                &server, &remote, "dev-a", offset, 3,
            );
            if entities.is_empty() {
                break;
            }
            offset += entities.len();
            for entity in entities {
                SqliteStorageBackend::apply_entity_blocking(&client, &entity).unwrap();
            }
            if !appended {
                appended = true;
                // Lands between the snapshot and the stream's end: seq 7
                // exists on the server but is not covered by the claim.
                upsert_usage_session(&server, &make_usage_session("s-4", "app-1")).unwrap();
                record_usage_sequence(
                    &server,
                    "usage_session",
                    "s-4",
                    "dev-a",
                    7,
                    "2026-10-01T00:00:07.000Z:000007:dev-a",
                    false,
                )
                .unwrap();
            }
        }

        let mut vector = stored_vector(&client);
        crate::protocol::apply_usage_complete_through(&mut vector, &through);
        save_stored_vector(&client, &vector);
        assert_eq!(
            usage_cursor(&client, "dev-a"),
            6,
            "the claim must not pass the mid-stream seq"
        );

        // The entry is delivered on the next pull and the cursor advances
        // past it via the new claim.
        assert!(pull_usage(&server, "dev-a", &client, true) >= 1);
        assert_eq!(usage_cursor(&client, "dev-a"), 7);
    }

    /// KOS-370 regression: a pull-side vector snapshot is stale by the time
    /// `is_last` persists it — the incoming session loads it at the first
    /// batch and keeps it in memory for the whole exchange. Keys written
    /// concurrently (local mutations, integration-replication bumps like
    /// `integration_credential_envelope:*`) must survive the save.
    #[tokio::test]
    async fn persist_pull_vector_preserves_concurrent_keys() {
        let shared = Arc::new(Mutex::new(setup_db()));
        let backend: Arc<dyn crate::sync_server::StorageBackend> =
            Arc::new(SqliteStorageBackend::new(shared.clone()));

        // The session snapshot predates the concurrent write.
        let mut pull_vector = VersionVector::new();
        for id in ["entity-a", "entity-b"] {
            pull_vector.insert(
                id.to_string(),
                "2026-01-01T00:00:00.000Z:000000:peer".to_string(),
            );
        }

        // A local writer bumps unrelated keys while the session runs: one
        // brand-new key, one that already exists in the snapshot with a
        // newer hlc (a local mutation to the same entity mid-pull wins).
        {
            let guard = shared.lock().unwrap_or_else(|e| e.into_inner());
            let mut stored = stored_vector(&guard);
            for (key, hlc) in [
                (
                    "integration_credential_envelope:integration-a:node-b:1",
                    "2026-01-03T00:00:00.000Z:000000:local",
                ),
                ("entity-b", "2026-01-02T00:00:00.000Z:000000:local"),
            ] {
                stored.insert(key.to_string(), hlc.to_string());
            }
            save_stored_vector(&guard, &stored);
        }

        crate::sync_server::persist_pull_vector(&backend, &mut pull_vector, true, true, None).await;

        let guard = shared.lock().unwrap_or_else(|e| e.into_inner());
        let stored = stored_vector(&guard);
        let get = |key: &str| stored.get(key).map(String::as_str);
        assert_eq!(
            get("integration_credential_envelope:integration-a:node-b:1"),
            Some("2026-01-03T00:00:00.000Z:000000:local"),
        );
        assert_eq!(
            get("entity-a"),
            Some("2026-01-01T00:00:00.000Z:000000:peer"),
        );
        assert_eq!(
            get("entity-b"),
            Some("2026-01-02T00:00:00.000Z:000000:local"),
        );
    }
