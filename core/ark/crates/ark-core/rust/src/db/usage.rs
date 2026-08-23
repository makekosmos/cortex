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

fn upsert_usage_day_fragment(
    conn: &Connection,
    write: &UsageSpanWrite,
    day: &str,
    day_start_unix: i64,
    fragment_start: i64,
    fragment_end: i64,
) -> Result<UsageDay, String> {
    let id = format!("usage-day:{}:{day}", write.device_id);
    let mut usage_day = load_usage_day(conn, &id)?.unwrap_or_else(|| UsageDay {
        id,
        device_id: write.device_id.clone(),
        day: day.to_string(),
        payload_json: json!({ "a": [], "t": [], "s": [] }),
        updated_at: write.updated_at.clone(),
    });
    let mut apps = payload_strings(&usage_day.payload_json, "a");
    let mut titles = payload_strings(&usage_day.payload_json, "t");
    let mut spans = payload_spans(&usage_day.payload_json);
    let app_index = dictionary_index(&mut apps, &write.tracked_app_id);
    let title_index = write
        .window_title
        .as_deref()
        .map(|title| dictionary_index(&mut titles, title))
        .unwrap_or(-1);
    let start_second = fragment_start.saturating_sub(day_start_unix);
    let duration_seconds = fragment_end.saturating_sub(fragment_start);
    let replacement = vec![
        start_second,
        duration_seconds,
        app_index,
        title_index,
        write.flags,
    ];
    if let Some(existing) = spans
        .iter_mut()
        .find(|span| span[0] == start_second && span[2] == app_index)
    {
        *existing = replacement;
    } else {
        spans.push(replacement);
    }
    usage_day.payload_json = json!({
        "a": apps,
        "t": titles,
        "s": merge_usage_spans(spans),
    });
    usage_day.updated_at.clone_from(&write.updated_at);
    upsert_usage_day(conn, &usage_day)?;
    Ok(usage_day)
}

pub fn upsert_usage_span(
    conn: &Connection,
    write: &UsageSpanWrite,
) -> Result<Vec<UsageDay>, String> {
    if write.ended_at_unix <= write.started_at_unix {
        return Ok(Vec::new());
    }
    let mut cursor = write.started_at_unix;
    let mut days = Vec::new();
    while cursor < write.ended_at_unix {
        let date = chrono::DateTime::from_timestamp(cursor, 0)
            .ok_or_else(|| format!("invalid usage span timestamp: {cursor}"))?
            .date_naive();
        let day = date.format("%Y-%m-%d").to_string();
        let day_start_unix = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| format!("invalid usage day: {day}"))?
            .and_utc()
            .timestamp();
        let fragment_end = write
            .ended_at_unix
            .min(day_start_unix.saturating_add(86_400));
        days.push(upsert_usage_day_fragment(
            conn,
            write,
            &day,
            day_start_unix,
            cursor,
            fragment_end,
        )?);
        cursor = fragment_end;
    }
    Ok(days)
}

pub fn get_usage_title_total(conn: &Connection, query: &str) -> Result<UsageTitleTotal, String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(UsageTitleTotal {
            active_seconds: 0,
            idle_seconds: 0,
        });
    }
    let mut statement = conn
        .prepare("SELECT payload_json FROM usage_days ORDER BY day ASC")
        .map_err(|error| error.to_string())?;
    let payloads = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    let mut total = UsageTitleTotal {
        active_seconds: 0,
        idle_seconds: 0,
    };
    for payload in payloads {
        let payload: Value = serde_json::from_str(&payload.map_err(|error| error.to_string())?)
            .unwrap_or_else(|_| json!({}));
        let titles = payload_strings(&payload, "t");
        for span in payload_spans(&payload) {
            let title_matches = usize::try_from(span[3])
                .ok()
                .and_then(|index| titles.get(index))
                .is_some_and(|title| title.to_lowercase().contains(&query));
            if !title_matches {
                continue;
            }
            if span[4] & 1 == 1 {
                total.idle_seconds = total.idle_seconds.saturating_add(span[1]);
            } else {
                total.active_seconds = total.active_seconds.saturating_add(span[1]);
            }
        }
    }
    Ok(total)
}

fn clamp_positive_i64(value: i64, fallback: i64) -> i64 {
    if value > 0 {
        value
    } else {
        fallback
    }
}

fn iso_date_days_ago(days_ago: i64) -> String {
    (Utc::now().date_naive() - Duration::days(days_ago))
        .format("%Y-%m-%d")
        .to_string()
}

fn enumerate_dates(range_days: i64) -> Vec<String> {
    (0..range_days)
        .map(|index| iso_date_days_ago(range_days - index - 1))
        .collect()
}

pub fn load_usage_analytics(
    conn: &Connection,
    range_days: i64,
    top_apps_limit: i64,
    recent_sessions_limit: i64,
) -> Result<UsageAnalyticsSnapshot, String> {
    let range_days = clamp_positive_i64(range_days, 21);
    let top_apps_limit = clamp_positive_i64(top_apps_limit, 8);
    let recent_sessions_limit = clamp_positive_i64(recent_sessions_limit, 24);
    let range_start = iso_date_days_ago(range_days - 1);
    let range_end = iso_date_days_ago(0);

    let summary = conn
        .query_row(
            "SELECT COUNT(DISTINCT tracked_apps.id) AS tracked_app_count,
                    COUNT(DISTINCT usage_sessions.id) AS session_count,
                    (SELECT COUNT(*) FROM usage_events) AS event_count,
                    COALESCE(SUM(usage_sessions.runtime_ms), 0) AS total_runtime_ms,
                    COALESCE(SUM(usage_sessions.foreground_ms), 0) AS total_foreground_ms,
                    COALESCE(SUM(usage_sessions.idle_ms), 0) AS total_idle_ms,
                    MIN(usage_sessions.started_at) AS first_recorded_at,
                    MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_recorded_at
             FROM tracked_apps
             LEFT JOIN usage_sessions ON usage_sessions.tracked_app_id = tracked_apps.id",
            [],
            |row| {
                Ok(UsageSummary {
                    tracked_app_count: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    session_count: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    event_count: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    total_runtime_ms: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    total_foreground_ms: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    total_idle_ms: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    first_recorded_at: row.get(6)?,
                    last_recorded_at: row.get(7)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT SUBSTR(COALESCE(ended_at, started_at), 1, 10) AS date,
                    COALESCE(SUM(foreground_ms), 0) AS foreground_ms,
                    COALESCE(SUM(idle_ms), 0) AS idle_ms,
                    COUNT(*) AS sessions
             FROM usage_sessions
             WHERE SUBSTR(COALESCE(ended_at, started_at), 1, 10) BETWEEN ?1 AND ?2
             GROUP BY date
             ORDER BY date ASC",
        )
        .map_err(|e| e.to_string())?;
    let trend_rows = stmt
        .query_map(params![range_start, range_end], |row| {
            Ok(DailyTrendPoint {
                date: row.get(0)?,
                foreground_ms: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                idle_ms: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                sessions: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let trend_by_date = trend_rows
        .into_iter()
        .map(|point| (point.date.clone(), point))
        .collect::<std::collections::HashMap<_, _>>();
    let daily_trend = enumerate_dates(range_days)
        .into_iter()
        .map(|date| {
            trend_by_date
                .get(&date)
                .cloned()
                .unwrap_or(DailyTrendPoint {
                    date,
                    foreground_ms: 0,
                    idle_ms: 0,
                    sessions: 0,
                })
        })
        .collect();

    let mut stmt = conn
        .prepare(
            "SELECT CAST(STRFTIME('%w', started_at) AS INTEGER) AS weekday,
                    CAST(STRFTIME('%H', started_at) AS INTEGER) AS hour,
                    COALESCE(SUM(foreground_ms), 0) AS foreground_ms
             FROM usage_sessions
             WHERE SUBSTR(COALESCE(ended_at, started_at), 1, 10) BETWEEN ?1 AND ?2
             GROUP BY weekday, hour",
        )
        .map_err(|e| e.to_string())?;
    let hourly_heatmap = stmt
        .query_map(params![range_start, range_end], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|row| match row {
            Ok((Some(weekday), Some(hour), foreground_ms)) => Some(Ok(HourlyHeatmapCell {
                weekday,
                hour,
                foreground_ms,
            })),
            Ok((_, _, _)) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT tracked_apps.id,
                    tracked_apps.display_name,
                    tracked_apps.process_name,
                    tracked_apps.normalized_exe_path AS normalized_path,
                    tracked_apps.icon_ref,
                    COALESCE(SUM(usage_sessions.runtime_ms), 0) AS runtime_ms,
                    COALESCE(SUM(usage_sessions.foreground_ms), 0) AS foreground_ms,
                    COALESCE(SUM(usage_sessions.idle_ms), 0) AS idle_ms,
                    COUNT(usage_sessions.id) AS sessions,
                    MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_seen_at
             FROM tracked_apps
             JOIN usage_sessions ON usage_sessions.tracked_app_id = tracked_apps.id
             GROUP BY tracked_apps.id
             ORDER BY runtime_ms DESC, foreground_ms DESC, last_seen_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let top_apps = stmt
        .query_map(params![top_apps_limit], |row| {
            let display_name: Option<String> = row.get(1)?;
            let process_name: String = row.get(2)?;
            Ok(TopAppEntry {
                id: row.get(0)?,
                display_name: display_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(&process_name)
                    .to_string(),
                process_name,
                normalized_path: row.get(3)?,
                icon_ref: row.get(4)?,
                runtime_ms: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                foreground_ms: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                idle_ms: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
                sessions: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
                last_seen_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT usage_sessions.id,
                    usage_sessions.tracked_app_id,
                    tracked_apps.display_name,
                    usage_sessions.process_name,
                    usage_sessions.platform,
                    usage_sessions.device_name,
                    usage_sessions.started_at,
                    usage_sessions.ended_at,
                    usage_sessions.runtime_ms,
                    usage_sessions.foreground_ms,
                    usage_sessions.idle_ms,
                    usage_sessions.window_title
             FROM usage_sessions
             JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
             ORDER BY usage_sessions.started_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let recent_sessions = stmt
        .query_map(params![recent_sessions_limit], |row| {
            let display_name: Option<String> = row.get(2)?;
            let process_name: String = row.get(3)?;
            Ok(RecentSessionEntry {
                id: row.get(0)?,
                tracked_app_id: row.get(1)?,
                display_name: display_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(&process_name)
                    .to_string(),
                process_name,
                platform: row.get(4)?,
                device_name: row.get(5)?,
                started_at: row.get(6)?,
                ended_at: row.get(7)?,
                runtime_ms: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
                foreground_ms: row.get::<_, Option<i64>>(9)?.unwrap_or(0),
                idle_ms: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                window_title: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(UsageAnalyticsSnapshot {
        generated_at: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        summary,
        daily_trend,
        hourly_heatmap,
        top_apps,
        recent_sessions,
    })
}

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
}

#[derive(Debug)]
struct UsageGameBindingIndex {
    game_names_by_id: HashMap<String, String>,
    game_ids_by_path: HashMap<String, String>,
    game_ids_by_process_name: HashMap<String, String>,
}

#[derive(Debug)]
struct UsageTrackedAggregateRow {
    normalized_path: Option<String>,
    normalized_process_name: Option<String>,
    total_seconds: i64,
    session_count: i64,
    last_played: Option<String>,
}

#[derive(Debug)]
struct UsageTrackedDailyRow {
    normalized_path: Option<String>,
    normalized_process_name: Option<String>,
    date: String,
    seconds: i64,
}

fn build_usage_game_binding_index(bindings: &[UsageGamePlaytimeBinding]) -> UsageGameBindingIndex {
    let mut index = UsageGameBindingIndex {
        game_names_by_id: HashMap::new(),
        game_ids_by_path: HashMap::new(),
        game_ids_by_process_name: HashMap::new(),
    };

    for binding in bindings {
        let game_id = binding.game_id.trim();
        if game_id.is_empty() {
            continue;
        }

        let game_name = binding.game_name.trim();
        index.game_names_by_id.insert(
            game_id.to_string(),
            if game_name.is_empty() {
                game_id.to_string()
            } else {
                game_name.to_string()
            },
        );

        let normalized = normalize_usage_binding_value(&binding.match_type, &binding.match_value);
        if normalized.is_empty() {
            continue;
        }

        if binding.match_type == "exe_path" {
            index
                .game_ids_by_path
                .insert(normalized, game_id.to_string());
        } else if binding.match_type == "process_name" {
            index
                .game_ids_by_process_name
                .insert(normalized, game_id.to_string());
        }
    }

    index
}

fn append_usage_binding_where_clause(
    index: &UsageGameBindingIndex,
    params: &mut Vec<String>,
) -> Option<String> {
    let mut clauses = Vec::new();

    if !index.game_ids_by_path.is_empty() {
        let placeholders = (0..index.game_ids_by_path.len())
            .map(|offset| format!("?{}", params.len() + offset + 1))
            .collect::<Vec<_>>()
            .join(", ");
        clauses.push(format!(
            "tracked_apps.normalized_exe_path IN ({placeholders})"
        ));
        params.extend(index.game_ids_by_path.keys().cloned());
    }

    if !index.game_ids_by_process_name.is_empty() {
        let placeholders = (0..index.game_ids_by_process_name.len())
            .map(|offset| format!("?{}", params.len() + offset + 1))
            .collect::<Vec<_>>()
            .join(", ");
        clauses.push(format!(
            "LOWER(COALESCE(tracked_apps.process_name, '')) IN ({placeholders})"
        ));
        params.extend(index.game_ids_by_process_name.keys().cloned());
    }

    if clauses.is_empty() {
        None
    } else {
        Some(format!("WHERE ({})", clauses.join(" OR ")))
    }
}

fn find_usage_game_for_tracked_row(
    normalized_path: Option<&str>,
    normalized_process_name: Option<&str>,
    index: &UsageGameBindingIndex,
) -> Option<(String, String)> {
    if let Some(path) = normalized_path {
        let normalized = normalize_usage_binding_value("exe_path", path);
        if let Some(game_id) = index.game_ids_by_path.get(&normalized) {
            let game_name = index
                .game_names_by_id
                .get(game_id)
                .cloned()
                .unwrap_or_else(|| game_id.clone());
            return Some((game_id.clone(), game_name));
        }
    }

    let process_name = normalized_process_name
        .map(|value| normalize_usage_binding_value("process_name", value))
        .unwrap_or_default();
    if process_name.is_empty() {
        return None;
    }

    index
        .game_ids_by_process_name
        .get(&process_name)
        .map(|game_id| {
            let game_name = index
                .game_names_by_id
                .get(game_id)
                .cloned()
                .unwrap_or_else(|| game_id.clone());
            (game_id.clone(), game_name)
        })
}

fn map_usage_tracked_aggregate_row(row: &Row<'_>) -> rusqlite::Result<UsageTrackedAggregateRow> {
    Ok(UsageTrackedAggregateRow {
        normalized_path: row.get(0)?,
        normalized_process_name: row.get(1)?,
        total_seconds: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
        session_count: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
        last_played: row.get(4)?,
    })
}

fn map_usage_tracked_daily_row(row: &Row<'_>) -> rusqlite::Result<UsageTrackedDailyRow> {
    Ok(UsageTrackedDailyRow {
        normalized_path: row.get(0)?,
        normalized_process_name: row.get(1)?,
        date: row.get(2)?,
        seconds: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
    })
}

pub fn load_usage_game_playtime_summary(
    conn: &Connection,
    bindings: &[UsageGamePlaytimeBinding],
    range_start: Option<&str>,
    range_end: Option<&str>,
) -> Result<UsageGamePlaytimeSummary, String> {
    let index = build_usage_game_binding_index(bindings);
    let mut aggregate_params = Vec::new();
    let Some(aggregate_where_sql) =
        append_usage_binding_where_clause(&index, &mut aggregate_params)
    else {
        return Ok(UsageGamePlaytimeSummary {
            aggregates: Vec::new(),
            daily_totals: Vec::new(),
            per_game_totals: Vec::new(),
        });
    };

    let aggregate_sql = format!(
        "SELECT tracked_apps.normalized_exe_path AS normalized_path,
                LOWER(COALESCE(tracked_apps.process_name, '')) AS normalized_process_name,
                CAST(SUM(usage_sessions.runtime_ms) / 1000 AS INTEGER) AS total_seconds,
                SUM(CASE WHEN usage_sessions.runtime_ms > 0 THEN 1 ELSE 0 END) AS session_count,
                MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_played
         FROM usage_sessions
         JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
         {aggregate_where_sql}
         GROUP BY tracked_apps.id, tracked_apps.normalized_exe_path, normalized_process_name"
    );
    let mut stmt = conn.prepare(&aggregate_sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params_from_iter(aggregate_params.iter()),
            map_usage_tracked_aggregate_row,
        )
        .map_err(|e| e.to_string())?;
    let tracked_rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut aggregates_by_game: HashMap<String, UsageGamePlaytimeAggregate> = HashMap::new();
    for row in tracked_rows {
        let Some((game_id, game_name)) = find_usage_game_for_tracked_row(
            row.normalized_path.as_deref(),
            row.normalized_process_name.as_deref(),
            &index,
        ) else {
            continue;
        };

        let entry =
            aggregates_by_game
                .entry(game_id.clone())
                .or_insert(UsageGamePlaytimeAggregate {
                    game_id,
                    game_name,
                    total_seconds: 0,
                    session_count: 0,
                    last_played: None,
                });
        entry.total_seconds += row.total_seconds;
        entry.session_count += row.session_count;
        let should_update_last_played = match (&row.last_played, &entry.last_played) {
            (Some(next), Some(current)) => next > current,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if should_update_last_played {
            entry.last_played = row.last_played;
        }
    }
    let mut aggregates = aggregates_by_game.into_values().collect::<Vec<_>>();
    aggregates.sort_by(|left, right| {
        right
            .total_seconds
            .cmp(&left.total_seconds)
            .then_with(|| left.game_name.cmp(&right.game_name))
    });

    let mut daily_totals = Vec::new();
    let mut per_game_totals = Vec::new();
    if let (Some(range_start), Some(range_end)) = (range_start, range_end) {
        let mut daily_params = Vec::new();
        let Some(mut daily_where_sql) =
            append_usage_binding_where_clause(&index, &mut daily_params)
        else {
            return Ok(UsageGamePlaytimeSummary {
                aggregates,
                daily_totals,
                per_game_totals,
            });
        };
        let start_placeholder = daily_params.len() + 1;
        let end_placeholder = daily_params.len() + 2;
        daily_where_sql.push_str(&format!(
            " AND SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10)
                  BETWEEN ?{start_placeholder} AND ?{end_placeholder}"
        ));
        daily_params.push(range_start.to_string());
        daily_params.push(range_end.to_string());

        let daily_sql = format!(
            "SELECT tracked_apps.normalized_exe_path AS normalized_path,
                    LOWER(COALESCE(tracked_apps.process_name, '')) AS normalized_process_name,
                    SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10) AS date,
                    CAST(SUM(usage_sessions.runtime_ms) / 1000 AS INTEGER) AS seconds
             FROM usage_sessions
             JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
             {daily_where_sql}
             GROUP BY tracked_apps.id, tracked_apps.normalized_exe_path, normalized_process_name, date
             HAVING seconds > 0
             ORDER BY date ASC, tracked_apps.id ASC"
        );
        let mut stmt = conn.prepare(&daily_sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                params_from_iter(daily_params.iter()),
                map_usage_tracked_daily_row,
            )
            .map_err(|e| e.to_string())?;
        let tracked_daily_rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut daily_totals_by_date: HashMap<String, i64> = HashMap::new();
        let mut per_game_totals_by_id: HashMap<String, UsageGameRangeTotal> = HashMap::new();
        for row in tracked_daily_rows {
            let Some((game_id, game_name)) = find_usage_game_for_tracked_row(
                row.normalized_path.as_deref(),
                row.normalized_process_name.as_deref(),
                &index,
            ) else {
                continue;
            };

            *daily_totals_by_date.entry(row.date.clone()).or_insert(0) += row.seconds;
            let entry =
                per_game_totals_by_id
                    .entry(game_id.clone())
                    .or_insert(UsageGameRangeTotal {
                        game_id,
                        game_name,
                        seconds: 0,
                    });
            entry.seconds += row.seconds;
        }
        daily_totals = daily_totals_by_date
            .into_iter()
            .map(|(date, seconds)| UsageGameDailyTotal { date, seconds })
            .collect();
        daily_totals.sort_by(|left, right| left.date.cmp(&right.date));

        per_game_totals = per_game_totals_by_id.into_values().collect();
        per_game_totals.sort_by(|left, right| {
            right
                .seconds
                .cmp(&left.seconds)
                .then_with(|| left.game_name.cmp(&right.game_name))
        });
    }

    Ok(UsageGamePlaytimeSummary {
        aggregates,
        daily_totals,
        per_game_totals,
    })
}

