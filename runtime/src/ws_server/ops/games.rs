use super::*;

/// Compatibility route: policy and domain logic live in the installed Arcadia worker.
pub(in crate::ws_server) async fn handle_games_op(
    subop: &str,
    params: serde_json::Value,
    packages: &PackageService,
) -> LocalResponse {
    match packages
        .invoke_worker_operation(&format!("games.{subop}"), params)
        .await
    {
        Ok(value) => LocalResponse::ok(value),
        Err(error) => LocalResponse::err(format!("games.{subop}: {error}")),
    }
}
