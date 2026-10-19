use ark_domain::canonical_type::registry::{canonicalize_legacy_definition, LEGACY_VERSION};
use ark_domain::canonical_type::TypeVersion;
use rusqlite::{params, Connection, OptionalExtension};

pub fn migrate_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT ark_store_type_registry_schema")
        .map_err(|error| error.to_string())?;
    let result = (|| {
        conn.execute_batch(concat!(
            "CREATE TABLE IF NOT EXISTS object_type_versions (type_id TEXT NOT NULL, ",
            "version TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL ",
            "DEFAULT '{}', content_contract_json TEXT NOT NULL DEFAULT '{}', relations_json ",
            "TEXT NOT NULL DEFAULT '[]', sync_policy_json TEXT NOT NULL DEFAULT '{}', ",
            "schema_hash TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(type_id,version), ",
            "FOREIGN KEY(type_id) REFERENCES object_types(id)); CREATE TABLE IF NOT EXISTS ",
            "object_type_aliases (alias TEXT PRIMARY KEY, canonical_type_id TEXT NOT NULL, ",
            "created_at TEXT NOT NULL, FOREIGN KEY(canonical_type_id) REFERENCES object_types(id));"
        ))
        .map_err(|error| error.to_string())?;
        conn.execute_batch(concat!(
            "CREATE TABLE IF NOT EXISTS sync_pending_objects (id TEXT PRIMARY KEY, ",
            "payload TEXT NOT NULL, awaited_type_id TEXT NOT NULL, received_at TEXT NOT ",
            "NULL); CREATE INDEX IF NOT EXISTS idx_sync_pending_awaited_type ON ",
            "sync_pending_objects(awaited_type_id);"
        ))
        .map_err(|error| error.to_string())?;
        for (column, definition) in [
            ("owner_kind", "TEXT NOT NULL DEFAULT 'system'"),
            ("owner_id", "TEXT"),
            ("current_version", "TEXT NOT NULL DEFAULT '0.0.0-legacy'"),
            ("status", "TEXT NOT NULL DEFAULT 'active'"),
            ("base_type_id", "TEXT"),
        ] {
            ensure_column(conn, "object_types", column, definition)?;
        }
        ensure_column(
            conn,
            "objects",
            "type_version",
            "TEXT NOT NULL DEFAULT '0.0.0-legacy'",
        )?;
        ensure_column(
            conn,
            "sync_pending_objects",
            "awaited_type_version",
            "TEXT NOT NULL DEFAULT '0.0.0-legacy'",
        )?;
        conn.execute_batch(concat!(
            "CREATE INDEX IF NOT EXISTS idx_object_type_versions_lookup ON ",
            "object_type_versions(type_id,version); CREATE INDEX IF NOT EXISTS ",
            "idx_object_type_aliases_canonical ON object_type_aliases(canonical_type_id); ",
            "CREATE INDEX IF NOT EXISTS idx_objects_type_version ON objects(type_id,",
            "type_version); ",
            "CREATE INDEX IF NOT EXISTS idx_sync_pending_awaited_type_version ON ",
            "sync_pending_objects(awaited_type_id,awaited_type_version);"
        ))
        .map_err(|error| error.to_string())?;
        migrate_legacy_versions(conn)?;
        Ok::<(), String>(())
    })();

    match result {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT ark_store_type_registry_schema")
            .map_err(|error| error.to_string()),
        Err(error) => {
            let _ = conn.execute_batch(concat!(
                "ROLLBACK TO SAVEPOINT ark_store_type_registry_schema; RELEASE SAVEPOINT ",
                "ark_store_type_registry_schema"
            ));
            Err(error)
        }
    }
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), String> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info(?1) WHERE name=?2",
            params![table, column],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .is_some();
    if !exists {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn migrate_legacy_versions(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id,schema_json,ui_schema_json,created_at,current_version FROM object_types",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(stmt);

    for (type_id, schema, ui_schema, created_at, current_version) in rows {
        if current_version != LEGACY_VERSION {
            continue;
        }
        let version = TypeVersion {
            type_id: type_id.clone(),
            version: LEGACY_VERSION.into(),
            schema_json: schema,
            ui_schema_json: ui_schema,
            content_contract_json: "{}".into(),
            relations_json: "[]".into(),
            sync_policy_json: "{}".into(),
            schema_hash: String::new(),
            created_at: created_at.clone(),
        };
        let canonical =
            canonicalize_legacy_definition(&version.schema_json, &version.ui_schema_json)?;
        conn.execute(
            concat!(
                "INSERT OR IGNORE INTO object_type_versions(type_id,version,schema_json,",
                "ui_schema_json,content_contract_json,relations_json,sync_policy_json,",
                "schema_hash,created_at) VALUES (?1,?2,?3,?4,'{}','[]','{}',?5,?6)"
            ),
            params![
                type_id,
                LEGACY_VERSION,
                canonical.schema_json,
                canonical.ui_schema_json,
                canonical.schema_hash,
                created_at
            ],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::migrate_schema;
    use rusqlite::Connection;

    #[test]
    fn pending_object_schema_migration_is_idempotent_and_supports_versioned_rows() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(concat!(
                "CREATE TABLE object_types (id TEXT PRIMARY KEY, schema_json TEXT NOT NULL, ",
                "ui_schema_json TEXT NOT NULL, created_at TEXT NOT NULL, current_version TEXT ",
                "NOT NULL); CREATE TABLE objects (type_id TEXT NOT NULL);"
            ))
            .expect("legacy type table is present");

        migrate_schema(&connection).expect("first migration succeeds");
        migrate_schema(&connection).expect("repeated migration succeeds");
        connection
            .execute(
                "INSERT INTO sync_pending_objects \
                 (id,payload,awaited_type_id,received_at,awaited_type_version) \
                 VALUES (?1,?2,?3,?4,?5)",
                ("pending", "{}", "note", "now", "1.0.0"),
            )
            .expect("versioned pending row is accepted");
        connection
            .execute("INSERT INTO objects(type_id) VALUES ('note')", [])
            .expect("legacy object row is accepted");

        let version: String = connection
            .query_row(
                "SELECT awaited_type_version FROM sync_pending_objects WHERE id='pending'",
                [],
                |row| row.get(0),
            )
            .expect("pending version reads");
        assert_eq!(version, "1.0.0");
        let object_version: String = connection
            .query_row(
                "SELECT type_version FROM objects WHERE type_id='note'",
                [],
                |row| row.get(0),
            )
            .expect("legacy object type version reads");
        assert_eq!(object_version, "0.0.0-legacy");
    }
}
