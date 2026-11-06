async fn handle_rpc(
    request: Request<Incoming>,
    client: AuthenticatedClient,
    correlation_id: Arc<String>,
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    request_timeout: Duration,
    operations: HttpOperationRegistry,
    protocol_usage: Arc<ProtocolUsageStore>,
) -> HttpResponse {
    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
    let collected = match body.collect().await {
        Ok(collected) => collected,
        Err(_) => {
            return json_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                json!({ "ok": false, "error": "request body exceeds 1 MiB" }),
            )
        }
    };
    let value = match serde_json::from_slice::<Value>(&collected.to_bytes()) {
        Ok(value) => value,
        Err(_) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({ "ok": false, "error": "malformed JSON body" }),
            )
        }
    };
    if value.get("operation").and_then(Value::as_str).is_none() || !value.is_object() {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({ "ok": false, "error": "body must contain string operation" }),
        );
    }
    let mut request = match crate::engine_dispatch::DispatchRequest::from_wire(value) {
        Ok(request) => request,
        Err(error) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({ "ok": false, "error": error.to_string() }),
            )
        }
    };
    if request.request_id.is_none() {
        request = request.with_request_id(request_id());
    }
    let owner = match dispatcher.allocate_owner() {
        Ok(owner) => owner,
        Err(error) => {
            return json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({ "ok": false, "error": error.to_string() }),
            )
        }
    };
    let request = request.with_client(crate::engine_dispatch::DispatchClient {
        pid: Some(client.pid),
        class: Some(client.class.clone()),
        version: Some(client.version.clone()),
        correlation_id: Some(correlation_id.as_ref().clone()),
        connection_id: Some(owner.id()),
        desktop_authorized: false,
        // The lock token alone is NOT Manager identity — any same-user
        // process can read engine.lock.json. What proves the Manager is the
        // caller's process image matching the installed Manager exe.
        manager_process: is_manager_process(client.pid),
    });
    let Some((_operation_id, receiver, mut response_guard)) =
        operations.start(request, dispatcher, owner).await
    else {
        return json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!(
                { "ok": false,
                "error": "HTTP operation capacity exhausted or server shutting down" }),
        );
    };
    if let Err(error) = protocol_usage.record(
        crate::protocol_usage::TransportKind::ApiV1,
        Some(&client.class),
        Some(&client.version),
    ) {
        tracing::warn!(error = %error, "protocol usage persistence failed");
    }
    match tokio::time::timeout(request_timeout, receiver).await {
        Ok(Ok(Ok(value))) => {
            response_guard.disarm();
            json_response(StatusCode::OK, value)
        }
        Ok(Ok(Err(error))) => {
            response_guard.disarm();
            json_response(
                StatusCode::BAD_GATEWAY,
                json!({ "ok": false, "error": error.to_string() }),
            )
        }
        Ok(Err(_)) => {
            response_guard.disarm();
            json_response(
                StatusCode::BAD_GATEWAY,
                json!({ "ok": false, "error": "Engine RPC task failed" }),
            )
        }
        Err(_) => json_response(
            StatusCode::BAD_GATEWAY,
            json!({ "ok": false, "error": "Engine RPC timed out" }),
        ),
    }
}
