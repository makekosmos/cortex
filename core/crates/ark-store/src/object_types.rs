use ark_domain::types::ObjectType;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyCompatibilityVersion {
    pub version: String,
    pub schema_hash: String,
}

pub fn legacy_alias_conflicts(conn: &Connection, type_id: &str) -> Result<bool, String> {
    let has_aliases: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND \
             name='object_type_aliases')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| error.to_string())?
        != 0;
    if !has_aliases {
        return Ok(false);
    }
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM object_type_aliases WHERE alias=?1)",
        params![type_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(|error| error.to_string())
}

pub fn upsert_legacy_type(
    conn: &Connection,
    object_type: &ObjectType,
    compatibility: &LegacyCompatibilityVersion,
) -> Result<String, String> {
    conn.execute(
        "INSERT INTO object_types
            (id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            schema_json = excluded.schema_json,
            ui_schema_json = excluded.ui_schema_json,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at,
            system_locked = excluded.system_locked",
        params![
            object_type.id,
            object_type.name,
            object_type.schema_json,
            object_type.ui_schema_json,
            object_type.created_at,
            object_type.updated_at,
            object_type.system_locked as i64,
        ],
    )
    .map_err(|error| error.to_string())?;

    if table_exists(conn, "object_type_versions")? {
        crate::type_registry::ensure_legacy_type_version(
            conn,
            &object_type.id,
            &object_type.schema_json,
            &object_type.ui_schema_json,
            &object_type.created_at,
        )?;
        if let Some(existing_hash) = conn
            .query_row(
                "SELECT schema_hash FROM object_type_versions WHERE type_id=?1 AND version=?2",
                params![object_type.id, compatibility.version],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
        {
            if existing_hash != compatibility.schema_hash {
                return Err("legacy compatibility version hash conflict".into());
            }
        } else {
            conn.execute(
                concat!(
                    "INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,",
                    "content_contract_json,relations_json,sync_policy_json,schema_hash,",
                    "created_at) VALUES (?1,?2,?3,?4,'{}','[]','{}',?5,?6)"
                ),
                params![
                    object_type.id,
                    compatibility.version,
                    object_type.schema_json,
                    object_type.ui_schema_json,
                    compatibility.schema_hash,
                    object_type.created_at
                ],
            )
            .map_err(|error| error.to_string())?;
        }
        conn.execute(
            "UPDATE object_types SET current_version=?1,status='active' WHERE id=?2",
            params![compatibility.version, object_type.id],
        )
        .map_err(|error| error.to_string())?;
    }

    let has_current_version: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('object_types') WHERE \
             name='current_version')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| error.to_string())?
        != 0;
    if has_current_version {
        conn.query_row(
            "SELECT current_version FROM object_types WHERE id=?1",
            params![object_type.id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
    } else {
        Ok("0.0.0-legacy".into())
    }
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        params![table],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(|error| error.to_string())
}

pub fn deprecate_legacy_type(conn: &Connection, type_id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE object_types SET status='deprecated' WHERE id=?1",
        params![type_id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn is_type_known(conn: &Connection, type_id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM object_types WHERE id=?1)",
        params![type_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(|error| error.to_string())
}

pub fn is_definition_known(
    conn: &Connection,
    type_id: &str,
    type_version: &str,
) -> Result<bool, String> {
    let Ok((canonical_type_id, canonical_version)) =
        crate::type_registry::resolve_object_type_identity(conn, type_id, Some(type_version))
    else {
        return Ok(false);
    };
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM object_type_versions WHERE type_id=?1 AND version=?2)",
        params![canonical_type_id, canonical_version],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(|error| error.to_string())
}

pub fn list(conn: &Connection) -> Result<Vec<ObjectType>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked
             FROM object_types WHERE status != 'deprecated'
             ORDER BY system_locked DESC, name ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], map_type_row)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn get(conn: &Connection, type_id: &str) -> Result<Option<ObjectType>, String> {
    conn.query_row(
        "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked
         FROM object_types WHERE id=?1 AND status != 'deprecated'",
        params![type_id],
        map_type_row,
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn map_type_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ObjectType> {
    Ok(ObjectType {
        id: row.get(0)?,
        name: row.get(1)?,
        schema_json: row.get(2)?,
        ui_schema_json: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        system_locked: row.get::<_, i64>(6)? != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("base schema creates");
        crate::type_registry::migrate_schema(&connection).expect("type schema migrates");
        connection
    }

    fn object_type() -> ObjectType {
        ObjectType {
            id: "custom.note".into(),
            name: "Note".into(),
            schema_json: "{}".into(),
            ui_schema_json: "{}".into(),
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
            system_locked: false,
        }
    }

    #[test]
    fn object_type_store_upserts_compatibility_and_filters_deprecated_rows() {
        let connection = database();
        let object_type = object_type();
        let (version, schema_hash) =
            ark_domain::canonical_type::registry::legacy_compatibility_version(
                &object_type.schema_json,
                &object_type.ui_schema_json,
            )
            .expect("compatibility version derives");

        assert_eq!(
            upsert_legacy_type(
                &connection,
                &object_type,
                &LegacyCompatibilityVersion {
                    version: version.clone(),
                    schema_hash: schema_hash.clone(),
                }
            )
            .expect("legacy definition persists"),
            version
        );
        assert!(is_type_known(&connection, "custom.note").expect("type lookup succeeds"));
        assert!(is_definition_known(&connection, "custom.note", &version)
            .expect("version lookup succeeds"));
        let listed = list(&connection).expect("types load");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, object_type.id);
        assert_eq!(listed[0].schema_json, object_type.schema_json);
        let loaded = get(&connection, "custom.note")
            .expect("type lookup succeeds")
            .expect("type exists");
        assert_eq!(loaded.id, object_type.id);
        assert_eq!(loaded.name, object_type.name);

        deprecate_legacy_type(&connection, "custom.note").expect("type deprecates");
        assert!(list(&connection)
            .expect("types load after deprecation")
            .is_empty());
        assert!(get(&connection, "custom.note")
            .expect("type lookup succeeds")
            .is_none());
        assert!(is_type_known(&connection, "custom.note").expect("stored type still exists"));
    }
}
