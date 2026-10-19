use chrono::{Duration, Utc};
use rusqlite::{params, params_from_iter, Connection, Row};
use serde_json::{json, Value};
use std::collections::HashMap;

use ark_domain::types::*;

include!("usage_queries/spans_and_title_totals.rs");
include!("usage_queries/analytics.rs");
include!("usage_queries/process_queries.rs");
include!("usage_queries/process_projection.rs");
include!("usage_queries/game_playtime_queries.rs");
include!("usage_queries/game_playtime_summary.rs");

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

include!("usage_queries/linked_app_usage.rs");
