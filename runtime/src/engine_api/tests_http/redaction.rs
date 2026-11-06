use super::*;

#[tokio::test]
pub(super) async fn user_data_endpoint_requires_desktop_host_and_round_trips_by_handle() {
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
    // fixture files past the tempdir cleanup.
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
}

/// End-to-end: a real `upsert_object` rejected by ark-core lands in the Engine
/// log as a wire code, and a producer that *does* quote a payload value (a
/// leaky package worker, say) still cannot reach the log —
/// `RejectionReason::Dispatch` collapses it to the public class.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
pub(super) async fn rejected_app_rpc_logs_the_wire_code_but_never_payload_values() {
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
    let ark_for_dispatch = ark.clone();
    let dispatcher = Arc::new(crate::engine_dispatch::EngineDispatcher::new(Arc::new(
        move |request: crate::engine_dispatch::DispatchRequest| {
            let ark = ark_for_dispatch.clone();
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
                    "typeId": "core.note",
                    "typeVersion": "0.1.0",
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
    // never be logged (browser sessions rely on that).
    assert!(!captured.contains(launch_token), "{captured}");

    // Drain connection/request tasks so nothing holds test
    // fixture files past the tempdir cleanup.
    let _ = server_shutdown_handle.shutdown().await;
    let _ = task.await;
    let ark = Arc::try_unwrap(ark).unwrap_or_else(|_| panic!("API task released ArkHost"));
    ark.shutdown(std::time::Instant::now() + Duration::from_secs(30))
        .await
        .expect("ARK closes before fixture cleanup");
}
