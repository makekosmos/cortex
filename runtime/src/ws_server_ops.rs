use super::*;

pub(super) struct LocalResponse {
    pub(super) ok: bool,
    pub(super) data: serde_json::Value,
    pub(super) error: Option<String>,
}

impl LocalResponse {
    pub(super) fn ok(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data,
            error: None,
        }
    }

    pub(super) fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: serde_json::Value::Null,
            error: Some(msg.into()),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn build_diagnostics_snapshot(
    ark_host: &Arc<ArkHost>,
    rpc_diagnostics: &SharedRpcDiagnostics,
    file_index: &Arc<FileIndex>,
    usage_diagnostics: &Arc<UsageTrackerDiagnosticsState>,
    app_index: &Arc<AppIndex>,
    protocol_usage: &Arc<ProtocolUsageStore>,
    package_service: &Arc<PackageService>,
    correlation_id: &str,
    data_dir: &std::path::Path,
) -> serde_json::Value {
    let app_index_snapshot = app_index.diagnostics_snapshot().await;
    let app_index_background_worker = app_index_snapshot.background_worker.clone();
    serde_json::json!({
        "rpc": rpc_diagnostics.snapshot(),
        "file_index": file_index.diagnostics_snapshot(),
        "usage_tracker": usage_diagnostics.snapshot(),
        "app_index": app_index_snapshot,
          "protocol_usage": protocol_usage.snapshot(),
          "package_workers": package_service.worker_diagnostics(),
        "correlation_id": correlation_id,
        "ark_core": {
            "stderr_tail": ark_host.stderr_tail_snapshot(),
        },
        "engine_supervisor": crate::engine_supervisor::diagnostics_snapshot(data_dir),
        "background_workers": {
            "app_index": app_index_background_worker,
            "db_backup": crate::db_backup::diagnostics_snapshot(),
        },
    })
}

pub(super) fn observe_rpc_payload(
    rpc_diagnostics: &SharedRpcDiagnostics,
    operation: &str,
    started: std::time::Instant,
    payload: &str,
) {
    rpc_diagnostics.observe_response(operation, started.elapsed(), payload.len());
}

pub(super) async fn handle_package_op(
    subop: &str,
    params: serde_json::Value,
    service: &Arc<PackageService>,
) -> LocalResponse {
    match subop {
        "list" => {
            let kind = match params.get("kind").and_then(Value::as_str) {
                None => None,
                Some("app") => Some(crate::package_service::PackageKind::App),
                Some("source") => Some(crate::package_service::PackageKind::Source),
                Some("bridge") => Some(crate::package_service::PackageKind::Bridge),
                Some(_) => return LocalResponse::err("packages.list: invalid-kind"),
            };
            let catalog_kind = kind.clone();
            let installed = package_blocking({
                let service = service.clone();
                move || service.list_filtered(kind)
            })
            .await;
            match installed {
                Ok(list) => {
                    let catalog = service
                        .catalog_packages(catalog_kind.as_ref())
                        .unwrap_or_default();
                    let mut value =
                        serde_json::to_value(&list).unwrap_or_else(|_| serde_json::json!({}));
                    value["catalog"] = serde_json::json!(catalog);
                    LocalResponse::ok(value)
                }
                Err(error) => package_response::<crate::package_service::PackageListSummary>(
                    subop,
                    Err(error),
                ),
            }
        }
        "trust_status" => LocalResponse::ok(serde_json::json!({
            "trust": service.trust_summary(),
            "catalog": service.catalog_summary(),
        })),
        "refresh_catalog" => package_response(subop, service.refresh_catalog().await),
        "catalog_apply" => {
            let Some(document) = params.get("document").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.catalog_apply: invalid-request");
            };
            let signatures = match package_signatures(&params) {
                Ok(value) => value,
                Err(response) => return response,
            };
            let document = document.as_bytes().to_vec();
            package_response(
                subop,
                package_blocking({
                    let service = service.clone();
                    move || service.apply_catalog(document, signatures)
                })
                .await,
            )
        }
        "transition_apply" => {
            let Some(document) = params.get("document").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.transition_apply: invalid-request");
            };
            let signatures = match package_signatures(&params) {
                Ok(value) => value,
                Err(response) => return response,
            };
            let document = document.as_bytes().to_vec();
            package_response(
                subop,
                package_blocking({
                    let service = service.clone();
                    move || service.apply_transition(&document, signatures)
                })
                .await
                .map(|()| serde_json::json!({ "applied": true })),
            )
        }
        "revocation_apply" => {
            let Some(document) = params.get("document").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.revocation_apply: invalid-request");
            };
            let signatures = match package_signatures(&params) {
                Ok(value) => value,
                Err(response) => return response,
            };
            package_response(
                subop,
                service
                    .apply_revocations_with_worker_stop(document.as_bytes(), signatures)
                    .await
                    .map(|()| serde_json::json!({ "applied": true })),
            )
        }
        "install" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.install: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.install: invalid-request");
            };
            let id = id.to_owned();
            let version = version.to_owned();
            if let Some(archive_path) = params
                .get("archive_path")
                .and_then(serde_json::Value::as_str)
            {
                let archive_path = archive_path.to_owned();
                package_response(
                    subop,
                    service
                        .install_from_path_with_worker_stop(&id, &version, archive_path)
                        .await,
                )
            } else {
                package_response(subop, service.install_from_catalog(&id, &version).await)
            }
        }
        "set_enabled" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.set_enabled: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.set_enabled: invalid-request");
            };
            let Some(enabled) = params.get("enabled").and_then(serde_json::Value::as_bool) else {
                return LocalResponse::err("packages.set_enabled: invalid-request");
            };
            package_response(subop, service.set_enabled(id, version, enabled).await)
        }
        "bridge_config" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.bridge_config: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.bridge_config: invalid-request");
            };
            package_response(subop, service.bridge_config(id, version))
        }
        "bridge_config_set" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.bridge_config_set: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.bridge_config_set: invalid-request");
            };
            let Some(config) = params
                .get("config")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok())
            else {
                return LocalResponse::err("packages.bridge_config_set: invalid-request");
            };
            package_response(subop, service.set_bridge_config(id, version, config).await)
        }
        "uninstall" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.uninstall: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.uninstall: invalid-request");
            };
            package_response(
                subop,
                service
                    .uninstall_with_worker_stop(id, version)
                    .await
                    .map(|()| serde_json::json!({ "uninstalled": true })),
            )
        }
        other => LocalResponse::err(format!("packages.{other}: unknown sub-operation")),
    }
}

pub(super) struct StoreCatalogIndex<'a> {
    packages: &'a PackageService,
}

impl PackageIndexLookup for StoreCatalogIndex<'_> {
    fn package_release(&self, package_id: &str, version: &str, is_bridge: bool) -> bool {
        self.packages
            .has_catalog_release(package_id, version, is_bridge)
    }

    fn canonical_type_version(&self, type_id: &str, versions: &str) -> bool {
        let Ok(requirement) = semver::VersionReq::parse(versions) else {
            return false;
        };
        ark_core::canonical_types::definitions::canonical_type_registrations()
            .ok()
            .is_some_and(|types| {
                types.into_iter().any(|registered| {
                    registered.type_id == type_id
                        && semver::Version::parse(&registered.version)
                            .ok()
                            .is_some_and(|version| requirement.matches(&version))
                })
            })
    }
}

pub(super) async fn handle_store_op(
    subop: &str,
    params: serde_json::Value,
    packages: &PackageService,
    catalog: Option<&StoreCatalogService>,
) -> LocalResponse {
    if subop == "external_url" {
        let listing_id = params.get("listing_id").and_then(Value::as_str);
        return match (catalog, listing_id) {
            (Some(catalog), Some(listing_id)) => catalog
                .external_url_at(listing_id, chrono::Utc::now())
                .map(|url| LocalResponse::ok(serde_json::json!({ "url": url })))
                .unwrap_or_else(|_| LocalResponse::err("store: unavailable")),
            _ => LocalResponse::err("store: unavailable"),
        };
    }
    let installed = match packages.store_installed_listings() {
        Ok(installed) => installed,
        Err(_) => return LocalResponse::err("store: installed-packages-unavailable"),
    };
    let response = match subop {
        "catalog" => Ok(match catalog {
            Some(catalog) => catalog.catalog(chrono::Utc::now(), installed),
            None => CatalogDto {
                state: "unavailable".into(),
                sequence: None,
                issued_at: None,
                expires_at: None,
                listings: Vec::new(),
                installed,
            },
        }),
        "refresh" => match catalog {
            Some(catalog) => {
                if packages.catalog_summary().is_none() && packages.refresh_catalog().await.is_err()
                {
                    Err("store: package-index-unavailable")
                } else {
                    match catalog.refresh(&StoreCatalogIndex { packages }).await {
                        Ok(_) => Ok(catalog.catalog(
                            chrono::Utc::now(),
                            packages.store_installed_listings().unwrap_or_default(),
                        )),
                        Err(_) => Err("store: refresh-unavailable"),
                    }
                }
            }
            None => Err("store: unavailable"),
        },
        other => return LocalResponse::err(format!("store.{other}: unknown sub-operation")),
    };
    match response
        .and_then(|result| serde_json::to_value(result).map_err(|_| "store: serialization-failed"))
    {
        Ok(value) => LocalResponse::ok(value),
        Err(error) => LocalResponse::err(error),
    }
}

pub(super) fn package_signatures(
    params: &serde_json::Value,
) -> Result<SignatureSet, LocalResponse> {
    params
        .get("signatures")
        .cloned()
        .ok_or_else(|| LocalResponse::err("packages: invalid-request"))
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|_| LocalResponse::err("packages: invalid-request"))
        })
}

pub(super) async fn package_blocking<T, F>(work: F) -> Result<T, PackageError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, PackageError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| PackageError::Persistence)?
}

pub(super) fn package_response<T: serde::Serialize>(
    operation: &str,
    result: Result<T, PackageError>,
) -> LocalResponse {
    match result {
        Ok(value) => match serde_json::to_value(value) {
            Ok(value) => LocalResponse::ok(value),
            Err(_) => LocalResponse::err(format!("packages.{operation}: serialization-failed")),
        },
        Err(error) => LocalResponse::err(format!(
            "packages.{operation}: {}",
            package_error_code(&error)
        )),
    }
}

pub(super) fn package_error_code(error: &PackageError) -> &'static str {
    match error {
        PackageError::TrustUnavailable => "trust-unavailable",
        PackageError::Invalid => "invalid-request",
        PackageError::Persistence => "persistence-failed",
        PackageError::Trust(TrustError::Expired) => "catalog-expired",
        PackageError::Trust(TrustError::Replay) => "replay-rejected",
        PackageError::Trust(TrustError::RevokedKey | TrustError::RevokedPackage) => "revoked",
        PackageError::Trust(_) => "trust-rejected",
        PackageError::Store(_) => "store-rejected",
    }
}

/// Диспатч `commands.<subop>` — обрабатывает register / unregister / list /
/// invoke, эмитит broadcast events где нужно.
pub(super) async fn handle_command_op(
    subop: &str,
    params: serde_json::Value,
    bus: &CommandBus,
    client_id: ClientId,
) -> LocalResponse {
    match subop {
        "register" => {
            let manifests = match params.get("commands") {
                Some(v) => match serde_json::from_value::<Vec<CommandManifest>>(v.clone()) {
                    Ok(m) => m,
                    Err(e) => {
                        return LocalResponse::err(format!(
                            "commands.register: invalid 'commands' array: {e}"
                        ));
                    }
                },
                None => return LocalResponse::err("commands.register: missing 'commands' array"),
            };
            bus.register(client_id, manifests).await;
            bus.broadcast_changed().await;
            LocalResponse::ok(serde_json::json!({ "ok": true }))
        }
        "unregister" => {
            let ids = match params.get("ids") {
                Some(v) => match serde_json::from_value::<Vec<String>>(v.clone()) {
                    Ok(v) => v,
                    Err(e) => {
                        return LocalResponse::err(format!(
                            "commands.unregister: invalid 'ids' array: {e}"
                        ));
                    }
                },
                None => return LocalResponse::err("commands.unregister: missing 'ids' array"),
            };
            bus.unregister(client_id, &ids).await;
            bus.broadcast_changed().await;
            LocalResponse::ok(serde_json::json!({ "ok": true }))
        }
        "list" => {
            let list = bus.list().await;
            LocalResponse::ok(serde_json::json!({ "commands": list }))
        }
        "invoke" => {
            let id = match params.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("commands.invoke: missing 'id'"),
            };
            let invoke_params = params
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            bus.broadcast_invoked(id, invoke_params);
            LocalResponse::ok(serde_json::json!({ "ok": true }))
        }
        other => LocalResponse::err(format!("commands.{other}: unknown sub-operation")),
    }
}

/// Dispatch `export.<subop>` (Phase 7).
///
/// Sub-operations:
///   - `export.list` → `{ converters: [...] }`
///   - `export.run { converter_id, format?, dest_dir }` → `{ files_written, bytes, errors }`
///
/// Read-only от ARK: fetch objects через `list_objects_by_type`, передаём в
/// converter, который пишет в dest_dir. Никаких writes в ARK.
pub(super) async fn handle_export_op(
    subop: &str,
    params: serde_json::Value,
    ark_host: &ArkHost,
) -> LocalResponse {
    match subop {
        "list" => {
            let converters = export::list_converters();
            LocalResponse::ok(serde_json::json!({ "converters": converters }))
        }
        "run" => {
            let converter_id = match params.get("converter_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("export.run: missing 'converter_id'"),
            };
            let dest_dir = match params.get("dest_dir").and_then(|v| v.as_str()) {
                Some(s) => std::path::PathBuf::from(s),
                None => return LocalResponse::err("export.run: missing 'dest_dir'"),
            };
            let converter = match export::find_converter(&converter_id) {
                Some(c) => c,
                None => {
                    return LocalResponse::err(format!(
                        "export.run: unknown converter '{converter_id}'"
                    ));
                }
            };
            let format = params
                .get("format")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| converter.default_format().to_string());
            if !converter.supported_formats().contains(&format.as_str()) {
                return LocalResponse::err(format!(
                    "export.run: format '{format}' not supported by '{converter_id}'"
                ));
            }

            // Ensure dest_dir exists.
            if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                return LocalResponse::err(format!("export.run: create dest_dir: {e}"));
            }

            // Fetch objects of converter's object_type через ark_host.
            let ark_resp = match ark_host
                .request(
                    "list_objects_by_type",
                    serde_json::json!({ "type_id": converter.object_type() }),
                )
                .await
            {
                Ok(r) => r,
                Err(e) => return LocalResponse::err(format!("export.run: ark_host: {e}")),
            };
            if !ark_resp.ok {
                return LocalResponse::err(format!(
                    "export.run: ark list_objects_by_type failed: {}",
                    ark_resp.error.unwrap_or_default()
                ));
            }
            let objects: Vec<ark_core::types::ArkObject> =
                match serde_json::from_value(ark_resp.data) {
                    Ok(v) => v,
                    Err(e) => {
                        return LocalResponse::err(format!(
                            "export.run: parse ArkObject array: {e}"
                        ));
                    }
                };

            let links_resp = match ark_host
                .request("list_object_links", serde_json::json!({}))
                .await
            {
                Ok(r) => r,
                Err(e) => return LocalResponse::err(format!("export.run: ark_host links: {e}")),
            };
            if !links_resp.ok {
                return LocalResponse::err(format!(
                    "export.run: ark list_object_links failed: {}",
                    links_resp.error.unwrap_or_default()
                ));
            }
            let links: Vec<ark_core::types::ObjectLink> =
                match serde_json::from_value(links_resp.data) {
                    Ok(v) => v,
                    Err(e) => {
                        return LocalResponse::err(format!(
                            "export.run: parse ObjectLink array: {e}"
                        ));
                    }
                };
            let envelopes = objects
                .into_iter()
                .map(|object| export::CanonicalEnvelope {
                    links: links
                        .iter()
                        .filter(|link| link.source_object_id == object.id)
                        .cloned()
                        .collect(),
                    object,
                })
                .collect::<Vec<_>>();
            let result = converter.convert_canonical(&envelopes, &format, &dest_dir);
            match serde_json::to_value(&result) {
                Ok(v) => LocalResponse::ok(v),
                Err(e) => LocalResponse::err(format!("export.run: serialize result: {e}")),
            }
        }
        other => LocalResponse::err(format!("export.{other}: unknown sub-operation")),
    }
}

/// Dispatch `arrancador.<subop>`.
///
/// Sub-operations (subagent A scope):
///   - `arrancador.scan` → сканирует Steam/Epic, persists through Game facade.
///   - `arrancador.launch { game_id }` → reads typed Game DTO, then spawns
///     процесс через `launcher::launch`.
///
/// `arrancador.rawg.*` и `arrancador.sqoba.*` будут добавлены subagent'ами B/C
/// в этот же match (один namespace, один диспатчер).
pub(super) async fn handle_arrancador_op(
    subop: &str,
    params: serde_json::Value,
    ark_host: &ArkHost,
) -> LocalResponse {
    let game_facade = arrancador::game_facade::GameFacade::new(ark_host);
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
                    let cfg = arrancador::config::load();
                    cfg.steam_library_override
                });
            let discovered = arrancador::scanner::scan_all(override_path.as_deref());

            // Read only canonical Games. Launcher/provider identity is owned by
            // the device-local Arrancador state, never by canonical props.
            let existing = match game_facade.objects().await {
                Ok(objects) => objects,
                Err(error) => {
                    return LocalResponse::err(format!("arrancador: game facade: {error}"));
                }
            };
            let mut cfg = arrancador::config::load();
            let mut added = 0u32;
            let mut updated = 0u32;
            let mut skipped = 0u32;
            let mut errors: Vec<String> = Vec::new();

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
                let upsert_obj = arrancador::game_facade::GameFacade::upsert_payload(
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
                            id,
                            arrancador::config::LocalGameState {
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
            if let Err(e) = arrancador::config::save(&cfg) {
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
            let save_paths: Vec<std::path::PathBuf> = params
                .get("save_paths")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::trim))
                        .filter(|s| !s.is_empty())
                        .map(std::path::PathBuf::from)
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
            let mut cfg = arrancador::config::load();
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
            let object = arrancador::game_facade::GameFacade::upsert_payload(
                existing_match,
                &id,
                &name,
                &props,
                &now,
            );
            match game_facade.upsert(object).await {
                Ok(_) => {
                    cfg.local_games.insert(
                        id.clone(),
                        arrancador::config::LocalGameState {
                            source: Some("manual".into()),
                            source_app_id: Some(source_app_id),
                            install_dir: exe_path.parent().map(|p| p.to_string_lossy().to_string()),
                            exe_path: Some(exe_path.to_string_lossy().to_string()),
                            save_paths: save_paths
                                .into_iter()
                                .map(|p| p.to_string_lossy().to_string())
                                .collect(),
                        },
                    );
                    if let Err(e) = arrancador::config::save(&cfg) {
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
            let local = arrancador::config::load()
                .local_games
                .get(&game.id)
                .cloned()
                .unwrap_or_default();
            let launch_game = arrancador::game_facade::GameFacade::launch_dto(&game);
            match arrancador::launcher::launch(&launch_game, &local) {
                Ok(result) => match serde_json::to_value(&result) {
                    Ok(v) => LocalResponse::ok(v),
                    Err(e) => LocalResponse::err(format!("arrancador.launch: serialize: {e}")),
                },
                Err(e) => LocalResponse::err(format!("arrancador.launch: {e}")),
            }
        }
        "config.get" => {
            let cfg = arrancador::config::load();
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
            let cfg = arrancador::config::load();
            LocalResponse::ok(
                serde_json::json!({ "configured": cfg.rawg_api_key.as_deref().is_some_and(|key| !key.is_empty()) }),
            )
        }
        "config.set_rawg_key" => {
            let key = params
                .get("key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let mut cfg = arrancador::config::load();
            cfg.rawg_api_key = key.filter(|s| !s.is_empty());
            match arrancador::config::save(&cfg) {
                Ok(()) => LocalResponse::ok(serde_json::json!({ "ok": true })),
                Err(e) => LocalResponse::err(format!("arrancador.config.set_rawg_key: {e}")),
            }
        }
        "rawg.search" => {
            let query = match params.get("query").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.rawg.search: missing 'query'"),
            };
            let cfg = arrancador::config::load();
            let api_key = match cfg.rawg_api_key.as_deref() {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return LocalResponse::err("RAWG API key not configured"),
            };
            match arrancador::rawg::search(&query, &api_key).await {
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
            let cfg = arrancador::config::load();
            let api_key = match cfg.rawg_api_key.as_deref() {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return LocalResponse::err("RAWG API key not configured"),
            };
            match arrancador::rawg::apply_to_game(&game_facade, &game_id, rawg_id, &api_key).await {
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
                arrancador::game_facade::GameFacade::sqoba_metadata(&game);
            match arrancador::sqoba::backup(&game_id, &game_name, manual_paths.as_deref()) {
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
            let backups = arrancador::sqoba::list_backups(&game_id);
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
            let path = match arrancador::sqoba::resolve_backup_path(game_id, &backup_id) {
                Some(p) => p,
                None => {
                    return LocalResponse::err(format!(
                        "arrancador.sqoba.restore: backup '{}' not found for game '{}'",
                        backup_id, game_id
                    ));
                }
            };
            match arrancador::sqoba::restore(&path) {
                Ok(r) => match serde_json::to_value(&r) {
                    Ok(mut v) => {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert("ok".into(), serde_json::Value::Bool(r.errors.is_empty()));
                        }
                        LocalResponse::ok(v)
                    }
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

pub(super) async fn handle_calculator_op(
    subop: &str,
    params: serde_json::Value,
    data_dir: &std::path::Path,
) -> LocalResponse {
    match subop {
        "evaluate" => {
            let query = match params.get("query").and_then(|value| value.as_str()) {
                Some(query) => query.to_string(),
                None => return LocalResponse::err("calculator.evaluate: missing 'query'"),
            };
            let normalized = calculator::normalize_query(&query);
            let rates =
                calculator::exchange_rates_for_query(&normalized.evaluation, data_dir).await;
            let expression = normalized.display_expression;
            let evaluation = normalized.evaluation;
            match tokio::task::spawn_blocking(move || {
                calculator::evaluate_preview_with_rates(&evaluation, rates)
            })
            .await
            {
                Ok(result) => LocalResponse::ok(serde_json::json!({
                    "result": result,
                    "expression": expression,
                })),
                Err(error) => LocalResponse::err(format!("calculator.evaluate: join: {error}")),
            }
        }
        other => LocalResponse::err(format!("calculator.{other}: unknown sub-operation")),
    }
}

/// Dispatch `app_index.<subop>` — App Launcher: search / launch / rescan.
///
/// Sub-operations:
///   - `app_index.search { query, limit? }` → `{ results: [{app, score}] }`
///   - `app_index.launch { id }` → `{ ok: true }`. Frecency tracking — TODO через ARK usage_event_obj.
///   - `app_index.rescan` → `{ added, updated, removed, total }`
pub(super) async fn handle_app_index_op(
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

pub(super) fn resolve_arrancador_manual_exec_path(
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

pub(super) fn app_icon_ref(app: &crate::app_index::App) -> Option<String> {
    app.icon_path
        .as_ref()
        .map(|_| format!("kosmos-icon://app/{}", app.id))
}

pub(super) fn app_index_entry_json(app: &crate::app_index::App) -> serde_json::Value {
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

/// Dispatch `file_index.<subop>` — host-local file search and settings.
pub(super) async fn handle_file_index_op(
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

pub(super) async fn send_message<S>(
    sink: &mut S,
    message: Message,
    shutdown: &WsShutdownHandle,
) -> Result<(), WsServerError>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    tokio::select! {
        _ = shutdown.cancelled() => Ok(()),
        result = tokio::time::timeout(WS_SEND_DEADLINE, sink.send(message)) => {
            match result {
                Ok(result) => result.map_err(WsServerError::from),
                Err(_) => Err(WsServerError::WebSocket(
                    tokio_tungstenite::tungstenite::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "WebSocket send timed out",
                    )),
                )),
            }
        }
    }
}

pub(super) async fn send_hello_error<S>(
    sink: &mut S,
    code: &str,
    message: &str,
    shutdown: &WsShutdownHandle,
) -> Result<(), WsServerError>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let payload = serde_json::to_string(&HelloErrorResponse {
        kind: "hello_error",
        code,
        message: message.to_string(),
    })?;
    send_message(sink, Message::Text(payload), shutdown).await?;
    send_message(sink, Message::Close(None), shutdown).await?;
    Ok(())
}
