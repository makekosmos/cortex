use ark_application::object_write::{NewObjectReceipt, ReceiptOutcome, StoredObjectReceipt};
use rusqlite::{params, Connection, OptionalExtension};

pub fn load_receipt(
    connection: &Connection,
    entity_id: &str,
    representation_kind: &str,
    source_hlc: &str,
) -> Result<Option<StoredObjectReceipt>, String> {
    connection
        .query_row(
            "SELECT change_seq, payload_sha256, payload_json, outcome, resulting_hlc
             FROM entity_write_receipts
             WHERE entity_kind = 'object' AND entity_id = ?1
               AND representation_kind = ?2 AND source_hlc = ?3",
            params![entity_id, representation_kind, source_hlc],
            |row| {
                let outcome: String = row.get(3)?;
                Ok(StoredObjectReceipt {
                    change_seq: row.get(0)?,
                    payload_sha256: row.get(1)?,
                    payload_json: row.get(2)?,
                    outcome: match outcome.as_str() {
                        "applied" => ReceiptOutcome::Applied,
                        "stale" => ReceiptOutcome::Stale,
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    },
                    resulting_hlc: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
}

pub fn insert_receipt(
    connection: &Connection,
    receipt: NewObjectReceipt,
) -> Result<StoredObjectReceipt, String> {
    let outcome = match receipt.outcome {
        ReceiptOutcome::Applied => "applied",
        ReceiptOutcome::Stale => "stale",
    };
    connection
        .execute(
            "INSERT INTO entity_write_receipts(
                entity_kind, entity_id, representation_kind, source_hlc,
                source_origin_device_id, type_id, payload_sha256, payload_json,
                outcome, resulting_hlc, authenticated_peer_id, committed_at
             ) VALUES ('object', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                       strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![
                receipt.entity_id,
                receipt.representation_kind,
                receipt.source_hlc,
                receipt.source_origin_device_id,
                receipt.type_id,
                receipt.payload_sha256,
                receipt.payload_json,
                outcome,
                receipt.resulting_hlc,
                receipt.authenticated_peer_id,
            ],
        )
        .map_err(|error| error.to_string())?;
    load_receipt(
        connection,
        &receipt.entity_id,
        &receipt.representation_kind,
        &receipt.source_hlc,
    )?
    .ok_or_else(|| "object_write_receipt_missing_after_insert".into())
}

#[cfg(test)]
mod tests {
    use super::{insert_receipt, load_receipt};
    use ark_application::object_write::{NewObjectReceipt, ReceiptOutcome};
    use rusqlite::Connection;

    #[test]
    fn receipt_round_trip_preserves_the_typed_outcome_and_payload() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(
                "CREATE TABLE entity_write_receipts (
                    change_seq INTEGER PRIMARY KEY AUTOINCREMENT,
                    entity_kind TEXT NOT NULL,
                    entity_id TEXT NOT NULL,
                    representation_kind TEXT NOT NULL,
                    source_hlc TEXT NOT NULL,
                    source_origin_device_id TEXT,
                    type_id TEXT,
                    payload_sha256 TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    outcome TEXT NOT NULL,
                    resulting_hlc TEXT,
                    authenticated_peer_id TEXT,
                    committed_at TEXT NOT NULL,
                    UNIQUE(entity_kind, entity_id, representation_kind, source_hlc)
                );",
            )
            .expect("receipt schema creates");
        let receipt = NewObjectReceipt {
            entity_id: "object-1".into(),
            representation_kind: "full".into(),
            source_hlc: "2026-10-20T00:00:00.000Z:000001:device-a".into(),
            source_origin_device_id: Some("device-a".into()),
            type_id: "note".into(),
            payload_sha256: "digest".into(),
            payload_json: "{\"title\":\"hello\"}".into(),
            outcome: ReceiptOutcome::Applied,
            resulting_hlc: Some("2026-10-20T00:00:00.000Z:000002:device-a".into()),
            authenticated_peer_id: Some("device-a".into()),
        };

        let stored = insert_receipt(&connection, receipt.clone()).expect("receipt persists");
        assert_eq!(stored.payload_sha256, receipt.payload_sha256);
        assert_eq!(stored.payload_json, receipt.payload_json);
        assert_eq!(stored.outcome, ReceiptOutcome::Applied);
        assert_eq!(
            load_receipt(
                &connection,
                &receipt.entity_id,
                &receipt.representation_kind,
                &receipt.source_hlc,
            )
            .expect("receipt loads"),
            Some(stored)
        );
    }
}
