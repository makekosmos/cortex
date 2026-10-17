use super::{canonical_operation, TaskMergeError};
use crate::application::sync_storage::{TaskMergeJournalEntry, TaskMergeJournalPage};
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
            "SELECT journal.position, operation.operation_json,
                    operation.operation_id, operation.payload_sha256,
                    guard.expected_conflict_digest
             FROM task_merge_journal AS journal
             JOIN task_merge_operations AS operation USING(operation_id)
             LEFT JOIN task_merge_resolution_guards AS guard USING(operation_id)
             WHERE journal.position > ?1 AND journal.position <= ?2
             ORDER BY journal.position LIMIT ?3",
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
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    let entries = rows
        .map(|row| {
            let (position, json, stored_id, stored_hash, expected_conflict_digest) =
                row.map_err(|error| TaskMergeError::Storage(error.to_string()))?;
            let operation = serde_json::from_str::<TaskMergeOperation>(&json)
                .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
            let (operation, canonical_json, payload_hash) = canonical_operation(operation)?;
            if operation.id.as_str() != stored_id
                || canonical_json != json
                || payload_hash != stored_hash
            {
                return Err(TaskMergeError::Invalid(
                    "task_merge_journal_payload_mismatch".into(),
                ));
            }
            Ok(TaskMergeJournalEntry {
                position,
                operation,
                expected_conflict_digest,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TaskMergeJournalPage {
        through_position,
        entries,
    })
}
