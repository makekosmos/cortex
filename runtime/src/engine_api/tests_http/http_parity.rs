use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
pub(super) async fn production_dispatcher_has_real_http_ws_socket_parity_and_owner_isolation() {
    let dir = tempfile::tempdir().expect("fixture dir");

    let ark = Arc::new(
        crate::ark_host::ArkHost::open(&dir.path().join("ark.db").to_string_lossy())
            .await
            .expect("ark host fixture"),
    );
    let app_index = Arc::new(
        crate::app_index::AppIndex::new(dir.path(), dir.path().join("icons")).expect("app index"),
    );
    let file_index =
        Arc::new(crate::file_index::FileIndex::new_disabled(dir.path()).expect("file index"));
    let package_service =
        Arc::new(crate::package_service::PackageService::open(dir.path()).expect("packages"));
    let usage =
        Arc::new(crate::protocol_usage::ProtocolUsageStore::open(dir.path()).expect("usage"));
    let token = "a".repeat(64);
    let ws = crate::ws_server::WsServer::bind(
        ark.clone(),
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
            .to_string()
            .into(),
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
            ws_write.to_string().into(),
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
                invalid.to_string().into(),
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
        .send(tokio_tungstenite::tungstenite::Message::Text(
            register.into(),
        ))
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
    let ark = Arc::try_unwrap(ark).unwrap_or_else(|_| panic!("HTTP and WS released ArkHost"));
    ark.shutdown(std::time::Instant::now() + Duration::from_secs(30))
        .await
        .expect("ARK closes before fixture cleanup");
    // Shutdown must deterministically release every fixture handle: the
    // data dir is removable immediately, with no retry window (KOS-270).
    dir.close()
        .expect("fixture dir still locked after shutdown");
}
