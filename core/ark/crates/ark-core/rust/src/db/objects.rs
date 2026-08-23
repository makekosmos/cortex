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

/// Проверить, существует ли `object_type` с указанным id.
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

pub fn upsert_object(conn: &Connection, object: &ArkObject) -> Result<(), String> {
    let registry_ready: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_versions')",
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
        // См. postmortems.md § 2026-06-04.
        conn.execute(
            "INSERT INTO objects
                (id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at)
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
            "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
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

fn canonical_query_type_id(conn: &Connection, type_id: &str) -> Result<String, String> {
    let registry_ready: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_aliases')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if registry_ready {
        return Ok(crate::type_registry::resolve_type_id(conn, type_id)?
            .unwrap_or_else(|| type_id.to_string()));
    }
    Ok(type_id.to_string())
}

pub fn list_objects_by_type(conn: &Connection, type_id: &str) -> Result<Vec<ArkObject>, String> {
    let type_id = canonical_query_type_id(conn, type_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
             FROM objects
             WHERE type_id = ?1
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![type_id], map_ark_object_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn list_object_summaries_by_type(
    conn: &Connection,
    type_id: &str,
) -> Result<Vec<ArkObjectSummary>, String> {
    let type_id = canonical_query_type_id(conn, type_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, type_id, type_version, title, props_json, created_at, updated_at, deleted_at
             FROM objects
             WHERE type_id = ?1
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![type_id], map_ark_object_summary_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Возвращает только running time_entry_obj (props.endedAt IS NULL),
/// опционально отфильтрованных по props.source. Без обхода всех записей
/// типа — фильтр на SQL уровне через `json_extract`. Hot path для
/// focus widget'а (`stopManualStopwatch`).
///
/// `deleted_at IS NULL` — чтобы tombstones не возвращались как running.
/// Сортировка: новейшие startedAt сверху (DESC), как у callers'ов раньше.
pub fn list_running_time_entries(
    conn: &Connection,
    source_filter: Option<&str>,
) -> Result<Vec<ArkObject>, String> {
    let time_entry_type_id = canonical_query_type_id(conn, "time_entry_obj")?;
    let base_sql =
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
         FROM objects
         WHERE type_id = ?1
           AND deleted_at IS NULL
           AND json_extract(props_json, '$.endedAt') IS NULL";
    let order = " ORDER BY json_extract(props_json, '$.startedAt') DESC";
    if let Some(source) = source_filter {
        let sql = format!("{base_sql} AND json_extract(props_json, '$.source') = ?2{order}");
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![time_entry_type_id, source], map_ark_object_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    } else {
        let sql = format!("{base_sql}{order}");
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![time_entry_type_id], map_ark_object_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

pub fn get_objects_by_ids(conn: &Connection, ids: &[String]) -> Result<Vec<ArkObject>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = (1..=ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
         FROM objects
         WHERE id IN ({placeholders})
         ORDER BY updated_at DESC, created_at DESC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(ids.iter()), map_ark_object_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_object(conn: &Connection, id: &str) -> Result<Option<ArkObject>, String> {
    conn.query_row(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
         FROM objects
         WHERE id = ?1",
        params![id],
        map_ark_object_row,
    )
    .optional()
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub file: String,
    pub line: usize,
    pub text: String,
    pub entry_id: String,
}

pub fn search_objects(conn: &Connection, query: &str) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let normalized_query = query.trim().to_lowercase();
    let query_terms = tokenize_search_text(query);
    if let Some(fts_query) = build_fts_match_query(&query_terms) {
        if let Ok(results) =
            search_objects_with_fts(conn, &fts_query, &normalized_query, &query_terms)
        {
            return Ok(results);
        }
    }

    search_objects_fallback(conn, &normalized_query, &query_terms)
}

fn search_objects_with_fts(
    conn: &Connection,
    fts_query: &str,
    normalized_query: &str,
    query_terms: &[String],
) -> Result<Vec<SearchResult>, String> {
    if !object_search_fts_exists(conn)? {
        return Err("object_search_fts is not available".to_string());
    }

    let mut stmt = conn
        .prepare(
            "SELECT objects.id, objects.type_id, objects.type_version, objects.title, objects.content_json, objects.props_json
             FROM object_search_fts
             JOIN objects ON objects.id = object_search_fts.object_id
             WHERE object_search_fts MATCH ?1
               AND objects.deleted_at IS NULL
             ORDER BY bm25(object_search_fts), objects.updated_at DESC, objects.created_at DESC
             LIMIT 30",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![fts_query], |row| {
            let content_json: String = row.get(4)?;
            let props_json: String = row.get(5)?;
            Ok(ArkObject {
                id: row.get(0)?,
                type_id: row.get(1)?,
                type_version: row.get(2)?,
                title: row.get(3)?,
                content_json: parse_json_or_default(content_json),
                props_json: parse_json_or_default(props_json),
                created_at: String::new(),
                updated_at: String::new(),
                deleted_at: None,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for object in rows {
        let object = object.map_err(|e| e.to_string())?;
        let body = object_search_body(&object);
        let (line, text) =
            build_search_context(&object.title, &body, normalized_query, query_terms);
        results.push(SearchResult {
            file: String::new(),
            line,
            text,
            entry_id: object.id,
        });
    }
    Ok(results)
}

fn search_objects_fallback(
    conn: &Connection,
    normalized_query: &str,
    query_terms: &[String],
) -> Result<Vec<SearchResult>, String> {
    let mut objects = list_objects(conn)?;
    objects.retain(|object| object.deleted_at.is_none());

    let mut matches = Vec::new();

    for object in objects {
        let body = object_search_body(&object);
        let title_matches = line_matches_query(&object.title, normalized_query, query_terms);
        let body_matches = body
            .lines()
            .any(|line| line_matches_query(line.trim(), normalized_query, query_terms));

        if !title_matches && !body_matches {
            continue;
        }

        let title_lower = object.title.to_lowercase();
        let body_lower = body.to_lowercase();
        let score = if title_lower.contains(normalized_query) {
            400
        } else if title_matches {
            300
        } else if body_lower.contains(normalized_query) {
            200
        } else {
            100
        };

        let (line, text) =
            build_search_context(&object.title, &body, normalized_query, query_terms);
        matches.push((
            score,
            SearchResult {
                file: String::new(),
                line,
                text,
                entry_id: object.id,
            },
        ));
    }

    matches.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    matches.truncate(30);

    Ok(matches.into_iter().map(|(_, result)| result).collect())
}

fn build_fts_match_query(query_terms: &[String]) -> Option<String> {
    if query_terms.is_empty() {
        return None;
    }

    let terms = query_terms
        .iter()
        .map(|term| format!("{term}*"))
        .collect::<Vec<_>>();
    Some(terms.join(" AND "))
}

fn object_search_body(object: &ArkObject) -> String {
    let content = extract_plain_text_from_value(&object.content_json);
    let props = extract_plain_text_from_value(&object.props_json);

    match (content.is_empty(), props.is_empty()) {
        (true, true) => String::new(),
        (false, true) => content,
        (true, false) => props,
        (false, false) => format!("{content}\n{props}"),
    }
}

fn build_search_context(
    title: &str,
    body: &str,
    normalized_query: &str,
    query_terms: &[String],
) -> (usize, String) {
    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if line_matches_query(trimmed, normalized_query, query_terms) {
            return (
                index + 1,
                build_snippet(trimmed, normalized_query, query_terms),
            );
        }
    }

    if line_matches_query(title, normalized_query, query_terms) {
        return (
            0,
            format!(
                "Название: {}",
                build_snippet(title.trim(), normalized_query, query_terms)
            ),
        );
    }

    let first_line = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();

    if !first_line.is_empty() {
        return (1, truncate_snippet(first_line, 140));
    }

    (0, format!("Название: {}", title.trim()))
}

fn line_matches_query(line: &str, normalized_query: &str, query_terms: &[String]) -> bool {
    if line.is_empty() {
        return false;
    }

    let line_lower = line.to_lowercase();
    if line_lower.contains(normalized_query) {
        return true;
    }

    let line_terms = tokenize_search_text(line);
    query_terms.iter().any(|query_term| {
        line_terms
            .iter()
            .any(|line_term| line_term.starts_with(query_term))
    })
}

fn build_snippet(line: &str, normalized_query: &str, query_terms: &[String]) -> String {
    let line_lower = line.to_lowercase();
    let direct_match = line_lower.find(normalized_query);
    let term_match = query_terms.iter().find_map(|term| line_lower.find(term));
    let match_start = direct_match.or(term_match).unwrap_or(0);
    let snippet_radius = 56;

    let start = char_boundary_before(line, match_start.saturating_sub(snippet_radius));
    let end = char_boundary_after(
        line,
        (match_start + normalized_query.len() + snippet_radius).min(line.len()),
    );
    let snippet = line[start..end].trim();

    if start == 0 && end == line.len() {
        truncate_snippet(snippet, 140)
    } else {
        let mut result = String::new();
        if start > 0 {
            result.push_str("...");
        }
        result.push_str(snippet);
        if end < line.len() {
            result.push_str("...");
        }
        result
    }
}

fn truncate_snippet(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        return line.to_string();
    }

    let truncated = line.chars().take(max_chars).collect::<String>();
    format!("{}...", truncated.trim_end())
}

fn char_boundary_before(value: &str, index: usize) -> usize {
    let mut safe_index = index.min(value.len());
    while safe_index > 0 && !value.is_char_boundary(safe_index) {
        safe_index -= 1;
    }
    safe_index
}

fn char_boundary_after(value: &str, index: usize) -> usize {
    let mut safe_index = index.min(value.len());
    while safe_index < value.len() && !value.is_char_boundary(safe_index) {
        safe_index += 1;
    }
    safe_index.min(value.len())
}

fn tokenize_search_text(value: &str) -> Vec<String> {
    value
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_lowercase())
        .collect()
}

fn extract_plain_text_from_value(value: &Value) -> String {
    let mut output = String::new();
    collect_plain_text(value, &mut output);
    output.trim().to_string()
}

fn collect_plain_text(node: &Value, output: &mut String) {
    if let Some(node_type) = node.get("type").and_then(|value| value.as_str()) {
        match node_type {
            "text" => {
                if let Some(text) = node.get("text").and_then(|value| value.as_str()) {
                    output.push_str(text);
                    output.push(' ');
                }
            }
            "hardBreak" => output.push('\n'),
            _ => {}
        }
    }

    if let Some(text) = node.as_str() {
        output.push_str(text);
        output.push(' ');
        return;
    }

    if let Some(children) = node.get("content").and_then(|value| value.as_array()) {
        for child in children {
            collect_plain_text(child, output);
        }
    }

    if let Some(values) = node.as_array() {
        for value in values {
            collect_plain_text(value, output);
        }
    }

    if let Some(values) = node.as_object() {
        let is_rich_text_text_node = matches!(
            node.get("type").and_then(|value| value.as_str()),
            Some("text")
        );
        for (key, value) in values {
            if key == "type" || key == "content" || (key == "text" && is_rich_text_text_node) {
                continue;
            }
            collect_plain_text(value, output);
        }
    }

    if matches!(
        node.get("type").and_then(|value| value.as_str()),
        Some("paragraph" | "heading" | "codeBlock" | "blockquote" | "listItem")
    ) {
        output.push('\n');
    }
}

pub fn upsert_object_link(conn: &Connection, link: &ObjectLink) -> Result<(), String> {
    // Do not use SQLite REPLACE here: it deletes the old row first.
    // См. postmortems.md § 2026-06-04.
    conn.execute(
        "INSERT INTO object_links
            (id, source_object_id, target_object_id, link_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            source_object_id = excluded.source_object_id,
            target_object_id = excluded.target_object_id,
            link_type = excluded.link_type,
            created_at = excluded.created_at",
        params![
            link.id,
            link.source_object_id,
            link.target_object_id,
            link.link_type,
            link.created_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_object_link(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM object_links WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_object_links(conn: &Connection) -> Result<Vec<ObjectLink>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, source_object_id, target_object_id, link_type, created_at
             FROM object_links
             ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ObjectLink {
                id: row.get(0)?,
                source_object_id: row.get(1)?,
                target_object_id: row.get(2)?,
                link_type: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

