use super::*;
pub(super) async fn dispatch_operation(
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
    snapshots: Arc<crate::package_worker_broker::SnapshotRegistry>,
    grants: Arc<GrantAuthorityRegistry>,
    desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    manager_state: ManagerState,
    correlation_id: Arc<String>,
    client_id: ClientId,
) -> Value {
    let operation = request.operation.as_str().to_owned();
    let req_id = request.request_id.clone();
    let started = Instant::now();
    #[cfg(test)]
    if operation == "test.stall" {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
    let response = if operation.starts_with("package.snapshot.") || operation.starts_with("grant.")
    {
        dispatch_special::dispatch_special(
            request,
            ark_host.clone(),
            command_bus.clone(),
            pomodoro_host.clone(),
            dictation_host.clone(),
            app_index.clone(),
            file_index.clone(),
            agents.clone(),
            agents_data_dir.clone(),
            agent_events.clone(),
            usage_diagnostics.clone(),
            rpc_diagnostics.clone(),
            protocol_usage.clone(),
            package_service.clone(),
            snapshots.clone(),
            grants.clone(),
            desktop_authority.clone(),
            manager_state.clone(),
            correlation_id.clone(),
            client_id,
        )
        .await
    } else {
        dispatch_standard::dispatch_standard(
            request,
            ark_host,
            command_bus,
            pomodoro_host,
            dictation_host,
            app_index,
            file_index,
            agents,
            agents_data_dir,
            agent_events,
            usage_diagnostics,
            rpc_diagnostics.clone(),
            protocol_usage,
            package_service,
            snapshots,
            grants,
            desktop_authority,
            manager_state,
            correlation_id,
            client_id,
        )
        .await
    };
    let mut envelope = serde_json::Map::new();
    if let Some(id) = req_id {
        envelope.insert("id".into(), Value::String(id));
    }
    envelope.insert("ok".into(), Value::Bool(response.ok));
    envelope.insert("data".into(), response.data);
    if let Some(error) = response.error {
        envelope.insert("error".into(), Value::String(error));
    }
    let payload = Value::Object(envelope);
    observe_rpc_payload(&rpc_diagnostics, &operation, started, &payload.to_string());
    payload
}
