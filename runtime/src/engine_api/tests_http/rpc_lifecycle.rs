use super::*;

#[tokio::test]
pub(super) async fn http_preserves_request_identity_and_defaults_client_context() {
    let observed = Arc::new(Mutex::new(None));
    let observed_for_handler = observed.clone();
    let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
        move |request| {
            let observed = observed_for_handler.clone();
            Box::pin(async move {
                *observed.lock().unwrap() = Some(request.clone());
                Ok(json!({
                    "id": request.request_id,
                    "ok": true,
                }))
            })
        },
    )));
    let token = "a".repeat(64);
    let (_fixture_dir, server) =
        EngineApiServer::bind_with_test_dispatcher(token.clone(), dispatcher, REQUEST_TIMEOUT)
            .await
            .expect("server");
    let port = server.port();
    let server_shutdown_handle = server.shutdown_handle();
    let task = tokio::spawn(server.run());
    let mut child_command = if cfg!(windows) {
        // Not PowerShell: killed mid-startup it leaves its
        // __PSScriptPolicyTest_* probe files in %TEMP%.
        let mut command = std::process::Command::new("ping.exe");
        command
            .args(["-n", "6", "127.0.0.1"])
            .stdout(std::process::Stdio::null());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
        command
    } else {
        let mut command = std::process::Command::new("sleep");
        command.arg("5");
        command
    };
    let mut child = child_command.spawn().expect("same-user child");
    let child_pid = child.id();
    assert_ne!(child_pid, std::process::id());
    let response = raw_http(
        port,
        &request_with_pid(
            &token,
            "POST",
            "/v1/rpc",
            r#"{"_req_id":"preserved-id","operation":"test.read"}"#,
            &child_pid.to_string(),
        ),
    )
    .await;
    let body = response_json(&response);
    assert_eq!(body["id"], "preserved-id");
    let request = observed.lock().unwrap().clone().expect("dispatch request");
    assert_eq!(request.request_id.as_deref(), Some("preserved-id"));
    assert_eq!(request.client.pid, Some(child_pid));
    assert_eq!(request.client.class.as_deref(), Some("engine-http"));
    assert_eq!(request.client.version.as_deref(), Some(API_VERSION));
    assert!(request.client.connection_id.is_some());
    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup.
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
    let _ = child.kill();
    let _ = child.wait();
}

#[tokio::test]
pub(super) async fn direct_http_rpc_records_only_authenticated_requests_in_bounded_api_v1_buckets()
{
    let dir = tempfile::tempdir().expect("usage dir");
    let usage = Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage"));
    let token = "a".repeat(64);
    let server = EngineApiServer::bind(
        token.clone(),
        9,
        usage.clone(),
        "00000000-0000-4000-8000-000000000001".into(),
        Arc::new(PackageService::open(dir.path()).expect("packages")),
        test_dispatcher(),
    )
    .await
    .expect("server");
    let port = server.port();
    let server_shutdown_handle = server.shutdown_handle();
    let task = tokio::spawn(server.run());

    let valid = request_with_client(
        &token,
        "POST",
        "/v1/rpc",
        r#"{"operation":"noted"}"#,
        "desktop-host",
        "1.2.3",
    );
    assert!(raw_http(port, &valid).await.starts_with("HTTP/1.1 502"));
    assert!(raw_http(port, &request(&token, "POST", "/v1/rpc", "{"))
        .await
        .starts_with("HTTP/1.1 400"));
    assert!(
        raw_http(port, "POST /v1/rpc HTTP/1.1\r\nConnection: close\r\n\r\n")
            .await
            .starts_with("HTTP/1.1 401")
    );
    assert!(raw_http(
        port,
        &request_with_pid(&token, "POST", "/v1/rpc", "{}", "2147483647")
    )
    .await
    .starts_with("HTTP/1.1 403"));

    let snapshot = usage.snapshot();
    assert_eq!(snapshot.api_v1.connections, 1);
    assert_eq!(
        snapshot
            .clients
            .get("api_v1:desktop-host@1.2.3")
            .map(|counter| counter.connections),
        Some(1)
    );
    assert_eq!(
        snapshot
            .clients
            .get("api_v1:engine-http@1.0.0")
            .map(|counter| counter.connections),
        None
    );
    let raw = std::fs::read_to_string(dir.path().join(crate::protocol_usage::FILE_NAME))
        .expect("usage file");
    assert!(!raw.contains("2147483647"));
    assert!(!raw.contains("noted"));
    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup.
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
}

#[tokio::test]
pub(super) async fn dropping_response_wait_starts_continuation_and_restores_capacity() {
    let cleanup_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
        Arc::new(|_| {
            Box::pin(async {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(json!({"never": "returns"}))
            })
        }),
        Arc::new({
            let cleanup_count = cleanup_count.clone();
            move |_| {
                let cleanup_count = cleanup_count.clone();
                cleanup_count.fetch_add(1, Ordering::SeqCst);
            }
        }),
    ));
    let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher_and_deadline(
        "a".repeat(64),
        dispatcher.clone(),
        Duration::from_secs(30),
        Duration::from_millis(20),
    )
    .await
    .expect("server");
    let owner = dispatcher.allocate_owner().unwrap();
    let owner_id = owner.id();
    let request =
        crate::engine_dispatch::DispatchRequest::from_wire(json!({"operation":"stalled"}))
            .expect("request")
            .with_client(crate::engine_dispatch::DispatchClient {
                connection_id: Some(owner_id),
                desktop_authorized: false,
                ..Default::default()
            });
    let (_, receiver, guard) = server
        .operations
        .start(request, dispatcher.clone(), owner)
        .await
        .expect("admitted");
    drop(receiver);
    drop(guard);
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
    assert!(server.operations.permits.available_permits() > 0);
}

#[tokio::test]
pub(super) async fn response_timeout_stays_primary_with_bounded_continuation() {
    let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cleanups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
        Arc::new({
            let writes = writes.clone();
            move |request| {
                let writes = writes.clone();
                Box::pin(async move {
                    writes.fetch_add(1, Ordering::SeqCst);
                    if request.operation.as_str() == "finishes-before-response-timeout" {
                        tokio::time::sleep(Duration::from_millis(35)).await;
                        Ok(json!({"ok": true}))
                    } else {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        Ok(json!({"ok": true}))
                    }
                })
            }
        }),
        Arc::new({
            let cleanups = cleanups.clone();
            move |_| {
                let cleanups = cleanups.clone();
                cleanups.fetch_add(1, Ordering::SeqCst);
            }
        }),
    ));
    let token = "a".repeat(64);
    let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher_and_deadline(
        token.clone(),
        dispatcher,
        Duration::from_millis(50),
        Duration::from_millis(30),
    )
    .await
    .expect("server");
    let port = server.port();
    let server_shutdown_handle = server.shutdown_handle();
    let task = tokio::spawn(server.run());

    assert!(raw_http(
        port,
        &request(
            &token,
            "POST",
            "/v1/rpc",
            r#"{"operation":"finishes-before-response-timeout"}"#
        )
    )
    .await
    .starts_with("HTTP/1.1 200"));
    assert!(raw_http(
        port,
        &request(&token, "POST", "/v1/rpc", r#"{"operation":"times-out"}"#)
    )
    .await
    .starts_with("HTTP/1.1 502"));
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert_eq!(writes.load(Ordering::SeqCst), 2);
    assert_eq!(cleanups.load(Ordering::SeqCst), 2);
    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup.
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
}

#[tokio::test]
pub(super) async fn http_timeout_keeps_started_write_exactly_once_and_cleans_connection() {
    let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cleanups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let writes_for_handler = writes.clone();
    let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
        Arc::new(move |request| {
            let writes = writes_for_handler.clone();
            Box::pin(async move {
                if request.operation.as_str() == "controlled.write" {
                    writes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(40)).await;
                }
                Ok(json!({"ok": true, "written": true}))
            })
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
    let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher(
        token.clone(),
        dispatcher,
        Duration::from_millis(10),
    )
    .await
    .expect("server");
    let port = server.port();
    let server_shutdown_handle = server.shutdown_handle();
    let task = tokio::spawn(server.run());

    let response = raw_http(
        port,
        &request(
            &token,
            "POST",
            "/v1/rpc",
            r#"{"_req_id":"write-1","operation":"controlled.write"}"#,
        ),
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 502"), "response: {response}");

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
    .expect("operation and cleanup must complete");
    assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(cleanups.load(std::sync::atomic::Ordering::SeqCst), 1);
    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup.
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
}
