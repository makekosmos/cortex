use super::*;

pub(super) async fn dispatch_operation(
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
) -> Value {
    let operation = request.operation.as_str().to_owned();
    let req_id = request.request_id.clone();
    let started = Instant::now();
    #[cfg(test)]
    if operation == "test.stall" {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
    let response = dispatch_standard::dispatch_standard(
        request,
        ark_host,
        command_bus,
        dictation_host,
        app_index,
        file_index,
        usage_diagnostics,
        rpc_diagnostics.clone(),
        protocol_usage,
        manager_state,
        correlation_id,
        client_id,
    )
    .await;
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
