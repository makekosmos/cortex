
fn clamp_usage_process_limit(limit: i64) -> i64 {
    limit.clamp(1, 25)
}

fn normalize_usage_binding_value(match_type: &str, value: &str) -> String {
    let trimmed = value.trim();
    if match_type == "exe_path" {
        trimmed.replace('/', "\\").to_lowercase()
    } else {
        trimmed.to_lowercase()
    }
}

fn build_usage_process_candidate(
    tracked_app_id: String,
    display_name: Option<String>,
    exe_path: Option<String>,
    process_name: Option<String>,
    last_seen_at: Option<String>,
    session_count: i64,
) -> Option<UsageProcessCandidate> {
    let exe_path = exe_path.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let process_name = process_name.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let (binding_match_type, binding_match_value) = match (&exe_path, &process_name) {
        (Some(value), _) => ("exe_path".to_string(), value.clone()),
        (None, Some(value)) => ("process_name".to_string(), value.clone()),
        (None, None) => return None,
    };
    let binding_normalized_value =
        normalize_usage_binding_value(&binding_match_type, &binding_match_value);
    let display_name = display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(process_name.as_deref())
        .or(exe_path.as_deref())
        .unwrap_or(&tracked_app_id)
        .to_string();

    Some(UsageProcessCandidate {
        tracked_app_id,
        display_name,
        exe_path,
        process_name,
        last_seen_at,
        session_count,
        binding_match_type,
        binding_match_value,
        binding_normalized_value,
    })
}

fn map_usage_process_candidate_row(
    row: &Row<'_>,
) -> rusqlite::Result<Option<UsageProcessCandidate>> {
    let tracked_app_id: String = row.get(0)?;
    let display_name: Option<String> = row.get(1)?;
    let exe_path: Option<String> = row.get(2)?;
    let process_name: Option<String> = row.get(3)?;
    let last_seen_at: Option<String> = row.get(4)?;
    let session_count = row.get::<_, Option<i64>>(5)?.unwrap_or(0);
    Ok(build_usage_process_candidate(
        tracked_app_id,
        display_name,
        exe_path,
        process_name,
        last_seen_at,
        session_count,
    ))
}

pub fn list_recent_usage_processes(
    conn: &Connection,
    limit: i64,
) -> Result<Vec<UsageProcessCandidate>, String> {
    let limit = clamp_usage_process_limit(limit);
    let mut stmt = conn
        .prepare(
            "SELECT tracked_apps.id AS tracked_app_id,
                    NULLIF(COALESCE(tracked_apps.display_name, tracked_apps.process_name, tracked_apps.exe_path), '') AS display_name,
                    NULLIF(tracked_apps.exe_path, '') AS exe_path,
                    NULLIF(tracked_apps.process_name, '') AS process_name,
                    MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_seen_at,
                    SUM(CASE WHEN usage_sessions.runtime_ms > 0 THEN 1 ELSE 0 END) AS session_count
             FROM usage_sessions
             JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
             WHERE NULLIF(COALESCE(tracked_apps.exe_path, tracked_apps.process_name), '') IS NOT NULL
             GROUP BY tracked_apps.id, tracked_apps.display_name, tracked_apps.exe_path, tracked_apps.process_name
             ORDER BY last_seen_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit], map_usage_process_candidate_row)
        .map_err(|e| e.to_string())?;
    rows.filter_map(|row| match row {
        Ok(Some(candidate)) => Some(Ok(candidate)),
        Ok(None) => None,
        Err(error) => Some(Err(error)),
    })
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
    .map(dedup_usage_process_candidates)
}

pub fn search_usage_processes(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<UsageProcessCandidate>, String> {
    let trimmed = query.trim().to_lowercase();
    if trimmed.is_empty() {
        return list_recent_usage_processes(conn, limit);
    }

    let limit = clamp_usage_process_limit(limit);
    let pattern = format!("%{trimmed}%");
    let mut stmt = conn
        .prepare(
            "SELECT tracked_apps.id AS tracked_app_id,
                    NULLIF(COALESCE(tracked_apps.display_name, tracked_apps.process_name, tracked_apps.exe_path), '') AS display_name,
                    NULLIF(tracked_apps.exe_path, '') AS exe_path,
                    NULLIF(tracked_apps.process_name, '') AS process_name,
                    MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_seen_at,
                    SUM(CASE WHEN usage_sessions.runtime_ms > 0 THEN 1 ELSE 0 END) AS session_count
             FROM usage_sessions
             JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
             WHERE (
                LOWER(COALESCE(tracked_apps.display_name, '')) LIKE ?1
                OR LOWER(COALESCE(tracked_apps.process_name, '')) LIKE ?1
                OR LOWER(COALESCE(tracked_apps.exe_path, '')) LIKE ?1
             )
             GROUP BY tracked_apps.id, tracked_apps.display_name, tracked_apps.exe_path, tracked_apps.process_name
             ORDER BY last_seen_at DESC
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![pattern, limit], map_usage_process_candidate_row)
        .map_err(|e| e.to_string())?;
    rows.filter_map(|row| match row {
        Ok(Some(candidate)) => Some(Ok(candidate)),
        Ok(None) => None,
        Err(error) => Some(Err(error)),
    })
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
    .map(dedup_usage_process_candidates)
}
