use super::*;
#[tokio::test]
async fn production_ws_shutdown_reaps_authenticated_and_stalled_lifecycles() {
    let (_dir, server) = production_ws_fixture().await;
    let shutdown = server.shutdown_handle();
    shutdown.set_response_deadline(Duration::from_millis(50));
    let port = server.port();
    let dispatcher = server.dispatcher();
    let bus = server.command_bus_handle();
    let task = tokio::spawn(server.run());

    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({
                "kind": "hello",
                "apiVersion": API_VERSION,
                "token": "test-token".repeat(8),
                "pid": std::process::id(),
            })
            .to_string(),
        ))
        .await
        .unwrap();
    assert!(socket
        .next()
        .await
        .unwrap()
        .unwrap()
        .to_text()
        .unwrap()
        .contains("hello_ok"));
    socket
            .send(Message::Text(
                r#"{"operation":"commands.register","commands":[{"id":"lifecycle.command","title":"Lifecycle","category":"test"}]}"#.into(),
            ))
            .await
            .unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(60), socket.next())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(60), async {
        while bus.registration_count_sync() != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();

    let mut raw = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let (mut no_hello, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .unwrap();
    let _ = (&mut raw, &mut no_hello);
    tokio::time::timeout(Duration::from_secs(60), async {
        while shutdown.task_count() < 3 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();

    shutdown.begin_shutdown().await;
    task.await.unwrap().unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(60), socket.next())
        .await
        .is_ok());
    let first_shutdown = shutdown.shutdown().await;
    assert!(
        first_shutdown.is_err(),
        "stalled lifecycle must report deadline breach"
    );
    shutdown.shutdown().await.unwrap();
    assert_eq!(shutdown.task_count(), 0);
    assert_eq!(dispatcher.live_owner_count(), 0);
    assert_eq!(bus.registration_count_sync(), 0);
}

#[tokio::test]
async fn production_ws_capacity_rejects_the_next_raw_socket_and_restores_capacity() {
    let (_dir, server) = production_ws_fixture().await;
    let port = server.port();
    let dispatcher = server.dispatcher();
    let shutdown = server.shutdown_handle();
    let task = tokio::spawn(server.run());
    let mut sockets = Vec::with_capacity(MAX_ACTIVE_WS_CONNECTIONS + 1);
    for _ in 0..=MAX_ACTIVE_WS_CONNECTIONS {
        sockets.push(
            tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .unwrap(),
        );
    }
    tokio::time::timeout(Duration::from_secs(60), async {
        while shutdown.task_count() != MAX_ACTIVE_WS_CONNECTIONS {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let mut rejected = sockets.pop().unwrap();
    let mut byte = [0u8; 1];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(60), rejected.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    shutdown.shutdown().await.unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(shutdown.task_count(), 0);
    assert_eq!(dispatcher.live_owner_count(), 0);
    assert_eq!(shutdown.available_capacity(), MAX_ACTIVE_WS_CONNECTIONS);
}
