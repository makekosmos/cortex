use super::*;

#[tokio::test]
pub(super) async fn live_http_rejects_unsupported_malformed_and_oversized_requests() {
    let dir = tempfile::tempdir().unwrap();
    let usage = Arc::new(ProtocolUsageStore::open(dir.path()).unwrap());
    let token = "a".repeat(64);
    let server = EngineApiServer::bind(
        token.clone(),
        9,
        usage,
        "00000000-0000-4000-8000-000000000001".into(),
        Arc::new(PackageService::open(dir.path()).unwrap()),
        test_dispatcher(),
    )
    .await
    .unwrap();
    let port = server.port();
    let server_shutdown_handle = server.shutdown_handle();
    let task = tokio::spawn(server.run());

    let unauthorized = raw_http(port, "GET /v1/health HTTP/1.1\r\nConnection: close\r\n\r\n").await;
    assert!(unauthorized.starts_with("HTTP/1.1 401"));

    let unsupported = raw_http(port, &request(&token, "POST", "/v1/health", "")).await;
    assert!(unsupported.starts_with("HTTP/1.1 405"));
    let unknown = raw_http(port, &request(&token, "GET", "/v1/unknown", "")).await;
    assert!(unknown.starts_with("HTTP/1.1 404"));

    let malformed = raw_http(port, &request(&token, "POST", "/v1/rpc", "{")).await;
    assert!(malformed.starts_with("HTTP/1.1 400"));

    let missing_operation = raw_http(port, &request(&token, "POST", "/v1/rpc", "{}")).await;
    assert!(missing_operation.starts_with("HTTP/1.1 400"));

    let oversized_body = format!(
        r#"{{"operation":"noop","padding":"{}"}}"#,
        "x".repeat(MAX_HTTP_BODY_BYTES)
    );
    let oversized = raw_http(port, &request(&token, "POST", "/v1/rpc", &oversized_body)).await;
    assert!(oversized.starts_with("HTTP/1.1 413"));

    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup (KOS-270).
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
}

// Multi-threaded: the server task and the hammering clients must not
// serialize on one runtime thread — fs reads and pid-auth syscalls block.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
pub(super) async fn loopback_http_launch_lease_lifecycle_uses_signed_installed_package() {
    let dir = tempfile::tempdir().expect("tempdir");
    let token = "a".repeat(64);
    let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
    let server = EngineApiServer::bind(
        token.clone(),
        9,
        Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
        "00000000-0000-4000-8000-000000000001".into(),
        Arc::new(service),
        test_dispatcher(),
    )
    .await
    .expect("server");
    let port = server.port();
    let server_shutdown_handle = server.shutdown_handle();
    let task = tokio::spawn(server.run());

    // Cover every new route against the real Engine v1 auth boundary;
    // no route is allowed to infer authorization from a launch-only test.
    for (method, path, body) in [
        ("POST", "/v1/apps/resolve", r#"{"id":"com.kosmos.demo"}"#),
        ("POST", "/v1/apps/launch", r#"{"id":"com.kosmos.demo"}"#),
        (
            "DELETE",
            "/v1/apps/launch/00000000-0000-4000-8000-000000000099",
            "",
        ),
    ] {
        let cases = [
            (
                "missing bearer",
                format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n\r\n"),
                "HTTP/1.1 401",
            ),
            (
                "wrong bearer",
                request_with_headers(
                    "b",
                    method,
                    path,
                    body,
                    &std::process::id().to_string(),
                    API_VERSION,
                ),
                "HTTP/1.1 401",
            ),
            (
                "dead pid",
                request_with_headers(&token, method, path, body, "2147483647", API_VERSION),
                "HTTP/1.1 403",
            ),
            (
                "incompatible API",
                request_with_headers(
                    &token,
                    method,
                    path,
                    body,
                    &std::process::id().to_string(),
                    "2.0.0",
                ),
                "HTTP/1.1 426",
            ),
        ];
        for (name, invalid, expected) in cases {
            let response = raw_http(port, &invalid).await;
            assert!(
                response.starts_with(expected),
                "{name} for {method} {path}: expected {expected}, got {response}"
            );
        }
    }

    let first = response_json(
        &raw_http(
            port,
            &request(
                &token,
                "POST",
                "/v1/apps/launch",
                r#"{"id":"com.kosmos.demo"}"#,
            ),
        )
        .await,
    );
    let first_data = &first["data"];
    let first_id = first_data["launch_id"]
        .as_str()
        .expect("launch id")
        .to_owned();
    let first_url = first_data["launch_url"]
        .as_str()
        .expect("asset url")
        .to_owned();
    let first_path = first_url
        .split_once(&format!(":{port}"))
        .expect("asset path in loopback URL")
        .1;
    assert!(raw_http(
        port,
        &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
    )
    .await
    .starts_with("HTTP/1.1 200"));

    // Hammer resolve to prove it never mints a lease. One connection per
    // request cost ~30 s of wall time in accept/teardown overhead, so the
    // workers keep their sockets alive (what a real desktop client does)
    // and read framed responses instead of waiting for close.
    let resolve_request = std::sync::Arc::new(
        request(
            &token,
            "POST",
            "/v1/apps/resolve",
            r#"{"id":"com.kosmos.demo"}"#,
        )
        .replace("Connection: close\r\n", ""),
    );
    const RESOLVE_TOTAL: usize = 10_000;
    const RESOLVE_WORKERS: usize = 128;
    let mut resolve_workers = tokio::task::JoinSet::new();
    for worker in 0..RESOLVE_WORKERS {
        let resolve_request = resolve_request.clone();
        let share =
            RESOLVE_TOTAL / RESOLVE_WORKERS + usize::from(worker < RESOLVE_TOTAL % RESOLVE_WORKERS);
        resolve_workers.spawn(async move {
            use tokio::io::AsyncWriteExt;
            // A 128-way loopback connect burst can exceed the platform
            // accept backlog (macOS somaxconn is 128) and take an RST
            // before the server drains it; a dropped permit also resets
            // the socket — either way the worker just reconnects, the
            // assertion under test is elsewhere.
            let mut sent = 0usize;
            'outer: while sent < share {
                let mut stream = None;
                for _ in 0..50 {
                    match tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
                        Ok(connected) => {
                            stream = Some(connected);
                            break;
                        }
                        Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
                    }
                }
                let mut stream = stream.expect("resolve connection");
                while sent < share {
                    if stream.write_all(resolve_request.as_bytes()).await.is_err() {
                        continue 'outer;
                    }
                    let Ok(response) = try_read_http_response(&mut stream).await else {
                        continue 'outer;
                    };
                    let resolved = response_json(&response);
                    assert!(resolved["data"].get("launch_id").is_none());
                    sent += 1;
                }
            }
        });
    }
    while let Some(worker) = resolve_workers.join_next().await {
        worker.expect("resolve worker");
    }
    assert!(raw_http(
        port,
        &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
    )
    .await
    .starts_with("HTTP/1.1 200"));

    let second = response_json(
        &raw_http(
            port,
            &request(
                &token,
                "POST",
                "/v1/apps/launch",
                r#"{"id":"com.kosmos.demo"}"#,
            ),
        )
        .await,
    );
    let second_id = second["data"]["launch_id"]
        .as_str()
        .expect("second launch id");
    let malformed = raw_http(
        port,
        &request(&token, "DELETE", "/v1/apps/launch/not-a-uuid", ""),
    )
    .await;
    assert!(malformed.starts_with("HTTP/1.1 404"));
    let unknown = raw_http(
        port,
        &request(
            &token,
            "DELETE",
            "/v1/apps/launch/00000000-0000-4000-8000-000000000099",
            "",
        ),
    )
    .await;
    assert!(unknown.starts_with("HTTP/1.1 404"));
    let second_path = second["data"]["launch_url"]
        .as_str()
        .expect("second url")
        .split_once(&format!(":{port}"))
        .expect("asset path in loopback URL")
        .1;
    assert!(raw_http(
        port,
        &format!("GET {second_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
    )
    .await
    .starts_with("HTTP/1.1 200"));
    assert_ne!(first_id, second_id);

    assert!(raw_http(
        port,
        &request(&token, "DELETE", &format!("/v1/apps/launch/{first_id}"), "")
    )
    .await
    .starts_with("HTTP/1.1 200"));
    assert!(raw_http(
        port,
        &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
    )
    .await
    .starts_with("HTTP/1.1 404"));
    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup (KOS-270).
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
}
