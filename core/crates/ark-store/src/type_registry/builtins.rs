use rusqlite::{params, Connection};

use ark_domain::canonical_type::registry::{canonicalize_legacy_definition, LEGACY_VERSION};

pub fn ensure_builtin_versions(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare(concat!(
            "SELECT id,schema_json,ui_schema_json,created_at FROM object_types WHERE NOT ",
            "EXISTS (SELECT 1 FROM object_type_versions v WHERE ",
            "v.type_id=object_types.id AND v.version=?1)"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![LEGACY_VERSION], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    for (id, schema, ui, created) in rows {
        insert_legacy_type_version(conn, &id, &schema, &ui, &created)?;
    }
    Ok(())
}

fn insert_legacy_type_version(
    conn: &Connection,
    type_id: &str,
    schema: &str,
    ui_schema: &str,
    created_at: &str,
) -> Result<(), String> {
    let canonical = canonicalize_legacy_definition(schema, ui_schema)?;
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
    .map_err(|e| e.to_string())?;
    Ok(())
}
