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
        for (column, definition) in [
            ("owner_kind", "TEXT NOT NULL DEFAULT 'system'"),
            ("owner_id", "TEXT"),
            ("current_version", "TEXT NOT NULL DEFAULT '0.0.0-legacy'"),
            ("status", "TEXT NOT NULL DEFAULT 'active'"),
            ("base_type_id", "TEXT"),
        ] {
            let exists = conn
                .query_row(
                    "SELECT 1 FROM pragma_table_info('object_types') WHERE name=?1",
                    params![column],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|error| error.to_string())?
                .is_some();
            if !exists {
                conn.execute_batch(&format!(
                    "ALTER TABLE object_types ADD COLUMN {column} {definition}"
                ))
                .map_err(|error| error.to_string())?;
            }
        }
        conn.execute_batch(concat!(
            "CREATE INDEX IF NOT EXISTS idx_object_type_versions_lookup ON ",
            "object_type_versions(type_id,version); CREATE INDEX IF NOT EXISTS ",
            "idx_object_type_aliases_canonical ON object_type_aliases(canonical_type_id);"
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
