use super::*;
use crate::domain::task_merge::{OperationId, TaskMergeOperation, TaskMergeUnit, TaskMergeValue};

#[path = "task_merge/journal.rs"]
mod journal;

fn id(suffix: char) -> OperationId {
    OperationId::parse(format!("{}{}", "0".repeat(31), suffix)).unwrap()
}

fn operation(operation_id: char, parents: &[char], title: &str) -> TaskMergeOperation {
    TaskMergeOperation {
        id: id(operation_id),
        object_id: "task-1".into(),
        unit: TaskMergeUnit::Title,
        parents: parents.iter().copied().map(id).collect(),
        value: TaskMergeValue::Field(serde_json::Value::String(title.into())),
    }
}

async fn open_merge_fixture() -> (tempfile::TempDir, String, DbHandle, DbOwner) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .join("task-merge.db")
        .to_string_lossy()
        .into_owned();
    let (handle, owner) = DbHandle::open(path.clone(), "merge-test-device".into())
        .await
        .unwrap();
    (directory, path, handle, owner)
}

async fn append(
    handle: &DbHandle,
    operation: TaskMergeOperation,
    expected_digest: Option<String>,
) -> Result<crate::db::task_merge::TaskMergeAppendResult, DbError> {
    handle
        .submit(
            CommandContext::engine(),
            AppendTaskMergeOperationCommand::new(operation, expected_digest),
        )
        .await
}

async fn read(handle: &DbHandle) -> crate::db::task_merge::TaskMergeReadResult {
    handle
        .query(
            CommandContext::engine(),
            TaskConflictSetQuery::new("task-1".into(), TaskMergeUnit::Title),
        )
        .await
        .unwrap()
}

async fn shutdown(owner: &mut DbOwner) {
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn operation_replay_conflict_guard_and_sqlite_reopen() {
    let (_directory, path, handle, mut owner) = open_merge_fixture().await;
    let first = append(&handle, operation('1', &[], "base"), None)
        .await
        .unwrap();
    let base_digest = first.conflict_set.unwrap().digest;
    let child = operation('2', &['1'], "edited");
    let committed = append(&handle, child.clone(), Some(base_digest.clone()))
        .await
        .unwrap();
    assert!(!committed.replayed);
    assert!(!committed.dependencies_pending);
    assert_eq!(
        committed.conflict_set.as_ref().unwrap().variants[0].value,
        TaskMergeValue::Field(serde_json::Value::String("edited".into()))
    );

    let replay = append(&handle, child.clone(), Some(base_digest))
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.conflict_set, committed.conflict_set);
    assert!(matches!(
        append(&handle, operation('2', &['1'], "different"), None).await,
        Err(DbError::Conflict(_))
    ));
    assert!(matches!(
        append(
            &handle,
            operation('3', &[], "stale resolution"),
            Some("stale".into())
        )
        .await,
        Err(DbError::Conflict(_))
    ));
    let rows: i64 = handle.with_connection(|connection| {
        connection
            .query_row("SELECT COUNT(*) FROM task_merge_operations", [], |row| {
                row.get(0)
            })
            .unwrap()
    });
    assert_eq!(rows, 2);
    let journal_rows: i64 = handle.with_connection(|connection| {
        connection
            .query_row("SELECT COUNT(*) FROM task_merge_journal", [], |row| {
                row.get(0)
            })
            .unwrap()
    });
    assert_eq!(journal_rows, 2);
    let journal: Vec<(i64, String)> = handle.with_connection(|connection| {
        let mut statement = connection
            .prepare("SELECT position, operation_id FROM task_merge_journal ORDER BY position")
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    });
    assert_eq!(
        journal,
        vec![
            (1, id('1').as_str().to_owned()),
            (2, id('2').as_str().to_owned())
        ]
    );
    shutdown(&mut owner).await;
    drop(handle);

    let (reopened, mut reopened_owner) = DbHandle::open(path, "merge-test-device".into())
        .await
        .unwrap();
    let after_reopen = read(&reopened).await;
    assert!(!after_reopen.dependencies_pending);
    assert_eq!(
        after_reopen.conflict_set.unwrap().variants[0].value,
        TaskMergeValue::Field(serde_json::Value::String("edited".into()))
    );
    let replay_after_reopen = append(&reopened, child, None).await.unwrap();
    assert!(replay_after_reopen.replayed);
    let journal_rows: i64 = reopened.with_connection(|connection| {
        connection
            .query_row("SELECT COUNT(*) FROM task_merge_journal", [], |row| {
                row.get(0)
            })
            .unwrap()
    });
    assert_eq!(journal_rows, 2);
    shutdown(&mut reopened_owner).await;
}

#[tokio::test]
async fn dangling_parents_stay_pending_until_dependencies_arrive() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('2', &['1'], "left"), None)
        .await
        .unwrap();
    append(&handle, operation('3', &['1'], "right"), None)
        .await
        .unwrap();
    let pending = read(&handle).await;
    assert!(pending.dependencies_pending);
    assert!(pending.conflict_set.is_none());

    append(&handle, operation('1', &[], "base"), None)
        .await
        .unwrap();
    let ready = read(&handle).await;
    assert!(!ready.dependencies_pending);
    let set = ready.conflict_set.unwrap();
    assert_eq!(set.common_ancestors.len(), 1);
    assert_eq!(set.common_ancestors[0].id, id('1'));
    assert_eq!(set.variants.len(), 2);
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn resolving_a_conflict_requires_the_current_digest() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('1', &[], "base"), None)
        .await
        .unwrap();
    append(&handle, operation('2', &['1'], "left"), None)
        .await
        .unwrap();
    append(&handle, operation('3', &['1'], "right"), None)
        .await
        .unwrap();
    let current = read(&handle).await.conflict_set.unwrap();
    assert!(current.has_conflict());
    let resolution = operation('4', &['2', '3'], "resolved");

    assert!(matches!(
        append(&handle, resolution.clone(), None).await,
        Err(DbError::Conflict(error)) if error == "task_merge_conflict_digest_required"
    ));
    assert!(matches!(
        append(&handle, resolution.clone(), Some("sha256:stale".into())).await,
        Err(DbError::Conflict(error)) if error == "task_merge_conflict_set_changed"
    ));
    let resolved = append(&handle, resolution, Some(current.digest))
        .await
        .unwrap();
    assert!(!resolved.dependencies_pending);
    assert!(!resolved.conflict_set.unwrap().has_conflict());
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn invalid_parent_shape_is_rejected_before_persisting_pending_operation() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    let result = append(&handle, operation('2', &['1', '1'], "invalid"), None).await;
    assert!(matches!(result, Err(DbError::Invalid(_))));
    let rows: i64 = handle.with_connection(|connection| {
        connection
            .query_row("SELECT COUNT(*) FROM task_merge_operations", [], |row| {
                row.get(0)
            })
            .unwrap()
    });
    assert_eq!(rows, 0);
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn failed_sqlite_insert_leaves_no_merge_operation() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch(
                "CREATE TRIGGER reject_task_merge_journal
                 BEFORE INSERT ON task_merge_journal
                 BEGIN
                     SELECT RAISE(ABORT, 'injected merge persistence failure');
                 END;",
            )
            .unwrap();
    });
    assert!(matches!(
        append(&handle, operation('1', &[], "value"), None).await,
        Err(DbError::Invalid(_)) | Err(DbError::Storage(_))
    ));
    let rows: i64 = handle.with_connection(|connection| {
        connection
            .query_row("SELECT COUNT(*) FROM task_merge_operations", [], |row| {
                row.get(0)
            })
            .unwrap()
    });
    assert_eq!(rows, 0);
    let journal_rows: i64 = handle.with_connection(|connection| {
        connection
            .query_row("SELECT COUNT(*) FROM task_merge_journal", [], |row| {
                row.get(0)
            })
            .unwrap()
    });
    assert_eq!(journal_rows, 0);
    shutdown(&mut owner).await;
}
