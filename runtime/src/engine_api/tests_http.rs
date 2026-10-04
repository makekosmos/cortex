    #[tokio::test]
    async fn http_preserves_request_identity_and_defaults_client_context() {
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
        // fixture files past the tempdir cleanup (KOS-270).
        let _ = server_shutdown_handle.shutdown().await;
        let _ = task.await;
        let _ = child.kill();
        let _ = child.wait();
    }

    #[tokio::test]
    async fn direct_http_rpc_records_only_authenticated_requests_in_bounded_api_v1_buckets() {
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
        // fixture files past the tempdir cleanup (KOS-270).
        let _ = server_shutdown_handle.shutdown().await;
        let _ = task.await;
    }

    #[tokio::test]
    async fn dropping_response_wait_starts_continuation_and_restores_capacity() {
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
    async fn response_timeout_is_not_replaced_by_secondary_deadline_and_continuation_is_bounded() {
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
        // fixture files past the tempdir cleanup (KOS-270).
        let _ = server_shutdown_handle.shutdown().await;
        let _ = task.await;
    }

    #[tokio::test]
    async fn http_timeout_keeps_started_write_exactly_once_and_cleans_connection() {
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
        // fixture files past the tempdir cleanup (KOS-270).
        let _ = server_shutdown_handle.shutdown().await;
        let _ = task.await;
    }

    #[tokio::test]
    async fn http_disconnect_keeps_owned_operation_and_cleanup_after_response_is_abandoned() {
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn production_dispatcher_has_real_http_ws_socket_parity_and_owner_isolation() {
        let dir = tempfile::tempdir().expect("fixture dir");

        let ark = Arc::new(
            crate::ark_host::ArkHost::open(&dir.path().join("ark.db").to_string_lossy())
                .await
                .expect("ark host fixture"),
        );
        let app_index = Arc::new(
            crate::app_index::AppIndex::new(dir.path(), dir.path().join("icons"))
                .expect("app index"),
        );
        let file_index =
            Arc::new(crate::file_index::FileIndex::new_disabled(dir.path()).expect("file index"));
        let package_service =
            Arc::new(crate::package_service::PackageService::open(dir.path()).expect("packages"));
        let usage =
            Arc::new(crate::protocol_usage::ProtocolUsageStore::open(dir.path()).expect("usage"));
        let token = "a".repeat(64);
        let ws = crate::ws_server::WsServer::bind(
            ark,
            token.clone(),
            dir.path().to_path_buf(),
            app_index,
            file_index,
            Arc::new(crate::usage_tracker::UsageTrackerDiagnosticsState::default()),
            usage.clone(),
            package_service.clone(),
            "00000000-0000-4000-8000-000000000001".into(),
        )
        .await
        .expect("ws bind");
        let ws_port = ws.port();
        let dispatcher = Arc::new(ws.dispatcher());
        struct ObserverState {
            events: Vec<(crate::engine_dispatch::DispatchPhase, u64, String)>,
            active: usize,
            max_active: usize,
        }
        let observed = Arc::new(std::sync::Mutex::new(ObserverState {
            events: Vec::new(),
            active: 0,
            max_active: 0,
        }));
        let observed_for_dispatch = observed.clone();
        dispatcher.set_test_observer(Some(Arc::new(move |phase, owner, operation| {
            let mut state = observed_for_dispatch.lock().unwrap();
            state.events.push((phase, owner, operation.to_string()));
            if phase == crate::engine_dispatch::DispatchPhase::Started {
                state.active += 1;
                state.max_active = state.max_active.max(state.active);
            } else {
                state.active -= 1;
            }
        })));
        let api = EngineApiServer::bind(
            token.clone(),
            ws_port,
            usage,
            "00000000-0000-4000-8000-000000000001".into(),
            package_service,
            dispatcher,
        )
        .await
        .expect("http bind");
        let http_port = api.port();
        let api_shutdown_handle = api.shutdown_handle();
        let api_task = tokio::spawn(api.run());

        // The HTTP adapter is independently useful: the legacy listener is
        // bound but deliberately not serving for this first real-socket call.
        let http_without_ws = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"http-without-ws","operation":"list_object_types"}"#,
            ),
        )
        .await;
        assert!(
            http_without_ws.starts_with("HTTP/1.1 200"),
            "{http_without_ws}"
        );
        {
            let mut state = observed.lock().unwrap();
            state.events.clear();
            state.active = 0;
            state.max_active = 0;
        }
        let mut keep_alive = tokio::net::TcpStream::connect(("127.0.0.1", http_port))
            .await
            .expect("keep-alive socket");
        for id in ["keep-1", "keep-2"] {
            keep_alive
                .write_all(
                    request(
                        &token,
                        "POST",
                        "/v1/rpc",
                        &format!(r#"{{"_req_id":"{id}","operation":"list_object_types"}}"#),
                    )
                    .replace("Connection: close", "Connection: keep-alive")
                    .as_bytes(),
                )
                .await
                .expect("keep-alive request");
            assert!(read_http_response(&mut keep_alive)
                .await
                .contains(&format!("\"id\":\"{id}\"")));
        }
        let sequential_starts: Vec<u64> = observed
            .lock()
            .unwrap()
            .events
            .iter()
            .filter(|(phase, _, operation)| {
                *phase == crate::engine_dispatch::DispatchPhase::Started
                    && operation == "list_object_types"
            })
            .map(|(_, owner, _)| *owner)
            .collect();
        assert_eq!(sequential_starts.len(), 2);
        assert!(sequential_starts.iter().all(|owner| *owner != 0));
        assert_ne!(sequential_starts[0], sequential_starts[1]);
        assert_eq!(observed.lock().unwrap().max_active, 1);

        let concurrent_request_a = request(
            &token,
            "POST",
            "/v1/rpc",
            r#"{"_req_id":"concurrent-a","operation":"list_object_types"}"#,
        );
        let concurrent_request_b = request(
            &token,
            "POST",
            "/v1/rpc",
            r#"{"_req_id":"concurrent-b","operation":"list_object_types"}"#,
        );
        let (concurrent_a, concurrent_b) = tokio::join!(
            raw_http(http_port, &concurrent_request_a),
            raw_http(http_port, &concurrent_request_b),
        );
        assert!(concurrent_a.contains("\"id\":\"concurrent-a\""));
        assert!(concurrent_b.contains("\"id\":\"concurrent-b\""));
        let concurrent_starts: Vec<u64> = observed
            .lock()
            .unwrap()
            .events
            .iter()
            .filter(|(phase, _, operation)| {
                *phase == crate::engine_dispatch::DispatchPhase::Started
                    && operation == "list_object_types"
            })
            .map(|(_, owner, _)| *owner)
            .skip(2)
            .collect();
        assert_eq!(concurrent_starts.len(), 2);
        assert_ne!(concurrent_starts[0], concurrent_starts[1]);
        assert!(observed.lock().unwrap().max_active >= 1);
        let ws_shutdown = ws.shutdown_handle();
        let ws_task = tokio::spawn(ws.run());

        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{ws_port}"))
            .await
            .expect("ws socket");
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::json!({
                    "kind": "hello",
                    "apiVersion": API_VERSION,
                    "token": token,
                    "pid": std::process::id(),
                })
                .to_string(),
            ))
            .await
            .expect("hello");
        let hello = socket
            .next()
            .await
            .expect("hello response")
            .expect("hello frame");
        assert!(hello.to_text().expect("hello text").contains("hello_ok"));

        let ws_read = r#"{"_req_id":"ws-read","operation":"list_object_types"}"#;
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                ws_read.into(),
            ))
            .await
            .expect("ws read");
        let ws_response = socket
            .next()
            .await
            .expect("ws read response")
            .expect("ws frame");
        let ws_json: Value =
            serde_json::from_str(ws_response.to_text().expect("ws text")).expect("ws json");
        let http_response = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"http-read","operation":"list_object_types"}"#,
            ),
        )
        .await;
        let http_json = response_json(&http_response);
        assert_eq!(ws_json["ok"], http_json["ok"]);
        assert_eq!(ws_json["data"], http_json["data"]);

        let ws_write = serde_json::json!({
            "_req_id": "ws-write-1",
            "operation": "upsert_object_type",
            "object_type": {
                "id": "ws-note",
                "name": "WS Note",
                "schemaJson": "{}",
                "uiSchemaJson": "{}",
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z",
                "systemLocked": false
            },
            "device_id": "socket-test-ws"
        });
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                ws_write.to_string(),
            ))
            .await
            .expect("ws write");
        let ws_write_response: Value = serde_json::from_str(
            socket
                .next()
                .await
                .expect("ws write response")
                .expect("ws write frame")
                .to_text()
                .expect("ws write text"),
        )
        .expect("ws write JSON");
        assert_eq!(ws_write_response["ok"], true);

        let mut ws_malformed = Value::Null;
        let mut ws_unknown = Value::Null;
        for invalid in [
            serde_json::json!({"_req_id":"ws-malformed","operation":"commands.register"}),
            serde_json::json!({"_req_id":"unknown-op","operation":"definitely.unknown"}),
        ] {
            socket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    invalid.to_string(),
                ))
                .await
                .expect("ws invalid request");
            let response: Value = serde_json::from_str(
                socket
                    .next()
                    .await
                    .expect("ws invalid response")
                    .expect("ws invalid frame")
                    .to_text()
                    .expect("ws invalid text"),
            )
            .expect("ws invalid JSON");
            if response["id"] == "ws-malformed" {
                ws_malformed = response.clone();
            } else {
                ws_unknown = response.clone();
            }
            assert_eq!(response["ok"], false);
        }

        let manifest = r#"[{"id":"ws.command","title":"WS","category":"open"}]"#;
        let register = format!(r#"{{"operation":"commands.register","commands":{manifest}}}"#);
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(register))
            .await
            .expect("ws register");
        let _ = socket.next().await.expect("ws register response");
        let http_register = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                concat!(
                    r#"{"operation":"commands.register","commands":[{"id":"http.command","#,
                    r#""title":"HTTP","category":"action"}]}"#
                ),
            ),
        )
        .await;
        assert!(response_json(&http_register)["ok"]
            .as_bool()
            .unwrap_or(false));
        let list = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"operation":"commands.list"}"#,
            ),
        )
        .await;
        let commands = &response_json(&list)["data"]["commands"];
        assert!(commands
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "ws.command"));
        assert!(!commands
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "http.command"));

        let malformed = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"malformed-params","operation":"commands.register"}"#,
            ),
        )
        .await;
        assert!(response_json(&malformed)["ok"] == false);
        let unknown = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"unknown-op","operation":"definitely.unknown"}"#,
            ),
        )
        .await;
        let unknown_json = response_json(&unknown);
        assert_eq!(unknown_json, ws_unknown);
        assert_eq!(unknown_json["id"], "unknown-op");
        assert_eq!(unknown_json["ok"], false);
        assert!(unknown_json["error"]
            .as_str()
            .unwrap_or_default()
            .contains("unknown variant"));
        let malformed_parity = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                r#"{"_req_id":"ws-malformed","operation":"commands.register"}"#,
            ),
        )
        .await;
        assert_eq!(response_json(&malformed_parity), ws_malformed);

        let type_write = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                concat!(
                    r#"{"_req_id":"type-write-1","operation":"upsert_object_type","#,
                    r#""object_type":{"id":"note","name":"Note","schemaJson":"{}","#,
                    r#""uiSchemaJson":"{}","createdAt":"2026-01-01T00:00:00Z","#,
                    r#""updatedAt":"2026-01-01T00:00:00Z","systemLocked":false},"#,
                    r#""device_id":"socket-test"}"#
                ),
            ),
        )
        .await;
        assert!(
            response_json(&type_write)["ok"].as_bool().unwrap_or(false),
            "{type_write}"
        );
        let write = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                concat!(
                    r#"{"_req_id":"write-1","operation":"upsert_object","#,
                    r#""object":{"id":"direct-dispatch-write","typeId":"note","title":"socket","#,
                    r#""contentJson":{},"propsJson":{},"createdAt":"2026-01-01T00:00:00Z","#,
                    r#""updatedAt":"2026-01-01T00:00:00Z","deletedAt":null},"#,
                    r#""device_id":"socket-test"}"#
                ),
            ),
        )
        .await;
        assert!(
            response_json(&write)["ok"].as_bool().unwrap_or(false),
            "{write}"
        );

        let invoke = raw_http(
            http_port,
            &request(
                &token,
                "POST",
                "/v1/rpc",
                concat!(
                    r#"{"_req_id":"invoke-1","operation":"commands.invoke","id":"ws.command","#,
                    r#""params":{"source":"http"}}"#
                ),
            ),
        )
        .await;
        assert!(
            response_json(&invoke)["ok"].as_bool().unwrap_or(false),
            "{invoke}"
        );
        let mut invoked_events = 0;
        let event_window = Instant::now() + Duration::from_millis(500);
        loop {
            let remaining = event_window.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let Some(Ok(frame)) = tokio::time::timeout(remaining, socket.next())
                .await
                .ok()
                .flatten()
            else {
                break;
            };
            let payload: Value =
                serde_json::from_str(frame.to_text().expect("event text")).expect("event JSON");
            if payload["event"] == "command_invoked" && payload["id"] == "ws.command" {
                invoked_events += 1;
                assert_eq!(payload["params"]["source"], "http");
            }
        }
        assert_eq!(invoked_events, 1, "event must be delivered exactly once");

        ws_shutdown.begin_shutdown().await;
        ws_task.await.expect("ws server task").expect("ws server");
        let first_ws_shutdown = ws_shutdown.shutdown().await;
        assert!(
            first_ws_shutdown.is_err(),
            "stalled lifecycle must report deadline breach"
        );
        ws_shutdown
            .shutdown()
            .await
            .expect("idempotent ws shutdown");
        // Drain connection/request tasks so nothing holds test
        // fixture files past the tempdir cleanup (KOS-270).
        let _ = api_shutdown_handle.shutdown().await;
        let _ = api_task.await;
        // Shutdown must deterministically release every fixture handle: the
        // data dir is removable immediately, with no retry window (KOS-270).
        dir.close()
            .expect("fixture dir still locked after shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn dropped_api_server_releases_package_service_back_reference() {
        // KOS-270: PackageService stores its package-definition dispatcher
        // as a Weak ref — a server dropped without shutdown() must not keep
        // the dispatcher (and every component its dispatch closure captures)
        // alive. The fixture dir is removable immediately after drop.
        let dir = tempfile::tempdir().expect("fixture dir");
        let ark = Arc::new(
            crate::ark_host::ArkHost::open(&dir.path().join("ark.db").to_string_lossy())
                .await
                .expect("ark host fixture"),
        );
        let app_index = Arc::new(
            crate::app_index::AppIndex::new(dir.path(), dir.path().join("icons"))
                .expect("app index"),
        );
        let file_index =
            Arc::new(crate::file_index::FileIndex::new_disabled(dir.path()).expect("file index"));
        let package_service =
            Arc::new(crate::package_service::PackageService::open(dir.path()).expect("packages"));
        let usage =
            Arc::new(crate::protocol_usage::ProtocolUsageStore::open(dir.path()).expect("usage"));
        let token = "a".repeat(64);
        let ws = crate::ws_server::WsServer::bind(
            ark,
            token.clone(),
            dir.path().to_path_buf(),
            app_index,
            file_index,
            Arc::new(crate::usage_tracker::UsageTrackerDiagnosticsState::default()),
            usage.clone(),
            package_service.clone(),
            "00000000-0000-4000-8000-000000000001".into(),
        )
        .await
        .expect("ws bind");
        let api = EngineApiServer::bind(
            token.clone(),
            ws.port(),
            usage,
            "00000000-0000-4000-8000-000000000001".into(),
            package_service,
            Arc::new(ws.dispatcher()),
        )
        .await
        .expect("http bind");
        drop(api);
        drop(ws);
        dir.close()
            .expect("fixture dir still locked after dropping servers");
    }

    #[tokio::test]
    async fn shutdown_handle_aborts_stalled_operation_and_releases_ownership_once() {
        let cleanups = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
            Arc::new(|_| {
                Box::pin(async {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    Ok(json!({"never": "returned"}))
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
            token,
            dispatcher.clone(),
            Duration::from_secs(30),
        )
        .await
        .expect("server");
        let owner = dispatcher.allocate_owner().unwrap();
        let owner_id = owner.id();

        let request = crate::engine_dispatch::DispatchRequest::from_wire(
            json!({"operation":"stalled.write"}),
        )
        .expect("request")
        .with_client(crate::engine_dispatch::DispatchClient {
            connection_id: Some(owner_id),
            desktop_authorized: false,
            ..Default::default()
        });
        let _response = server.operations.start(request, dispatcher, owner).await;
        let handle = server.shutdown_handle();
        let _ = tokio::time::timeout(Duration::from_secs(60), handle.shutdown())
            .await
            .expect("shutdown deadline");
        assert_eq!(cleanups.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(server.operations.closed.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn shutdown_closes_admission_and_completes_synchronous_cleanup() {
        let cleanup_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::with_cleanup(
            Arc::new(|_| Box::pin(async { Ok(json!({"ok": true})) })),
            Arc::new({
                let cleanup_count = cleanup_count.clone();
                move |_| {
                    cleanup_count.fetch_add(1, Ordering::SeqCst);
                }
            }),
        ));
        let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher(
            "a".repeat(64),
            dispatcher.clone(),
            Duration::from_secs(60),
        )
        .await
        .expect("server");
        let owner = dispatcher.allocate_owner().unwrap();
        let owner_id = owner.id();
        let request = crate::engine_dispatch::DispatchRequest::from_wire(
            json!({"operation":"controlled.cleanup"}),
        )
        .expect("request")
        .with_client(crate::engine_dispatch::DispatchClient {
            connection_id: Some(owner_id),
            desktop_authorized: false,
            ..Default::default()
        });
        assert!(server
            .operations
            .start(request, dispatcher.clone(), owner)
            .await
            .is_some());
        server.shutdown_handle().shutdown().await.expect("shutdown");
        assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
        assert!(server.operations.closed.load(Ordering::Acquire));
        assert!(server.operations.permits.available_permits() > 0);
    }

    #[tokio::test]
    async fn http_dispatch_starts_only_after_registry_install() {
        let registry = HttpOperationRegistry::default();
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed_registry = registry.clone();
        let observed_started = started.clone();
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
            move |_| {
                let installed = !observed_registry
                    .continuations
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .is_empty();
                observed_started.store(installed, Ordering::SeqCst);
                Box::pin(async { Ok(json!({"ok": true})) })
            },
        )));
        let owner = dispatcher.allocate_owner().unwrap();
        let request = crate::engine_dispatch::DispatchRequest::from_wire(
            json!({"operation":"install-gated"}),
        )
        .unwrap()
        .with_client(crate::engine_dispatch::DispatchClient {
            connection_id: Some(owner.id()),
            ..Default::default()
        });

        let (_, receiver, _guard) = registry
            .start(request, dispatcher, owner)
            .await
            .expect("operation installed");
        assert!(receiver.await.unwrap().is_ok());
        assert!(started.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn admission_racing_shutdown_cannot_insert_after_drain() {
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
            |_| Box::pin(async { Ok(json!({"ok": true})) }),
        )));
        let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher(
            "a".repeat(64),
            dispatcher.clone(),
            Duration::from_secs(60),
        )
        .await
        .expect("server");
        let mut admissions = Vec::new();
        for _ in 0..64 {
            let registry = server.operations.clone();
            let dispatcher = dispatcher.clone();
            admissions.push(tokio::spawn(async move {
                let owner = dispatcher.allocate_owner().unwrap();
                let owner_id = owner.id();
                let request =
                    crate::engine_dispatch::DispatchRequest::from_wire(json!({"operation":"race"}))
                        .expect("request")
                        .with_client(crate::engine_dispatch::DispatchClient {
                            connection_id: Some(owner_id),
                            desktop_authorized: false,
                            ..Default::default()
                        });
                registry.start(request, dispatcher, owner).await.is_some()
            }));
        }
        let shutdown_handle = server.shutdown_handle();
        let shutdown = tokio::spawn(async move { shutdown_handle.shutdown().await });
        for admission in admissions {
            let _ = admission.await.expect("admission task");
        }
        let _ = shutdown.await.expect("shutdown task");
        assert!(server.operations.closed.load(Ordering::Acquire));
        assert!(server.operations.continuations.lock().unwrap().is_empty());
        assert_eq!(dispatcher.live_owner_count(), 0);
    }

    #[tokio::test]
    async fn user_data_endpoint_requires_desktop_host_and_round_trips_by_handle() {
        let dir = tempfile::tempdir().expect("user data dir");
        let root = dir.path().join("userdata");
        std::fs::create_dir_all(&root).expect("root dir");
        let token = "a".repeat(64);
        let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher(
            token.clone(),
            test_dispatcher(),
            REQUEST_TIMEOUT,
        )
        .await
        .expect("server");
        let port = server.port();
        let server_shutdown_handle = server.shutdown_handle();
        let task = tokio::spawn(server.run());

        // Non-host client classes are rejected before touching the filesystem.
        let denied = request_with_client(
            &token,
            "POST",
            "/v1/user-data",
            r#"{"operation":"stat","root_id":"x","app_id":"app","key":"a"}"#,
            "engine-http",
            "1.0.0",
        );
        assert!(raw_http(port, &denied).await.starts_with("HTTP/1.1 403"));

        let open = format!(
            r#"{{"operation":"open_root","root":{}}}"#,
            serde_json::to_string(root.to_str().expect("utf8 root")).expect("json root")
        );
        let response = raw_http(
            port,
            &request_with_client(
                &token,
                "POST",
                "/v1/user-data",
                &open,
                "desktop-host",
                "1.0.0",
            ),
        )
        .await;
        let body = response_json(&response);
        assert_eq!(body["ok"], true, "{response}");
        let root_id = body["data"]["root_id"]
            .as_str()
            .expect("root id")
            .to_owned();

        // Binary writes arrive over PUT so the payload never crosses a JSON
        // or WebSocket ceiling.
        let put = format!(
            "PUT /v1/user-data HTTP/1.1\r\n\
             Authorization: Bearer {token}\r\n\
             X-Kosmos-Client-Pid: {}\r\n\
             X-Kosmos-Api-Version: {API_VERSION}\r\n\
             X-Kosmos-Client-Class: desktop-host\r\n\
             X-Kosmos-User-Data-Root: {root_id}\r\n\
             X-Kosmos-User-Data-App: com.kosmos.agenda\r\n\
             X-Kosmos-User-Data-Key: attachments/task-1.bin\r\n\
             Content-Length: 7\r\n\
             Connection: close\r\n\r\n\x00\x01inert",
            std::process::id()
        );
        let response = raw_http(port, &put).await;
        let body = response_json(&response);
        assert_eq!(body["ok"], true, "{response}");
        assert_eq!(body["data"]["size_bytes"], 7);

        let stat = format!(
            "{{\"operation\":\"stat\",\"root_id\":\"{root_id}\",\"app_id\":\"com.kosmos.agenda\",\
                \"key\":\"attachments/task-1.bin\"}}"
        );
        let body = response_json(
            &raw_http(
                port,
                &request_with_client(
                    &token,
                    "POST",
                    "/v1/user-data",
                    &stat,
                    "desktop-host",
                    "1.0.0",
                ),
            )
            .await,
        );
        assert_eq!(body["data"]["size_bytes"], 7);

        let read = format!(
            "{{\"operation\":\"read\",\"root_id\":\"{root_id}\",\"app_id\":\"com.kosmos.agenda\",\
                \"key\":\"attachments/task-1.bin\"}}"
        );
        let body = response_json(
            &raw_http(
                port,
                &request_with_client(
                    &token,
                    "POST",
                    "/v1/user-data",
                    &read,
                    "desktop-host",
                    "1.0.0",
                ),
            )
            .await,
        );
        use base64::Engine as _;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(body["data"]["bytes"].as_str().expect("base64 bytes"))
            .expect("decode");
        assert_eq!(decoded, b"\x00\x01inert");
        assert_eq!(
            std::fs::read(root.join("extension-data/com.kosmos.agenda/attachments/task-1.bin"))
                .expect("on-disk payload"),
            b"\x00\x01inert"
        );

        let traversal = format!(
            "{{\"operation\":\"read\",\"root_id\":\"{root_id}\",\"app_id\":\"com.kosmos.agenda\",\
                \"key\":\"../escape\"}}"
        );
        let body = response_json(
            &raw_http(
                port,
                &request_with_client(
                    &token,
                    "POST",
                    "/v1/user-data",
                    &traversal,
                    "desktop-host",
                    "1.0.0",
                ),
            )
            .await,
        );
        assert_eq!(body["ok"], false);
        assert_eq!(body["error"], "invalid-key");

        let delete = format!(
            "{{\"operation\":\"delete\",\"root_id\":\"{root_id}\",\"app_id\":\"com.kosmos.agenda\",\
                \"key\":\"attachments/task-1.bin\"}}"
        );
        let body = response_json(
            &raw_http(
                port,
                &request_with_client(
                    &token,
                    "POST",
                    "/v1/user-data",
                    &delete,
                    "desktop-host",
                    "1.0.0",
                ),
            )
            .await,
        );
        assert_eq!(body["data"]["deleted"], true);
        let body = response_json(
            &raw_http(
                port,
                &request_with_client(
                    &token,
                    "POST",
                    "/v1/user-data",
                    &stat,
                    "desktop-host",
                    "1.0.0",
                ),
            )
            .await,
        );
        assert_eq!(body["ok"], false);
        assert_eq!(body["error"], "not-found");
        // Drain connection/request tasks so nothing holds test
        // fixture files past the tempdir cleanup (KOS-270).
        let _ = server_shutdown_handle.shutdown().await;
        let _ = task.await;
    }

    #[tokio::test]
    async fn user_data_endpoint_rejects_unknown_roots_and_missing_fields() {
        let token = "a".repeat(64);
        let (_fixture_dir, server) = EngineApiServer::bind_with_test_dispatcher(
            token.clone(),
            test_dispatcher(),
            REQUEST_TIMEOUT,
        )
        .await
        .expect("server");
        let port = server.port();
        let server_shutdown_handle = server.shutdown_handle();
        let task = tokio::spawn(server.run());

        let unknown = request_with_client(
            &token,
            "POST",
            "/v1/user-data",
            concat!(
                r#"{"operation":"read","root_id":"00000000-0000-4000-8000-0000000000aa","#,
                r#""app_id":"app","key":"a.bin"}"#
            ),
            "desktop-host",
            "1.0.0",
        );
        let response = raw_http(port, &unknown).await;
        assert!(response.starts_with("HTTP/1.1 404"), "{response}");
        assert_eq!(response_json(&response)["error"], "unknown-root");

        let missing_key = request_with_client(
            &token,
            "POST",
            "/v1/user-data",
            r#"{"operation":"read","root_id":"x","app_id":"app"}"#,
            "desktop-host",
            "1.0.0",
        );
        let response = raw_http(port, &missing_key).await;
        assert!(response.starts_with("HTTP/1.1 400"), "{response}");

        let unknown_op = request_with_client(
            &token,
            "POST",
            "/v1/user-data",
            r#"{"operation":"symlink_root","root_id":"x"}"#,
            "desktop-host",
            "1.0.0",
        );
        let response = raw_http(port, &unknown_op).await;
        assert!(response.starts_with("HTTP/1.1 400"), "{response}");

        // A PUT without the key header is rejected before the body is read.
        let put = format!(
            "PUT /v1/user-data HTTP/1.1\r\n\
             Authorization: Bearer {token}\r\n\
             X-Kosmos-Client-Pid: {}\r\n\
             X-Kosmos-Api-Version: {API_VERSION}\r\n\
             X-Kosmos-Client-Class: desktop-host\r\n\
             X-Kosmos-User-Data-Root: x\r\n\
             X-Kosmos-User-Data-App: app\r\n\
             Content-Length: 1\r\n\
             Connection: close\r\n\r\nx",
            std::process::id()
        );
        let response = raw_http(port, &put).await;
        assert!(response.starts_with("HTTP/1.1 400"), "{response}");
        // Drain connection/request tasks so nothing holds test
        // fixture files past the tempdir cleanup (KOS-270).
        let _ = server_shutdown_handle.shutdown().await;
        let _ = task.await;
    }

    /// KOS-298 end-to-end: a real `upsert_object` rejected by ark-core lands
    /// in the Engine log as a wire code, and a producer that *does* quote a
    /// payload value (a leaky package worker, say) still cannot reach the
    /// log — `RejectionReason::Dispatch` collapses it to the public class.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn rejected_app_rpc_logs_the_wire_code_but_never_payload_values() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let ark = Arc::new(
            crate::ark_host::ArkHost::open(&dir.path().join("ark.db").to_string_lossy())
                .await
                .expect("ark host fixture"),
        );
        let package_service =
            Arc::new(crate::package_service::tests::enabled_note_write_app_service(dir.path()));
        // The rejection is logged inside the spawned server task, so only a
        // global subscriber sees it. This is the one test in the binary that
        // installs a global default.
        let log = crate::observability::app_rpc::tests::CaptureBuf::default();
        tracing_subscriber::fmt()
            .with_writer(log.clone())
            .with_ansi(false)
            .try_init()
            .expect("global log capture");
        let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
            move |request: crate::engine_dispatch::DispatchRequest| {
                let ark = ark.clone();
                Box::pin(async move {
                    let mut params = request.params.clone();
                    let mode = params
                        .as_object_mut()
                        .and_then(|p| p.remove("mode"))
                        .and_then(|v| v.as_str().map(str::to_owned));
                    if mode.as_deref() == Some("leak") {
                        // A producer that embeds the rejected value in its
                        // error string — the shape a buggy worker/serde
                        // message would take.
                        let title = params["object"]["title"].as_str().unwrap_or("?");
                        return Err(crate::engine_dispatch::DispatchError::Failed(format!(
                            "schema error: \"{title}\" is not a valid note"
                        )));
                    }
                    match ark.request(request.operation.as_str(), params).await {
                        Ok(reply) => Ok(serde_json::json!({
                            "ok": reply.ok,
                            "data": reply.data,
                            "error": reply.error,
                        })),
                        Err(error) => Err(crate::engine_dispatch::DispatchError::Failed(
                            error.to_string(),
                        )),
                    }
                })
            },
        )));
        let token = "a".repeat(64);
        let server = EngineApiServer::bind(
            token.clone(),
            9,
            Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
            "00000000-0000-4000-8000-000000000001".into(),
            package_service,
            dispatcher,
        )
        .await
        .expect("server");
        let port = server.port();
        let server_shutdown_handle = server.shutdown_handle();
        let task = tokio::spawn(server.run());
        let launched = response_json(
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
        let launch_id = launched["data"]["launch_id"].as_str().expect("launch id");
        let launch_token = launched["data"]["broker_token"].as_str().expect("token");

        for mode in ["real", "leak"] {
            let body = serde_json::json!({
                "operation": "upsert_object",
                "params": {
                    "mode": mode,
                    "device_id": "log-test",
                    "object": {
                        "id": "log-test-object",
                        "typeId": "com.kosmos.note",
                        "typeVersion": "1.0.0",
                        "title": "SECRET-TITLE-456",
                        "propsJson": {
                            "description": ["SECRET-VALUE-123"],
                            "extensions": {}
                        }
                    }
                }
            })
            .to_string();
            let request = request_with_client(
                &token,
                "POST",
                &format!("/v1/apps/launch/{launch_id}/ark"),
                &body,
                "memoria-test",
                API_VERSION,
            )
            .replace(
                "Content-Length:",
                &format!("X-Kosmos-Launch-Token: {launch_token}\r\nContent-Length:"),
            );
            let response = response_json(&raw_http(port, &request).await);
            assert_eq!(response["ok"], false, "{mode}: {response}");
            assert_eq!(response["error"], "unavailable", "{mode}: {response}");
        }

        let captured = log.text();
        // The real ark rejection surfaces as its stable wire code.
        assert!(
            captured.contains("canonical_ingress:invalid_request"),
            "{captured}"
        );
        assert!(captured.contains("app RPC rejected"), "{captured}");
        assert!(
            captured.contains("canonical_field:/description"),
            "{captured}"
        );
        assert!(captured.contains("upsert_object"), "{captured}");
        // The leaky producer's value collapse is all the log may keep.
        assert!(captured.contains("reason=unavailable"), "{captured}");
        assert!(!captured.contains("SECRET-VALUE-123"), "{captured}");
        assert!(!captured.contains("SECRET-TITLE-456"), "{captured}");
        // The launch credential is a bearer for the data channel — it must
        // never be logged (KOS-299 browser sessions rely on that).
        assert!(!captured.contains(launch_token), "{captured}");

        // Drain connection/request tasks so nothing holds test
        // fixture files past the tempdir cleanup (KOS-270).
        let _ = server_shutdown_handle.shutdown().await;
        let _ = task.await;
    }

    fn request(token: &str, method: &str, path: &str, body: &str) -> String {
        request_with_pid(token, method, path, body, &std::process::id().to_string())
    }

    fn request_with_pid(token: &str, method: &str, path: &str, body: &str, pid: &str) -> String {
        request_with_headers(token, method, path, body, pid, API_VERSION)
    }

    fn request_with_client(
        token: &str,
        method: &str,
        path: &str,
        body: &str,
        class: &str,
        version: &str,
    ) -> String {
        request_with_headers(
            token,
            method,
            path,
            body,
            &std::process::id().to_string(),
            API_VERSION,
        )
        .replace(
            "Content-Length:",
            &format!(
                "X-Kosmos-Client-Class: {class}\r\nX-Kosmos-Client-Version: \
                     {version}\r\nContent-Length:"
            ),
        )
    }

    fn request_with_headers(
        token: &str,
        method: &str,
        path: &str,
        body: &str,
        pid: &str,
        api_version: &str,
    ) -> String {
        format!(
            "{method} {path} HTTP/1.1\r\n\
             Authorization: Bearer {token}\r\n\
             X-Kosmos-Client-Pid: {pid}\r\n\
             X-Kosmos-Api-Version: {api_version}\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn response_json(response: &str) -> Value {
        serde_json::from_str(response.split_once("\r\n\r\n").expect("HTTP body").1)
            .expect("JSON response")
    }

    fn test_dispatcher() -> Arc<crate::engine_dispatch::EngineDispatcher> {
        Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
            |_| {
                Box::pin(async {
                    Err::<Value, crate::engine_dispatch::DispatchError>(
                        crate::engine_dispatch::DispatchError::Unavailable,
                    )
                })
            },
        )))
    }

    async fn raw_http(port: u16, request: &str) -> String {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        String::from_utf8(response).unwrap()
    }

    async fn try_read_http_response(stream: &mut tokio::net::TcpStream) -> std::io::Result<String> {
        use tokio::io::AsyncReadExt;
        let mut response = Vec::new();
        let header_end = loop {
            let mut chunk = [0_u8; 1024];
            let count = stream.read(&mut chunk).await?;
            if count == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "HTTP socket closed before response",
                ));
            }
            response.extend_from_slice(&chunk[..count]);
            if let Some(end) = response.windows(4).position(|window| window == b"\r\n\r\n") {
                break end + 4;
            }
        };
        let headers = String::from_utf8_lossy(&response[..header_end]);
        let length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .map(str::to_owned)
            })
            .and_then(|value| value.trim().parse::<usize>().ok())
            .expect("content length");
        while response.len() < header_end + length {
            let mut chunk = [0_u8; 1024];
            let count = stream.read(&mut chunk).await?;
            if count == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "HTTP socket closed before body",
                ));
            }
            response.extend_from_slice(&chunk[..count]);
        }
        Ok(String::from_utf8(response).expect("HTTP response text"))
    }

    async fn read_http_response(stream: &mut tokio::net::TcpStream) -> String {
        try_read_http_response(stream).await.expect("HTTP response")
    }
