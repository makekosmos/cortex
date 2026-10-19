use rusqlite::{params, Connection, OptionalExtension};

pub fn get_object_local_state_json(
    conn: &Connection,
    object_id: &str,
    device_id: &str,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT data_json FROM object_local_state WHERE object_id = ?1 AND device_id = ?2",
        params![object_id, device_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub fn upsert_object_local_state_json(
    conn: &Connection,
    object_id: &str,
    device_id: &str,
    data_json: &str,
    updated_at: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO object_local_state(object_id, device_id, data_json, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(object_id, device_id) DO UPDATE SET
            data_json = excluded.data_json, updated_at = excluded.updated_at",
        params![object_id, device_id, data_json, updated_at],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn get_migration_quarantine_fields_json(
    conn: &Connection,
    object_id: &str,
    contract_version: &str,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT fields_json FROM object_migration_quarantine
         WHERE object_id = ?1 AND contract_version = ?2",
        params![object_id, contract_version],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub fn delete_migration_quarantine(
    conn: &Connection,
    object_id: &str,
    contract_version: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM object_migration_quarantine
         WHERE object_id = ?1 AND contract_version = ?2",
        params![object_id, contract_version],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn upsert_migration_quarantine(
    conn: &Connection,
    object_id: &str,
    contract_version: &str,
    source_type_id: &str,
    fields_json: &str,
    source_hash: &str,
    updated_at: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO object_migration_quarantine(
            object_id, contract_version, source_type_id, fields_json, source_hash, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(object_id, contract_version) DO UPDATE SET
            fields_json = excluded.fields_json, source_hash = excluded.source_hash,
            updated_at = excluded.updated_at",
        params![
            object_id,
            contract_version,
            source_type_id,
            fields_json,
            source_hash,
            updated_at
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_state_and_quarantine_reads_use_exact_keys() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(
                "CREATE TABLE object_local_state (
                    object_id TEXT NOT NULL, device_id TEXT NOT NULL,
                    data_json TEXT NOT NULL, updated_at TEXT NOT NULL,
                    PRIMARY KEY(object_id, device_id)
                 );
                 CREATE TABLE object_migration_quarantine (
                    object_id TEXT NOT NULL, contract_version TEXT NOT NULL,
                    source_type_id TEXT NOT NULL, fields_json TEXT NOT NULL,
                    source_hash TEXT NOT NULL, updated_at TEXT NOT NULL,
                    PRIMARY KEY(object_id, contract_version)
                 );
                 INSERT INTO object_local_state(object_id, device_id, data_json, updated_at)
                 VALUES ('first', 'phone', '{\"value\":\"phone\"}', '2026-01-01'),
                        ('first', 'laptop', '{\"value\":\"laptop\"}', '2026-01-01');
                 INSERT INTO object_migration_quarantine
                    (object_id, contract_version, source_type_id, fields_json,
                     source_hash, updated_at)
                 VALUES ('first', 'v1', 'note', '{\"fields\":[\"v1\"]}', 'v1-hash', '2026-01-01'),
                        ('first', 'v2', 'note', '{\"fields\":[\"v2\"]}', 'v2-hash', '2026-01-01');",
            )
            .expect("local and quarantine rows insert");

        assert_eq!(
            get_object_local_state_json(&connection, "first", "phone").expect("phone loads"),
            Some(r#"{"value":"phone"}"#.into())
        );
        assert_eq!(
            get_object_local_state_json(&connection, "first", "missing").expect("missing loads"),
            None
        );
        upsert_object_local_state_json(
            &connection,
            "first",
            "phone",
            r#"{"value":"updated"}"#,
            "2026-02-01",
        )
        .expect("local state updates");
        assert_eq!(
            get_object_local_state_json(&connection, "first", "phone")
                .expect("updated state loads"),
            Some(r#"{"value":"updated"}"#.into())
        );
        assert_eq!(
            get_migration_quarantine_fields_json(&connection, "first", "v2")
                .expect("matching quarantine loads"),
            Some(r#"{"fields":["v2"]}"#.into())
        );
        assert_eq!(
            get_migration_quarantine_fields_json(&connection, "first", "v3")
                .expect("missing quarantine loads"),
            None
        );
        upsert_migration_quarantine(
            &connection,
            "first",
            "v2",
            "note",
            r#"{"fields":["updated"]}"#,
            "updated-hash",
            "2026-02-01",
        )
        .expect("quarantine updates");
        assert_eq!(
            get_migration_quarantine_fields_json(&connection, "first", "v2")
                .expect("updated quarantine loads"),
            Some(r#"{"fields":["updated"]}"#.into())
        );
        delete_migration_quarantine(&connection, "first", "v2").expect("quarantine deletes");
        assert_eq!(
            get_migration_quarantine_fields_json(&connection, "first", "v2")
                .expect("deleted quarantine loads"),
            None
        );
    }
}
