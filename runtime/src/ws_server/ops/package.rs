use super::*;
pub(in crate::ws_server) async fn handle_package_op(
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
            match package_blocking({
                let service = service.clone();
                let query_kind = kind.clone();
                move || service.list_filtered(query_kind)
            })
            .await
            {
                Ok(list) => {
                    let catalog = service.catalog_packages(kind.as_ref()).unwrap_or_default();
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
        "catalog_status" => LocalResponse::ok(serde_json::json!({
            "catalog": service.catalog_summary(),
            "fault": service.catalog_fault(),
        })),
        "verify_replacement" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(Value::as_str)
            else {
                return LocalResponse::err("packages.verify_replacement: invalid-request");
            };
            let Some(version) = params.get("version").and_then(Value::as_str) else {
                return LocalResponse::err("packages.verify_replacement: invalid-request");
            };
            let Some(hash) = params.get("hash").and_then(Value::as_str) else {
                return LocalResponse::err("packages.verify_replacement: invalid-request");
            };
            let Some(catalog_sequence) = params.get("catalog_sequence").and_then(Value::as_u64)
            else {
                return LocalResponse::err("packages.verify_replacement: invalid-request");
            };
            package_response(
                subop,
                service.verify_installed_app(id, version, hash, catalog_sequence),
            )
        }
        "disclosure" => match package_id_version(subop, &params) {
            Ok((id, version)) => package_response(subop, service.disclosure(id, version)),
            Err(response) => return response,
        },
        "refresh_catalog" => package_response(subop, service.refresh_catalog().await),
        "revoke_legacy_grants" => {
            let Some(source_ids) = params.get("source_ids").and_then(Value::as_array) else {
                return LocalResponse::err("packages.revoke_legacy_grants: invalid-request");
            };
            let Some(source_ids) = source_ids
                .iter()
                .map(Value::as_str)
                .collect::<Option<Vec<_>>>()
            else {
                return LocalResponse::err("packages.revoke_legacy_grants: invalid-request");
            };
            let source_ids = source_ids
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            package_response(
                subop,
                service
                    .revoke_legacy_grants_transaction(&source_ids)
                    .map(|result| {
                        serde_json::json!({
                            "revoked": result.revoked,
                            "transaction_token": result.transaction_token,
                        })
                    }),
            )
        }
        "restore_migration_snapshot" => restore_migration_snapshot(subop, params, service),
        "commit_legacy_grants" => {
            let Some(token) = params.get("transaction_token").and_then(Value::as_str) else {
                return LocalResponse::err("packages.commit_legacy_grants: invalid-request");
            };
            package_response(
                subop,
                service
                    .commit_legacy_grants(token)
                    .map(|()| serde_json::json!({ "committed": true })),
            )
        }
        "validate_legacy_grants" => {
            let Some(source_ids) = params.get("source_ids").and_then(Value::as_array) else {
                return LocalResponse::err("packages.validate_legacy_grants: invalid-request");
            };
            let Some(source_ids) = source_ids
                .iter()
                .map(Value::as_str)
                .collect::<Option<Vec<_>>>()
            else {
                return LocalResponse::err("packages.validate_legacy_grants: invalid-request");
            };
            let source_ids = source_ids
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            package_response(
                subop,
                service
                    .validate_legacy_grants(&source_ids)
                    .map(|()| serde_json::json!({ "valid": true })),
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
        "install_development" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.install_development: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.install_development: invalid-request");
            };
            let Some(archive_path) = params
                .get("archive_path")
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.install_development: invalid-request");
            };
            let result = service.install_development_app_from_path(id, version, archive_path);
            if let Err(error) = &result {
                tracing::warn!(%id, %version, error = ?error, "development package install failed");
            }
            package_response(subop, result)
        }
        // `open` mints (or reuses) a launch lease for an App-kind package —
        // the same machinery as POST /v1/apps/launch, exposed to the Manager
        // over the shared dispatch channel. Only installed, enabled,
        // non-revoked app packages resolve; every other answer is typed.
        "open" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(Value::as_str)
            else {
                return LocalResponse::err("packages.open: invalid-request");
            };
            let version = params.get("version").and_then(Value::as_str);
            match service.open_app(id, version).await {
                Ok(data) => LocalResponse::ok(data),
                Err(error) => LocalResponse::err(format!("packages.open: {}", error.code())),
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
        "bridge_config" => match package_id_version(subop, &params) {
            Ok((id, version)) => package_response(subop, service.bridge_config(id, version)),
            Err(response) => return response,
        },
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
