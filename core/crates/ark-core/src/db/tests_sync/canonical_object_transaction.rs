use crate::replication_storage::{AuthenticatedEntityBackend, PeerIdentityBackend};
use ark_application::sync_storage::CommittedFeedPort;
use std::time::Instant;

async fn admit_test_registry_peer(backend: &SqliteStorageBackend, peer_id: &str) {
    backend
        .enroll_paired_device(
            "device-local",
            peer_id,
            &ark_net::iroh::SecretKey::generate().public().to_string(),
        )
        .await
        .unwrap();
}

fn expect_feed_deliveries(
    page: ark_application::sync_storage::CommittedFeedDeliveryPage,
) -> Vec<ark_application::sync_storage::CommittedFeedDelivery> {
    match page {
        ark_application::sync_storage::CommittedFeedDeliveryPage::Deliveries(deliveries) => {
            deliveries
        }
        ark_application::sync_storage::CommittedFeedDeliveryPage::TooLarge { .. } => {
            panic!("test feed entity exceeded the bounded projection size")
        }
        ark_application::sync_storage::CommittedFeedDeliveryPage::SkipTo { .. } => {
            panic!("test feed page unexpectedly contained only skipped entities")
        }
    }
}

#[tokio::test]
async fn snapshot_page_pins_filter_identity_and_rejects_policy_change_before_next_page() {
    let backend = SqliteStorageBackend::for_test(Arc::new(Mutex::new(setup_db())));
    let entity = sync_object("snapshot-policy-page", "note_obj", "Pinned selection");
    backend.apply_entity(&entity).await.unwrap();
    let full_rule = ark_application::sync_policy::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "note_obj".into(),
        mode: ark_domain::selective_sync::SyncMode::Full,
        filter: ark_domain::selective_sync::FilterV1::match_all(),
    };
    let full_digest =
        ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&full_rule));
    let selected = backend
        .accept_peer_sync_policy(
            "peer-a",
            1,
            &full_digest,
            true,
            std::slice::from_ref(&full_rule),
        )
        .await
        .unwrap();
    let page = backend
        .load_policy_bound_entities_page_for_peer(
            "peer-a",
            ark_application::sync_storage::SyncReadClass::Backfill,
            &crate::types::VersionVector::default(),
            0,
            16,
            &selected.identity(),
        )
        .await
        .unwrap();
    assert_eq!(page.identity, selected.identity());

    let omitted_rule = ark_application::sync_policy::SyncProfileRule {
        resource_id: "different_type".into(),
        ..full_rule
    };
    let omitted_digest =
        ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&omitted_rule));
    let updated = backend
        .accept_peer_sync_policy("peer-a", 2, &omitted_digest, true, &[omitted_rule])
        .await
        .unwrap();
    assert_ne!(page.identity, updated.identity());
    let error = backend
        .load_policy_bound_entities_page_for_peer(
            "peer-a",
            ark_application::sync_storage::SyncReadClass::Backfill,
            &crate::types::VersionVector::default(),
            0,
            16,
            &page.identity,
        )
        .await
        .unwrap_err();
    assert!(error.contains("stale_or_unnegotiated_sync_policy"));
}

#[test]
fn initial_sync_snapshot_omits_tasks_owned_by_the_merge_journal() {
    let conn = setup_db();
    let task = full_core_task_entity(
        "snapshot-merge-task",
        "Must use the task merge journal",
        "2026-10-15T00:00:00.000Z:000011:source-device",
    );
    SqliteStorageBackend::apply_entity_blocking(&conn, &task).unwrap();
    let note = sync_object("snapshot-note", "note_obj", "Still uses the ordinary feed");
    SqliteStorageBackend::apply_entity_blocking(&conn, &note).unwrap();
    let vector = crate::types::VersionVector::default();

    let before =
        SqliteStorageBackend::collect_entities_page_blocking(&conn, &vector, "device", 0, 16)
            .unwrap();
    assert!(before.iter().any(|entity| entity.id == task.id));
    assert!(before.iter().any(|entity| entity.id == note.id));

    conn.execute(
        "INSERT INTO task_merge_bootstraps(
             bootstrap_id, object_id, bootstrap_json, payload_sha256
         ) VALUES ('00000000000000000000000000000001', ?1, '{}', 'digest')",
        [&task.id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO sync_tombstones(id, entity_type, hlc, deleted_at, type_id)
         VALUES (?1, 'object', 'r2', '2026-10-15T00:00:01Z', 'core.task')",
        [&task.id],
    )
    .unwrap();

    let after =
        SqliteStorageBackend::collect_entities_page_blocking(&conn, &vector, "device", 0, 16)
            .unwrap();
    assert!(after.iter().all(|entity| entity.id != task.id));
    assert!(after.iter().any(|entity| entity.id == note.id));
}

#[test]
fn initial_sync_snapshot_fails_when_merge_mode_cannot_be_checked() {
    let conn = setup_db();
    conn.execute_batch("DROP TABLE task_merge_bootstraps")
        .unwrap();

    let error = SqliteStorageBackend::collect_entities_page_blocking(
        &conn,
        &crate::types::VersionVector::default(),
        "device",
        0,
        16,
    )
    .unwrap_err();
    assert!(error.contains("task_merge_bootstraps"));
}
#[test]
fn failed_outer_apply_commit_rolls_back_and_releases_connection() {
    let conn = setup_db();
    conn.execute_batch(
        "CREATE TABLE deferred_probe (
             parent_id TEXT REFERENCES objects(id) DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TRIGGER add_deferred_probe AFTER INSERT ON objects
         BEGIN
             INSERT INTO deferred_probe(parent_id) VALUES ('missing-parent');
         END;",
    )
    .unwrap();

    let entity = sync_object("deferred-commit-failure", "note_obj", "Deferred failure");
    let error = SqliteStorageBackend::apply_entity_blocking(&conn, &entity).unwrap_err();
    assert!(!error.is_empty());
    assert!(
        conn.is_autocommit(),
        "failed outer RELEASE must roll back the transaction"
    );
    assert!(get_object(&conn, &entity.id).unwrap().is_none());

    conn.execute_batch("DROP TRIGGER add_deferred_probe")
        .unwrap();
    let next = sync_object("after-deferred-failure", "note_obj", "Next write");
    SqliteStorageBackend::apply_entity_blocking(&conn, &next).unwrap();
    assert!(get_object(&conn, &next.id).unwrap().is_some());
}

#[tokio::test]
async fn failed_authenticated_core_task_commit_rolls_back_every_write_and_releases_connection() {
    let conn = setup_db();
    conn.execute_batch(
        "CREATE TABLE deferred_core_task_probe (
             parent_id TEXT REFERENCES objects(id) DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TRIGGER fail_core_task_commit AFTER INSERT ON objects
         BEGIN
             INSERT INTO deferred_core_task_probe(parent_id) VALUES ('missing-parent');
         END;",
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(conn));
    let backend = SqliteStorageBackend::for_test(shared.clone());
    admit_test_registry_peer(&backend, "device-a").await;
    let entity = full_core_task_entity(
        "failed-core-task-commit",
        "Commit must roll back",
        "2026-10-15T00:00:00.000Z:000001:device-a",
    );

    let error = backend
        .apply_authenticated_entity(&entity, "device-a")
        .await
        .unwrap_err();
    assert!(!error.is_empty());
    {
        let conn = shared.lock().unwrap();
        assert!(
            conn.is_autocommit(),
            "failed COMMIT must release the connection"
        );
        assert!(get_object(&conn, &entity.id).unwrap().is_none());
        let fts_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM object_search_fts WHERE object_id = ?1",
                [&entity.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fts_rows, 0);
        let sync_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM object_sync_versions WHERE object_id = ?1",
                [&entity.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sync_rows, 0);
        let receipt_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                [&entity.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(receipt_rows, 0);
        let inbox_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ark_sync_inbox WHERE peer_device_id = ?1",
                ["device-a"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(inbox_rows, 1, "durable receipt survives failed apply");
        let inbox_state: String = conn
            .query_row(
                "SELECT state FROM ark_sync_inbox WHERE peer_device_id = 'device-a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(inbox_state, "received", "failed apply remains replayable");
        let feed_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ark_committed_feed WHERE entity_id = ?1",
                [&entity.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(feed_rows, 0, "feed row must roll back with failed apply");
        assert!(
            !ark_store::sync_kv::get_sync_kv(&conn, "lan_sync.version_vector")
                .unwrap_or_default()
                .unwrap_or_default()
                .contains(&entity.id)
        );
        conn.execute_batch("DROP TRIGGER fail_core_task_commit")
            .unwrap();
    }

    let next = full_core_task_entity(
        "after-core-task-commit-failure",
        "Connection remains usable",
        "2026-10-15T00:00:00.000Z:000002:device-a",
    );
    backend
        .apply_authenticated_entity(&next, "device-a")
        .await
        .unwrap();
    let conn = shared.lock().unwrap();
    assert!(get_object(&conn, &next.id).unwrap().is_some());
}

#[tokio::test]
async fn durable_inbox_receipt_replays_after_writer_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("feed-restart.db");
    let entity = full_core_task_entity(
        "feed-restart-task",
        "Replay after writer restart",
        "2026-10-16T00:00:00.000Z:000001:device-a",
    );
    let (handle, mut owner) =
        crate::db::DbHandle::open(path.to_string_lossy().into_owned(), "device-local".into())
            .await
            .unwrap();
    let backend = SqliteStorageBackend::with_db_handle(handle);
    admit_test_registry_peer(&backend, "device-a").await;
    let failpoint = rusqlite::Connection::open(&path).unwrap();
    failpoint
        .execute_batch(
            "CREATE TABLE deferred_feed_probe (
             parent_id TEXT REFERENCES objects(id) DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TRIGGER fail_feed_commit AFTER INSERT ON objects
         BEGIN
             INSERT INTO deferred_feed_probe(parent_id) VALUES ('missing-parent');
         END;",
        )
        .unwrap();
    assert!(backend
        .apply_authenticated_entity(&entity, "device-a")
        .await
        .is_err());
    owner
        .shutdown(Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();

    let check = rusqlite::Connection::open(&path).unwrap();
    let (state, bytes): (String, i64) = check
        .query_row(
            "SELECT state, payload_bytes FROM ark_sync_inbox WHERE peer_device_id = 'device-a'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, "received");
    assert!(bytes > 0, "pending receipt retains payload for replay");
    check
        .execute_batch("DROP TRIGGER fail_feed_commit")
        .unwrap();
    drop(check);

    let (handle, mut owner) =
        crate::db::DbHandle::open(path.to_string_lossy().into_owned(), "device-local".into())
            .await
            .unwrap();
    let backend = SqliteStorageBackend::with_db_handle(handle);
    backend
        .apply_authenticated_entity(&entity, "device-a")
        .await
        .unwrap();
    let snapshot = backend.committed_feed_snapshot().await.unwrap();
    let feed = backend
        .committed_feed_page(0, snapshot.high_watermark, 8)
        .await
        .unwrap();
    assert!(feed.iter().any(|entry| entry.entity_id == entity.id));
    let check = rusqlite::Connection::open(&path).unwrap();
    let (state, bytes): (String, i64) = check
        .query_row(
            "SELECT state, payload_bytes FROM ark_sync_inbox WHERE peer_device_id = 'device-a'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((state.as_str(), bytes), ("applied", 0));
    owner
        .shutdown(Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn authenticated_apply_rechecks_removed_peer_before_persisting() {
    let conn = setup_db();
    ark_store::sync_kv::set_sync_kv(&conn, "sync.removed_peers", r#"["device-blocked"]"#).unwrap();
    let backend = SqliteStorageBackend::for_test(Arc::new(Mutex::new(conn)));
    let mut entity = sync_object("blocked-core-task", "core.task", "Blocked task");
    entity.data.insert("typeVersion".into(), json!("0.1.0"));
    entity.data.insert(
        "propsJson".into(),
        json!({
            "state": {"kind": "open", "at": null},
            "planning": {"kind": "inbox", "period": "any", "date": null},
            "deadline": null,
            "reminders": [],
            "recurrence": null,
            "priority": "none",
            "checklist": [],
            "extensions": {}
        }),
    );

    let result = backend
        .apply_authenticated_entity(&entity, "device-blocked")
        .await;
    assert!(
        result.is_err(),
        "blocked authenticated peer must be rejected"
    );
    let shared = backend.db.shared_connection_for_test();
    let conn = shared.lock().unwrap();
    assert!(get_object(&conn, &entity.id).unwrap().is_none());
}

include!("canonical_object_transaction/object_feed.rs");
include!("canonical_object_transaction/object_feed_replay.rs");
include!("canonical_object_transaction/pending_receipt.rs");
include!("canonical_object_transaction/receipt_ack.rs");

fn pop_feed_live_change_id(
    rx: &mut tokio::sync::mpsc::Receiver<ark_sync::protocol::LanSyncMessage>,
) -> String {
    match rx.try_recv().unwrap() {
        ark_sync::protocol::LanSyncMessage::LiveChange { change_id, .. } => change_id,
        message => panic!("unexpected message: {message:?}"),
    }
}
