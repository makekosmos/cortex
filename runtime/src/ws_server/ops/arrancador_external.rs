use super::*;
pub(super) async fn handle_arrancador_external_op(
    subop: &str,
    params: serde_json::Value,
    game_facade: &crate::arrancador::game_facade::GameFacade<'_>,
) -> LocalResponse {
    match subop {
        "config.get" => {
            let cfg = crate::arrancador::config::load();
            // Не возвращаем raw rawg_api_key — только статус.
            let payload = serde_json::json!({
                "rawg_api_key_set": cfg.rawg_api_key.as_deref().map(|s| !s.is_empty()).unwrap_or(false),
                "custom_scan_paths": cfg.custom_scan_paths,
                "sqoba_dest_dir": cfg.sqoba_dest_dir,
                "keep_backups": cfg.keep_backups,
            });
            LocalResponse::ok(payload)
        }
        "config.get_rawg_key" => {
            let cfg = crate::arrancador::config::load();
            LocalResponse::ok(
                serde_json::json!({ "configured": cfg.rawg_api_key.as_deref().is_some_and(|key| !key.is_empty()) }),
            )
        }
        "config.set_rawg_key" => {
            let key = params
                .get("key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let mut cfg = crate::arrancador::config::load();
            cfg.rawg_api_key = key.filter(|s| !s.is_empty());
            match crate::arrancador::config::save(&cfg) {
                Ok(()) => LocalResponse::ok(serde_json::json!({ "ok": true })),
                Err(e) => LocalResponse::err(format!("arrancador.config.set_rawg_key: {e}")),
            }
        }
        "rawg.search" => {
            let query = match params.get("query").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.rawg.search: missing 'query'"),
            };
            let cfg = crate::arrancador::config::load();
            let api_key = match cfg.rawg_api_key.as_deref() {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return LocalResponse::err("RAWG API key not configured"),
            };
            match crate::arrancador::rawg::search(&query, &api_key).await {
                Ok(results) => match serde_json::to_value(&results) {
                    Ok(v) => LocalResponse::ok(serde_json::json!({ "results": v })),
                    Err(e) => LocalResponse::err(format!("arrancador.rawg.search: serialize: {e}")),
                },
                Err(e) => LocalResponse::err(format!("arrancador.rawg.search: {e}")),
            }
        }
        "rawg.apply" => {
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.rawg.apply: missing 'game_id'"),
            };
            let rawg_id = match params.get("rawg_id").and_then(|v| v.as_u64()) {
                Some(n) => match u32::try_from(n) {
                    Ok(id) => id,
                    Err(_) => {
                        return LocalResponse::err(format!(
                            "arrancador.rawg.apply: rawg_id {n} exceeds u32 range"
                        ));
                    }
                },
                None => return LocalResponse::err("arrancador.rawg.apply: missing 'rawg_id'"),
            };
            let cfg = crate::arrancador::config::load();
            let api_key = match cfg.rawg_api_key.as_deref() {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return LocalResponse::err("RAWG API key not configured"),
            };
            match crate::arrancador::rawg::apply_to_game(&game_facade, &game_id, rawg_id, &api_key)
                .await
            {
                Ok(()) => LocalResponse::ok(serde_json::json!({ "ok": true })),
                Err(e) => LocalResponse::err(format!("arrancador.rawg.apply: {e}")),
            }
        }
        "sqoba.backup" => {
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.sqoba.backup: missing 'game_id'"),
            };
            let game = match game_facade.object(&game_id).await {
                Ok(game) => game,
                Err(error) => {
                    return LocalResponse::err(format!(
                        "arrancador.sqoba.backup: game facade: {error}"
                    ));
                }
            };
            let (game_name, manual_paths) =
                crate::arrancador::game_facade::GameFacade::sqoba_metadata(&game);
            let cfg = crate::arrancador::config::load();
            let manual_paths = manual_paths.or_else(|| {
                cfg.local_games.get(&game_id).and_then(|local| {
                    (!local.save_paths.is_empty()).then(|| {
                        local
                            .save_paths
                            .iter()
                            .map(std::path::PathBuf::from)
                            .collect::<Vec<_>>()
                    })
                })
            });
            match crate::arrancador::sqoba::backup(&game_id, &game_name, manual_paths.as_deref()) {
                Ok(b) => match serde_json::to_value(&b) {
                    Ok(v) => LocalResponse::ok(v),
                    Err(e) => {
                        LocalResponse::err(format!("arrancador.sqoba.backup: serialize: {e}"))
                    }
                },
                Err(e) => LocalResponse::err(format!("arrancador.sqoba.backup: {e}")),
            }
        }
        "sqoba.list" => {
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.sqoba.list: missing 'game_id'"),
            };
            let backups = crate::arrancador::sqoba::list_backups(&game_id);
            match serde_json::to_value(&backups) {
                Ok(v) => LocalResponse::ok(serde_json::json!({ "backups": v })),
                Err(e) => LocalResponse::err(format!("arrancador.sqoba.list: serialize: {e}")),
            }
        }
        "sqoba.restore" => {
            let backup_id = match params.get("backup_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.sqoba.restore: missing 'backup_id'"),
            };
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return LocalResponse::err("arrancador.sqoba.restore: missing 'game_id'"),
            };
            if let Err(error) = game_facade.object(game_id).await {
                return LocalResponse::err(format!(
                    "arrancador.sqoba.restore: game facade: {error}"
                ));
            }
            let path = match crate::arrancador::sqoba::resolve_backup_path(game_id, &backup_id) {
                Some(p) => p,
                None => {
                    return LocalResponse::err(format!(
                        "arrancador.sqoba.restore: backup '{}' not found for game '{}'",
                        backup_id, game_id
                    ));
                }
            };
            match crate::arrancador::sqoba::restore_for_game(&path, Some(game_id)) {
                Ok(r) => match serde_json::to_value(&r) {
                    Ok(v) => LocalResponse::ok(v),
                    Err(e) => {
                        LocalResponse::err(format!("arrancador.sqoba.restore: serialize: {e}"))
                    }
                },
                Err(e) => LocalResponse::err(format!("arrancador.sqoba.restore: {e}")),
            }
        }
        other => LocalResponse::err(format!("arrancador.{other}: unknown sub-operation")),
    }
}
