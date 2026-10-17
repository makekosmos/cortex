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
