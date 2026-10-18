use super::*;

#[cfg(test)]
mod listener_tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    struct RejectPing;

    #[async_trait::async_trait]
    impl SessionOutboundGuard for RejectPing {
        async fn allow(&self, message: &ark_sync::protocol::LanSyncMessage) -> bool {
            !matches!(message, ark_sync::protocol::LanSyncMessage::Ping { .. })
        }
    }

    #[tokio::test]
    async fn listener_accepts_a_websocket_and_receives_a_text_frame() {
        let listener = Listener::bind("127.0.0.1:0").await.expect("listener binds");
        let address = listener
            .listener
            .local_addr()
            .expect("listener address is available");
        let client = tokio::spawn(async move {
            let (mut socket, _) = connect_async(format!("ws://{address}"))
                .await
                .expect("client handshake succeeds");
            socket
                .send(Message::Text("hello".into()))
                .await
                .expect("client sends text");
        });
        let (mut socket, peer_address) = listener.accept().await.expect("server accepts client");
        assert!(peer_address.ip().is_loopback());
        assert!(matches!(
            socket.next().await,
            Some(Ok(Message::Text(text))) if text == "hello"
        ));
        client.await.expect("client task completes");
    }

    #[tokio::test]
    async fn session_io_translates_frames_and_joins_after_close() {
        let listener = Listener::bind("127.0.0.1:0").await.expect("listener binds");
        let address = listener
            .listener
            .local_addr()
            .expect("listener address is available");
        let client = tokio::spawn(async move {
            let (mut socket, _) = connect_async(format!("ws://{address}"))
                .await
                .expect("client connects");
            socket
                .send(Message::Text(
                    serde_json::to_string(&ark_sync::protocol::LanSyncMessage::Ping { ts: 42 })
                        .expect("ping serializes")
                        .into(),
                ))
                .await
                .expect("client sends ping");
            let Some(Ok(Message::Text(text))) = socket.next().await else {
                panic!("client receives server protocol message");
            };
            assert!(matches!(
                serde_json::from_str::<ark_sync::protocol::LanSyncMessage>(&text)
                    .expect("server message deserializes"),
                ark_sync::protocol::LanSyncMessage::Pong { ts: 42 }
            ));
            socket
                .send(Message::Text("malformed".into()))
                .await
                .expect("client sends malformed protocol message");
            socket.close(None).await.expect("client closes websocket");
        });

        let (websocket, _) = listener.accept().await.expect("server accepts client");
        let mut session = session_io(websocket).expect("session transport starts");
        assert!(matches!(
            session.receive().await,
            Some(SessionIoEvent::Message(message))
                if matches!(*message, ark_sync::protocol::LanSyncMessage::Ping { ts: 42 })
        ));
        session
            .send(ark_sync::protocol::LanSyncMessage::Pong { ts: 42 })
            .await
            .expect("session sends protocol message");
        assert!(matches!(
            session.receive().await,
            Some(SessionIoEvent::MalformedMessage)
        ));
        assert!(matches!(
            session.receive().await,
            Some(SessionIoEvent::Closed)
        ));
        client.await.expect("client completes");
        session.finish().await.expect("transport worker joins");
    }

    #[tokio::test]
    async fn session_io_rechecks_outbound_messages_at_the_write_boundary() {
        let listener = Listener::bind("127.0.0.1:0").await.expect("listener binds");
        let address = listener
            .listener
            .local_addr()
            .expect("listener address is available");
        let client = tokio::spawn(async move {
            let (mut socket, _) = connect_async(format!("ws://{address}"))
                .await
                .expect("client connects");
            let Some(Ok(Message::Text(text))) = socket.next().await else {
                panic!("authorized protocol message reaches the socket");
            };
            assert!(matches!(
                serde_json::from_str::<ark_sync::protocol::LanSyncMessage>(&text)
                    .expect("message deserializes"),
                ark_sync::protocol::LanSyncMessage::Pong { ts: 7 }
            ));
            socket.close(None).await.expect("client closes websocket");
        });

        let (websocket, _) = listener.accept().await.expect("server accepts client");
        let guard = std::sync::Arc::new(RejectPing);
        let mut session =
            session_io_with_guard(websocket, Some(guard)).expect("session transport starts");
        session
            .send(ark_sync::protocol::LanSyncMessage::Ping { ts: 1 })
            .await
            .expect("queued ping awaits write-boundary policy");
        session
            .send(ark_sync::protocol::LanSyncMessage::Pong { ts: 7 })
            .await
            .expect("pong is allowed");
        assert!(matches!(
            session.receive().await,
            Some(SessionIoEvent::Closed)
        ));
        client.await.expect("client completes");
        session.finish().await.expect("transport worker joins");
    }
}

#[cfg(test)]
mod race_connect_tests {
    use super::race_connect;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    #[tokio::test]
    async fn connects_to_the_first_available_websocket_address() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener binds");
        let address = listener
            .local_addr()
            .expect("test listener address loads")
            .to_string();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("client connects");
            accept_async(stream)
                .await
                .expect("websocket handshake succeeds")
        });

        let connected = race_connect(&["127.0.0.1:0".into(), address.clone()])
            .await
            .expect("available websocket address connects");
        assert_eq!(connected.1, address);
        drop(connected);
        drop(server.await.expect("websocket server finishes"));
    }
}
