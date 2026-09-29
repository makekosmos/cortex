use super::*;

/// `system.*` ops — Engine-owned system controls.
///
/// `system.privileged.status` — service + pipe state, user-mode.
/// `system.privileged.enable` — the one-time grant: spawns
/// `mundus-engine privileged install` via the `runas` verb (one UAC prompt),
/// waits for the elevated process, then reports the resulting status.
/// Requires `desktop_authorized` — it can trigger a UAC prompt and a machine
/// state change.
pub(in crate::ws_server) async fn handle_system_op(
    rest: &str,
    _params: Value,
    client: &crate::engine_dispatch::DispatchClient,
) -> LocalResponse {
    match rest {
        "privileged.status" => {
            let status = crate::privileged::client::status();
            match serde_json::to_value(&status) {
                Ok(v) => LocalResponse::ok(v),
                Err(e) => LocalResponse::err(format!("system.privileged.status: {e}")),
            }
        }
        "privileged.enable" => {
            if !client.desktop_authorized {
                return LocalResponse::err("system.privileged.enable: unauthorized client");
            }
            match tokio::task::spawn_blocking(crate::privileged::client::enable).await {
                Ok(Ok(status)) => match serde_json::to_value(&status) {
                    Ok(v) => LocalResponse::ok(v),
                    Err(e) => LocalResponse::err(format!("system.privileged.enable: {e}")),
                },
                Ok(Err(e)) => LocalResponse::err(format!("system.privileged.enable: {e}")),
                Err(e) => LocalResponse::err(format!("system.privileged.enable: {e}")),
            }
        }
        other => LocalResponse::err(format!("system.{other}: unknown-operation")),
    }
}
