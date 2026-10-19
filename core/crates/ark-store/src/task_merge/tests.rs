use super::{
    canonical_bootstrap, canonical_operation, read_journal_page, verify_bootstrap_payload,
    TaskMergeStoreError,
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
                );",
        )
        .expect("task merge fixture schema creates");
    connection
}

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
    connection
        .execute(
            "INSERT INTO task_merge_operations(
                    operation_id, object_id, unit, operation_json, payload_sha256
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                operation.id.as_str(),
                operation.object_id,
                "title",
                json,
                hash
            ],
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
