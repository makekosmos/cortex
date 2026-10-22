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
    let backend = make_backend().await;
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
    assert_eq!(
        error,
        ark_application::sync_storage::PolicyBoundSnapshotReadError::StalePolicy
    );
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn authenticated_core_task_commit_failure_is_reported_and_fails_closed() {
    use ark_store::test_support::ProductionActorFailureProbe as FailureProbe;

    let database =
        ark_store::test_support::TestDatabase::create().expect("test database initializes");
    FailureProbe::install_deferred_object_commit_failure_path(database.path()).unwrap();
    let backend = SqliteStorageBackend::open_for_test(
        database.path().to_string_lossy().into_owned(),
        "device-local",
    )
    .await
    .unwrap();
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
    FailureProbe::remove_deferred_object_commit_failure_path(database.path()).unwrap();

    let next = full_core_task_entity(
        "after-core-task-commit-failure",
        "Failed actor must reject later writes",
        "2026-10-15T00:00:00.000Z:000002:device-a",
    );
    assert!(
        backend
            .apply_authenticated_entity(&next, "device-a")
            .await
            .is_err(),
        "writer actor accepted a write after COMMIT failed"
    );
    backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn durable_inbox_receipt_replays_after_writer_restart() {
    let directory = crate::test_support::temp_dir();
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
    ark_store::test_support::ProductionActorFailureProbe::install_deferred_feed_commit_failure(
        &path,
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

    ark_store::test_support::ProductionActorFailureProbe::remove_deferred_feed_commit_failure(
        &path,
    )
    .unwrap();

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
    owner
        .shutdown(Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn authenticated_apply_rejects_removed_peer() {
    let database =
        ark_store::test_support::TestDatabase::create().expect("test database initializes");
    ark_store::test_support::SyncTestState::add_removed_peer(database.path(), "device-blocked")
        .unwrap();
    let backend = SqliteStorageBackend::open_for_test(
        database.path().to_string_lossy().into_owned(),
        "device-local",
    )
    .await
    .unwrap();
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
    assert_eq!(
        backend
            .committed_feed_snapshot()
            .await
            .unwrap()
            .high_watermark,
        0,
        "a removed peer must not advance the committed feed"
    );
    backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

include!("canonical_object_transaction/object_feed.rs");
include!("canonical_object_transaction/object_feed_replay.rs");
include!("canonical_object_transaction/pending_receipt.rs");
include!("canonical_object_transaction/receipt_ack.rs");

fn pop_feed_live_change_id(
    rx: &mut tokio::sync::mpsc::Receiver<ark_sync_contracts::wire::LanSyncMessage>,
) -> String {
    match rx.try_recv().unwrap() {
        ark_sync_contracts::wire::LanSyncMessage::LiveChange { change_id, .. } => change_id,
        message => panic!("unexpected message: {message:?}"),
    }
}
