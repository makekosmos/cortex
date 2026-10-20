use super::*;

#[tokio::test]
async fn inbound_batch_rolls_back_earlier_entries_when_a_later_entry_fails() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    let peer_id = "rollback-peer";
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("merge-test-device".into(), peer_id.into(), endpoint),
        )
        .await
        .unwrap();
    let identity = ark_application::sync_policy::default_sync_policy_identity();
    let batch = ark_application::sync_storage::TaskMergeBatch {
        batch_id: "d1".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 2,
        entries: vec![
            ark_application::sync_storage::TaskMergeJournalEntry {
                position: 1,
                record: ark_application::sync_storage::TaskMergeJournalRecord::Operation {
                    operation: operation('1', &[], "first entry"),
                    expected_conflict_digest: None,
                },
            },
            ark_application::sync_storage::TaskMergeJournalEntry {
                position: 2,
                record: ark_application::sync_storage::TaskMergeJournalRecord::Operation {
                    operation: operation('1', &[], "conflicting duplicate"),
                    expected_conflict_digest: None,
                },
            },
        ],
    };

    assert!(
        apply_batch(&handle, peer_id, "merge-test-device", identity, batch)
            .await
            .is_err()
    );
    let (operation_count, journal_count, progress_count): (i64, i64, i64) =
        handle.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM task_merge_operations),
                            (SELECT COUNT(*) FROM task_merge_journal),
                            (SELECT COUNT(*) FROM task_merge_inbound_progress)",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap()
        });
    assert_eq!((operation_count, journal_count, progress_count), (0, 0, 0));
    shutdown(&mut owner).await;
}
