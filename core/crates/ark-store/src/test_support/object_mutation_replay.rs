use std::path::Path;

use ark_domain::object_write::ObjectWriteSnapshot;
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMutationReplayState {
    pub snapshot: ObjectWriteSnapshot,
    pub upsert_receipts: i64,
    pub delete_receipts: i64,
    pub tombstone_id: Option<String>,
}

/// Reads persisted state for Core's replay and idempotency behavior test.
pub struct ObjectMutationReplayFixture;

impl ObjectMutationReplayFixture {
    pub fn inspect(
        path: &Path,
        object_id: &str,
        upsert_operation_id: &str,
        delete_operation_id: &str,
    ) -> Result<ObjectMutationReplayState, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "object mutation replay path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let snapshot = crate::object_write::read_object_write_snapshot(&connection, object_id)?;
        let upsert_receipts = receipt_count(&connection, upsert_operation_id)?;
        let delete_receipts = receipt_count(&connection, delete_operation_id)?;
        let tombstone_id = connection
            .query_row(
                "SELECT id FROM sync_tombstones WHERE id = ?1",
                [object_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        Ok(ObjectMutationReplayState {
            snapshot,
            upsert_receipts,
            delete_receipts,
            tombstone_id,
        })
    }
}

fn receipt_count(connection: &Connection, operation_id: &str) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM ark_operation_receipts
             WHERE principal = 'engine' AND operation_id = ?1",
            [operation_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}
