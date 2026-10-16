use super::*;
use rusqlite::Connection;
use std::time::Duration;

fn actor_path(directory: &tempfile::TempDir) -> String {
    directory
        .path()
        .join("actor.db")
        .to_string_lossy()
        .into_owned()
}

fn engine() -> CommandContext {
    CommandContext::engine()
}

async fn open_failure_actor() -> (tempfile::TempDir, DbHandle, DbOwner) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, owner) = DbHandle::open(actor_path(&directory), "actor-test-device".into())
        .await
        .unwrap();
    (directory, handle, owner)
}

fn failure_probe_state(handle: &DbHandle, operation_id: &str) -> (i64, i64) {
    handle.with_connection(|connection| {
        let mutations = connection
            .query_row(
                "SELECT COUNT(*) FROM production_actor_failure_probe",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let receipts = connection
            .query_row(
                "SELECT COUNT(*) FROM ark_operation_receipts
                 WHERE operation_id = ?1",
                [operation_id],
                |row| row.get(0),
            )
            .unwrap();
        (mutations, receipts)
    })
}

async fn apply_failure_mutation(
    handle: &DbHandle,
    operation_id: &str,
    payload_bytes: i64,
) -> Result<super::super::super::OperationResult<String>, DbError> {
    handle
        .submit_with_operation_id(
            engine(),
            operation_id.to_owned(),
            "production_actor.failure_probe.v1",
            FailureMutation { payload_bytes },
        )
        .await
}

#[tokio::test]
async fn production_actor_sqlite_busy_rolls_back_and_accepts_a_retry() {
    let (directory, handle, mut owner) = open_failure_actor().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch("CREATE TABLE production_actor_failure_probe(payload BLOB NOT NULL);")
            .unwrap();
    });
    let competing_writer = Connection::open(actor_path(&directory)).unwrap();
    competing_writer.execute_batch("BEGIN IMMEDIATE;").unwrap();

    let failed = apply_failure_mutation(&handle, "busy-operation", 0).await;
    let is_busy = matches!(
        failed,
        Err(DbError::Storage(ref message)) if message.contains("database is locked")
    );
    assert!(
        is_busy,
        "competing writer should make the actor return SQLITE_BUSY, got {failed:?}"
    );
    assert_eq!(failure_probe_state(&handle, "busy-operation"), (0, 0));

    competing_writer.execute_batch("ROLLBACK;").unwrap();
    let retry = apply_failure_mutation(&handle, "busy-operation", 0)
        .await
        .unwrap();
    assert_eq!(retry.value, "inserted:0");
    assert!(!retry.replayed);
    assert_eq!(failure_probe_state(&handle, "busy-operation"), (1, 1));

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_sqlite_full_rolls_back_mutation_and_receipt_then_recovers() {
    let (_directory, handle, mut owner) = open_failure_actor().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch("CREATE TABLE production_actor_failure_probe(payload BLOB NOT NULL);")
            .unwrap();
    });
    let page_count = handle.with_connection(|connection| {
        connection
            .query_row("PRAGMA page_count", [], |row| row.get::<_, u32>(0))
            .unwrap()
    });
    let limited_page_count = page_count + 1;
    assert_eq!(
        handle
            .submit_maintenance(
                CommandContext::engine_maintenance(),
                SetMaxPageCount {
                    page_count: limited_page_count,
                },
            )
            .await
            .unwrap(),
        limited_page_count
    );

    let failed = apply_failure_mutation(&handle, "full-operation", 1_048_576).await;
    let is_full = matches!(
        failed,
        Err(DbError::Storage(ref message))
            if message.contains("database or disk is full")
    );
    assert!(
        is_full,
        "page limit should make the actor return SQLITE_FULL, got {failed:?}"
    );
    assert_eq!(failure_probe_state(&handle, "full-operation"), (0, 0));

    let restored_page_count = page_count + 1024;
    assert_eq!(
        handle
            .submit_maintenance(
                CommandContext::engine_maintenance(),
                SetMaxPageCount {
                    page_count: restored_page_count,
                },
            )
            .await
            .unwrap(),
        restored_page_count
    );
    let retry = apply_failure_mutation(&handle, "full-operation", 1_048_576)
        .await
        .unwrap();
    assert_eq!(retry.value, "inserted:1048576");
    assert!(!retry.replayed);
    assert_eq!(failure_probe_state(&handle, "full-operation"), (1, 1));

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}
