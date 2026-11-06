use super::*;

#[tokio::test]
pub(super) async fn http_disconnect_keeps_owned_operation_and_cleanup_after_response_is_abandoned()
{
    let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cleanups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
        Arc::new({
            let writes = writes.clone();
            move |_| {
                let writes = writes.clone();
                Box::pin(async move {
                    writes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    Ok(json!({"ok": true}))
                })
            }
        }),
        Arc::new({
            let cleanups = cleanups.clone();
            move |_| {
                let cleanups = cleanups.clone();
                cleanups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }),
    ));
    let token = "a".repeat(64);
    let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher_and_deadline(
        token.clone(),
        dispatcher.clone(),
        Duration::from_secs(60),
        Duration::from_millis(20),
    )
    .await
    .expect("server");
    let port = server.port();
    let permits = server.operations.permits.clone();
    let server_shutdown_handle = server.shutdown_handle();
    let task = tokio::spawn(server.run());
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let request = request(
        &token,
        "POST",
        "/v1/rpc",
        r#"{"operation":"controlled.write"}"#,
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if writes.load(std::sync::atomic::Ordering::SeqCst) == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("dispatch must start before disconnect");
    stream.shutdown().await.unwrap();
    drop(stream);
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if writes.load(std::sync::atomic::Ordering::SeqCst) == 1
                && cleanups.load(std::sync::atomic::Ordering::SeqCst) == 1
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("owned operation must finish and clean up after disconnect");
    assert!(permits.available_permits() > 0);
    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup (KOS-270).
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
}
