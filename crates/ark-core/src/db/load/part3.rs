
fn load_all_usage_sessions(conn: &Connection) -> Result<Vec<UsageSession>, String> {
    load_usage_sessions_page(conn, -1, 0)
}

fn load_usage_session(conn: &Connection, id: &str) -> Result<Option<UsageSession>, String> {
    conn.query_row(
        "SELECT id, tracked_app_id, device_id, device_name, platform, started_at,
                ended_at, runtime_ms, foreground_ms, idle_ms, window_title, process_name,
                exe_path, pid_start, pid_end, meta_json
         FROM usage_sessions WHERE id = ?1",
        params![id],
        |row| {
            let meta_json: String = row.get(15)?;
            Ok(UsageSession {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                device_id: row.get(2)?,
                device_name: row.get(3)?,
                platform: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                runtime_ms: row.get(7)?,
                foreground_ms: row.get(8)?,
                idle_ms: row.get(9)?,
                window_title: row.get(10)?,
                process_name: row.get(11)?,
                exe_path: row.get(12)?,
                pid_start: row.get(13)?,
                pid_end: row.get(14)?,
                meta_json: serde_json::from_str(&meta_json).unwrap_or_else(|_| json!({})),
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn load_usage_sessions_page(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<UsageSession>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, tracked_app_id, device_id, device_name, platform, started_at,
                    ended_at, runtime_ms, foreground_ms, idle_ms, window_title, process_name,
                    exe_path, pid_start, pid_end, meta_json
             FROM usage_sessions
             ORDER BY id ASC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit, offset], |row| {
            let meta_json_str: String = row.get(15)?;
            let meta_json: Value = serde_json::from_str(&meta_json_str).unwrap_or(json!({}));
            Ok(UsageSession {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                device_id: row.get(2)?,
                device_name: row.get(3)?,
                platform: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                runtime_ms: row.get(7)?,
                foreground_ms: row.get(8)?,
                idle_ms: row.get(9)?,
                window_title: row.get(10)?,
                process_name: row.get(11)?,
                exe_path: row.get(12)?,
                pid_start: row.get(13)?,
                pid_end: row.get(14)?,
                meta_json,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_all_usage_events(conn: &Connection) -> Result<Vec<UsageEvent>, String> {
    load_usage_events_page(conn, -1, 0)
}

fn load_usage_event(conn: &Connection, id: &str) -> Result<Option<UsageEvent>, String> {
    conn.query_row(
        "SELECT id, tracked_app_id, usage_session_id, device_id, device_name, platform,
                occurred_at, kind, window_title, process_name, exe_path, pid,
                is_foreground, is_idle, meta_json
         FROM usage_events WHERE id = ?1",
        params![id],
        |row| {
            let meta_json: String = row.get(14)?;
            Ok(UsageEvent {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                usage_session_id: row.get(2)?,
                device_id: row.get(3)?,
                device_name: row.get(4)?,
                platform: row.get(5)?,
                occurred_at: row.get(6)?,
                kind: row.get(7)?,
                window_title: row.get(8)?,
                process_name: row.get(9)?,
                exe_path: row.get(10)?,
                pid: row.get(11)?,
                is_foreground: row.get::<_, i64>(12)? != 0,
                is_idle: row.get::<_, i64>(13)? != 0,
                meta_json: serde_json::from_str(&meta_json).unwrap_or_else(|_| json!({})),
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn load_usage_events_page(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<UsageEvent>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, tracked_app_id, usage_session_id, device_id, device_name, platform,
                    occurred_at, kind, window_title, process_name, exe_path, pid,
                    is_foreground, is_idle, meta_json
             FROM usage_events
             ORDER BY id ASC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit, offset], |row| {
            let meta_json_str: String = row.get(14)?;
            let meta_json: Value = serde_json::from_str(&meta_json_str).unwrap_or(json!({}));
            Ok(UsageEvent {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                usage_session_id: row.get(2)?,
                device_id: row.get(3)?,
                device_name: row.get(4)?,
                platform: row.get(5)?,
                occurred_at: row.get(6)?,
                kind: row.get(7)?,
                window_title: row.get(8)?,
                process_name: row.get(9)?,
                exe_path: row.get(10)?,
                pid: row.get(11)?,
                is_foreground: row.get::<_, i64>(12)? != 0,
                is_idle: row.get::<_, i64>(13)? != 0,
                meta_json,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

