use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;

pub fn payload_hash(command_type: &str, command: &impl Serialize) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    let payload = serde_json::to_value(("ark-operation-v1", command_type, command))
        .map_err(|error| error.to_string())?;
    let canonical_payload = serde_json::to_vec(&payload).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(canonical_payload)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationReceipt {
    pub command_type: String,
    pub payload_sha256: String,
    pub result_json: String,
}

pub fn load(
    transaction: &Transaction<'_>,
    principal: &str,
    operation_id: &str,
) -> rusqlite::Result<Option<OperationReceipt>> {
    transaction
        .query_row(
            "SELECT command_type, payload_sha256, result_json
             FROM ark_operation_receipts
             WHERE principal = ?1 AND operation_id = ?2",
            params![principal, operation_id],
            |row| {
                Ok(OperationReceipt {
                    command_type: row.get(0)?,
                    payload_sha256: row.get(1)?,
                    result_json: row.get(2)?,
                })
            },
        )
        .optional()
}

pub fn insert(
    transaction: &Transaction<'_>,
    principal: &str,
    operation_id: &str,
    receipt: &OperationReceipt,
) -> rusqlite::Result<()> {
    transaction.execute(
        "INSERT INTO ark_operation_receipts
         (principal, operation_id, command_type, payload_sha256, result_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            principal,
            operation_id,
            receipt.command_type,
            receipt.payload_sha256,
            receipt.result_json,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{insert, load, OperationReceipt};
    use rusqlite::Connection;

    #[test]
    fn receipt_round_trip_obeys_the_callers_transaction() {
        let mut connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("store schema initializes");
        let receipt = OperationReceipt {
            command_type: "object.upsert".into(),
            payload_sha256: "payload-hash".into(),
            result_json: "{\"id\":\"object-1\"}".into(),
        };

        let transaction = connection.transaction().expect("transaction begins");
        insert(&transaction, "engine:test", "operation-1", &receipt).expect("receipt inserts");
        assert_eq!(
            load(&transaction, "engine:test", "operation-1").expect("receipt loads"),
            Some(receipt.clone())
        );
        transaction.rollback().expect("transaction rolls back");

        let transaction = connection.transaction().expect("new transaction begins");
        assert_eq!(
            load(&transaction, "engine:test", "operation-1").expect("missing receipt loads"),
            None
        );
    }

    #[test]
    fn operation_id_is_scoped_by_principal() {
        let mut connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("store schema initializes");
        let engine_receipt = OperationReceipt {
            command_type: "object.upsert".into(),
            payload_sha256: "engine-payload".into(),
            result_json: "{\"id\":\"engine-object\"}".into(),
        };
        let package_receipt = OperationReceipt {
            command_type: "object.upsert".into(),
            payload_sha256: "package-payload".into(),
            result_json: "{\"id\":\"package-object\"}".into(),
        };

        let transaction = connection.transaction().expect("transaction begins");
        insert(&transaction, "engine", "shared-id", &engine_receipt)
            .expect("engine receipt inserts");
        insert(
            &transaction,
            "application-package:reader",
            "shared-id",
            &package_receipt,
        )
        .expect("package receipt has an independent key");
        assert_eq!(
            load(&transaction, "engine", "shared-id").expect("engine receipt loads"),
            Some(engine_receipt)
        );
        assert_eq!(
            load(&transaction, "application-package:reader", "shared-id")
                .expect("package receipt loads"),
            Some(package_receipt)
        );
        transaction.commit().expect("principal receipts commit");
    }

    #[test]
    fn receipt_insert_failure_rolls_back_the_callers_mutation() {
        let mut connection = Connection::open_in_memory().expect("database opens");
        crate::open::init_schema(&connection).expect("store schema initializes");
        connection
            .execute_batch(
                "INSERT INTO objects(id, type_id, title, created_at, updated_at)
                 VALUES ('note-1', 'com.kosmos.note', 'Before', 'now', 'now');",
            )
            .expect("object mutation fixture is created");
        connection
            .execute_batch(
                "CREATE TRIGGER reject_operation_receipt
                 BEFORE INSERT ON ark_operation_receipts
                 BEGIN SELECT RAISE(ABORT, 'injected receipt failure'); END;",
            )
            .expect("receipt failpoint installs");
        let snapshot = crate::object_write::read_object_write_snapshot(&connection, "note-1")
            .expect("object snapshot reads");
        let error = crate::object_mutation::apply_local_object_delete(
            &mut connection,
            ark_application::object_mutation::LocalObjectDelete {
                id: "note-1".into(),
                expected_snapshot: Some(snapshot),
                device_id: "device-1".into(),
            },
            "engine",
            Some("operation-1"),
        )
        .expect_err("receipt failure aborts the Store mutation");
        assert!(matches!(
            error,
            crate::writer::StoreTransactionFailure::Operation(
                ark_application::object_mutation::ObjectDeleteError::Storage(message)
            ) if message.contains("injected receipt failure")
        ));
        assert_eq!(
            crate::objects::get_object(&connection, "note-1")
                .expect("object reads after failed write")
                .expect("failed transaction preserves object")
                .title,
            "Before"
        );
        let transaction = connection.transaction().expect("verification begins");
        assert_eq!(
            load(&transaction, "engine", "operation-1").expect("receipt lookup succeeds"),
            None
        );
    }
}
