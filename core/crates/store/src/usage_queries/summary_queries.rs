use super::process_queries::normalize_usage_binding_value;
use domain::types::UsageTimeBinding;
use rusqlite::Row;
use std::collections::HashMap;

#[derive(Debug)]
pub(super) struct UsageTimeBindingIndex {
    names_by_id: HashMap<String, String>,
    ids_by_path: HashMap<String, String>,
    ids_by_process_name: HashMap<String, String>,
}

#[derive(Debug)]
pub(super) struct UsageTrackedAggregateRow {
    pub(super) normalized_path: Option<String>,
    pub(super) normalized_process_name: Option<String>,
    pub(super) total_seconds: i64,
    pub(super) session_count: i64,
    pub(super) last_seen: Option<String>,
}

#[derive(Debug)]
pub(super) struct UsageTrackedDailyRow {
    pub(super) normalized_path: Option<String>,
    pub(super) normalized_process_name: Option<String>,
    pub(super) date: String,
    pub(super) seconds: i64,
}

pub(super) fn build_usage_binding_index(bindings: &[UsageTimeBinding]) -> UsageTimeBindingIndex {
    let mut index = UsageTimeBindingIndex {
        names_by_id: HashMap::new(),
        ids_by_path: HashMap::new(),
        ids_by_process_name: HashMap::new(),
    };

    for binding in bindings {
        let binding_id = binding.binding_id.trim();
        if binding_id.is_empty() {
            continue;
        }

        let binding_name = binding.binding_name.trim();
        index.names_by_id.insert(
            binding_id.to_string(),
            if binding_name.is_empty() {
                binding_id.to_string()
            } else {
                binding_name.to_string()
            },
        );

        let normalized = normalize_usage_binding_value(&binding.match_type, &binding.match_value);
        if normalized.is_empty() {
            continue;
        }

        if binding.match_type == "exe_path" {
            index.ids_by_path.insert(normalized, binding_id.to_string());
        } else if binding.match_type == "process_name" {
            index
                .ids_by_process_name
                .insert(normalized, binding_id.to_string());
        }
    }

    index
}

pub(super) fn append_usage_binding_where_clause(
    index: &UsageTimeBindingIndex,
    params: &mut Vec<String>,
) -> Option<String> {
    let mut clauses = Vec::new();

    if !index.ids_by_path.is_empty() {
        let placeholders = (0..index.ids_by_path.len())
            .map(|offset| format!("?{}", params.len() + offset + 1))
            .collect::<Vec<_>>()
            .join(", ");
        clauses.push(format!(
            "tracked_apps.normalized_exe_path IN ({placeholders})"
        ));
        params.extend(index.ids_by_path.keys().cloned());
    }

    if !index.ids_by_process_name.is_empty() {
        let placeholders = (0..index.ids_by_process_name.len())
            .map(|offset| format!("?{}", params.len() + offset + 1))
            .collect::<Vec<_>>()
            .join(", ");
        clauses.push(format!(
            "LOWER(COALESCE(tracked_apps.process_name, '')) IN ({placeholders})"
        ));
        params.extend(index.ids_by_process_name.keys().cloned());
    }

    if clauses.is_empty() {
        None
    } else {
        Some(format!("WHERE ({})", clauses.join(" OR ")))
    }
}

pub(super) fn find_usage_binding_for_tracked_row(
    normalized_path: Option<&str>,
    normalized_process_name: Option<&str>,
    index: &UsageTimeBindingIndex,
) -> Option<(String, String)> {
    if let Some(path) = normalized_path {
        let normalized = normalize_usage_binding_value("exe_path", path);
        if let Some(binding_id) = index.ids_by_path.get(&normalized) {
            let binding_name = index
                .names_by_id
                .get(binding_id)
                .cloned()
                .unwrap_or_else(|| binding_id.clone());
            return Some((binding_id.clone(), binding_name));
        }
    }

    let process_name = normalized_process_name
        .map(|value| normalize_usage_binding_value("process_name", value))
        .unwrap_or_default();
    if process_name.is_empty() {
        return None;
    }

    index
        .ids_by_process_name
        .get(&process_name)
        .map(|binding_id| {
            let binding_name = index
                .names_by_id
                .get(binding_id)
                .cloned()
                .unwrap_or_else(|| binding_id.clone());
            (binding_id.clone(), binding_name)
        })
}

pub(super) fn map_usage_tracked_aggregate_row(
    row: &Row<'_>,
) -> rusqlite::Result<UsageTrackedAggregateRow> {
    Ok(UsageTrackedAggregateRow {
        normalized_path: row.get(0)?,
        normalized_process_name: row.get(1)?,
        total_seconds: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
        session_count: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
        last_seen: row.get(4)?,
    })
}

pub(super) fn map_usage_tracked_daily_row(row: &Row<'_>) -> rusqlite::Result<UsageTrackedDailyRow> {
    Ok(UsageTrackedDailyRow {
        normalized_path: row.get(0)?,
        normalized_process_name: row.get(1)?,
        date: row.get(2)?,
        seconds: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
    })
}
