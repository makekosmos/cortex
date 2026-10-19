use super::super::{
    load_inbound_progress, record_inbound_batch, update_inbound_policy_identity,
    TaskMergeInboundBatchReceipt, TaskMergeInboundProgress, TaskMergeStoreError,
};
use super::connection;

#[test]
fn inbound_progress_store_reads_receipts_and_updates_policy_identity() {
    let mut connection = connection();
    let transaction = connection
        .transaction()
        .expect("actor-style transaction begins");
    let identity = ark_application::sync_policy::PolicyIdentity::new(3, "c".repeat(64), true)
        .expect("policy identity is valid");
    let receipt = TaskMergeInboundBatchReceipt {
        batch_id: "batch-1".into(),
        after: 0,
        through: 4,
        payload_digest: "d".repeat(64),
    };
    record_inbound_batch(&transaction, "peer-1", &identity, &receipt)
        .expect("batch receipt persists");
    assert_eq!(
        load_inbound_progress(&transaction, "peer-1").expect("progress reads"),
        Some(TaskMergeInboundProgress {
            received_through: 4,
            last_batch: Some(receipt),
        })
    );

    let next_identity = ark_application::sync_policy::PolicyIdentity::new(4, "e".repeat(64), false)
        .expect("next policy identity is valid");
    update_inbound_policy_identity(&transaction, "peer-1", &next_identity)
        .expect("policy identity updates");
    let stored_revision: i64 = transaction
        .query_row(
            "SELECT policy_revision FROM task_merge_inbound_progress WHERE peer_device_id = ?1",
            ["peer-1"],
            |row| row.get(0),
        )
        .expect("policy revision reads");
    assert_eq!(stored_revision, next_identity.revision);
    transaction
        .commit()
        .expect("actor-style transaction commits");
}

#[test]
fn inbound_progress_store_rejects_partial_receipt_rows() {
    let connection = connection();
    connection
        .execute(
            "INSERT INTO task_merge_inbound_progress(
                peer_device_id, policy_revision, policy_digest, policy_active,
                last_batch_id
             ) VALUES ('peer-1', 1, ?1, 1, 'batch-1')",
            ["a".repeat(64)],
        )
        .expect("partial row inserts in unconstrained fixture");

    assert!(matches!(
        load_inbound_progress(&connection, "peer-1"),
        Err(TaskMergeStoreError::Invalid(ref error))
            if error == "task_merge_inbound_progress_corrupt"
    ));
}
