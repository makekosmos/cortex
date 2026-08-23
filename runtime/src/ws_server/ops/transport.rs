use super::*;
pub(in crate::ws_server) async fn send_message<S>(
    sink: &mut S,
    message: Message,
    shutdown: &WsShutdownHandle,
) -> Result<(), WsServerError>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    tokio::select! {
        _ = shutdown.cancelled() => Ok(()),
        result = tokio::time::timeout(WS_SEND_DEADLINE, sink.send(message)) => {
            match result {
                Ok(result) => result.map_err(WsServerError::from),
                Err(_) => Err(WsServerError::WebSocket(
                    tokio_tungstenite::tungstenite::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "WebSocket send timed out",
                    )),
                )),
            }
        }
    }
}

pub(in crate::ws_server) async fn send_hello_error<S>(
    sink: &mut S,
    code: &str,
    message: &str,
    shutdown: &WsShutdownHandle,
) -> Result<(), WsServerError>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let payload = serde_json::to_string(&HelloErrorResponse {
        kind: "hello_error",
        code,
        message: message.to_string(),
    })?;
    send_message(sink, Message::Text(payload), shutdown).await?;
    send_message(sink, Message::Close(None), shutdown).await?;
    Ok(())
}
