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
            // WS: the bound desktop shell. HTTP `/v1/rpc`: the authenticated
            // Manager channel — the gpui Manager cannot take a desktop lease
            // (leases are minted for the Electron shell PID via the control
            // socket), but its channel is already the lock-token credential.
            if !(client.desktop_authorized || client.manager_channel) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The UAC grant is limited to the two trusted channels — the bound WS
    /// desktop and the authenticated Manager HTTP channel. A bare client is
    /// denied *before* `enable()` could spawn the elevated install (the
    /// allow path itself is untestable: it would trigger a real UAC prompt).
    #[tokio::test]
    async fn privileged_enable_requires_a_trusted_client() {
        let response = handle_system_op(
            "privileged.enable",
            serde_json::Value::Null,
            &crate::engine_dispatch::DispatchClient::default(),
        )
        .await;
        assert!(!response.ok);
        assert_eq!(
            response.error.as_deref(),
            Some("system.privileged.enable: unauthorized client")
        );
    }
}
