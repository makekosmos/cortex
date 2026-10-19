use super::{
    canonical_operation, check_operation, persist_operation, read_conflict_set, read_journal_page,
    TaskMergeOperationCheck, TaskMergeOperationOrigin, TaskMergeStoreError,
};
use ark_application::sync_storage::TaskMergeJournalRecord;
use ark_domain::task_merge::{
    OperationId, TaskMergeBootstrap, TaskMergeOperation, TaskMergeUnit, TaskMergeValue,
};
use ark_domain::types::{ArkObject, ObjectLink};
use rusqlite::Connection;
use serde_json::json;

fn connection() -> Connection {
    let connection = Connection::open_in_memory().expect("database opens");
    connection
        .execute_batch(
            "CREATE TABLE task_merge_operations (
                    operation_id TEXT PRIMARY KEY,
                    object_id TEXT NOT NULL,
                    unit TEXT NOT NULL,
                    operation_json TEXT NOT NULL,
                    payload_sha256 TEXT NOT NULL
                );
                CREATE TABLE task_merge_bootstraps (
                    bootstrap_id TEXT PRIMARY KEY,
                    object_id TEXT NOT NULL,
                    bootstrap_json TEXT NOT NULL,
                    payload_sha256 TEXT NOT NULL
                );
                CREATE TABLE task_merge_journal (
                    position INTEGER PRIMARY KEY AUTOINCREMENT,
                    record_kind TEXT NOT NULL,
                    record_id TEXT NOT NULL
                );
                CREATE TABLE task_merge_resolution_guards (
                    operation_id TEXT PRIMARY KEY,
                    expected_conflict_digest TEXT NOT NULL
                );
                CREATE TABLE task_merge_outbound_progress (
                    peer_device_id TEXT PRIMARY KEY,
                    policy_revision INTEGER NOT NULL,
                    policy_digest TEXT NOT NULL,
                    policy_active INTEGER NOT NULL,
                    acknowledged_through INTEGER NOT NULL DEFAULT 0,
                    in_flight_batch_id TEXT,
                    in_flight_after INTEGER,
                    in_flight_through INTEGER,
                    last_acknowledged_batch_id TEXT,
                    last_acknowledged_through INTEGER,
                    updated_at TEXT NOT NULL DEFAULT ''
                );
                CREATE TABLE task_merge_inbound_progress (
                    peer_device_id TEXT PRIMARY KEY,
                    policy_revision INTEGER NOT NULL,
                    policy_digest TEXT NOT NULL,
                    policy_active INTEGER NOT NULL,
                    received_through INTEGER NOT NULL DEFAULT 0,
                    last_batch_id TEXT,
                    last_batch_after INTEGER,
                    last_batch_through INTEGER,
                    last_batch_digest TEXT,
                    updated_at TEXT NOT NULL DEFAULT ''
                );
                CREATE TABLE objects (
                    id TEXT PRIMARY KEY,
                    type_id TEXT NOT NULL,
                    type_version TEXT NOT NULL,
                    title TEXT NOT NULL,
                    content_json TEXT NOT NULL,
                    props_json TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    deleted_at TEXT
                );
                CREATE TABLE object_links (
                    id TEXT PRIMARY KEY,
                    source_object_id TEXT NOT NULL,
                    target_object_id TEXT NOT NULL,
                    link_type TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE sync_kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                CREATE TABLE object_sync_versions (
                    object_id TEXT PRIMARY KEY,
                    hlc TEXT NOT NULL,
                    deleted INTEGER NOT NULL
                );
                CREATE TABLE sync_tombstones (
                    id TEXT NOT NULL,
                    entity_type TEXT NOT NULL,
                    hlc TEXT NOT NULL,
                    PRIMARY KEY (id, entity_type)
                );",
        )
        .expect("task merge fixture schema creates");
    connection
}

#[path = "tests_local_mutation.rs"]
mod local_mutation_tests;

fn operation() -> TaskMergeOperation {
    TaskMergeOperation {
        id: OperationId::parse("00000000000000000000000000000001").expect("valid id"),
        object_id: "task-1".into(),
        unit: TaskMergeUnit::Title,
        parents: Vec::new(),
        observed_heads: Vec::new(),
        hlc: None,
        value: TaskMergeValue::Field(json!("title")),
    }
}

fn bootstrap() -> TaskMergeBootstrap {
    TaskMergeBootstrap {
        id: OperationId::parse("10000000000000000000000000000000").expect("valid id"),
        object: ArkObject {
            id: "task-1".into(),
            type_id: "core.task".into(),
            type_version: "0.1.0".into(),
            title: "Task".into(),
            content_json: json!({"type":"doc","content":[]}),
            props_json: json!({
                "state":{"kind":"open","at":null},
                "planning":{"kind":"inbox","period":"any","date":null},
                "deadline":null,"reminders":[],"recurrence":null,
                "priority":"none","checklist":[]
            }),
            created_at: "2026-10-17T00:00:00Z".into(),
            updated_at: "2026-10-17T00:00:00Z".into(),
            deleted_at: None,
        },
        links: vec![ObjectLink {
            id: "link-1".into(),
            source_object_id: "task-1".into(),
            target_object_id: "project-1".into(),
            link_type: "task.project".into(),
            created_at: "2026-10-17T00:00:00Z".into(),
        }],
        source_hlc: "2026-10-17T00:00:00.000Z:000001:source-device".into(),
        baseline_ids: TaskMergeUnit::ALL
            .iter()
            .enumerate()
            .map(|(index, unit)| {
                (
                    *unit,
                    OperationId::parse(format!("{:032x}", index + 1)).expect("valid id"),
                )
            })
            .collect(),
    }
}

fn insert_operation(connection: &Connection, hash: &str) {
    let (operation, json, _) = canonical_operation(operation()).expect("operation canonicalizes");
    let unit = serde_json::to_string(&operation.unit).expect("unit serializes");
    connection
        .execute(
            "INSERT INTO task_merge_operations(
                    operation_id, object_id, unit, operation_json, payload_sha256
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![operation.id.as_str(), operation.object_id, unit, json, hash],
        )
        .expect("operation inserts");
    connection
        .execute(
            "INSERT INTO task_merge_journal(record_kind, record_id) VALUES ('operation', ?1)",
            [operation.id.as_str()],
        )
        .expect("journal entry inserts");
}

#[test]
fn journal_page_decodes_and_verifies_operation_payload() {
    let connection = connection();
    let (_, _, hash) = canonical_operation(operation()).expect("operation canonicalizes");
    insert_operation(&connection, &hash);

    let page = read_journal_page(&connection, 0, 1, 10).expect("journal page reads");
    assert_eq!(page.through_position, 1);
    assert_eq!(page.entries.len(), 1);
    assert!(matches!(
        &page.entries[0].record,
        TaskMergeJournalRecord::Operation { operation: stored, .. }
            if stored.id.as_str() == "00000000000000000000000000000001"
    ));
}

#[test]
fn journal_page_rejects_invalid_cursor_limit_and_future_watermark() {
    let connection = connection();
    let invalid_cursor =
        read_journal_page(&connection, -1, 0, 1).expect_err("negative cursor is rejected");
    assert!(
        matches!(invalid_cursor, TaskMergeStoreError::Invalid(ref error)
            if error == "task_merge_invalid_cursor")
    );

    let invalid_range =
        read_journal_page(&connection, 2, 1, 1).expect_err("reversed cursor range is rejected");
    assert!(
        matches!(invalid_range, TaskMergeStoreError::Invalid(ref error)
            if error == "task_merge_invalid_cursor")
    );

    let zero_limit = read_journal_page(&connection, 0, 0, 0).expect_err("zero limit is rejected");
    assert!(matches!(zero_limit, TaskMergeStoreError::Invalid(ref error)
            if error == "task_merge_page_limit_exceeded"));

    let excessive_limit =
        read_journal_page(&connection, 0, 0, 101).expect_err("excessive limit is rejected");
    assert!(
        matches!(excessive_limit, TaskMergeStoreError::Invalid(ref error)
            if error == "task_merge_page_limit_exceeded")
    );

    let future_watermark =
        read_journal_page(&connection, 0, 1, 1).expect_err("future watermark is rejected");
    assert!(
        matches!(future_watermark, TaskMergeStoreError::Invalid(ref error)
            if error == "task_merge_watermark_from_future")
    );
}

#[test]
fn journal_page_rejects_mismatched_operation_payload() {
    let connection = connection();
    insert_operation(&connection, "wrong-hash");

    let error =
        read_journal_page(&connection, 0, 1, 10).expect_err("payload hash mismatch is rejected");
    assert!(matches!(error, TaskMergeStoreError::Invalid(ref error)
            if error == "task_merge_journal_payload_mismatch"));
}

#[test]
fn conflict_read_decodes_persisted_operations_and_reports_missing_parents() {
    let connection = connection();
    let mut operation = operation();
    operation
        .parents
        .push(OperationId::parse("ffffffffffffffffffffffffffffffff").expect("valid parent id"));
    let (operation, json, hash) = canonical_operation(operation).expect("operation canonicalizes");
    let unit = serde_json::to_string(&operation.unit).expect("unit serializes");
    connection
        .execute(
            "INSERT INTO task_merge_operations(
                operation_id, object_id, unit, operation_json, payload_sha256
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![operation.id.as_str(), operation.object_id, unit, json, hash],
        )
        .expect("operation inserts");

    let read = read_conflict_set(&connection, "task-1", TaskMergeUnit::Title)
        .expect("persisted conflict read succeeds");
    assert!(read.conflict_set.is_none());
    assert!(read.dependencies_pending);
}

#[test]
fn conflict_read_returns_the_current_conflict_set_for_a_valid_history() {
    let connection = connection();
    let (_, _, hash) = canonical_operation(operation()).expect("operation canonicalizes");
    insert_operation(&connection, &hash);

    let read = read_conflict_set(&connection, "task-1", TaskMergeUnit::Title)
        .expect("persisted conflict read succeeds");
    assert!(read.conflict_set.is_some());
    assert!(!read.dependencies_pending);
}

#[test]
fn conflict_read_rejects_a_row_whose_id_differs_from_its_payload() {
    let connection = connection();
    let (operation, json, hash) =
        canonical_operation(operation()).expect("operation canonicalizes");
    let unit = serde_json::to_string(&operation.unit).expect("unit serializes");
    connection
        .execute(
            "INSERT INTO task_merge_operations(
                operation_id, object_id, unit, operation_json, payload_sha256
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "ffffffffffffffffffffffffffffffff",
                operation.object_id,
                unit,
                json,
                hash
            ],
        )
        .expect("mismatched operation row inserts");

    let error = read_conflict_set(&connection, "task-1", TaskMergeUnit::Title)
        .expect_err("stored identity mismatch is rejected");
    assert!(matches!(error, TaskMergeStoreError::Invalid(ref error)
        if error == "task_merge_operation_identity_mismatch"));
}

#[test]
fn append_persistence_is_idempotent_and_rejects_operation_id_collisions() {
    let connection = connection();
    let operation = operation();
    let (canonical, _, hash) = canonical_operation(operation.clone()).expect("canonicalizes");
    insert_operation(&connection, &hash);

    assert!(matches!(
        check_operation(
            &connection,
            &canonical,
            None,
            TaskMergeOperationOrigin::Local
        )
        .expect("matching replay reads"),
        TaskMergeOperationCheck::Replay(_)
    ));

    let mut conflicting = operation;
    conflicting.value = TaskMergeValue::Field(json!("different payload"));
    let error = check_operation(
        &connection,
        &conflicting,
        None,
        TaskMergeOperationOrigin::Local,
    )
    .expect_err("same operation id with another payload collides");
    assert!(matches!(error, TaskMergeStoreError::OperationIdCollision));
}

#[test]
fn append_persistence_keeps_resolution_guard_and_publication_policy() {
    let mut connection = connection();
    let transaction = connection
        .transaction()
        .expect("actor-style transaction begins");
    let operation = operation();
    let prepared = match check_operation(
        &transaction,
        &operation,
        Some("expected-conflict"),
        TaskMergeOperationOrigin::Remote,
    )
    .expect("new remote operation checks")
    {
        TaskMergeOperationCheck::New(prepared) => prepared,
        TaskMergeOperationCheck::Replay(_) => panic!("operation unexpectedly replays"),
    };
    persist_operation(&transaction, prepared).expect("remote operation persists");

    assert!(matches!(
        check_operation(
            &transaction,
            &operation,
            Some("expected-conflict"),
            TaskMergeOperationOrigin::Remote,
        )
        .expect("matching guarded replay reads"),
        TaskMergeOperationCheck::Replay(_)
    ));
    assert!(matches!(
        check_operation(
            &transaction,
            &operation,
            None,
            TaskMergeOperationOrigin::Remote,
        ),
        Err(TaskMergeStoreError::OperationIdCollision)
    ));

    let journal_count: i64 = transaction
        .query_row("SELECT COUNT(*) FROM task_merge_journal", [], |row| {
            row.get(0)
        })
        .expect("journal count reads");
    assert_eq!(
        journal_count, 0,
        "remote pre-bootstrap entries stay unpublished"
    );
    transaction
        .commit()
        .expect("actor-style transaction commits");
}

#[test]
fn append_persistence_rolls_back_operation_when_journal_insert_fails() {
    let mut connection = connection();
    connection
        .execute_batch(
            "CREATE TRIGGER reject_task_merge_journal BEFORE INSERT ON task_merge_journal
             BEGIN SELECT RAISE(ABORT, 'injected journal failure'); END;",
        )
        .expect("failure trigger creates");
    let transaction = connection
        .transaction()
        .expect("actor-style transaction begins");
    let prepared = match check_operation(
        &transaction,
        &operation(),
        Some("guard"),
        TaskMergeOperationOrigin::Local,
    )
    .expect("new local operation checks")
    {
        TaskMergeOperationCheck::New(prepared) => prepared,
        TaskMergeOperationCheck::Replay(_) => panic!("operation unexpectedly replays"),
    };
    let error = persist_operation(&transaction, prepared)
        .expect_err("journal insertion failure is returned");
    assert!(matches!(error, TaskMergeStoreError::Storage(_)));
    drop(transaction);

    let operation_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM task_merge_operations", [], |row| {
            row.get(0)
        })
        .expect("operation count reads after rollback");
    let guard_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM task_merge_resolution_guards",
            [],
            |row| row.get(0),
        )
        .expect("guard count reads after rollback");
    assert_eq!(operation_count, 0);
    assert_eq!(guard_count, 0);
}

#[path = "tests_bootstrap.rs"]
mod bootstrap_tests;
#[path = "tests_inbound.rs"]
mod inbound_tests;
#[path = "tests_progress.rs"]
mod progress_tests;
