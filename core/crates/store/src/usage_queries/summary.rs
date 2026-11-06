use super::summary_queries::{
    append_usage_binding_where_clause, build_usage_binding_index,
    find_usage_binding_for_tracked_row, map_usage_tracked_aggregate_row,
    map_usage_tracked_daily_row,
};
use domain::types::{
    UsageTimeAggregate, UsageTimeBinding, UsageTimeDailyTotal, UsageTimeRangeTotal,
    UsageTimeSummary,
};
use rusqlite::{params_from_iter, Connection};
use std::collections::HashMap;

pub fn load_usage_summary(
    conn: &Connection,
    bindings: &[UsageTimeBinding],
    range_start: Option<&str>,
    range_end: Option<&str>,
) -> Result<UsageTimeSummary, String> {
    let index = build_usage_binding_index(bindings);
    let mut aggregate_params = Vec::new();
    let Some(aggregate_where_sql) =
        append_usage_binding_where_clause(&index, &mut aggregate_params)
    else {
        return Ok(UsageTimeSummary {
            aggregates: Vec::new(),
            daily_totals: Vec::new(),
            per_binding_totals: Vec::new(),
        });
    };

    let aggregate_sql = format!(
        "SELECT tracked_apps.normalized_exe_path AS normalized_path,
                LOWER(COALESCE(tracked_apps.process_name, '')) AS normalized_process_name,
                CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS total_seconds,
                SUM(CASE WHEN usage_sessions.foreground_ms > 0 THEN 1 ELSE 0 END) AS \
                session_count,
                MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_seen
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

    let mut aggregates_by_game: HashMap<String, UsageTimeAggregate> = HashMap::new();
    for row in tracked_rows {
        let Some((binding_id, binding_name)) = find_usage_binding_for_tracked_row(
            row.normalized_path.as_deref(),
            row.normalized_process_name.as_deref(),
            &index,
        ) else {
            continue;
        };

        let entry = aggregates_by_game
            .entry(binding_id.clone())
            .or_insert(UsageTimeAggregate {
                binding_id,
                binding_name,
                total_seconds: 0,
                session_count: 0,
                last_seen: None,
            });
        entry.total_seconds += row.total_seconds;
        entry.session_count += row.session_count;
        let should_update_last_seen = match (&row.last_seen, &entry.last_seen) {
            (Some(next), Some(current)) => next > current,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if should_update_last_seen {
            entry.last_seen = row.last_seen;
        }
    }
    let mut aggregates = aggregates_by_game.into_values().collect::<Vec<_>>();
    aggregates.sort_by(|left, right| {
        right
            .total_seconds
            .cmp(&left.total_seconds)
            .then_with(|| left.binding_name.cmp(&right.binding_name))
    });

    let mut daily_totals = Vec::new();
    let mut per_binding_totals = Vec::new();
    if let (Some(range_start), Some(range_end)) = (range_start, range_end) {
        let mut daily_params = Vec::new();
        let Some(mut daily_where_sql) =
            append_usage_binding_where_clause(&index, &mut daily_params)
        else {
            return Ok(UsageTimeSummary {
                aggregates,
                daily_totals,
                per_binding_totals,
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
                    SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, \
                    10) AS date,
                    CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS seconds
             FROM usage_sessions
             JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
             {daily_where_sql}
             GROUP BY tracked_apps.id, tracked_apps.normalized_exe_path, \
             normalized_process_name, date
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
        let mut per_binding_totals_by_id: HashMap<String, UsageTimeRangeTotal> = HashMap::new();
        for row in tracked_daily_rows {
            let Some((binding_id, binding_name)) = find_usage_binding_for_tracked_row(
                row.normalized_path.as_deref(),
                row.normalized_process_name.as_deref(),
                &index,
            ) else {
                continue;
            };

            *daily_totals_by_date.entry(row.date.clone()).or_insert(0) += row.seconds;
            let entry = per_binding_totals_by_id
                .entry(binding_id.clone())
                .or_insert(UsageTimeRangeTotal {
                    binding_id,
                    binding_name,
                    seconds: 0,
                });
            entry.seconds += row.seconds;
        }
        daily_totals = daily_totals_by_date
            .into_iter()
            .map(|(date, seconds)| UsageTimeDailyTotal { date, seconds })
            .collect();
        daily_totals.sort_by(|left, right| left.date.cmp(&right.date));

        per_binding_totals = per_binding_totals_by_id.into_values().collect();
        per_binding_totals.sort_by(|left, right| {
            right
                .seconds
                .cmp(&left.seconds)
                .then_with(|| left.binding_name.cmp(&right.binding_name))
        });
    }

    Ok(UsageTimeSummary {
        aggregates,
        daily_totals,
        per_binding_totals,
    })
}
