use super::*;

fn entry_operation(
    entry: &ark_application::sync_storage::TaskMergeJournalEntry,
) -> &TaskMergeOperation {
    match &entry.record {
        ark_application::sync_storage::TaskMergeJournalRecord::Operation { operation, .. } => {
            operation
        }
        ark_application::sync_storage::TaskMergeJournalRecord::Bootstrap(_) => {
            panic!("expected operation journal record")
        }
    }
}

fn expected_digest(entry: &ark_application::sync_storage::TaskMergeJournalEntry) -> Option<&str> {
    match &entry.record {
        ark_application::sync_storage::TaskMergeJournalRecord::Operation {
            expected_conflict_digest,
            ..
        } => expected_conflict_digest.as_deref(),
        ark_application::sync_storage::TaskMergeJournalRecord::Bootstrap(_) => None,
    }
}

async fn prepare_batch(
    handle: &DbHandle,
    peer_id: &str,
    identity: ark_application::sync_storage::PolicyIdentity,
    limit: usize,
) -> Result<Option<ark_application::sync_storage::TaskMergeBatch>, DbError> {
    handle
        .submit(
            CommandContext::authenticated_peer(peer_id.into()),
            PrepareTaskMergeBatchCommand::new(peer_id.into(), identity, limit),
        )
        .await
}

async fn acknowledge_batch(
    handle: &DbHandle,
    peer_id: &str,
    identity: ark_application::sync_storage::PolicyIdentity,
    batch: &ark_application::sync_storage::TaskMergeBatch,
    through: i64,
) -> Result<i64, DbError> {
    handle
        .submit(
            CommandContext::authenticated_peer(peer_id.into()),
            AcknowledgeTaskMergeBatchCommand::new(
                peer_id.into(),
                identity,
                batch.batch_id.clone(),
                through,
            ),
        )
        .await
}

#[tokio::test]
async fn merge_batch_requires_matching_policy_and_full_task_selection() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('1', &[], "first"), None)
        .await
        .unwrap();
    let limited_policy = ark_application::sync_policy::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "core.task".into(),
        mode: ark_domain::selective_sync::SyncMode::Metadata,
        filter: ark_domain::selective_sync::FilterV1::match_all(),
    };
    let digest =
        ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&limited_policy));
    handle.with_connection(|connection| {
        ark_store::data_platform::accept_peer_sync_policy(
            connection,
            "peer-a",
            1,
            &digest,
            true,
            std::slice::from_ref(&limited_policy),
        )
        .unwrap();
    });
    let identity = ark_application::sync_storage::PolicyIdentity {
        revision: 1,
        digest,
        active: true,
    };
    let denied = handle
        .submit(
            CommandContext::authenticated_peer("peer-a".into()),
            PrepareTaskMergeBatchCommand::new("peer-a".into(), identity.clone(), 1),
        )
        .await;
    assert!(matches!(denied, Err(DbError::Permission(_))));

    let stale = handle
        .submit(
            CommandContext::authenticated_peer("peer-a".into()),
            PrepareTaskMergeBatchCommand::new(
                "peer-a".into(),
                ark_application::sync_policy::default_sync_policy_identity(),
                1,
            ),
        )
        .await;
    assert!(matches!(stale, Err(DbError::Permission(_))));

    let filtered_policy = ark_application::sync_policy::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "core.task".into(),
        mode: ark_domain::selective_sync::SyncMode::Full,
        filter: ark_domain::selective_sync::FilterV1::parse(
            r#"{"version":1,"where":{"eq":{"field":"title","value":"visible"}}}"#,
        )
        .unwrap(),
    };
    let filtered_digest =
        ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&filtered_policy));
    handle.with_connection(|connection| {
        ark_store::data_platform::accept_peer_sync_policy(
            connection,
            "peer-a",
            2,
            &filtered_digest,
            true,
            std::slice::from_ref(&filtered_policy),
        )
        .unwrap();
    });
    let filtered = handle
        .submit(
            CommandContext::authenticated_peer("peer-a".into()),
            PrepareTaskMergeBatchCommand::new(
                "peer-a".into(),
                ark_application::sync_storage::PolicyIdentity {
                    revision: 2,
                    digest: filtered_digest,
                    active: true,
                },
                1,
            ),
        )
        .await;
    assert!(matches!(filtered, Err(DbError::Permission(_))));
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn outbound_batches_are_stable_until_ack_and_isolated_by_peer() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('1', &[], "first"), None)
        .await
        .unwrap();
    append(&handle, operation('2', &['1'], "second"), None)
        .await
        .unwrap();
    let identity = ark_application::sync_policy::default_sync_policy_identity();
    let peer_mismatch = handle
        .submit(
            CommandContext::authenticated_peer("peer-b".into()),
            PrepareTaskMergeBatchCommand::new("peer-a".into(), identity.clone(), 1),
        )
        .await;
    assert!(matches!(peer_mismatch, Err(DbError::Permission(_))));

    let first = prepare_batch(&handle, "peer-a", identity.clone(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.after_position, 0);
    assert_eq!(first.through_position, 1);
    assert_eq!(entry_operation(&first.entries[0]).id, id('1'));
    let retry = prepare_batch(&handle, "peer-a", identity.clone(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retry, first);

    let wrong_end = acknowledge_batch(&handle, "peer-a", identity.clone(), &first, 2).await;
    assert!(matches!(wrong_end, Err(DbError::Invalid(_))));
    assert_eq!(
        acknowledge_batch(&handle, "peer-a", identity.clone(), &first, 1)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        acknowledge_batch(&handle, "peer-a", identity.clone(), &first, 1)
            .await
            .unwrap(),
        1
    );

    let second = prepare_batch(&handle, "peer-a", identity.clone(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.after_position, 1);
    assert_eq!(entry_operation(&second.entries[0]).id, id('2'));
    let other_peer = prepare_batch(&handle, "peer-b", identity, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entry_operation(&other_peer.entries[0]).id, id('1'));
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn prepared_batch_replays_identically_after_sqlite_reopen() {
    let (_directory, path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('1', &[], "first"), None)
        .await
        .unwrap();
    let identity = ark_application::sync_policy::default_sync_policy_identity();
    let prepared = prepare_batch(&handle, "peer-a", identity.clone(), 1)
        .await
        .unwrap()
        .unwrap();
    shutdown(&mut owner).await;

    let (reopened, mut reopened_owner) = DbHandle::open(path, "merge-test-device".into())
        .await
        .unwrap();
    let replay = prepare_batch(&reopened, "peer-a", identity.clone(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replay, prepared);
    assert_eq!(
        acknowledge_batch(&reopened, "peer-a", identity, &replay, 1)
            .await
            .unwrap(),
        1
    );
    shutdown(&mut reopened_owner).await;
}

#[tokio::test]
async fn outbound_journal_preserves_the_conflict_resolution_guard() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('1', &[], "first"), None)
        .await
        .unwrap();
    let conflict = append(&handle, operation('2', &[], "second"), None)
        .await
        .unwrap()
        .conflict_set
        .unwrap();
    let digest = conflict.digest;
    let resolution = operation('3', &['1', '2'], "resolved");
    append(&handle, resolution.clone(), Some(digest.clone()))
        .await
        .unwrap();
    assert!(append(&handle, resolution, None).await.unwrap().replayed);

    let batch = prepare_batch(
        &handle,
        "peer-a",
        ark_application::sync_policy::default_sync_policy_identity(),
        100,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        expected_digest(batch.entries.last().unwrap()),
        Some(digest.as_str())
    );
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn outbound_merge_batches_obey_the_wire_size_budget() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    for suffix in ['1', '2'] {
        let mut large = operation(suffix, &[], "");
        large.value = ark_domain::task_merge::TaskMergeValue::Field(serde_json::Value::String(
            "x".repeat(600_000),
        ));
        append(&handle, large, None).await.unwrap();
    }

    let identity = ark_application::sync_policy::default_sync_policy_identity();
    let first = prepare_batch(&handle, "peer-a", identity.clone(), 100)
        .await
        .unwrap()
        .unwrap();
    let encoded = serde_json::to_vec(&first).unwrap();
    assert!(encoded.len() <= 1_048_576 - 4_096);
    assert_eq!(first.entries.len(), 1);
    assert_eq!(first.through_position, 1);
    acknowledge_batch(&handle, "peer-a", identity.clone(), &first, 1)
        .await
        .unwrap();

    let mut oversized = operation('3', &[], "");
    oversized.value = ark_domain::task_merge::TaskMergeValue::Field(serde_json::Value::String(
        "x".repeat(1_050_000),
    ));
    append(&handle, oversized, None).await.unwrap();
    let second = prepare_batch(&handle, "peer-a", identity.clone(), 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.entries.len(), 1);
    assert_eq!(second.through_position, 2);
    acknowledge_batch(&handle, "peer-a", identity.clone(), &second, 2)
        .await
        .unwrap();

    let next = prepare_batch(&handle, "peer-a", identity, 100).await;
    assert!(matches!(next, Err(DbError::Invalid(_))));
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn policy_exclusion_preserves_local_journal_and_widening_resumes_from_ack() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('1', &[], "first"), None)
        .await
        .unwrap();
    append(&handle, operation('2', &['1'], "second"), None)
        .await
        .unwrap();
    let initial_identity = ark_application::sync_policy::default_sync_policy_identity();
    let first = prepare_batch(&handle, "peer-a", initial_identity.clone(), 1)
        .await
        .unwrap()
        .unwrap();
    acknowledge_batch(&handle, "peer-a", initial_identity.clone(), &first, 1)
        .await
        .unwrap();
    let pending_before_exclusion = prepare_batch(&handle, "peer-a", initial_identity, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        entry_operation(&pending_before_exclusion.entries[0]).id,
        id('2')
    );

    let excluded = ark_application::sync_policy::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "core.task".into(),
        mode: ark_domain::selective_sync::SyncMode::Metadata,
        filter: ark_domain::selective_sync::FilterV1::match_all(),
    };
    let excluded_digest =
        ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&excluded));
    handle.with_connection(|connection| {
        ark_store::data_platform::accept_peer_sync_policy(
            connection,
            "peer-a",
            1,
            &excluded_digest,
            true,
            std::slice::from_ref(&excluded),
        )
        .unwrap();
    });
    let excluded_identity = ark_application::sync_storage::PolicyIdentity {
        revision: 1,
        digest: excluded_digest,
        active: true,
    };
    let denied = prepare_batch(&handle, "peer-a", excluded_identity, 1).await;
    assert!(matches!(denied, Err(DbError::Permission(_))));
    let local_operations: i64 = handle.with_connection(|connection| {
        connection
            .query_row("SELECT COUNT(*) FROM task_merge_operations", [], |row| {
                row.get(0)
            })
            .unwrap()
    });
    assert_eq!(local_operations, 2);

    let full = ark_application::sync_policy::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "core.task".into(),
        mode: ark_domain::selective_sync::SyncMode::Full,
        filter: ark_domain::selective_sync::FilterV1::match_all(),
    };
    let full_digest = ark_application::sync_policy::digest_sync_policy(std::slice::from_ref(&full));
    handle.with_connection(|connection| {
        ark_store::data_platform::accept_peer_sync_policy(
            connection,
            "peer-a",
            2,
            &full_digest,
            true,
            std::slice::from_ref(&full),
        )
        .unwrap();
    });
    let widened_identity = ark_application::sync_storage::PolicyIdentity {
        revision: 2,
        digest: full_digest,
        active: true,
    };
    let resumed = prepare_batch(&handle, "peer-a", widened_identity.clone(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed.after_position, 1);
    assert_eq!(resumed.batch_id, pending_before_exclusion.batch_id);
    assert_eq!(entry_operation(&resumed.entries[0]).id, id('2'));
    let replayed_batch_ack = acknowledge_batch(
        &handle,
        "peer-a",
        widened_identity,
        &pending_before_exclusion,
        pending_before_exclusion.through_position,
    )
    .await;
    assert_eq!(replayed_batch_ack.unwrap(), 2);
    let other_peer = prepare_batch(
        &handle,
        "peer-b",
        ark_application::sync_policy::default_sync_policy_identity(),
        1,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(entry_operation(&other_peer.entries[0]).id, id('1'));
    shutdown(&mut owner).await;
}
