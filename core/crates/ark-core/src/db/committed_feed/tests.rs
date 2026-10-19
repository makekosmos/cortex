#[cfg(test)]
mod tests {
    use super::*;
    use ark_store::committed_feed::append_committed_change;
    use rusqlite::Connection;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(ark_store::schema::CREATE_TABLES)
            .unwrap();
        ark_store::data_platform::ensure_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn production_database_open_uses_full_wal_durability() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ark.db");
        let conn = ark_store::open::open_db(path.to_str().unwrap()).unwrap();
        let synchronous: i64 = conn
            .pragma_query_value(None, "synchronous", |row| row.get(0))
            .unwrap();
        let journal_mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        assert_eq!(synchronous, 2, "FULL has SQLite synchronous value 2");
    }

    #[test]
    fn feed_commit_rolls_back_with_object_transaction_and_survives_commit_before_notify() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ark.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(ark_store::schema::CREATE_TABLES)
            .unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        append_committed_change(&conn, "object", "a", "r1", false, Some("device-a"), None).unwrap();
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(committed_feed_snapshot(&conn).unwrap().high_watermark, 0);

        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        conn.execute(
            "INSERT INTO sync_kv(key, value) VALUES('object:a', 'r1')",
            [],
        )
        .unwrap();
        append_committed_change(&conn, "object", "a", "r1", false, Some("device-a"), None).unwrap();
        conn.execute_batch("COMMIT").unwrap();
        drop(conn);
        let conn = Connection::open(path).unwrap();
        // Simulates a crash before any live notification is emitted.
        assert_eq!(committed_feed_snapshot(&conn).unwrap().high_watermark, 1);
        assert_eq!(read_committed_feed_page(&conn, 0, 1, 10).unwrap().len(), 1);
    }

    #[test]
    fn fixed_watermark_excludes_concurrent_commit_and_detects_a_gap() {
        let conn = db();
        append_committed_change(&conn, "object", "a", "r1", false, None, None).unwrap();
        let snapshot = committed_feed_snapshot(&conn).unwrap();
        append_committed_change(&conn, "object", "b", "r2", false, None, None).unwrap();
        assert_eq!(
            read_committed_feed_page(&conn, 0, snapshot.high_watermark, 10)
                .unwrap()
                .len(),
            1
        );
        conn.execute("DELETE FROM ark_committed_feed WHERE position = 1", [])
            .unwrap();
        assert_eq!(
            read_committed_feed_page(&conn, 0, 2, 10).unwrap_err(),
            "feed_gap_resync_required"
        );
    }

    #[test]
    fn committed_feed_watermark_survives_pruning_and_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ark.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(ark_store::schema::CREATE_TABLES)
            .unwrap();
        append_committed_change(&conn, "object", "a", "r1", false, None, None).unwrap();
        append_committed_change(&conn, "object", "b", "r2", false, None, None).unwrap();

        conn.execute("DELETE FROM ark_committed_feed", []).unwrap();
        assert_eq!(committed_feed_snapshot(&conn).unwrap().high_watermark, 2);
        drop(conn);

        let conn = Connection::open(&path).unwrap();
        assert_eq!(committed_feed_snapshot(&conn).unwrap().high_watermark, 2);
        let next = append_committed_change(&conn, "object", "c", "r3", false, None, None).unwrap();
        assert_eq!(next, 3);
        assert_eq!(committed_feed_snapshot(&conn).unwrap().high_watermark, 3);
    }

    #[test]
    fn delivery_projection_keeps_selected_locator_and_payload_in_one_read_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("feed-snapshot.db");
        let reader = ark_store::open::open_db(path.to_str().unwrap()).unwrap();
        crate::db::init_schema(&reader).unwrap();
        let writer = ark_store::open::open_db(path.to_str().unwrap()).unwrap();
        crate::db::init_schema(&writer).unwrap();
        let initial = crate::types::ArkObject {
            id: "snapshot-object".into(),
            type_id: "note_obj".into(),
            type_version: "0.0.0-legacy".into(),
            title: "Selected payload".into(),
            content_json: serde_json::json!({"type":"doc","content":[]}),
            props_json: serde_json::json!({}),
            created_at: "2026-10-16T00:00:00.000Z".into(),
            updated_at: "2026-10-16T00:00:00.000Z".into(),
            deleted_at: None,
        };
        crate::db::upsert_object(&reader, &initial).unwrap();
        append_committed_change(&reader, "object", &initial.id, "r1", false, None, None).unwrap();

        let page = read_committed_feed_delivery_page_with_hook(&reader, 0, 1, 8, || {
            let mut updated = initial.clone();
            updated.title = "Newer concurrent payload".into();
            updated.updated_at = "2026-10-16T00:00:01.000Z".into();
            crate::db::upsert_object(&writer, &updated).unwrap();
            append_committed_change(&writer, "object", &updated.id, "r2", false, None, None)
                .unwrap();
        })
        .unwrap();

        let deliveries = match page {
            CommittedFeedDeliveryPage::Deliveries(deliveries) => deliveries,
            CommittedFeedDeliveryPage::SkipTo { .. } => panic!("feed page only skipped entries"),
            CommittedFeedDeliveryPage::TooLarge { .. } => panic!("small test payload is bounded"),
        };
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].entry.position, 1);
        assert_eq!(deliveries[0].entry.revision, "r1");
        assert_eq!(
            deliveries[0].entity.data.get("title"),
            Some(&serde_json::json!("Selected payload"))
        );
    }

    #[test]
    fn inbox_replay_requires_same_digest_and_applied_is_distinct_from_receipt() {
        let conn = db();
        let first = receive_inbox_payload(&conn, "peer-a", "object:a:r1", "{\"x\":1}").unwrap();
        assert_eq!(first, InboxState::Received);
        assert_eq!(
            receive_inbox_payload(&conn, "peer-a", "object:a:r1", "{\"x\":1}").unwrap(),
            InboxState::Received
        );
        assert_eq!(
            receive_inbox_payload(&conn, "peer-a", "object:a:r1", "{\"x\":2}").unwrap_err(),
            "sync_inbox_identity_digest_mismatch"
        );
        set_inbox_state(&conn, "peer-a", "object:a:r1", InboxState::Applied).unwrap();
        assert_eq!(
            receive_inbox_payload(&conn, "peer-a", "object:a:r1", "{\"x\":1}").unwrap(),
            InboxState::Applied
        );
    }

    #[test]
    fn inbox_rejects_oversized_payload_before_insert() {
        let conn = db();
        let payload = "x".repeat(MAX_INBOX_PAYLOAD_BYTES + 1);
        assert_eq!(
            receive_inbox_payload(&conn, "peer-a", "message-1", &payload).unwrap_err(),
            "sync_inbox_payload_limit_exceeded"
        );
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM ark_sync_inbox", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn outbound_receipt_and_applied_cursors_are_distinct_and_policy_scoped() {
        let conn = db();
        let progress = begin_outbound_policy(&conn, "peer-a", 4).unwrap();
        assert!(!progress.initial_snapshot.is_complete());
        let progress = complete_outbound_snapshot(&conn, "peer-a", 4).unwrap();
        assert!(progress.initial_snapshot.is_complete());
        for index in 1..=3 {
            append_committed_change(
                &conn,
                "object",
                &format!("o{index}"),
                "r1",
                false,
                None,
                None,
            )
            .unwrap();
        }
        let progress = begin_outbound_policy(&conn, "peer-a", 4).unwrap();
        assert_eq!(progress.policy_generation, 4);
        let progress = record_outbound_scan(&conn, "peer-a", 4, 3).unwrap();
        assert_eq!(progress.scanned_through, 3);
        let progress =
            acknowledge_outbound(&conn, "peer-a", 4, 2, OutboundAck::DurableReceipt).unwrap();
        assert_eq!(progress.receipt_through, 2);
        assert_eq!(progress.applied_through, 0);
        let progress = acknowledge_outbound(&conn, "peer-a", 4, 1, OutboundAck::Applied).unwrap();
        assert_eq!(progress.receipt_through, 2);
        assert_eq!(progress.applied_through, 1);
        let reset = begin_outbound_policy(&conn, "peer-a", 5).unwrap();
        assert_eq!(
            reset.receipt_through, 0,
            "new policy generation replays feed for re-filtering"
        );
        assert_eq!(
            acknowledge_outbound(&conn, "peer-a", 5, 3, OutboundAck::Applied).unwrap_err(),
            "invalid_outbound_feed_ack"
        );
    }

    #[test]
    fn initial_snapshot_watermark_survives_restart_and_only_advances_after_completion() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("initial-snapshot.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(ark_store::schema::CREATE_TABLES)
            .unwrap();
        append_committed_change(&conn, "object", "snapshot-row", "r1", false, None, None).unwrap();

        let started = begin_outbound_policy(&conn, "peer-a", 7).unwrap();
        let baseline = started.initial_snapshot.through();
        assert_eq!(baseline, 1);
        assert!(!started.initial_snapshot.is_complete());
        assert_eq!(started.applied_through, 0);
        append_committed_change(&conn, "object", "interactive-row", "r2", false, None, None)
            .unwrap();
        drop(conn);

        let conn = Connection::open(&path).unwrap();
        let resumed = begin_outbound_policy(&conn, "peer-a", 7).unwrap();
        assert_eq!(resumed.initial_snapshot.through(), baseline);
        assert!(!resumed.initial_snapshot.is_complete());
        assert_eq!(
            resumed.applied_through, 0,
            "restart mid-backfill cannot skip it"
        );

        let complete = complete_outbound_snapshot(&conn, "peer-a", 7).unwrap();
        assert!(complete.initial_snapshot.is_complete());
        assert_eq!(complete.applied_through, baseline);
        let post_snapshot = read_committed_feed_page(&conn, baseline, 2, 8).unwrap();
        assert_eq!(post_snapshot.len(), 1);
        assert_eq!(post_snapshot[0].entity_id, "interactive-row");

        let changed_policy = begin_outbound_policy(&conn, "peer-a", 8).unwrap();
        assert!(changed_policy.initial_snapshot.is_complete());
        assert_eq!(changed_policy.initial_snapshot.through(), 0);
        assert_eq!(changed_policy.applied_through, 0);
    }

    #[test]
    fn snapshot_completion_rejects_the_identity_from_an_older_accepted_policy() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("policy-bound-snapshot.db");
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(ark_store::schema::CREATE_TABLES)
            .unwrap();
        ark_store::data_platform::ensure_schema(&conn).unwrap();
        let rule = ark_application::sync_policy::SyncProfileRule {
            resource_kind: "type".into(),
            resource_id: "books".into(),
            mode: ark_domain::selective_sync::SyncMode::Full,
            filter: ark_domain::selective_sync::FilterV1::match_all(),
        };
        let digest = ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&rule));
        let first = ark_store::data_platform::accept_peer_sync_policy(
            &conn,
            "peer-a",
            1,
            &digest,
            true,
            std::slice::from_ref(&rule),
        )
        .unwrap()
        .identity();
        let pending = begin_outbound_policy_identity(&conn, "peer-a", &first).unwrap();
        assert!(!pending.initial_snapshot.is_complete());

        let next_rule = ark_application::sync_policy::SyncProfileRule {
            resource_id: "tasks".into(),
            ..rule
        };
        let next_digest =
            ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&next_rule));
        let next = ark_store::data_platform::accept_peer_sync_policy(
            &conn,
            "peer-a",
            2,
            &next_digest,
            true,
            &[next_rule],
        )
        .unwrap()
        .identity();

        assert_eq!(
            complete_outbound_snapshot_identity(&conn, "peer-a", &first).unwrap_err(),
            "outbound_feed_policy_changed"
        );
        let restarted = begin_outbound_policy_identity(&conn, "peer-a", &next).unwrap();
        assert!(!restarted.initial_snapshot.is_complete());
        assert!(complete_outbound_snapshot_identity(&conn, "peer-a", &next)
            .unwrap()
            .initial_snapshot
            .is_complete());
    }

    #[test]
    fn bound_feed_ack_rejects_digest_or_active_mismatch_at_same_revision() {
        let conn = db();
        let rule = ark_application::sync_policy::SyncProfileRule {
            resource_kind: "type".into(),
            resource_id: "books".into(),
            mode: ark_domain::selective_sync::SyncMode::Full,
            filter: ark_domain::selective_sync::FilterV1::match_all(),
        };
        let digest = ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&rule));
        let current = ark_store::data_platform::accept_peer_sync_policy(
            &conn,
            "peer-a",
            1,
            &digest,
            true,
            std::slice::from_ref(&rule),
        )
        .unwrap()
        .identity();
        begin_outbound_policy_identity(&conn, "peer-a", &current).unwrap();
        complete_outbound_snapshot_identity(&conn, "peer-a", &current).unwrap();
        append_committed_change(&conn, "object", "book", "r1", false, None, None).unwrap();
        record_outbound_scan_identity(&conn, "peer-a", &current, 1).unwrap();

        for stale in [
            ark_application::sync_storage::PolicyIdentity {
                revision: 1,
                digest: "0".repeat(64),
                active: true,
            },
            ark_application::sync_storage::PolicyIdentity {
                revision: 1,
                digest: digest.clone(),
                active: false,
            },
        ] {
            assert_eq!(
                acknowledge_outbound_identity(
                    &conn,
                    "peer-a",
                    &stale,
                    1,
                    OutboundAck::DurableReceipt,
                )
                .unwrap_err(),
                "outbound_feed_policy_changed"
            );
        }
        let progress = acknowledge_outbound_identity(
            &conn,
            "peer-a",
            &current,
            1,
            OutboundAck::DurableReceipt,
        )
        .unwrap();
        assert_eq!(progress.receipt_through, 1);
        assert_eq!(progress.applied_through, 0);
    }

    #[test]
    fn policy_change_after_pinned_feed_page_prevents_sending_that_page() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("policy-bound-feed-page.db");
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(ark_store::schema::CREATE_TABLES)
            .unwrap();
        ark_store::data_platform::ensure_schema(&conn).unwrap();
        let rule = |resource_id: &str| ark_application::sync_policy::SyncProfileRule {
            resource_kind: "type".into(),
            resource_id: resource_id.into(),
            mode: ark_domain::selective_sync::SyncMode::Full,
            filter: ark_domain::selective_sync::FilterV1::match_all(),
        };
        let first_rule = rule("books");
        let first_digest =
            ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&first_rule));
        let first = ark_store::data_platform::accept_peer_sync_policy(
            &conn,
            "peer-a",
            1,
            &first_digest,
            true,
            &[first_rule],
        )
        .unwrap()
        .identity();
        conn.execute(
            "INSERT INTO sync_tombstones(id,entity_type,hlc,deleted_at,type_id) \
             VALUES('book-1','object','r1','now','books')",
            [],
        )
        .unwrap();
        append_committed_change(&conn, "object", "book-1", "r1", true, None, None).unwrap();
        let high_watermark = committed_feed_snapshot(&conn).unwrap().high_watermark;
        let pinned = read_committed_feed_delivery_page_for_peer(
            &conn,
            "peer-a",
            &first,
            0,
            high_watermark,
            16,
        )
        .unwrap();
        assert_eq!(pinned.identity, first);
        let deliveries = match pinned.page {
            CommittedFeedDeliveryPage::Deliveries(deliveries) => deliveries,
            CommittedFeedDeliveryPage::SkipTo { .. } => Vec::new(),
            CommittedFeedDeliveryPage::TooLarge { .. } => Vec::new(),
        };
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].entity.id, "book-1");

        let next_rule = rule("tasks");
        let next_digest =
            ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&next_rule));
        ark_store::data_platform::accept_peer_sync_policy(
            &conn,
            "peer-a",
            2,
            &next_digest,
            true,
            &[next_rule],
        )
        .unwrap();
        assert_eq!(
            require_current_policy_identity(&conn, "peer-a", &pinned.identity).unwrap_err(),
            "outbound_feed_policy_changed"
        );
    }
}
