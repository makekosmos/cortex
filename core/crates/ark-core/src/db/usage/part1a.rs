// ---------------------------------------------------------------------------
// Usage tracking CRUD
// ---------------------------------------------------------------------------

pub fn upsert_tracked_app(conn: &Connection, tracked_app: &TrackedApp) -> Result<(), String> {
    conn.execute(
        "INSERT INTO tracked_apps
            (id, platform, exe_path, normalized_exe_path, process_name,
             display_name, publisher, icon_ref, first_seen_at, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
            platform = excluded.platform,
            exe_path = excluded.exe_path,
            normalized_exe_path = excluded.normalized_exe_path,
            process_name = excluded.process_name,
            display_name = excluded.display_name,
            publisher = excluded.publisher,
            icon_ref = excluded.icon_ref,
            first_seen_at = excluded.first_seen_at,
            last_seen_at = excluded.last_seen_at",
        params![
            tracked_app.id,
            tracked_app.platform,
            tracked_app.exe_path,
            tracked_app.normalized_exe_path,
            tracked_app.process_name,
            tracked_app.display_name,
            tracked_app.publisher,
            tracked_app.icon_ref,
            tracked_app.first_seen_at,
            tracked_app.last_seen_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_tracked_app(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM tracked_apps WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn upsert_usage_session(conn: &Connection, session: &UsageSession) -> Result<(), String> {
    let meta_json = serde_json::to_string(&session.meta_json).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO usage_sessions
            (id, tracked_app_id, device_id, device_name, platform, started_at, ended_at,
             runtime_ms, foreground_ms, idle_ms, window_title, process_name, exe_path,
             pid_start, pid_end, meta_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                 ?8, ?9, ?10, ?11, ?12, ?13,
                 ?14, ?15, ?16)
         ON CONFLICT(id) DO UPDATE SET
            tracked_app_id = excluded.tracked_app_id,
            device_id = excluded.device_id,
            device_name = excluded.device_name,
            platform = excluded.platform,
            started_at = excluded.started_at,
            ended_at = excluded.ended_at,
            runtime_ms = excluded.runtime_ms,
            foreground_ms = excluded.foreground_ms,
            idle_ms = excluded.idle_ms,
            window_title = excluded.window_title,
            process_name = excluded.process_name,
            exe_path = excluded.exe_path,
            pid_start = excluded.pid_start,
            pid_end = excluded.pid_end,
            meta_json = excluded.meta_json",
        params![
            session.id,
            session.tracked_app_id,
            session.device_id,
            session.device_name,
            session.platform,
            session.started_at,
            session.ended_at,
            session.runtime_ms,
            session.foreground_ms,
            session.idle_ms,
            session.window_title,
            session.process_name,
            session.exe_path,
            session.pid_start,
            session.pid_end,
            meta_json,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_usage_session(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM usage_sessions WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn upsert_usage_event(conn: &Connection, event: &UsageEvent) -> Result<(), String> {
    let meta_json = serde_json::to_string(&event.meta_json).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO usage_events
            (id, tracked_app_id, usage_session_id, device_id, device_name, platform,
             occurred_at, kind, window_title, process_name, exe_path, pid,
             is_foreground, is_idle, meta_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6,
                 ?7, ?8, ?9, ?10, ?11, ?12,
                 ?13, ?14, ?15)
         ON CONFLICT(id) DO UPDATE SET
            tracked_app_id = excluded.tracked_app_id,
            usage_session_id = excluded.usage_session_id,
            device_id = excluded.device_id,
            device_name = excluded.device_name,
            platform = excluded.platform,
            occurred_at = excluded.occurred_at,
            kind = excluded.kind,
            window_title = excluded.window_title,
            process_name = excluded.process_name,
            exe_path = excluded.exe_path,
            pid = excluded.pid,
            is_foreground = excluded.is_foreground,
            is_idle = excluded.is_idle,
            meta_json = excluded.meta_json",
        params![
            event.id,
            event.tracked_app_id,
            event.usage_session_id,
            event.device_id,
            event.device_name,
            event.platform,
            event.occurred_at,
            event.kind,
            event.window_title,
            event.process_name,
            event.exe_path,
            event.pid,
            event.is_foreground as i64,
            event.is_idle as i64,
            meta_json,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_usage_event(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM usage_events WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn upsert_usage_day(conn: &Connection, day: &UsageDay) -> Result<(), String> {
    let payload_json = serde_json::to_string(&day.payload_json).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO usage_days (id, device_id, day, payload_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            device_id = excluded.device_id,
            day = excluded.day,
            payload_json = excluded.payload_json,
            updated_at = excluded.updated_at",
        params![day.id, day.device_id, day.day, payload_json, day.updated_at,],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_usage_day(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM usage_days WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn load_usage_day(conn: &Connection, id: &str) -> Result<Option<UsageDay>, String> {
    conn.query_row(
        "SELECT id, device_id, day, payload_json, updated_at
         FROM usage_days WHERE id = ?1",
        params![id],
        |row| {
            let payload: String = row.get(3)?;
            Ok(UsageDay {
                id: row.get(0)?,
                device_id: row.get(1)?,
                day: row.get(2)?,
                payload_json: serde_json::from_str(&payload)
                    .unwrap_or_else(|_| json!({ "a": [], "t": [], "s": [] })),
                updated_at: row.get(4)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn payload_strings(payload: &Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn payload_spans(payload: &Value) -> Vec<Vec<i64>> {
    payload
        .get("s")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_array)
                .filter_map(|span| span.iter().map(Value::as_i64).collect::<Option<Vec<_>>>())
                .filter(|span| span.len() == 5 && span[1] > 0)
                .collect()
        })
        .unwrap_or_default()
}

fn dictionary_index(values: &mut Vec<String>, value: &str) -> i64 {
    if let Some(index) = values.iter().position(|item| item == value) {
        index as i64
    } else {
        values.push(value.to_string());
        (values.len() - 1) as i64
    }
}

fn merge_usage_spans(mut spans: Vec<Vec<i64>>) -> Vec<Vec<i64>> {
    spans.sort_by_key(|span| span[0]);
    let mut merged: Vec<Vec<i64>> = Vec::with_capacity(spans.len());
    for span in spans {
        if let Some(previous) = merged.last_mut() {
            let previous_end = previous[0].saturating_add(previous[1]);
            if previous[2..] == span[2..] && span[0] <= previous_end {
                let span_end = span[0].saturating_add(span[1]);
                previous[1] = previous[1].max(span_end.saturating_sub(previous[0]));
                continue;
            }
        }
        merged.push(span);
    }
    merged
}
