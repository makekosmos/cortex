use std::path::Path;

use rusqlite::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalTaskIdempotencyState {
    pub entity_write_receipts: i64,
    pub operation_receipts: i64,
}

/// Inspects persistence receipts for Core's canonical task idempotency test.
pub struct CanonicalTaskIdempotencyFixture;

impl CanonicalTaskIdempotencyFixture {
    pub fn inspect(
        path: &Path,
        object_id: &str,
        operation_id: &str,
    ) -> Result<CanonicalTaskIdempotencyState, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "canonical task idempotency path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let entity_write_receipts = connection
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts
                 WHERE entity_kind = 'object' AND entity_id = ?1
                   AND representation_kind = 'full' AND outcome = 'applied'",
                [object_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let operation_receipts = operation_receipt_count(&connection, operation_id)?;
        Ok(CanonicalTaskIdempotencyState {
            entity_write_receipts,
            operation_receipts,
        })
    }
}

fn operation_receipt_count(connection: &Connection, operation_id: &str) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM ark_operation_receipts
             WHERE principal = 'engine' AND operation_id = ?1",
            [operation_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}
