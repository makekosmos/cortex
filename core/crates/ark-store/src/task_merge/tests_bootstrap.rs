use super::super::{
    apply_inbound_bootstrap, canonical_bootstrap, check_bootstrap, has_bootstrap, load_bootstrap,
    persist_bootstrap, verify_bootstrap_payload, TaskMergeBootstrapCheck, TaskMergeStoreError,
};
use super::{bootstrap, canonical_operation, connection, operation};
use ark_domain::task_merge::{OperationId, TaskMergeUnit};

#[test]
fn bootstrap_persistence_records_baseline_and_replays_idempotently() {
    let mut connection = connection();
    let transaction = connection
        .transaction()
        .expect("actor-style transaction begins");
    let bootstrap = bootstrap();
    let prepared = match check_bootstrap(&transaction, &bootstrap).expect("bootstrap checks") {
        TaskMergeBootstrapCheck::New(prepared) => prepared,
        TaskMergeBootstrapCheck::Replay => panic!("new bootstrap unexpectedly replays"),
    };
    let canonical = prepared.bootstrap().clone();
    persist_bootstrap(&transaction, prepared).expect("bootstrap persists");

    assert!(has_bootstrap(&transaction, "task-1").expect("bootstrap presence reads"));
    assert_eq!(
        load_bootstrap(&transaction, "task-1").expect("bootstrap loads"),
        canonical
    );
    let operation_count: i64 = transaction
        .query_row("SELECT COUNT(*) FROM task_merge_operations", [], |row| {
            row.get(0)
        })
        .expect("baseline operation count reads");
    assert_eq!(operation_count as usize, TaskMergeUnit::ALL.len());
    let journal_count: i64 = transaction
        .query_row("SELECT COUNT(*) FROM task_merge_journal", [], |row| {
            row.get(0)
        })
        .expect("journal count reads");
    assert_eq!(
        journal_count, 1,
        "baseline operations stay behind bootstrap"
    );

    assert!(matches!(
        check_bootstrap(&transaction, &canonical).expect("identical bootstrap checks"),
        TaskMergeBootstrapCheck::Replay
    ));
    transaction
        .commit()
        .expect("actor-style transaction commits");
}

#[test]
fn inbound_bootstrap_atomically_projects_object_links_and_remote_revision() {
    let mut connection = connection();
    let transaction = connection
        .transaction()
        .expect("actor-style transaction begins");
    let incoming = bootstrap();

    assert!(!apply_inbound_bootstrap(&transaction, &incoming).expect("bootstrap applies"));
    assert!(apply_inbound_bootstrap(&transaction, &incoming).expect("bootstrap replays"));

    assert_eq!(
        crate::objects::get_object(&transaction, "task-1").expect("object reads"),
        Some(incoming.object.clone())
    );
    assert_eq!(
        crate::objects::list_object_links(&transaction)
            .expect("links read")
            .into_iter()
            .filter(|link| link.source_object_id == "task-1")
            .collect::<Vec<_>>(),
        incoming.links
    );
    let revision: String = transaction
        .query_row(
            "SELECT hlc FROM object_sync_versions WHERE object_id = 'task-1'",
            [],
            |row| row.get(0),
        )
        .expect("remote object revision reads");
    assert_eq!(revision, incoming.source_hlc);
    let feed_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM ark_committed_feed WHERE entity_id = 'task-1'",
            [],
            |row| row.get(0),
        )
        .expect("feed count reads");
    assert_eq!(feed_count, 1);
    transaction
        .commit()
        .expect("actor-style transaction commits");
}

#[test]
fn inbound_bootstrap_rolls_back_projection_links_and_feed_when_revision_write_fails() {
    let mut connection = connection();
    connection
        .execute_batch(
            "CREATE TRIGGER reject_inbound_bootstrap_revision
             BEFORE INSERT ON object_sync_versions
             BEGIN SELECT RAISE(ABORT, 'injected revision failure'); END;",
        )
        .expect("failure trigger creates");
    let transaction = connection
        .transaction()
        .expect("actor-style transaction begins");

    assert!(matches!(
        apply_inbound_bootstrap(&transaction, &bootstrap()),
        Err(TaskMergeStoreError::Storage(_))
    ));
    drop(transaction);

    for table in [
        "objects",
        "object_links",
        "task_merge_bootstraps",
        "task_merge_operations",
        "task_merge_journal",
        "object_sync_versions",
        "ark_committed_feed",
        "sync_kv",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("rollback table count reads");
        assert_eq!(count, 0, "{table} should roll back with the bootstrap");
    }
}

#[test]
fn bootstrap_persistence_rejects_id_collision_and_journals_pending_operations() {
    let mut connection = connection();
    let mut pending = operation();
    pending.id = OperationId::parse("ffffffffffffffffffffffffffffffff").expect("valid id");
    let (pending, json, hash) = canonical_operation(pending).expect("operation canonicalizes");
    let unit = serde_json::to_string(&pending.unit).expect("unit serializes");
    connection
        .execute(
            "INSERT INTO task_merge_operations(
                operation_id, object_id, unit, operation_json, payload_sha256
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![pending.id.as_str(), pending.object_id, unit, json, hash],
        )
        .expect("pending operation inserts");

    let transaction = connection
        .transaction()
        .expect("actor-style transaction begins");
    let prepared = match check_bootstrap(&transaction, &bootstrap()).expect("bootstrap checks") {
        TaskMergeBootstrapCheck::New(prepared) => prepared,
        TaskMergeBootstrapCheck::Replay => panic!("new bootstrap unexpectedly replays"),
    };
    persist_bootstrap(&transaction, prepared).expect("bootstrap persists");
    let pending_journaled: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM task_merge_journal
             WHERE record_kind = 'operation' AND record_id = ?1)",
            [pending.id.as_str()],
            |row| row.get(0),
        )
        .expect("pending journal state reads");
    assert!(pending_journaled);

    let mut conflicting = bootstrap();
    conflicting.object.title = "another payload".into();
    assert!(matches!(
        check_bootstrap(&transaction, &conflicting),
        Err(TaskMergeStoreError::OperationIdCollision)
    ));
    transaction
        .commit()
        .expect("actor-style transaction commits");
}

#[test]
fn bootstrap_persistence_rolls_back_baseline_when_journal_insert_fails() {
    let mut connection = connection();
    connection
        .execute_batch(
            "CREATE TRIGGER reject_bootstrap_journal BEFORE INSERT ON task_merge_journal
             WHEN NEW.record_kind = 'bootstrap'
             BEGIN SELECT RAISE(ABORT, 'injected bootstrap journal failure'); END;",
        )
        .expect("failure trigger creates");
    let transaction = connection
        .transaction()
        .expect("actor-style transaction begins");
    let prepared = match check_bootstrap(&transaction, &bootstrap()).expect("bootstrap checks") {
        TaskMergeBootstrapCheck::New(prepared) => prepared,
        TaskMergeBootstrapCheck::Replay => panic!("new bootstrap unexpectedly replays"),
    };
    assert!(matches!(
        persist_bootstrap(&transaction, prepared),
        Err(TaskMergeStoreError::Storage(_))
    ));
    drop(transaction);

    let bootstrap_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM task_merge_bootstraps", [], |row| {
            row.get(0)
        })
        .expect("bootstrap count reads after rollback");
    let operation_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM task_merge_operations", [], |row| {
            row.get(0)
        })
        .expect("baseline operation count reads after rollback");
    assert_eq!(bootstrap_count, 0);
    assert_eq!(operation_count, 0);
}

#[test]
fn bootstrap_payload_helpers_preserve_integrity_checks() {
    let (bootstrap, json, digest) =
        canonical_bootstrap(bootstrap()).expect("bootstrap canonicalizes");
    assert_eq!(
        verify_bootstrap_payload(bootstrap.clone(), &json, &digest)
            .expect("valid bootstrap verifies"),
        bootstrap
    );
    let error = verify_bootstrap_payload(bootstrap, &json, "wrong-digest")
        .expect_err("bootstrap payload mismatch is rejected");
    assert!(matches!(error, TaskMergeStoreError::Invalid(ref error)
            if error == "task_merge_bootstrap_payload_mismatch"));
}
