use serde_json::Value;

mod analytics;
mod linked_app_usage;
mod process_projection;
mod process_queries;
mod spans_and_title_totals;
mod summary;
mod summary_queries;

pub use analytics::load_usage_analytics;
pub use linked_app_usage::{summarize_linked_app_usage, LinkedAppUsageSummary};
pub use process_projection::{canonical_app_key, is_system_path, merge_top_apps};
pub use process_queries::{list_recent_usage_processes, search_usage_processes};
pub use spans_and_title_totals::{get_usage_title_total, upsert_usage_span};
pub use summary::load_usage_summary;

pub(super) fn payload_strings(payload: &Value, key: &str) -> Vec<String> {
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

pub(super) fn payload_spans(payload: &Value) -> Vec<Vec<i64>> {
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

pub(super) fn dictionary_index(values: &mut Vec<String>, value: &str) -> i64 {
    if let Some(index) = values.iter().position(|item| item == value) {
        index as i64
    } else {
        values.push(value.to_string());
        (values.len() - 1) as i64
    }
}

pub(super) fn merge_usage_spans(mut spans: Vec<Vec<i64>>) -> Vec<Vec<i64>> {
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
