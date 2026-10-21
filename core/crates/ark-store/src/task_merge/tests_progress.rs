use super::super::{
    acknowledge_outbound_batch, claim_outbound_batch, create_outbound_progress,
    load_outbound_progress, task_merge_watermark, update_outbound_policy_identity,
    TaskMergeAcknowledgedBatch, TaskMergeInFlightBatch,
};
use super::connection;

#[test]
fn outbound_progress_store_preserves_policy_flight_and_ack_state() {
    let mut connection = connection();
    let transaction = connection
        .transaction()
        .expect("actor-style transaction begins");
    let initial_identity =
        ark_application::sync_policy::PolicyIdentity::new(1, "a".repeat(64), true)
            .expect("initial policy identity is valid");
    create_outbound_progress(&transaction, "peer-1", &initial_identity)
        .expect("progress row creates");
    transaction
        .execute(
            "INSERT INTO task_merge_journal(record_kind, record_id) VALUES ('operation', 'op-1')",
            [],
        )
        .expect("journal entry inserts");
    assert_eq!(
        task_merge_watermark(&transaction).expect("watermark reads"),
        1
    );
    let initial = load_outbound_progress(&transaction, "peer-1")
        .expect("progress reads")
        .expect("progress row exists");
    assert_eq!(initial.identity, initial_identity);
    assert_eq!(initial.acknowledged_through, 0);
    assert!(initial.in_flight.is_none());

    let next_identity = ark_application::sync_policy::PolicyIdentity::new(2, "b".repeat(64), false)
        .expect("next policy identity is valid");
    update_outbound_policy_identity(&transaction, "peer-1", &next_identity)
        .expect("policy identity updates");
    assert!(claim_outbound_batch(&transaction, "peer-1", "batch-1", 0, 3).expect("batch claims"));
    let in_flight = load_outbound_progress(&transaction, "peer-1")
        .expect("progress reads")
        .expect("progress row exists");
    assert_eq!(in_flight.identity, next_identity);
    assert_eq!(
        in_flight.in_flight,
        Some(TaskMergeInFlightBatch {
            batch_id: "batch-1".into(),
            after: 0,
            through: 3,
        })
    );
    assert!(
        acknowledge_outbound_batch(&transaction, "peer-1", "batch-1", 3, 0)
            .expect("batch acknowledges")
    );
    let acknowledged = load_outbound_progress(&transaction, "peer-1")
        .expect("progress reads")
        .expect("progress row exists");
    assert_eq!(acknowledged.acknowledged_through, 3);
    assert!(acknowledged.in_flight.is_none());
    assert_eq!(
        acknowledged.last_acknowledged,
        Some(TaskMergeAcknowledgedBatch {
            batch_id: "batch-1".into(),
            through: 3,
        })
    );
    assert!(
        !claim_outbound_batch(&transaction, "peer-1", "batch-2", 0, 4)
            .expect("stale cursor cannot claim")
    );
    transaction
        .commit()
        .expect("actor-style transaction commits");
}
