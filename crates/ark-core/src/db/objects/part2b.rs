
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
