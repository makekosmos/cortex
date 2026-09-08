use super::*;

fn replication_requires_authority(rest: &str) -> bool {
    rest.starts_with("replication_")
}

pub(super) async fn dispatch_standard(
    request: crate::engine_dispatch::DispatchRequest,
    ark_host: Arc<ArkHost>,
    command_bus: Arc<CommandBus>,
    pomodoro_host: Arc<PomodoroHost>,
    dictation_host: Arc<DictationHost>,
    app_index: Arc<AppIndex>,
    file_index: Arc<FileIndex>,
    agents: Arc<tokio::sync::OnceCell<Arc<AgentsService>>>,
    agents_data_dir: Arc<std::path::PathBuf>,
    agent_events: tokio::sync::broadcast::Sender<serde_json::Value>,
    usage_diagnostics: Arc<UsageTrackerDiagnosticsState>,
    rpc_diagnostics: SharedRpcDiagnostics,
    protocol_usage: Arc<ProtocolUsageStore>,
    package_service: Arc<PackageService>,
    store_catalog: Option<Arc<StoreCatalogService>>,
    _snapshots: Arc<crate::package_worker_broker::SnapshotRegistry>,
    _grants: Arc<GrantAuthorityRegistry>,
    _desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    manager_state: ManagerState,
    correlation_id: Arc<String>,
    client_id: ClientId,
) -> LocalResponse {
    let operation = request.operation.as_str().to_owned();
    let params = request.params;
    let client = request.client;
    let connection_id = client_id;
    let response = if operation == "diagnostics.snapshot" {
        LocalResponse::ok(
            build_diagnostics_snapshot(
                &ark_host,
                &rpc_diagnostics,
                &file_index,
                &usage_diagnostics,
                &app_index,
                &protocol_usage,
                &package_service,
                &correlation_id,
                &agents_data_dir,
            )
            .await,
        )
    } else if let Some(rest) = operation.strip_prefix("agents.") {
        match agents
            .get_or_try_init(|| async {
                AgentsService::new_with_events(&agents_data_dir, agent_events.clone())
            })
            .await
        {
            Ok(service) => service
                .handle(rest, params, client)
                .await
                .map(LocalResponse::ok)
                .unwrap_or_else(LocalResponse::err),
            Err(error) => LocalResponse::err(error.clone()),
        }
    } else if let Some(rest) = operation.strip_prefix("dictation.") {
        {
            if rest == "trigger" {
                return match package_service
                    .invoke_worker_operation("dictation.trigger", params)
                    .await
                {
                    Ok(value) => LocalResponse::ok(value),
                    Err(error) => LocalResponse::err(format!(
                        "dictation.trigger: {}",
                        package_error_code(&error)
                    )),
                };
            }
            let result = if rest == "lifecycle.set_autostart" {
                manager_state
                    .set_autostart(
                        params
                            .get("enabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    )
                    .map(|state| {
                        crate::dictation::DictationResponse::ok(serde_json::json!({
                            "enabled": state.get("enabled").and_then(Value::as_bool).unwrap_or(false),
                        }))
                    })
                    .unwrap_or_else(crate::dictation::DictationResponse::err)
            } else {
                handle_dictation_op(rest, params, &dictation_host).await
            };
            LocalResponse {
                ok: result.ok,
                data: result.data,
                error: result.error,
            }
        }
    } else if let Some(rest) = operation.strip_prefix("pomodoro.") {
        {
            let result = handle_pomodoro_op(rest, params, &pomodoro_host).await;
            LocalResponse {
                ok: result.ok,
                data: result.data,
                error: result.error,
            }
        }
    } else if let Some(rest) = operation.strip_prefix("export.") {
        handle_export_op(rest, params, &ark_host).await
    } else if let Some(rest) = operation.strip_prefix("arrancador.") {
        handle_games_op(rest, params, &package_service).await
    } else if let Some(rest) = operation.strip_prefix("games.") {
        handle_games_op(rest, params, &package_service).await
    } else if let Some(rest) = operation.strip_prefix("focus.") {
        {
            let result = handle_focus_op(rest, params, &ark_host).await;
            LocalResponse {
                ok: result.ok,
                data: result.data,
                error: result.error,
            }
        }
    } else if let Some(rest) = operation.strip_prefix("app_index.") {
        handle_app_index_op(rest, params, &app_index).await
    } else if let Some(rest) = operation.strip_prefix("calculator.") {
        handle_calculator_op(rest, params, &agents_data_dir).await
    } else if let Some(rest) = operation.strip_prefix("integrations.") {
        if replication_requires_authority(rest) && !client.desktop_authorized {
            return LocalResponse::err("integration replication authority denied");
        }
        integrations::handle_operation(rest, params, &ark_host, &agents_data_dir, &package_service)
            .await
            .map(LocalResponse::ok)
            .unwrap_or_else(LocalResponse::err)
    } else if let Some(rest) = operation.strip_prefix("file_index.") {
        handle_file_index_op(rest, params, &file_index).await
    } else if let Some(rest) = operation.strip_prefix("commands.") {
        handle_command_op(rest, params, &command_bus, connection_id).await
    } else if let Some(rest) = operation.strip_prefix("store.") {
        handle_store_op(rest, params, &package_service, store_catalog.as_deref()).await
    } else if let Some(rest) = operation.strip_prefix("packages.") {
        handle_package_op(rest, params, &package_service).await
    } else if matches!(
        operation.as_str(),
        "engine.settings.get"
            | "engine.settings.set"
            | "engine.autostart.get"
            | "engine.autostart.set"
    ) {
        match operation.as_str() {
            "engine.settings.get" => manager_state
                .settings()
                .map(LocalResponse::ok)
                .unwrap_or_else(LocalResponse::err),
            "engine.settings.set" => manager_state
                .set_settings_patch(
                    params
                        .get("warm_timeout_seconds")
                        .and_then(Value::as_u64)
                        .or_else(|| {
                            params
                                .pointer("/desktop_host/warm_timeout_seconds")
                                .and_then(Value::as_u64)
                        }),
                    params
                        .get("usage_tracker_enabled")
                        .and_then(Value::as_bool)
                        .or_else(|| {
                            params
                                .pointer("/usage_tracker/enabled")
                                .and_then(Value::as_bool)
                        }),
                )
                .map(LocalResponse::ok)
                .unwrap_or_else(LocalResponse::err),
            "engine.autostart.get" => LocalResponse::ok(manager_state.autostart()),
            "engine.autostart.set" => manager_state
                .set_autostart(
                    params
                        .get("enabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )
                .map(LocalResponse::ok)
                .unwrap_or_else(LocalResponse::err),
            _ => unreachable!(),
        }
    } else if let Some(rest) = operation.strip_prefix("manager.") {
        let result = match rest {
            "data.summary" => manager_state
                .data_summary(&ark_host, package_service.storage_root())
                .await
                .map(LocalResponse::ok),
            "data.types" => manager_state
                .data_types(&ark_host)
                .await
                .map(LocalResponse::ok),
            "data.list" => manager_state
                .data_list(&ark_host, &params)
                .await
                .map(LocalResponse::ok),
            "data.search" => manager_state
                .data_search(&ark_host, &params)
                .await
                .map(LocalResponse::ok),
            "diagnostics.snapshot" => Ok(LocalResponse::ok(
                manager_state
                    .diagnostics_snapshot(
                        &rpc_diagnostics,
                        &usage_diagnostics,
                        &protocol_usage,
                        &package_service,
                    )
                    .await,
            )),
            "diagnostics.log_tail" => Ok(LocalResponse::ok(
                manager_state.log_tail(&rpc_diagnostics).await,
            )),
            "diagnostics.support_bundle.create" => manager_state
                .create_bundle(
                    manager_state
                        .diagnostics_snapshot(
                            &rpc_diagnostics,
                            &usage_diagnostics,
                            &protocol_usage,
                            &package_service,
                        )
                        .await,
                    manager_state.log_tail(&rpc_diagnostics).await,
                )
                .await
                .map(LocalResponse::ok),
            "diagnostics.support_bundle.save" => manager_state
                .save_bundle(
                    params
                        .get("handle")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    params
                        .get("destination")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )
                .await
                .map(LocalResponse::ok),
            "diagnostics.support_bundle.cancel" => manager_state
                .cancel_bundle(
                    params
                        .get("handle")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )
                .await
                .map(LocalResponse::ok),
            _ => Err(format!("manager.{rest}: unknown-operation")),
        };
        result.unwrap_or_else(LocalResponse::err)
    } else {
        match ark_host.request(&operation, params).await {
            Ok(result) => LocalResponse {
                ok: result.ok,
                data: result.data,
                error: result.error,
            },
            Err(error) => LocalResponse::err(format!("ark_host: {error}")),
        }
    };
    response
}

#[cfg(test)]
mod tests {
    use super::replication_requires_authority;

    #[test]
    fn every_replication_operation_requires_desktop_authority() {
        for operation in [
            "replication_acquire_refresh_lease",
            "replication_send_signed_sync",
            "replication_publish_credential_envelope_v2",
        ] {
            assert!(replication_requires_authority(operation));
        }
        assert!(!replication_requires_authority("list"));
    }
}
