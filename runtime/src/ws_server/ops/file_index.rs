use super::*;
pub(in crate::ws_server) async fn handle_file_index_op(
    subop: &str,
    params: serde_json::Value,
    file_index: &Arc<FileIndex>,
) -> LocalResponse {
    match subop {
        "search" => {
            let query = match params.get("query").and_then(|v| v.as_str()) {
                Some(query) => query,
                None => return LocalResponse::err("file_index.search: missing 'query'"),
            };
            let limit = params
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(8)
                .min(50) as usize;
            let index = file_index.clone();
            let query = query.to_string();
            match tokio::task::spawn_blocking(move || index.search(&query, limit)).await {
                Ok(Ok(results)) => {
                    match serde_json::to_value(serde_json::json!({ "results": results })) {
                        Ok(value) => LocalResponse::ok(value),
                        Err(e) => LocalResponse::err(format!("file_index.search: serialize: {e}")),
                    }
                }
                Ok(Err(e)) => LocalResponse::err(format!("file_index.search: {e}")),
                Err(e) => LocalResponse::err(format!("file_index.search: join: {e}")),
            }
        }
        "open" => {
            let path = match params.get("path").and_then(|v| v.as_str()) {
                Some(path) => path,
                None => return LocalResponse::err("file_index.open: missing 'path'"),
            };
            match file_index.open(path) {
                Ok(()) => LocalResponse::ok(serde_json::json!({ "ok": true })),
                Err(e) => LocalResponse::err(format!("file_index.open: {e}")),
            }
        }
        "rescan" => match file_index.request_rescan() {
            Ok(stats) => match serde_json::to_value(stats) {
                Ok(value) => LocalResponse::ok(value),
                Err(e) => LocalResponse::err(format!("file_index.rescan: serialize: {e}")),
            },
            Err(e) => LocalResponse::err(format!("file_index.rescan: {e}")),
        },
        "clear_cache" => match file_index.clear_cache() {
            Ok(stats) => match serde_json::to_value(stats) {
                Ok(value) => LocalResponse::ok(value),
                Err(e) => LocalResponse::err(format!("file_index.clear_cache: serialize: {e}")),
            },
            Err(e) => LocalResponse::err(format!("file_index.clear_cache: {e}")),
        },
        "diagnostics" => match file_index.diagnostics() {
            Ok(diag) => match serde_json::to_value(diag) {
                Ok(value) => LocalResponse::ok(value),
                Err(e) => LocalResponse::err(format!("file_index.diagnostics: serialize: {e}")),
            },
            Err(e) => LocalResponse::err(format!("file_index.diagnostics: {e}")),
        },
        "estimate_root" => {
            let path = match params.get("path").and_then(|v| v.as_str()) {
                Some(path) => path.to_string(),
                None => return LocalResponse::err("file_index.estimate_root: missing 'path'"),
            };
            let index = file_index.clone();
            match tokio::task::spawn_blocking(move || index.estimate_root(&path)).await {
                Ok(Ok(estimate)) => match serde_json::to_value(estimate) {
                    Ok(value) => LocalResponse::ok(value),
                    Err(e) => {
                        LocalResponse::err(format!("file_index.estimate_root: serialize: {e}"))
                    }
                },
                Ok(Err(e)) => LocalResponse::err(format!("file_index.estimate_root: {e}")),
                Err(e) => LocalResponse::err(format!("file_index.estimate_root: join: {e}")),
            }
        }
        "settings_get" => match file_index.settings() {
            Ok(settings) => match serde_json::to_value(settings) {
                Ok(value) => LocalResponse::ok(value),
                Err(e) => LocalResponse::err(format!("file_index.settings_get: serialize: {e}")),
            },
            Err(e) => LocalResponse::err(format!("file_index.settings_get: {e}")),
        },
        "settings_set" => {
            let patch: FileIndexSettingsPatch = match serde_json::from_value(params) {
                Ok(patch) => patch,
                Err(e) => return LocalResponse::err(format!("file_index.settings_set: {e}")),
            };
            match file_index.set_settings(patch).await {
                Ok(stats) => LocalResponse::ok(serde_json::json!({ "stats": stats })),
                Err(e) => LocalResponse::err(format!("file_index.settings_set: {e}")),
            }
        }
        "scope_add" => {
            let path = match params.get("path").and_then(|v| v.as_str()) {
                Some(path) => path,
                None => return LocalResponse::err("file_index.scope_add: missing 'path'"),
            };
            match file_index.add_root(path).await {
                Ok(stats) => LocalResponse::ok(serde_json::json!({ "stats": stats })),
                Err(e) => LocalResponse::err(format!("file_index.scope_add: {e}")),
            }
        }
        "scope_remove" => {
            let path = match params.get("path").and_then(|v| v.as_str()) {
                Some(path) => path,
                None => return LocalResponse::err("file_index.scope_remove: missing 'path'"),
            };
            match file_index.remove_root(path).await {
                Ok(stats) => LocalResponse::ok(serde_json::json!({ "stats": stats })),
                Err(e) => LocalResponse::err(format!("file_index.scope_remove: {e}")),
            }
        }
        "ignore_add" => {
            let pattern = match params.get("pattern").and_then(|v| v.as_str()) {
                Some(pattern) => pattern,
                None => return LocalResponse::err("file_index.ignore_add: missing 'pattern'"),
            };
            match file_index.add_ignore_pattern(pattern).await {
                Ok(stats) => LocalResponse::ok(serde_json::json!({ "stats": stats })),
                Err(e) => LocalResponse::err(format!("file_index.ignore_add: {e}")),
            }
        }
        "ignore_remove" => {
            let pattern = match params.get("pattern").and_then(|v| v.as_str()) {
                Some(pattern) => pattern,
                None => return LocalResponse::err("file_index.ignore_remove: missing 'pattern'"),
            };
            match file_index.remove_ignore_pattern(pattern).await {
                Ok(stats) => LocalResponse::ok(serde_json::json!({ "stats": stats })),
                Err(e) => LocalResponse::err(format!("file_index.ignore_remove: {e}")),
            }
        }
        other => LocalResponse::err(format!("file_index.{other}: unknown sub-operation")),
    }
}
