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
