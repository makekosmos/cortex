// Generic object model CRUD
// ---------------------------------------------------------------------------

fn serialize_json(value: &Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| e.to_string())
}

fn parse_json_or_default(raw: String) -> Value {
    serde_json::from_str(&raw).unwrap_or_else(|_| json!({}))
}

pub fn upsert_object_type(conn: &Connection, object_type: &ObjectType) -> Result<(), String> {
    // A canonical ID cannot reuse an existing alias name. Check this before
    // mutating object_types so the legacy write remains fail-closed and atomic.
    let has_aliases: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_aliases')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if has_aliases
        && conn
            .query_row(
                "SELECT 1 FROM object_type_aliases WHERE alias=?1 LIMIT 1",
                params![object_type.id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .is_some()
    {
        return Err("alias collides with canonical type".into());
    }

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
    .map_err(|e| e.to_string())?;
    let has_versions: bool = conn
        .query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_versions')", [], |row| row.get::<_, i64>(0))
        .map_err(|e| e.to_string())? != 0;
    if has_versions {
        type_registry::ensure_legacy_type_version(
            conn,
            &object_type.id,
            &object_type.schema_json,
            &object_type.ui_schema_json,
            &object_type.created_at,
        )?;
        let (compat_version, full_hash) = type_registry::legacy_compatibility_version(
            &object_type.schema_json,
            &object_type.ui_schema_json,
        )?;
        let existing_hash = conn
            .query_row(
                "SELECT schema_hash FROM object_type_versions WHERE type_id=?1 AND version=?2",
                params![object_type.id, compat_version],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(existing_hash) = existing_hash {
            if existing_hash != full_hash {
                return Err("legacy compatibility version hash conflict".into());
            }
        } else {
            conn.execute("INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES (?1,?2,?3,?4,'{}','[]','{}',?5,?6)", params![object_type.id, compat_version, object_type.schema_json, object_type.ui_schema_json, full_hash, object_type.created_at]).map_err(|e| e.to_string())?;
        }
        conn.execute(
            "UPDATE object_types SET current_version=?1,status='active' WHERE id=?2",
            params![compat_version, object_type.id],
        )
        .map_err(|e| e.to_string())?;
    }
    let has_current_version: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('object_types') WHERE name='current_version')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    let current_version = if has_current_version {
        conn.query_row(
            "SELECT current_version FROM object_types WHERE id=?1",
            params![object_type.id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?
    } else {
        "0.0.0-legacy".to_string()
    };
    replay_pending_for_type(conn, &object_type.id, &current_version)?;
    if current_version != type_registry::LEGACY_VERSION {
        replay_pending_for_type(conn, &object_type.id, type_registry::LEGACY_VERSION)?;
    }
    Ok(())
}

pub fn delete_object_type(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE object_types SET status='deprecated' WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase 2: schema drift hold-and-replay (sync_pending_objects)
// ---------------------------------------------------------------------------

/// РџСЂРѕРІРµСЂРёС‚СЊ, СЃСѓС‰РµСЃС‚РІСѓРµС‚ Р»Рё `object_type` СЃ СѓРєР°Р·Р°РЅРЅС‹Рј id.
pub fn is_object_type_known(conn: &Connection, type_id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM object_types WHERE id = ?1",
        params![type_id],
        |_| Ok(true),
    )
    .optional()
    .map(|opt| opt.unwrap_or(false))
    .map_err(|e| e.to_string())
}

pub fn is_object_definition_known(
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
        "SELECT 1 FROM object_type_versions WHERE type_id=?1 AND version=?2",
        params![canonical_type_id, canonical_version],
        |_| Ok(true),
    )
    .optional()
    .map(|opt| opt.unwrap_or(false))
    .map_err(|e| e.to_string())
}

pub fn list_object_types(conn: &Connection) -> Result<Vec<ObjectType>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked
             FROM object_types
             WHERE status != 'deprecated'
             ORDER BY system_locked DESC, name ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ObjectType {
                id: row.get(0)?,
                name: row.get(1)?,
                schema_json: row.get(2)?,
                ui_schema_json: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                system_locked: row.get::<_, i64>(6)? != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_object_type(conn: &Connection, id: &str) -> Result<Option<ObjectType>, String> {
    conn.query_row(
        "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked
         FROM object_types
         WHERE id = ?1 AND status != 'deprecated'",
        params![id],
        |row| {
            Ok(ObjectType {
                id: row.get(0)?,
                name: row.get(1)?,
                schema_json: row.get(2)?,
                ui_schema_json: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                system_locked: row.get::<_, i64>(6)? != 0,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn ensure_object_search_fts(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(CREATE_OBJECT_SEARCH_FTS)
        .map_err(|e| e.to_string())
}

fn object_search_fts_exists(conn: &Connection) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM sqlite_master
             WHERE type = 'table' AND name = 'object_search_fts'
         )",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(|e| e.to_string())
}

fn rebuild_object_search_fts(conn: &Connection) -> Result<(), String> {
    if !object_search_fts_exists(conn)? {
        return Ok(());
    }

    conn.execute("DELETE FROM object_search_fts", [])
        .map_err(|e| e.to_string())?;
    for object in list_objects(conn)? {
        index_object_for_search(conn, &object)?;
    }
    Ok(())
}

fn index_object_for_search(conn: &Connection, object: &ArkObject) -> Result<(), String> {
    if !object_search_fts_exists(conn)? {
        return Ok(());
    }

    conn.execute(
        "DELETE FROM object_search_fts WHERE object_id = ?1",
        params![object.id],
    )
    .map_err(|e| e.to_string())?;

    if object.deleted_at.is_some() {
        return Ok(());
    }

    conn.execute(
        "INSERT INTO object_search_fts (object_id, title, body, props)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            object.id,
            object.title,
            extract_plain_text_from_value(&object.content_json),
            extract_plain_text_from_value(&object.props_json),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn remove_object_from_search(conn: &Connection, id: &str) -> Result<(), String> {
    if !object_search_fts_exists(conn)? {
        return Ok(());
    }

    conn.execute(
        "DELETE FROM object_search_fts WHERE object_id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn rollback_savepoint(conn: &Connection, name: &str) {
    let _ = conn.execute_batch(&format!(
        "ROLLBACK TO SAVEPOINT {name}; RELEASE SAVEPOINT {name};"
    ));
}
