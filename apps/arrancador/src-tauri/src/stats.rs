use crate::database::with_db;
use chrono::{Duration, NaiveDate, Utc};
use rusqlite::params;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct DailyTotal {
    pub date: String,
    pub seconds: i64,
}

#[derive(Debug, Serialize)]
pub struct GameTotal {
    pub id: String,
    pub name: String,
    pub seconds: i64,
}

#[derive(Debug, Serialize)]
pub struct PlaytimeStats {
    pub range_start: String,
    pub range_end: String,
    pub total_seconds: i64,
    pub daily_totals: Vec<DailyTotal>,
    pub per_game_totals: Vec<GameTotal>,
}

fn parse_date(input: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(input, "%Y-%m-%d").ok()
}

#[tauri::command]
pub fn get_playtime_stats(
    start: Option<String>,
    end: Option<String>,
) -> Result<PlaytimeStats, String> {
    let mut end_date = end
        .as_deref()
        .and_then(parse_date)
        .unwrap_or_else(|| Utc::now().date_naive());
    let mut start_date = start
        .as_deref()
        .and_then(parse_date)
        .unwrap_or_else(|| end_date - Duration::days(29));

    if start_date > end_date {
        std::mem::swap(&mut start_date, &mut end_date);
    }

    let range_start = start_date.format("%Y-%m-%d").to_string();
    let range_end = end_date.format("%Y-%m-%d").to_string();

    with_db(|conn| {
        let mut daily_stmt = conn.prepare(
            "SELECT date, SUM(seconds) as seconds
             FROM playtime_daily
             WHERE date BETWEEN ?1 AND ?2
             GROUP BY date
             ORDER BY date",
        )?;

        let mut daily_map: HashMap<String, i64> = HashMap::new();
        let rows = daily_stmt.query_map(params![&range_start, &range_end], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for (date, seconds) in rows.flatten() {
            daily_map.insert(date, seconds);
        }

        let mut daily_totals = Vec::new();
        let mut total_seconds = 0;
        let mut cursor = start_date;
        while cursor <= end_date {
            let date = cursor.format("%Y-%m-%d").to_string();
            let seconds = *daily_map.get(&date).unwrap_or(&0);
            total_seconds += seconds;
            daily_totals.push(DailyTotal { date, seconds });
            cursor += Duration::days(1);
        }

        let mut game_stmt = conn.prepare(
            "SELECT games.id, games.name, SUM(playtime_daily.seconds) as seconds
             FROM playtime_daily
             JOIN games ON games.id = playtime_daily.game_id
             WHERE playtime_daily.date BETWEEN ?1 AND ?2
             GROUP BY games.id, games.name
             HAVING seconds > 0
             ORDER BY seconds DESC",
        )?;
        let per_game_totals = game_stmt
            .query_map(params![&range_start, &range_end], |row| {
                Ok(GameTotal {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    seconds: row.get(2)?,
                })
            })?
            .filter_map(|row| row.ok())
            .collect();

        Ok(PlaytimeStats {
            range_start,
            range_end,
            total_seconds,
            daily_totals,
            per_game_totals,
        })
    })
    .map_err(|e| e.to_string())
}
