use super::*;

/// Dispatch `arrancador.<subop>`.
///
/// Sub-operations (subagent A scope):
///   - `arrancador.scan` → сканирует Steam/Epic, persists through Game facade.
///   - `arrancador.launch { game_id }` → reads typed Game DTO, then spawns
///     процесс через `launcher::launch`.
///
/// `arrancador.rawg.*` и `arrancador.sqoba.*` будут добавлены subagent'ами B/C
/// в этот же match (один namespace, один диспатчер).
pub(in crate::ws_server) async fn handle_arrancador_op(
    subop: &str,
    params: serde_json::Value,
    ark_host: &ArkHost,
) -> LocalResponse {
    let game_facade = crate::arrancador::game_facade::GameFacade::new(ark_host);
    match subop {
        "list" | "read" => {
            let result = if subop == "read" {
                let id = match params.get("id").and_then(Value::as_str) {
                    Some(id) => id,
                    None => return LocalResponse::err("arrancador.read: missing id"),
                };
                game_facade.read(id).await
            } else {
                game_facade.list().await
            };
            match result {
                Ok(value) => LocalResponse::ok(value),
                Err(error) => LocalResponse::err(format!("arrancador.{subop}: {error}")),
            }
        }
        "scan" => {
            // Override path — для тестов / non-standard Steam install.
            let override_path: Option<std::path::PathBuf> = params
                .get("steam_library_override")
                .and_then(|v| v.as_str())
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    let cfg = crate::arrancador::config::load();
                    cfg.steam_library_override
                });
            let discovered = crate::arrancador::scanner::scan_all(override_path.as_deref());

            // Read only canonical Games. Launcher/provider identity is owned by
            // the device-local Arrancador state, never by canonical props.
            let existing = match game_facade.objects().await {
                Ok(objects) => objects,
                Err(error) => {
                    return LocalResponse::err(format!("arrancador: game facade: {error}"));
                }
            };
            let mut cfg = crate::arrancador::config::load();
            let mut added = 0u32;
            let mut updated = 0u32;
            let mut skipped = 0u32;
            let mut errors: Vec<String> = Vec::new();
            let mut cleared_quarantine_ids = Vec::new();

            for game in &discovered {
                let existing_match = existing.iter().find(|obj| {
                    cfg.local_games
                        .get(&obj.id)
                        .and_then(|s| s.source.as_deref())
                        == Some(game.source.as_str())
                        && cfg
                            .local_games
                            .get(&obj.id)
                            .and_then(|s| s.source_app_id.as_deref())
                            == Some(game.source_app_id.as_str())
                });
                let props = serde_json::json!({ "platforms": [] });
                let now = chrono::Utc::now().to_rfc3339();
                let id = existing_match
                    .map(|obj| obj.id.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let upsert_obj = crate::arrancador::game_facade::GameFacade::upsert_payload(
                    existing_match,
                    &id,
                    &game.name,
                    &props,
                    &now,
                );
                let is_new = existing_match.is_none();
                match game_facade.upsert(upsert_obj).await {
                    Ok(_) => {
                        cfg.local_games.insert(
                            id.clone(),
                            crate::arrancador::config::LocalGameState {
                                source: Some(game.source.clone()),
                                source_app_id: Some(game.source_app_id.clone()),
                                install_dir: Some(game.install_dir.to_string_lossy().to_string()),
                                exe_path: game
                                    .exe_candidate
                                    .as_ref()
                                    .map(|p| p.to_string_lossy().to_string()),
                                save_paths: Vec::new(),
                            },
                        );
                        cleared_quarantine_ids.push(id.clone());
                        if is_new {
                            added += 1;
                        } else {
                            updated += 1;
                        }
                    }
                    Err(e) => {
                        skipped += 1;
                        errors.push(format!("{}: ark_host: {e}", game.name));
                    }
                }
            }
            if let Err(e) =
                crate::arrancador::config::save_with_quarantine(&cfg, &cleared_quarantine_ids)
            {
                return LocalResponse::err(format!(
                    "arrancador.scan: local state save failed: {e}"
                ));
            }

            LocalResponse::ok(serde_json::json!({
                "added": added,
                "updated": updated,
                "skipped": skipped,
                "discovered": discovered.len(),
                "errors": errors,
            }))
        }
        "add_manual" => {
            let name = match params.get("name").and_then(|v| v.as_str()).map(str::trim) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => return LocalResponse::err("arrancador.add_manual: missing 'name'"),
            };
            let input_path = match params
                .get("exe_path")
                .and_then(|v| v.as_str())
                .map(str::trim)
            {
                Some(s) if !s.is_empty() => std::path::PathBuf::from(s),
                _ => return LocalResponse::err("arrancador.add_manual: missing 'exe_path'"),
            };
            let exe_path = match resolve_arrancador_manual_exec_path(&input_path) {
                Ok(path) => path,
                Err(e) => return LocalResponse::err(format!("arrancador.add_manual: {e}")),
            };
            let save_paths: Vec<String> = params
                .get("save_paths")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::trim))
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();

            let props = serde_json::json!({
                "playStatus": "notStarted",
            });
            let existing = match game_facade.objects().await {
                Ok(objects) => objects,
                Err(error) => {
                    return LocalResponse::err(format!("arrancador: game facade: {error}"));
                }
            };
            let mut cfg = crate::arrancador::config::load();
            let mut cleared_quarantine_ids = Vec::new();
            let source_app_id = exe_path.to_string_lossy().to_string();
            let existing_match = existing.iter().find(|obj| {
                cfg.local_games
                    .get(&obj.id)
                    .and_then(|s| s.source.as_deref())
                    == Some("manual")
                    && cfg
                        .local_games
                        .get(&obj.id)
                        .and_then(|s| s.source_app_id.as_deref())
                        == Some(source_app_id.as_str())
            });
            let now = chrono::Utc::now().to_rfc3339();
            let id = existing_match
                .map(|obj| obj.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let mut object = crate::arrancador::game_facade::GameFacade::upsert_payload(
                existing_match,
                &id,
                &name,
                &props,
                &now,
            );
            object.local.save_paths = save_paths.clone();
            match game_facade.upsert(object).await {
                Ok(_) => {
                    cfg.local_games.insert(
                        id.clone(),
                        crate::arrancador::config::LocalGameState {
                            source: Some("manual".into()),
                            source_app_id: Some(source_app_id),
                            install_dir: exe_path.parent().map(|p| p.to_string_lossy().to_string()),
                            exe_path: Some(exe_path.to_string_lossy().to_string()),
                            save_paths,
                        },
                    );
                    cleared_quarantine_ids.push(id.clone());
                    if let Err(e) = crate::arrancador::config::save_with_quarantine(
                        &cfg,
                        &cleared_quarantine_ids,
                    ) {
                        return LocalResponse::err(format!(
                            "arrancador.add_manual: local state save failed: {e}"
                        ));
                    }
                    LocalResponse::ok(serde_json::json!({ "ok": true, "id": id }))
                }
                Err(e) => LocalResponse::err(format!("arrancador.add_manual: ark_host: {e}")),
            }
        }
        "launch" => {
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.launch: missing 'game_id'"),
            };
            let game = match game_facade.object(&game_id).await {
                Ok(game) => game,
                Err(error) => {
                    return LocalResponse::err(format!("arrancador.launch: game facade: {error}"));
                }
            };
            let cfg = crate::arrancador::config::load();
            let local = cfg.local_games.get(&game.id).cloned().unwrap_or_default();
            if cfg.quarantined_local_games.contains_key(&game.id) {
                tracing::warn!(
                    target: "arrancador.launch",
                    game_id = %game.id,
                    source = local.source.as_deref().unwrap_or("unknown"),
                    method = "validation",
                    result = "rejected",
                    reason = "quarantined",
                    "game launch"
                );
                return LocalResponse::err(
                    "arrancador.launch: launcher state quarantined; rescan or re-add the game",
                );
            }
            let launch_game = crate::arrancador::game_facade::GameFacade::launch_dto(&game);
            match crate::arrancador::launcher::launch(&launch_game, &local) {
                Ok(result) => match serde_json::to_value(&result) {
                    Ok(v) => LocalResponse::ok(v),
                    Err(e) => LocalResponse::err(format!("arrancador.launch: serialize: {e}")),
                },
                Err(e) => {
                    if let Some(code) = e.quarantine_code() {
                        if let Err(save_error) =
                            crate::arrancador::config::record_quarantine(&game.id, code)
                        {
                            tracing::warn!(
                                target: "arrancador.launch",
                                game_id = %game.id,
                                error = %save_error,
                                "failed to persist launcher quarantine"
                            );
                            return LocalResponse::err(format!(
                                "arrancador.launch: {e}; quarantine save failed: {save_error}"
                            ));
                        }
                    }
                    LocalResponse::err(format!("arrancador.launch: {e}"))
                }
            }
        }
        _ if subop.contains('.') => {
            super::arrancador_external::handle_arrancador_external_op(subop, params, &game_facade)
                .await
        }
        other => LocalResponse::err(format!("arrancador.{other}: unknown sub-operation")),
    }
}
