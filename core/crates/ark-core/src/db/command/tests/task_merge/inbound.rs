use super::*;

async fn apply_batch(
    handle: &DbHandle,
    peer_id: &str,
    local_device_id: &str,
    identity: crate::application::sync_storage::PolicyIdentity,
    batch: crate::application::sync_storage::TaskMergeBatch,
) -> Result<crate::application::sync_storage::TaskMergeBatchAck, DbError> {
    handle
        .submit(
            CommandContext::authenticated_peer(peer_id.into()),
            ApplyTaskMergeBatchCommand::new(
                peer_id.into(),
                local_device_id.into(),
                identity,
                batch,
            ),
        )
        .await
}

#[tokio::test]
async fn two_sqlite_peers_converge_on_the_same_conflict_set() {
    let (_left_dir, _left_path, left, mut left_owner) = open_merge_fixture().await;
    let right_dir = tempfile::tempdir().unwrap();
    let right_path = right_dir
        .path()
        .join("task-merge-right.db")
        .to_string_lossy()
        .into_owned();
    let (right, mut right_owner) = DbHandle::open(right_path.clone(), "right-device".into())
        .await
        .unwrap();
    let peer_id = "merge-test-device";
    let left_endpoint = iroh::SecretKey::generate().public().to_string();
    let right_endpoint = iroh::SecretKey::generate().public().to_string();
    for (handle, local_id, remote_id, endpoint) in [
        (&left, "merge-test-device", "right-device", right_endpoint),
        (&right, "right-device", peer_id, left_endpoint),
    ] {
        handle
            .submit(
                CommandContext::engine(),
                EnrollPairedDeviceCommand::new(local_id.into(), remote_id.into(), endpoint),
            )
            .await
            .unwrap();
    }
    append(&left, operation('1', &[], "common base"), None)
        .await
        .unwrap();
    for (id, title) in [('2', "left edit"), ('3', "right edit")] {
        append(&left, operation(id, &['1'], title), None)
            .await
            .unwrap();
    }
    let identity = crate::data_platform::default_sync_policy_identity();
    let batch = left
        .submit(
            CommandContext::authenticated_peer("right-device".into()),
            PrepareTaskMergeBatchCommand::new("right-device".into(), identity.clone(), 100),
        )
        .await
        .unwrap()
        .unwrap();
    apply_batch(&right, peer_id, "merge-test-device", identity, batch)
        .await
        .unwrap();

    let conflict_set = right
        .query(
            CommandContext::engine(),
            TaskConflictSetQuery::new(
                "task-1".into(),
                crate::domain::task_merge::TaskMergeUnit::Title,
            ),
        )
        .await
        .unwrap()
        .conflict_set
        .unwrap();
    assert_eq!(conflict_set.common_ancestors.len(), 1);
    assert_eq!(conflict_set.variants.len(), 2);
    assert!(conflict_set.has_conflict());
    shutdown(&mut left_owner).await;
    shutdown(&mut right_owner).await;
}

#[tokio::test]
async fn inbound_batch_atomically_applies_remote_resolution_and_replays_idempotently() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    let peer_id = "paired-peer";
    let endpoint = iroh::SecretKey::generate().public().to_string();
    handle
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("merge-test-device".into(), peer_id.into(), endpoint),
        )
        .await
        .unwrap();
    let identity = crate::data_platform::default_sync_policy_identity();
    let batch = crate::application::sync_storage::TaskMergeBatch {
        batch_id: "a1".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 2,
        entries: vec![
            crate::application::sync_storage::TaskMergeJournalEntry {
                position: 1,
                operation: operation('1', &[], "remote base"),
                expected_conflict_digest: None,
            },
            crate::application::sync_storage::TaskMergeJournalEntry {
                position: 2,
                operation: operation('2', &['1'], "remote resolution"),
                expected_conflict_digest: Some(format!("sha256:{}", "ab".repeat(32))),
            },
        ],
    };

    let applied = apply_batch(
        &handle,
        peer_id,
        "merge-test-device",
        identity.clone(),
        batch.clone(),
    )
    .await
    .unwrap();
    assert!(!applied.replayed);
    assert_eq!(applied.through_position, 2);
    let replayed = apply_batch(
        &handle,
        peer_id,
        "merge-test-device",
        identity.clone(),
        batch.clone(),
    )
    .await
    .unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.through_position, 2);

    let mut altered = batch.clone();
    altered.entries[1].operation = operation('2', &['1'], "altered resolution");
    assert!(
        apply_batch(&handle, peer_id, "merge-test-device", identity, altered)
            .await
            .is_err()
    );
    let (operation_count, journal_count, cursor, guard): (i64, i64, i64, String) = handle
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT
                        (SELECT COUNT(*) FROM task_merge_operations),
                        (SELECT COUNT(*) FROM task_merge_journal),
                        (SELECT received_through FROM task_merge_inbound_progress
                            WHERE peer_device_id = ?1),
                        (SELECT expected_conflict_digest FROM task_merge_resolution_guards
                            WHERE operation_id = ?2)",
                    rusqlite::params![peer_id, id('2').as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap()
        });
    assert_eq!((operation_count, journal_count, cursor), (2, 2, 2));
    assert_eq!(guard, format!("sha256:{}", "ab".repeat(32)));
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn excluded_local_task_category_rejects_incoming_batch_without_advancing_cursor() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    let peer_id = "paired-peer";
    let endpoint = iroh::SecretKey::generate().public().to_string();
    handle
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("merge-test-device".into(), peer_id.into(), endpoint),
        )
        .await
        .unwrap();
    let profile = handle
        .submit(
            CommandContext::engine(),
            SaveLocalSyncProfileCommand::new(
                "merge-test-device".into(),
                "phone".into(),
                "Phone".into(),
                true,
                None,
                vec![crate::data_platform::SyncProfileRule {
                    resource_kind: "type".into(),
                    resource_id: "core.task".into(),
                    mode: crate::data_platform::SyncMode::None,
                    filter: crate::data_platform::FilterV1::match_all(),
                }],
            ),
        )
        .await
        .unwrap();
    let identity = crate::protocol::sync_policy_identity(Some(&profile));
    let batch = crate::application::sync_storage::TaskMergeBatch {
        batch_id: "bb".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 1,
        entries: vec![crate::application::sync_storage::TaskMergeJournalEntry {
            position: 1,
            operation: operation('1', &[], "private on this device"),
            expected_conflict_digest: None,
        }],
    };
    assert!(matches!(
        apply_batch(&handle, peer_id, "merge-test-device", identity, batch).await,
        Err(DbError::Permission(_))
    ));
    let (operation_count, cursor_count): (i64, i64) = handle.with_connection(|connection| {
        connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM task_merge_operations),
                        (SELECT COUNT(*) FROM task_merge_inbound_progress)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    });
    assert_eq!((operation_count, cursor_count), (0, 0));
    shutdown(&mut owner).await;
}
