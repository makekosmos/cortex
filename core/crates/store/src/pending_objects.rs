use rusqlite::{params, Connection, OptionalExtension};

mod migrations;
mod workflow;

pub use migrations::{is_v3_schema, migrate_phase2_to_v3, validate_entity_tuple};
pub use workflow::{insert_pending_object, replay_pending_for_type, ReplayNotice};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingObjectSchema {
    Legacy,
    Versioned,
}

pub fn schema(connection: &Connection) -> Result<PendingObjectSchema, String> {
    if is_v3_schema(connection)? {
        Ok(PendingObjectSchema::Versioned)
    } else {
        Ok(PendingObjectSchema::Legacy)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingObjectRow {
    pub id: String,
    pub payload: String,
    pub awaited_type_id: String,
    pub awaited_type_version: String,
    pub received_at: String,
}

pub fn load_payload(
    connection: &Connection,
    schema: PendingObjectSchema,
    id: &str,
    awaited_type_id: &str,
    awaited_type_version: &str,
) -> Result<Option<String>, String> {
    let result = if schema == PendingObjectSchema::Versioned {
        connection.query_row(
            "SELECT payload FROM sync_pending_objects WHERE id=?1 AND awaited_type_id=?2
             AND awaited_type_version=?3",
            params![id, awaited_type_id, awaited_type_version],
            |row| row.get(0),
        )
    } else {
        connection.query_row(
            "SELECT payload FROM sync_pending_objects WHERE id=?1",
            [id],
            |row| row.get(0),
        )
    };
    result.optional().map_err(|error| error.to_string())
}

pub fn upsert(
    connection: &Connection,
    schema: PendingObjectSchema,
    row: &PendingObjectRow,
) -> Result<(), String> {
    let sql = if schema == PendingObjectSchema::Versioned {
        "INSERT INTO sync_pending_objects
         (id,payload,awaited_type_id,awaited_type_version,received_at)
         VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(id,awaited_type_id,awaited_type_version) DO UPDATE SET
         payload=excluded.payload,received_at=excluded.received_at"
    } else {
        "INSERT INTO sync_pending_objects
         (id,payload,awaited_type_id,awaited_type_version,received_at)
         VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,
         awaited_type_id=excluded.awaited_type_id,
         awaited_type_version=excluded.awaited_type_version,
         received_at=excluded.received_at"
    };
    connection
        .execute(
            sql,
            params![
                row.id,
                row.payload,
                row.awaited_type_id,
                row.awaited_type_version,
                row.received_at,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn table_exists(connection: &Connection) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table'
             AND name='sync_pending_objects')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

pub fn load_for_type(
    connection: &Connection,
    type_id: &str,
    type_version: &str,
) -> Result<Vec<PendingObjectRow>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id,payload,awaited_type_id,awaited_type_version,received_at
             FROM sync_pending_objects WHERE awaited_type_id=?1
             AND awaited_type_version=?2 ORDER BY id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![type_id, type_version], |row| {
            Ok(PendingObjectRow {
                id: row.get(0)?,
                payload: row.get(1)?,
                awaited_type_id: row.get(2)?,
                awaited_type_version: row.get(3)?,
                received_at: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(rows)
}

pub fn delete(
    connection: &Connection,
    id: &str,
    awaited_type_id: &str,
    awaited_type_version: &str,
) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM sync_pending_objects WHERE id=?1 AND awaited_type_id=?2
             AND awaited_type_version=?3",
            params![id, awaited_type_id, awaited_type_version],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn count_for_type(connection: &Connection, type_id: &str) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM sync_pending_objects WHERE awaited_type_id=?1",
            [type_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        count_for_type, delete, load_for_type, load_payload, schema, upsert, PendingObjectRow,
    };
    use rusqlite::Connection;

    #[test]
    fn versioned_pending_rows_round_trip_and_delete_by_exact_type_tuple() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE sync_pending_objects (
                    id TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    awaited_type_id TEXT NOT NULL,
                    awaited_type_version TEXT NOT NULL,
                    received_at TEXT NOT NULL,
                    PRIMARY KEY (id, awaited_type_id, awaited_type_version)
                )",
            )
            .expect("create versioned pending table");
        let first = PendingObjectRow {
            id: "object-1".into(),
            payload: "payload-v1".into(),
            awaited_type_id: "book".into(),
            awaited_type_version: "1.0.0".into(),
            received_at: "2026-10-19T00:00:00Z".into(),
        };
        let second = PendingObjectRow {
            awaited_type_version: "2.0.0".into(),
            payload: "payload-v2".into(),
            ..first.clone()
        };

        let schema = schema(&connection).expect("read pending schema");
        upsert(&connection, schema, &first).expect("insert first awaited version");
        upsert(&connection, schema, &second).expect("insert second awaited version");

        assert_eq!(
            load_payload(&connection, schema, "object-1", "book", "1.0.0").expect("load first"),
            Some("payload-v1".into())
        );
        assert_eq!(
            count_for_type(&connection, "book").expect("count book rows"),
            2
        );
        assert_eq!(
            load_for_type(&connection, "book", "2.0.0")
                .expect("load second version")
                .len(),
            1
        );

        delete(&connection, "object-1", "book", "1.0.0").expect("delete exact tuple");
        assert_eq!(
            count_for_type(&connection, "book").expect("count remaining rows"),
            1
        );
    }
}
