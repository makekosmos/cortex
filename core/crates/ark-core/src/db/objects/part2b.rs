/// Every stored type_id that answers a by-type query: the canonical id plus
/// all registered aliases. Objects written before a type was canonicalised
/// keep the alias in `objects.type_id`; matching only the canonical id makes
/// them invisible to readers even though they belong to the type.
fn query_type_ids(conn: &Connection, type_id: &str) -> Result<Vec<String>, String> {
    let registry_ready: bool = conn
        .query_row(
            concat!(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND ",
                "name='object_type_aliases')"
            ),
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if !registry_ready {
        return Ok(vec![type_id.to_string()]);
    }
    let canonical = crate::type_registry::resolve_type_id(conn, type_id)?
        .unwrap_or_else(|| type_id.to_string());
    let mut ids = vec![canonical.clone()];
    let mut stmt = conn
        .prepare("SELECT alias FROM object_type_aliases WHERE canonical_type_id=?1 ORDER BY alias")
        .map_err(|e| e.to_string())?;
    let aliases = stmt
        .query_map(params![canonical], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    ids.extend(aliases);
    Ok(ids)
}

fn type_id_in_clause(ids: &[String]) -> String {
    let placeholders = (1..=ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("type_id IN ({placeholders})")
}

/// Estimates the JSON payload size visible under each exact type/version and
/// field scope. Object contents are projected and measured inside Ark Core;
/// only counts and byte totals leave the storage boundary.
pub(crate) fn estimate_data_volume(
    conn: &Connection,
    scopes: &[crate::service::DataVolumeScope],
) -> Result<Vec<Value>, String> {
    if scopes.len() > 256 {
        return Err("too many data volume scopes".into());
    }
    scopes
        .iter()
        .map(|scope| estimate_data_volume_scope(conn, scope))
        .collect()
}

fn estimate_data_volume_scope(
    conn: &Connection,
    scope: &crate::service::DataVolumeScope,
) -> Result<Value, String> {
    if scope.type_id.is_empty()
        || scope.type_id.len() > 128
        || scope.type_version.is_empty()
        || scope.type_version.len() > 64
        || scope.fields.len() > 256
        || scope.fields.iter().any(|field| {
            field.len() > 128
                || !(field == "title"
                    || field == "content"
                    || field
                        .strip_prefix("props.")
                        .is_some_and(|key| !key.is_empty()))
        })
    {
        return Err("invalid data volume scope".into());
    }
    let type_ids = query_type_ids(conn, &scope.type_id)?;
    let mut values = type_ids
        .iter()
        .cloned()
        .map(rusqlite::types::Value::Text)
        .collect::<Vec<_>>();
    values.push(rusqlite::types::Value::Text(scope.type_version.clone()));
    let mut terms = vec![
        "length(CAST(id AS BLOB)) + length(CAST(type_id AS BLOB)) + \
         length(CAST(type_version AS BLOB)) + length(CAST(created_at AS BLOB)) + \
         length(CAST(updated_at AS BLOB)) + 58"
            .to_owned(),
    ];
    for field in &scope.fields {
        match field.as_str() {
            "title" => terms.push("length(CAST(title AS BLOB)) + 10".into()),
            "content" => terms.push("length(CAST(content_json AS BLOB)) + 17".into()),
            field => {
                let property = field.strip_prefix("props.").expect("validated field");
                let quoted_property =
                    serde_json::to_string(property).map_err(|error| error.to_string())?;
                let path = format!("$.{quoted_property}");
                values.push(rusqlite::types::Value::Text(path));
                let parameter = values.len();
                terms.push(format!(
                    "CASE WHEN json_type(props_json, ?{parameter}) IS NULL THEN 0 \
                     ELSE length(CAST(json_extract(props_json, ?{parameter}) AS BLOB)) + 16 END"
                ));
            }
        }
    }
    let sql = format!(
        "SELECT COUNT(*), COALESCE(SUM({}), 0) FROM objects WHERE {} \
         AND type_version = ?{} AND deleted_at IS NULL",
        terms.join(" + "),
        type_id_in_clause(&type_ids),
        type_ids.len() + 1
    );
    let (count, logical_bytes): (u64, u64) = conn
        .query_row(&sql, rusqlite::params_from_iter(values.iter()), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "type_id": scope.type_id,
        "type_version": scope.type_version,
        "object_count": count,
        "estimated_json_bytes": logical_bytes,
    }))
}

pub fn list_objects_by_type(conn: &Connection, type_id: &str) -> Result<Vec<ArkObject>, String> {
    let type_ids = query_type_ids(conn, type_id)?;
    let sql = format!(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at,
         deleted_at
         FROM objects
         WHERE {}
         ORDER BY updated_at DESC, created_at DESC",
        type_id_in_clause(&type_ids)
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(type_ids.iter()), map_ark_object_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn list_object_summaries_by_type(
    conn: &Connection,
    type_id: &str,
) -> Result<Vec<ArkObjectSummary>, String> {
    let type_ids = query_type_ids(conn, type_id)?;
    let sql = format!(
        "SELECT id, type_id, type_version, title, props_json, created_at, updated_at, deleted_at
         FROM objects
         WHERE {}
         ORDER BY updated_at DESC, created_at DESC",
        type_id_in_clause(&type_ids)
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params_from_iter(type_ids.iter()),
            map_ark_object_summary_row,
        )
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Р’РѕР·РІСЂР°С‰Р°РµС‚ С‚РѕР»СЊРєРѕ running time_entry_obj (props.endedAt IS NULL),
/// РѕРїС†РёРѕРЅР°Р»СЊРЅРѕ РѕС‚С„РёР»СЊС‚СЂРѕРІР°РЅРЅС‹С… РїРѕ props.source. Р‘РµР· РѕР±С…РѕРґР°
/// РІСЃРµС… Р·Р°РїРёСЃРµР№
/// С‚РёРїР° вЂ” С„РёР»СЊС‚СЂ РЅР° SQL СѓСЂРѕРІРЅРµ С‡РµСЂРµР· `json_extract`. Hot path РґР»СЏ
/// focus widget'Р° (`stopManualStopwatch`).
///
/// `deleted_at IS NULL` вЂ” С‡С‚РѕР±С‹ tombstones РЅРµ РІРѕР·РІСЂР°С‰Р°Р»РёСЃСЊ РєР°Рє running.
/// РЎРѕСЂС‚РёСЂРѕРІРєР°: РЅРѕРІРµР№С€РёРµ startedAt СЃРІРµСЂС…Сѓ (DESC), РєР°Рє Сѓ callers'РѕРІ
/// СЂР°РЅСЊС€Рµ.
pub fn list_running_time_entries(
    conn: &Connection,
    source_filter: Option<&str>,
) -> Result<Vec<ArkObject>, String> {
    let time_entry_type_ids = query_type_ids(conn, "time_entry_obj")?;
    let base_sql = format!(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at,
         deleted_at
         FROM objects
         WHERE {}
         AND deleted_at IS NULL
         AND json_extract(props_json, '$.endedAt') IS NULL",
        type_id_in_clause(&time_entry_type_ids)
    );
    let order = " ORDER BY json_extract(props_json, '$.startedAt') DESC";
    let mut values: Vec<&dyn rusqlite::ToSql> = time_entry_type_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();
    // `source_value` is declared outside the branch so `&source_value` inside
    // `values` (a `&&str` — ToSql is implemented on the reference) outlives
    // the query below.
    let source_value;
    let sql = if let Some(source) = source_filter {
        source_value = source;
        values.push(&source_value);
        format!(
            "{base_sql} AND json_extract(props_json, '$.source') = ?{}{order}",
            time_entry_type_ids.len() + 1
        )
    } else {
        format!("{base_sql}{order}")
    };
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(values.as_slice(), map_ark_object_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
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
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at,
         deleted_at
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
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at,
         deleted_at
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
