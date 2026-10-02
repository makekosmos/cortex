pub fn upsert_object(conn: &Connection, object: &ArkObject) -> Result<(), String> {
    let registry_ready: bool = conn
        .query_row(
            concat!(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND ",
                "name='object_type_versions')"
            ),
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if registry_ready {
        let (canonical_type_id, canonical_version) =
            crate::type_registry::resolve_object_type_identity(
                conn,
                &object.type_id,
                Some(&object.type_version),
            )?;
        let mut canonical_object = object.clone();
        canonical_object.type_id = canonical_type_id;
        canonical_object.type_version = canonical_version;
        return upsert_object_inner(conn, &canonical_object);
    }
    upsert_object_inner(conn, object)
}

fn upsert_object_inner(conn: &Connection, object: &ArkObject) -> Result<(), String> {
    let has_type_version: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('objects') WHERE name='type_version')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if !has_type_version {
        conn.execute_batch(
            "ALTER TABLE objects ADD COLUMN type_version TEXT NOT NULL DEFAULT '0.0.0-legacy'",
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute_batch("SAVEPOINT ark_upsert_object")
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        // Do not use SQLite REPLACE here: it deletes the old row first and cascades.
        // РЎРј. postmortems.md В§ 2026-06-04.
        conn.execute(
            "INSERT INTO objects
                (id, type_id, type_version, title, content_json, props_json, created_at, \
                updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                type_id = excluded.type_id,
                type_version = excluded.type_version,
                title = excluded.title,
                content_json = excluded.content_json,
                props_json = excluded.props_json,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at",
            params![
                object.id,
                object.type_id,
                object.type_version,
                object.title,
                serialize_json(&object.content_json)?,
                serialize_json(&object.props_json)?,
                object.created_at,
                object.updated_at,
                object.deleted_at,
            ],
        )
        .map_err(|e| e.to_string())?;
        index_object_for_search(conn, object)
    })();

    if let Err(error) = result {
        rollback_savepoint(conn, "ark_upsert_object");
        return Err(error);
    }

    conn.execute_batch("RELEASE SAVEPOINT ark_upsert_object")
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_object(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT ark_delete_object")
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        conn.execute("DELETE FROM objects WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        remove_object_from_search(conn, id)
    })();

    if let Err(error) = result {
        rollback_savepoint(conn, "ark_delete_object");
        return Err(error);
    }

    conn.execute_batch("RELEASE SAVEPOINT ark_delete_object")
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn map_ark_object_row(row: &Row<'_>) -> rusqlite::Result<ArkObject> {
    Ok(ArkObject {
        id: row.get(0)?,
        type_id: row.get(1)?,
        type_version: row.get(2)?,
        title: row.get(3)?,
        content_json: parse_json_or_default(row.get(4)?),
        props_json: parse_json_or_default(row.get(5)?),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
    })
}

fn map_ark_object_summary_row(row: &Row<'_>) -> rusqlite::Result<ArkObjectSummary> {
    Ok(ArkObjectSummary {
        id: row.get(0)?,
        type_id: row.get(1)?,
        type_version: row.get(2)?,
        title: row.get(3)?,
        props_json: parse_json_or_default(row.get(4)?),
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        deleted_at: row.get(7)?,
    })
}

pub fn list_objects(conn: &Connection) -> Result<Vec<ArkObject>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, type_version, title, content_json, props_json, created_at,
             updated_at, deleted_at
             FROM objects
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_ark_object_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn list_object_summaries(conn: &Connection) -> Result<Vec<ArkObjectSummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, type_version, title, props_json, created_at, updated_at, deleted_at
             FROM objects
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_ark_object_summary_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
