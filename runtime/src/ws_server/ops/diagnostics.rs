use super::*;
#[allow(clippy::too_many_arguments)]
pub(in crate::ws_server) async fn build_diagnostics_snapshot(
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
            "mode": "in-process",
            "panics_recovered": ark_host.panic_count(),
        },
        "engine_supervisor": crate::engine_supervisor::diagnostics_snapshot(data_dir),
        "background_workers": {
            "app_index": app_index_background_worker,
            "db_backup": crate::db_backup::diagnostics_snapshot(),
        },
    })
}

pub(in crate::ws_server) fn observe_rpc_payload(
    rpc_diagnostics: &SharedRpcDiagnostics,
    operation: &str,
    started: std::time::Instant,
    payload: &str,
) {
    rpc_diagnostics.observe_response(operation, started.elapsed(), payload.len());
}
