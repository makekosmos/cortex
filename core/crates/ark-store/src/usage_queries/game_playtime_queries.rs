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
