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
