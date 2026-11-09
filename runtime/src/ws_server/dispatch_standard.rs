use super::*;

#[allow(
    clippy::too_many_arguments,
    reason = "dispatch wiring shares the server's host handles"
)]
pub(super) async fn dispatch_standard(
    request: crate::engine_dispatch::DispatchRequest,
    ark_host: Arc<ArkHost>,
    command_bus: Arc<CommandBus>,
    dictation_host: Arc<DictationHost>,
    app_index: Arc<AppIndex>,
    file_index: Arc<FileIndex>,
    usage_diagnostics: Arc<UsageTrackerDiagnosticsState>,
    rpc_diagnostics: SharedRpcDiagnostics,
    protocol_usage: Arc<ProtocolUsageStore>,
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
                &correlation_id,
                manager_state.data_dir(),
            )
            .await,
        )
    } else if crate::appearance::is_appearance_operation(&operation) {
        let appearance = manager_state.appearance();
        let result = match operation.as_str() {
            "appearance.get" => appearance.get(&params, &client).await,
            "appearance.set" => appearance.set(&params, &client).await,
            other => {
                return LocalResponse::err(format!("unknown appearance operation {other}"));
            }
        };
        result
            .map(LocalResponse::ok)
            .unwrap_or_else(LocalResponse::err)
    } else if let Some(rest) = operation.strip_prefix("dictation.") {
        {
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
                            "enabled": state.get(
                                "enabled"
                            ).and_then(Value::as_bool).unwrap_or(false),
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
    } else if let Some(rest) = operation.strip_prefix("plugins.") {
        let mut result = crate::plugins::handle_plugin_op(rest, params).await;
        if rest == "settings.set" && result.ok {
            // The settings are stored; the plugin's own operations apply them
            // to whatever is running, so the page and the app never disagree.
            if let Err(error) = apply_plugin_settings(&result.data, &dictation_host).await {
                result.ok = false;
                result.error = Some(error);
            }
        }
        LocalResponse {
            ok: result.ok,
            data: result.data,
            error: result.error,
        }
    } else if crate::hosts::is_hosts_operation(&operation) {
        crate::hosts::sync(&params)
            .map(LocalResponse::ok)
            .unwrap_or_else(LocalResponse::err)
    } else if let Some(rest) = operation.strip_prefix("updater.") {
        let result =
            crate::updater::handle_updater_op(rest, params, &manager_state.updater()).await;
        LocalResponse {
            ok: result.ok,
            data: result.data,
            error: result.error,
        }
    } else if let Some(rest) = operation.strip_prefix("app_index.") {
        handle_app_index_op(rest, params, &app_index).await
    } else if let Some(rest) = operation.strip_prefix("file_index.") {
        handle_file_index_op(rest, params, &file_index).await
    } else if let Some(rest) = operation.strip_prefix("system.") {
        handle_system_op(rest, params, &client).await
    } else if let Some(rest) = operation.strip_prefix("commands.") {
        handle_command_op(rest, params, &command_bus, connection_id).await
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
            other => LocalResponse::err(format!("unknown engine operation {other}")),
        }
    } else if let Some(rest) = operation.strip_prefix("manager.") {
        let result = match rest {
            "data.summary" => manager_state
                .data_summary(&ark_host)
                .await
                .map(LocalResponse::ok),
            "data.storage" => manager_state
                .data_storage(manager_state.data_dir())
                .await
                .map(LocalResponse::ok),
            "data.quarantine.clear" => manager_state
                .clear_legacy_quarantine()
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
                    .diagnostics_snapshot(&rpc_diagnostics, &usage_diagnostics, &protocol_usage)
                    .await,
            )),
            "diagnostics.log_tail" => Ok(LocalResponse::ok(
                manager_state.log_tail(&rpc_diagnostics).await,
            )),
            "diagnostics.support_bundle.create" => manager_state
                .create_bundle(
                    manager_state
                        .diagnostics_snapshot(&rpc_diagnostics, &usage_diagnostics, &protocol_usage)
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
            _ if rest.starts_with("db_backups.") => {
                db_backup::handle_manager_op(rest, &ark_host, manager_state.data_dir(), &params)
                    .await
                    .map(LocalResponse::ok)
            }
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

/// Run the Engine operations a plugin's settings name (`plugins.settings.set`
/// answers with them). A plugin tells the Engine *what* applies it; the Engine
/// still decides *which* operations exist.
async fn apply_plugin_settings(
    data: &Value,
    dictation_host: &Arc<DictationHost>,
) -> Result<(), String> {
    let Some(applies) = data.get("applies").and_then(Value::as_array) else {
        return Ok(());
    };
    for apply in applies {
        let op = apply.get("op").and_then(Value::as_str).unwrap_or_default();
        let params = apply.get("params").cloned().unwrap_or(Value::Null);
        let Some(rest) = op.strip_prefix("dictation.") else {
            return Err(format!("движок не умеет применять настройку через «{op}»"));
        };
        let result = handle_dictation_op(rest, params, dictation_host).await;
        if !result.ok {
            return Err(result
                .error
                .unwrap_or_else(|| format!("{op}: настройка не применилась")));
        }
    }
    Ok(())
}
