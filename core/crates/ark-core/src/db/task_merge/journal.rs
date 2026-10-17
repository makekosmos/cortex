use super::{canonical_operation, verify_bootstrap_payload, TaskMergeError};
use crate::application::sync_storage::{
    TaskMergeJournalEntry, TaskMergeJournalPage, TaskMergeJournalRecord,
};
use crate::domain::task_merge::TaskMergeOperation;
use rusqlite::Connection;

const MAX_TASK_MERGE_PAGE_SIZE: usize = 100;

pub(crate) fn read_journal_page(
    connection: &Connection,
    after_position: i64,
    through_position: i64,
    limit: usize,
) -> Result<TaskMergeJournalPage, TaskMergeError> {
    if after_position < 0 || through_position < after_position {
        return Err(TaskMergeError::Invalid("task_merge_invalid_cursor".into()));
    }
    if limit == 0 || limit > MAX_TASK_MERGE_PAGE_SIZE {
        return Err(TaskMergeError::Invalid(
            "task_merge_page_limit_exceeded".into(),
        ));
    }
    let watermark: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(position), 0) FROM task_merge_journal",
            [],
            |row| row.get(0),
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    if through_position > watermark {
        return Err(TaskMergeError::Invalid(
            "task_merge_watermark_from_future".into(),
        ));
    }

    let mut statement = connection
        .prepare(
            "SELECT position, record_kind, record_id FROM task_merge_journal
             WHERE position > ?1 AND position <= ?2
             ORDER BY position LIMIT ?3",
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    let rows = statement
        .query_map(
            rusqlite::params![after_position, through_position, limit as i64],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    let entries = rows
        .map(|row| {
            let (position, record_kind, record_id) =
                row.map_err(|error| TaskMergeError::Storage(error.to_string()))?;
            let record = match record_kind.as_str() {
                "operation" => read_operation(connection, &record_id)?,
                "bootstrap" => read_bootstrap(connection, &record_id)?,
                _ => {
                    return Err(TaskMergeError::Invalid(
                        "task_merge_journal_record_kind_invalid".into(),
                    ));
                }
            };
            Ok(TaskMergeJournalEntry { position, record })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TaskMergeJournalPage {
        through_position,
        entries,
    })
}

fn read_operation(
    connection: &Connection,
    stored_id: &str,
) -> Result<TaskMergeJournalRecord, TaskMergeError> {
    let (json, stored_hash, expected_conflict_digest) = connection
        .query_row(
            "SELECT operation.operation_json, operation.payload_sha256,
                    guard.expected_conflict_digest
             FROM task_merge_operations AS operation
             LEFT JOIN task_merge_resolution_guards AS guard
                 ON guard.operation_id = operation.operation_id
             WHERE operation.operation_id = ?1",
            [stored_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    let operation = serde_json::from_str::<TaskMergeOperation>(&json)
        .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    let (operation, canonical_json, payload_hash) = canonical_operation(operation)?;
    if operation.id.as_str() != stored_id || canonical_json != json || payload_hash != stored_hash {
        return Err(TaskMergeError::Invalid(
            "task_merge_journal_payload_mismatch".into(),
        ));
    }
    Ok(TaskMergeJournalRecord::Operation {
        operation,
        expected_conflict_digest,
    })
}

fn read_bootstrap(
    connection: &Connection,
    stored_id: &str,
) -> Result<TaskMergeJournalRecord, TaskMergeError> {
    let (json, stored_hash) = connection
        .query_row(
            "SELECT bootstrap_json, payload_sha256 FROM task_merge_bootstraps
             WHERE bootstrap_id = ?1",
            [stored_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    let bootstrap =
        serde_json::from_str(&json).map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    let bootstrap = verify_bootstrap_payload(bootstrap, &json, &stored_hash)?;
    if bootstrap.id.as_str() != stored_id {
        return Err(TaskMergeError::Invalid(
            "task_merge_journal_payload_mismatch".into(),
        ));
    }
    Ok(TaskMergeJournalRecord::Bootstrap(bootstrap))
}
