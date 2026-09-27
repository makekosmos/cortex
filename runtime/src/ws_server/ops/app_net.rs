use super::*;

/// `bookMetadata.*` / `images.*` — Engine-owned app network surface
/// (KOS-152). Ops fetch on pinned HTTPS origins, cap bodies, and return
/// normalized data; permissioning for packaged launches lives in
/// `authorize_app_request` via `app_network_operation_scope`.
pub(in crate::ws_server) async fn handle_app_network_op(
    operation: &str,
    params: &Value,
) -> LocalResponse {
    let ctx = match crate::app_network::AppNetworkCtx::engine() {
        Ok(ctx) => ctx,
        Err(error) => return LocalResponse::err(error),
    };
    match crate::app_network::handle(operation, params, &ctx).await {
        Ok(value) => LocalResponse::ok(value),
        Err(error) => LocalResponse::err(error),
    }
}
