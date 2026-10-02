//! `manager.data.*` — объектная статистика и списки для страницы «Данные».
//!
//! Counts are derived from the objects themselves (`list_object_summaries`),
//! not from the type registry: rows written before canonicalisation keep a
//! legacy alias in `objects.type_id`, and a registry-driven count reports
//! them as zero. Grouping by canonical identity keeps the page honest for
//! both stored forms.

use super::ManagerState;
use crate::ark_host::ArkHost;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

const MAX_BROWSE_PAGE: usize = 200;
const MAX_SEARCH_RESULTS: usize = 100;
const CANONICAL_VERSION: &str = "1.0.0";
const CANONICAL_TYPES: &[(&str, &str, &str)] = &[
    ("com.kosmos.note", "Заметка", "note_obj"),
    ("com.kosmos.task", "Задача", "task_obj"),
    ("com.kosmos.game", "Игра", "game_obj"),
];

fn canonical_identity(type_id: &str) -> Option<(&'static str, &'static str)> {
    CANONICAL_TYPES.iter().find_map(|(canonical, _, alias)| {
        (*canonical == type_id || *alias == type_id).then_some((*canonical, CANONICAL_VERSION))
    })
}

/// The grouping key for one stored type id: the canonical id when known,
/// the raw id otherwise (package-defined and system types have no alias).
fn bucket_key(type_id: &str) -> &str {
    canonical_identity(type_id)
        .map(|(id, _)| id)
        .unwrap_or(type_id)
}

fn canonical_name(bucket: &str) -> Option<&'static str> {
    CANONICAL_TYPES
        .iter()
        .find(|(id, _, _)| *id == bucket)
        .map(|(_, name, _)| *name)
}

fn row_type_id(row: &Value) -> Option<&str> {
    row.get("type_id")
        .or_else(|| row.get("typeId"))
        .and_then(Value::as_str)
}

fn row_logical_bytes(row: &Value) -> u64 {
    serde_json::to_vec(row).map(|v| v.len() as u64).unwrap_or(0)
}

/// Per-type counts and sizes over live rows, ordered canonically first.
fn collect_type_buckets(rows: &[Value]) -> (Vec<Value>, u64) {
    let mut buckets: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for row in rows.iter().filter(|row| is_live_public_row(row)) {
        let Some(type_id) = row_type_id(row) else {
            continue;
        };
        let entry = buckets.entry(bucket_key(type_id).to_owned()).or_default();
        entry.0 += 1;
        entry.1 += row_logical_bytes(row);
    }
    let mut total = 0_u64;
    let mut out = Vec::with_capacity(buckets.len());
    let mut emit = |key: &str, count: usize, bytes: u64, total: &mut u64| {
        *total = total.saturating_add(bytes);
        let name = canonical_name(key).unwrap_or_default();
        let version = canonical_identity(key)
            .map(|(_, v)| json!(v))
            .unwrap_or(Value::Null);
        out.push(json!({
            "id": key,
            "type_id": key,
            "type_version": version,
            "name": name,
            "count": count,
            "logical_bytes": bytes,
        }));
    };
    for (canonical, _, _) in CANONICAL_TYPES {
        if let Some((count, bytes)) = buckets.remove(*canonical) {
            emit(canonical, count, bytes, &mut total);
        }
    }
    for (key, (count, bytes)) in buckets {
        emit(&key, count, bytes, &mut total);
    }
    (out, total)
}

impl ManagerState {
    pub async fn data_summary(&self, ark: &Arc<ArkHost>) -> Result<Value, String> {
        let rows = ark
            .request("list_object_summaries", json!({}))
            .await
            .map_err(|e| e.to_string())?;
        let (types, total) =
            collect_type_buckets(rows.data.as_array().map(Vec::as_slice).unwrap_or_default());
        Ok(json!({
            "collected_at": chrono::Utc::now().to_rfc3339(),
            "types": types,
            "logical_bytes": total,
        }))
    }

    pub async fn data_types(&self, ark: &Arc<ArkHost>) -> Result<Value, String> {
        let response = ark
            .request("list_object_summaries", json!({}))
            .await
            .map_err(|e| e.to_string())?;
        let (types, _) = collect_type_buckets(
            response
                .data
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default(),
        );
        Ok(Value::Array(types))
    }

    pub async fn data_list(&self, ark: &Arc<ArkHost>, params: &Value) -> Result<Value, String> {
        let limit = bounded_limit(params.get("limit"), MAX_BROWSE_PAGE)?;
        let offset = parse_cursor(params.get("cursor"))?;
        // One unfiltered read: rows stored under a legacy alias still belong
        // to the requested canonical type, so filtering happens on the
        // bucket key, not on the stored `type_id` string.
        let bucket = params
            .get("type_id")
            .and_then(Value::as_str)
            .map(|t| bucket_key(t).to_owned());
        let response = ark
            .request("list_object_summaries", json!({}))
            .await
            .map_err(|e| e.to_string())?;
        let rows = response.data.as_array().cloned().unwrap_or_default();
        let all: Vec<&Value> = rows
            .iter()
            .filter(|row| is_live_public_row(row))
            .filter(|row| {
                bucket
                    .as_ref()
                    .is_none_or(|key| row_type_id(row).map(bucket_key) == Some(key.as_str()))
            })
            .collect();
        let (items, next_cursor) = browse_page(&all, offset, limit);
        Ok(json!({"items": items, "next_cursor": next_cursor}))
    }

    pub async fn data_search(&self, ark: &Arc<ArkHost>, params: &Value) -> Result<Value, String> {
        let query = params
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if query.is_empty() || query.len() > 256 {
            return Err("invalid-query".into());
        }
        let response = ark
            .request("search_objects", json!({"query": query}))
            .await
            .map_err(|e| e.to_string())?;
        let hits = response.data.as_array().cloned().unwrap_or_default();
        let ids: Vec<String> = hits
            .iter()
            .filter_map(|hit| {
                hit.get("entry_id")
                    .or_else(|| hit.get("entryId"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect();
        if ids.is_empty() {
            return Ok(json!({"items": [], "truncated": false}));
        }
        let objects = ark
            .request("get_objects_by_ids", json!({"ids": ids}))
            .await
            .map_err(|e| e.to_string())?;
        let mut by_id: std::collections::HashMap<&str, &Value> = objects
            .data
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|row| row.get("id").and_then(Value::as_str).map(|id| (id, row)))
            .collect();
        let items = hits
            .iter()
            .take(MAX_SEARCH_RESULTS)
            .filter_map(|hit| {
                let id = hit
                    .get("entry_id")
                    .or_else(|| hit.get("entryId"))
                    .and_then(Value::as_str)?;
                let row = by_id.remove(id)?;
                let mut item = safe_row(row, Some(query))?;
                let snippet = hit.get("text").and_then(Value::as_str).unwrap_or_default();
                item["excerpt"] = json!(crate::observability::redact_text(snippet)
                    .chars()
                    .take(256)
                    .collect::<String>());
                Some(item)
            })
            .collect::<Vec<_>>();
        Ok(json!({"items": items, "truncated": items.len() == MAX_SEARCH_RESULTS}))
    }
}

fn bounded_limit(value: Option<&Value>, max: usize) -> Result<usize, String> {
    let limit = value.and_then(Value::as_u64).unwrap_or(100);
    if limit == 0 || limit as usize > max {
        return Err("invalid-limit".into());
    }
    Ok(limit as usize)
}

fn parse_cursor(value: Option<&Value>) -> Result<usize, String> {
    let Some(value) = value else { return Ok(0) };
    let Some(cursor) = value.as_str() else {
        return Err("invalid-cursor".into());
    };
    cursor.parse::<usize>().map_err(|_| "invalid-cursor".into())
}

fn browse_page(all: &[&Value], offset: usize, limit: usize) -> (Vec<Value>, Option<String>) {
    // The cursor is an index into `all` (source rows), so it must advance by
    // the rows this page *scanned*, not the rows the projection kept —
    // otherwise a page of dropped rows hands the client the same offset
    // forever.
    let scanned = all.len().saturating_sub(offset).min(limit);
    let items: Vec<Value> = all
        .iter()
        .skip(offset)
        .take(limit)
        .filter_map(|row| safe_row(row, None))
        .collect();
    let next_cursor = (offset + scanned < all.len()).then(|| (offset + scanned).to_string());
    (items, next_cursor)
}

fn is_live_public_row(row: &Value) -> bool {
    row.get("deleted_at")
        .or_else(|| row.get("deletedAt"))
        .map(Value::is_null)
        .unwrap_or(true)
        && row_type_id(row)
            .is_none_or(|id| !id.is_empty() && !id.starts_with('_') && !id.starts_with("ark_"))
}

fn safe_row(row: &Value, query: Option<&str>) -> Option<Value> {
    let raw_type = row_type_id(row)?;
    if raw_type.is_empty() || raw_type.starts_with('_') || raw_type.starts_with("ark_") {
        return None;
    }
    let type_id = bucket_key(raw_type);
    // Objects keep the version they were written with (pre-registry rows say
    // "0.0.0-legacy"); echo it instead of pretending the row is the current
    // contract version — hiding it is what made real data look absent.
    let type_version = row
        .get("type_version")
        .or_else(|| row.get("typeVersion"))
        .and_then(Value::as_str)
        .map_or(Value::Null, |v| json!(v));
    let id = row.get("id").and_then(Value::as_str)?;
    let title = row.get("title").and_then(Value::as_str).unwrap_or_default();
    let props = row
        .get("props_json")
        .or_else(|| row.get("propsJson"))
        .or_else(|| row.get("props"))
        .and_then(Value::as_object);
    // Only the canonical contracts have a vetted allow-list; every other
    // type projects to identity + timestamps and nothing else.
    let allowed = match type_id {
        "com.kosmos.note" => ["description"].as_slice(),
        "com.kosmos.task" => [
            "status",
            "priority",
            "scheduledAt",
            "dueAt",
            "reminderAt",
            "completedAt",
            "canceledAt",
            "recurrence",
            "checklist",
        ]
        .as_slice(),
        "com.kosmos.game" => [
            "playStatus",
            "userRating",
            "released",
            "description",
            "genres",
            "platforms",
        ]
        .as_slice(),
        _ => &[] as &[&str],
    };
    let mut fields = Map::new();
    if let Some(props) = props {
        for key in allowed {
            if let Some(value) = props.get(*key) {
                fields.insert((*key).to_string(), value.clone());
            }
        }
    }
    let excerpt = query
        .and_then(|needle| {
            title
                .to_ascii_lowercase()
                .find(&needle.to_ascii_lowercase())
                .map(|_| title)
        })
        .unwrap_or(title);
    let links = row
        .get("links")
        .and_then(Value::as_array)
        .map(|links| {
            links
                .iter()
                .filter_map(|link| {
                    let target = link
                        .get("target_object_id")
                        .or_else(|| link.get("targetObjectId"))
                        .and_then(Value::as_str)?;
                    let link_type = link
                        .get("link_type")
                        .or_else(|| link.get("linkType"))
                        .and_then(Value::as_str)?;
                    Some(json!(
                        {"id": link.get("id").and_then(Value::as_str).unwrap_or_default(),
                        "target_object_id": target,
                        "link_type": link_type}))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(json!({
        "id": id,
        "type_id": type_id,
        "type_version": type_version,
        "title": crate::observability::redact_text(title),
        "fields": fields,
        "created_at": row.get(
            "created_at"
        ).or_else(|| row.get("createdAt")).and_then(Value::as_str),
        "updated_at": row.get(
            "updated_at"
        ).or_else(|| row.get("updatedAt")).and_then(Value::as_str),
        "excerpt": crate::observability::redact_text(excerpt).chars().take(256).collect::<String>(),
        "links": links,
    }))
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
