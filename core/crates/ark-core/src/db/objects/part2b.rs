
/// Every stored type_id that answers a by-type query: the canonical id plus
/// all registered aliases. Objects written before a type was canonicalised
/// keep the alias in `objects.type_id`; matching only the canonical id makes
/// them invisible to readers even though they belong to the type.
fn query_type_ids(conn: &Connection, type_id: &str) -> Result<Vec<String>, String> {
    let registry_ready: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_aliases')",
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

pub fn list_objects_by_type(conn: &Connection, type_id: &str) -> Result<Vec<ArkObject>, String> {
    let type_ids = query_type_ids(conn, type_id)?;
    let sql = format!(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
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
/// РѕРїС†РёРѕРЅР°Р»СЊРЅРѕ РѕС‚С„РёР»СЊС‚СЂРѕРІР°РЅРЅС‹С… РїРѕ props.source. Р‘РµР· РѕР±С…РѕРґР° РІСЃРµС… Р·Р°РїРёСЃРµР№
/// С‚РёРїР° вЂ” С„РёР»СЊС‚СЂ РЅР° SQL СѓСЂРѕРІРЅРµ С‡РµСЂРµР· `json_extract`. Hot path РґР»СЏ
/// focus widget'Р° (`stopManualStopwatch`).
///
/// `deleted_at IS NULL` вЂ” С‡С‚РѕР±С‹ tombstones РЅРµ РІРѕР·РІСЂР°С‰Р°Р»РёСЃСЊ РєР°Рє running.
/// РЎРѕСЂС‚РёСЂРѕРІРєР°: РЅРѕРІРµР№С€РёРµ startedAt СЃРІРµСЂС…Сѓ (DESC), РєР°Рє Сѓ callers'РѕРІ СЂР°РЅСЊС€Рµ.
pub fn list_running_time_entries(
    conn: &Connection,
    source_filter: Option<&str>,
) -> Result<Vec<ArkObject>, String> {
    let time_entry_type_ids = query_type_ids(conn, "time_entry_obj")?;
    let base_sql = format!(
        "SELECT id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at
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
