use super::*;
pub(in crate::ws_server) async fn handle_app_index_op(
    subop: &str,
    params: serde_json::Value,
    app_index: &Arc<AppIndex>,
) -> LocalResponse {
    use crate::app_index::ranking::UsageStats;

    match subop {
        "list_all" => {
            // Lightweight command-list payload: no bulk inline icons in the WS hot path.
            // См. postmortems.md § 2026-06-08 — WS hot path инлайнил сотни иконок.
            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(500) as usize;
            let out: Vec<_> = app_index
                .all(limit)
                .await
                .into_iter()
                .map(|app| app_index_entry_json(&app))
                .collect();
            LocalResponse::ok(serde_json::json!({ "apps": out }))
        }
        "search" => {
            let query = match params.get("query").and_then(|v| v.as_str()) {
                Some(q) => q.to_string(),
                None => return LocalResponse::err("app_index.search: missing 'query'"),
            };
            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
            // Frecency: пустой UsageStats в v1. TODO: join из ARK usage_event_obj.
            let usage = UsageStats::empty();
            let results: Vec<_> = app_index
                .search(&query, limit, &usage)
                .await
                .into_iter()
                .map(|scored| {
                    serde_json::json!({
                        "app": app_index_entry_json(&scored.app),
                        "score": scored.score,
                    })
                })
                .collect();
            match serde_json::to_value(serde_json::json!({ "results": results })) {
                Ok(v) => LocalResponse::ok(v),
                Err(e) => LocalResponse::err(format!("app_index.search: serialize: {e}")),
            }
        }
        "icon_path" => {
            let id = match params.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("app_index.icon_path: missing 'id'"),
            };
            let app = match app_index.find(&id).await {
                Some(a) => a,
                None => return LocalResponse::err(format!("app_index.icon_path: not found: {id}")),
            };
            LocalResponse::ok(serde_json::json!({ "path": app.icon_path }))
        }
        "launch" => {
            let id = match params.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("app_index.launch: missing 'id'"),
            };
            let app = match app_index.find(&id).await {
                Some(a) => a,
                None => return LocalResponse::err(format!("app_index.launch: not found: {id}")),
            };
            match app_index.launch(&app) {
                Ok(_) => LocalResponse::ok(serde_json::json!({ "ok": true })),
                Err(e) => LocalResponse::err(format!("app_index.launch: {e}")),
            }
        }
        "rescan" => match app_index.rescan().await {
            Ok(stats) => match serde_json::to_value(&stats) {
                Ok(v) => LocalResponse::ok(v),
                Err(e) => LocalResponse::err(format!("app_index.rescan: serialize: {e}")),
            },
            Err(e) => LocalResponse::err(format!("app_index.rescan: {e}")),
        },
        other => LocalResponse::err(format!("app_index.{other}: unknown sub-operation")),
    }
}

pub(in crate::ws_server) fn resolve_arrancador_manual_exec_path(
    input_path: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    if !input_path.is_file() {
        return Err(format!("path is not a file: {}", input_path.display()));
    }

    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    #[cfg(target_os = "windows")]
    if ext.as_deref() == Some("lnk") {
        let target =
            crate::app_index::platform::windows::start_menu::resolve_lnk_target_path(input_path)
                .map_err(|e| format!("failed to read shortcut: {e}"))?
                .ok_or_else(|| format!("shortcut has no target: {}", input_path.display()))?;
        if !target.is_file() {
            return Err(format!(
                "shortcut target is not a file: {}",
                target.display()
            ));
        }
        return Ok(target);
    }

    match std::fs::canonicalize(input_path) {
        Ok(path) => {
            let value = path.to_string_lossy().to_string();
            if let Some(stripped) = value.strip_prefix(r"\\?\") {
                Ok(std::path::PathBuf::from(stripped))
            } else {
                Ok(path)
            }
        }
        Err(_) => Ok(input_path.to_path_buf()),
    }
}

pub(in crate::ws_server) fn app_icon_ref(app: &crate::app_index::App) -> Option<String> {
    app.icon_path
        .as_ref()
        .map(|_| format!("kosmos-icon://app/{}", app.id))
}

pub(in crate::ws_server) fn app_index_entry_json(app: &crate::app_index::App) -> serde_json::Value {
    serde_json::json!({
        "id": &app.id,
        "name": &app.name,
        "exec_path": &app.exec_path,
        // См. postmortems.md § 2026-06-09: app-index hot paths return refs;
        // renderer loads only visible icons through the Electron protocol.
        "icon_path": null,
        "icon_ref": app_icon_ref(app),
        "kind": &app.kind,
        "source": &app.source,
        "mtime": app.mtime,
    })
}
