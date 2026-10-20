use super::*;

#[tokio::test]
async fn inbound_batch_rolls_back_earlier_entries_when_a_later_entry_fails() {
    let (_directory, path, handle, mut owner) = open_merge_fixture().await;
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
    let state = super::inspect_inbound(&path, peer_id, "");
    assert_eq!(state.operation_count, 0);
    assert!(state.journal.is_empty());
    assert_eq!(state.inbound_progress_count, 0);
    shutdown(&mut owner).await;
}
