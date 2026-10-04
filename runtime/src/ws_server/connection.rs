use super::connection_loop::run as run_connection_loop;
use super::connection_types::ConnectionLoopArgs;
use super::ops::{send_hello_error, send_message};
use super::*;

struct WsConnectionOwnerGuard {
    dispatcher: crate::engine_dispatch::EngineDispatcher,
    lease: Option<crate::engine_dispatch::OwnerLease>,
}

impl WsConnectionOwnerGuard {
    fn new(
        dispatcher: crate::engine_dispatch::EngineDispatcher,
        lease: crate::engine_dispatch::OwnerLease,
    ) -> Self {
        Self {
            dispatcher,
            lease: Some(lease),
        }
    }

    fn id(&self) -> u64 {
        self.lease.as_ref().expect("owner guard lease").id()
    }
}

impl Drop for WsConnectionOwnerGuard {
    fn drop(&mut self) {
        let Some(lease) = self.lease.take() else {
            return;
        };
        let dispatcher = self.dispatcher.clone();
        let _ = catch_unwind(AssertUnwindSafe(|| {
            dispatcher.cleanup_connection_sync(&lease);
        }));
        lease.release();
    }
}

pub(super) async fn handle_connection(
    stream: tokio::net::TcpStream,
    ark_host: Arc<ArkHost>,
    expected_token: Arc<String>,
    command_bus: Arc<CommandBus>,
    pomodoro_host: Arc<PomodoroHost>,
    dictation_host: Arc<DictationHost>,
    agent_events: tokio::sync::broadcast::Sender<serde_json::Value>,
    protocol_usage: Arc<ProtocolUsageStore>,
    correlation_id: Arc<String>,
    dispatcher: crate::engine_dispatch::EngineDispatcher,
    owner_lease: crate::engine_dispatch::OwnerLease,
    desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    snapshots: Arc<crate::package_worker_broker::SnapshotRegistry>,
    grants: Arc<GrantAuthorityRegistry>,
    shutdown: WsShutdownHandle,
) -> Result<(), WsServerError> {
    let owner_guard = WsConnectionOwnerGuard::new(dispatcher.clone(), owner_lease);
    let client_id = owner_guard.id();
    let ws_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MAX_WS_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_WS_MESSAGE_BYTES));
    let ws = tokio::select! {
        _ = shutdown.cancelled() => return Ok(()),
        result = tokio_tungstenite::accept_async_with_config(stream, Some(ws_config)) => result?,
    };
    let (mut sink, mut stream) = ws.split();

    // 1. Hello.
    let hello_msg = match tokio::select! {
        _ = shutdown.cancelled() => return Ok(()),
        frame = stream.next() => frame,
    } {
        Some(Ok(Message::Text(text))) => text,
        Some(Ok(Message::Binary(_))) | Some(Ok(_)) => {
            send_hello_error(
                &mut sink,
                handshake_errors::MALFORMED_HELLO,
                "first frame must be text JSON hello",
                &shutdown,
            )
            .await?;
            return Ok(());
        }
        Some(Err(e)) => return Err(e.into()),
        None => return Ok(()),
    };

    let hello: HelloMessage = match serde_json::from_str(&hello_msg) {
        Ok(h) => h,
        Err(e) => {
            send_hello_error(
                &mut sink,
                handshake_errors::MALFORMED_HELLO,
                &format!("hello JSON parse failed: {e}"),
                &shutdown,
            )
            .await?;
            return Ok(());
        }
    };

    match validate_hello(&hello, &expected_token) {
        HelloOutcome::Reject { code, message } => {
            send_hello_error(&mut sink, code, &message, &shutdown).await?;
            return Ok(());
        }
        HelloOutcome::Accept {
            compatibility,
            transport,
        } => {
            if let Err(error) = protocol_usage.record(
                transport,
                hello.client_class.as_deref(),
                hello.client_version.as_deref(),
            ) {
                tracing::warn!(error = %error, "protocol usage persistence failed");
            }
            let response = HelloOkResponse {
                kind: "hello_ok",
                api_version: API_VERSION,
                compatibility: compatibility_label(&compatibility),
            };
            let payload = serde_json::to_string(&response)?;
            send_message(&mut sink, Message::Text(payload.into()), &shutdown).await?;
        }
    }

    run_connection_loop(ConnectionLoopArgs {
        sink,
        stream,
        command_bus,
        pomodoro_host,
        dictation_host,
        ark_host,
        agent_events,
        correlation_id,
        client_id,
        desktop_authority,
        snapshots,
        grants,
        shutdown,
        dispatcher,
        hello,
    })
    .await
}
