use super::*;
#[tokio::test]
async fn production_ws_reaps_each_sequential_request_on_one_connection() {
    let (_dir, server) = production_ws_fixture().await;
    let shutdown = server.shutdown_handle();
    let dispatcher = server.dispatcher();
    let bus = server.command_bus_handle();
    let port = server.port();
    let task = tokio::spawn(server.run());
    let mut socket = authenticated_socket(port, &"test-token".repeat(8)).await;

    for id in 0..10_000_u32 {
        socket
            .send(Message::Text(
                serde_json::json!({
                    "id": id.to_string(),
                    "operation": "commands.unregister",
                    "ids": [],
                })
                .to_string(),
            ))
            .await
            .unwrap();
        loop {
            let response = socket.next().await.unwrap().unwrap();
            let value: serde_json::Value =
                serde_json::from_str(response.to_text().unwrap()).unwrap();
            if value.get("id") == Some(&serde_json::Value::String(id.to_string())) {
                assert_eq!(value["ok"], true);
                break;
            }
        }
        assert_eq!(shutdown.request_task_count(), 0);
    }

    assert_eq!(shutdown.request_task_count(), 0);
    assert_eq!(
        shutdown.lifecycle.request_capacity.available_permits(),
        MAX_ACTIVE_WS_REQUESTS
    );
    drop(socket);
    shutdown.shutdown().await.unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(shutdown.task_count(), 0);
    assert_eq!(shutdown.request_task_count(), 0);
    assert_eq!(dispatcher.live_owner_count(), 0);
    assert_eq!(bus.registration_count_sync(), 0);
}

#[tokio::test]
async fn production_ws_disconnect_cancels_stalled_request_without_replay() {
    let (_dir, server) = production_ws_fixture().await;
    let shutdown = server.shutdown_handle();
    let dispatcher = server.dispatcher();
    let bus = server.command_bus_handle();
    let entered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let entered_observer = entered.clone();
    dispatcher.set_test_observer(Some(Arc::new(move |phase, _, operation| {
        if matches!(phase, crate::engine_dispatch::DispatchPhase::Started)
            && operation == "test.stall"
        {
            entered_observer.fetch_add(1, Ordering::SeqCst);
        }
    })));
    let port = server.port();
    let task = tokio::spawn(server.run());
    let mut socket = authenticated_socket(port, &"test-token".repeat(8)).await;
    socket
        .send(Message::Text(
            serde_json::json!({
                "id": "stalled",
                "operation": "test.stall",
            })
            .to_string(),
        ))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(60), async {
        while shutdown.request_task_count() != 1 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({
                "id": "second",
                "operation": "test.stall",
            })
            .to_string(),
        ))
        .await
        .unwrap();
    let busy = tokio::time::timeout(Duration::from_secs(60), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let busy: serde_json::Value = serde_json::from_str(busy.to_text().unwrap()).unwrap();
    assert_eq!(busy["ok"], false);
    assert_eq!(busy["error"], "WS request busy");
    tokio::time::timeout(Duration::from_secs(60), async {
        while entered.load(Ordering::SeqCst) != 1 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    drop(socket);
    tokio::time::timeout(Duration::from_secs(60), async {
        while shutdown.request_task_count() != 0 || dispatcher.live_owner_count() != 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(entered.load(Ordering::SeqCst), 1);
    shutdown.shutdown().await.unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(shutdown.request_task_count(), 0);
    assert_eq!(dispatcher.live_owner_count(), 0);
    assert_eq!(bus.registration_count_sync(), 0);
}
