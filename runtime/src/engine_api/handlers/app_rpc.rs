async fn handle_app_rpc(
    request: Request<Incoming>,
    client: AuthenticatedClient,
    correlation_id: Arc<String>,
    package_service: Arc<PackageService>,
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    request_timeout: Duration,
    operations: HttpOperationRegistry,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    path: String,
    protocol_usage: Arc<ProtocolUsageStore>,
) -> HttpResponse {
    let launch_id = path
        .strip_prefix("/v1/apps/launch/")
        .and_then(|value| value.strip_suffix("/ark"))
        .filter(|value| uuid::Uuid::parse_str(value).is_ok());
    let Some(launch_id) = launch_id else {
        return json_response(
            StatusCode::NOT_FOUND,
            json!({ "ok": false, "error": "launch not found" }),
        );
    };
    let Some(launch_token) = request
        .headers()
        .get(APP_LAUNCH_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return json_response(
            StatusCode::UNAUTHORIZED,
            json!({ "ok": false, "error": "missing launch token" }),
        );
    };
    let Some((asset_grant, typed_grant)) = ({
        let mut leases = launch_leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        leases.typed_grant(launch_id, launch_token)
    }) else {
        return json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "launch authority denied" }),
        );
    };
    if typed_grant.package_id != asset_grant.id
        || typed_grant.package_version != asset_grant.version
        || !launch_binding_current(&package_service, &asset_grant, &typed_grant)
    {
        return json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "launch authority denied" }),
        );
    }
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
    let (wire_request_id, operation, params) = match parse_app_rpc(
        serde_json::from_slice(&collected.to_bytes()).unwrap_or(Value::Null),
        &typed_grant,
    ) {
        Ok(value) => value,
        Err(error) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({ "ok": false, "error": error }),
            )
        }
    };
    let app_client = DispatchClient {
        pid: Some(client.pid),
        class: Some(client.class.clone()),
        version: Some(client.version.clone()),
        correlation_id: Some(correlation_id.as_ref().clone()),
        connection_id: None,
        desktop_authorized: false,
    };
    let params = match authorize_app_request(
        &operation,
        params,
        &typed_grant,
        &dispatcher,
        &app_client,
    )
    .await
    {
        Ok(params) => params,
        Err(error) => {
            return json_response(
                StatusCode::FORBIDDEN,
                json!({ "ok": false, "error": error }),
            )
        }
    };
    let owner = match dispatcher.allocate_owner() {
        Ok(owner) => owner,
        Err(error) => {
            return json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({ "ok": false, "error": error.to_string() }),
            )
        }
    };
    let request = DispatchRequest {
        request_id: wire_request_id.or_else(|| Some(request_id())),
        operation: Operation::Named(operation.clone()),
        params,
        client: DispatchClient {
            connection_id: Some(owner.id()),
            ..app_client.clone()
        },
    };
    let Some((_operation_id, receiver, mut response_guard)) =
        operations.start(request, dispatcher.clone(), owner).await
    else {
        return json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "ok": false, "error": "HTTP operation capacity exhausted or server shutting down" }),
        );
    };
    let response = match tokio::time::timeout(request_timeout, receiver).await {
        Ok(Ok(Ok(value))) => {
            response_guard.disarm();
            filter_app_response(&operation, value, &typed_grant, &dispatcher, &app_client).await
        }
        Ok(Ok(Err(error))) => {
            response_guard.disarm();
            json!({ "ok": false, "error": error.to_string() })
        }
        Ok(Err(_)) => {
            response_guard.disarm();
            json!({ "ok": false, "error": "Engine RPC task failed" })
        }
        Err(_) => json!({ "ok": false, "error": "Engine RPC timed out" }),
    };
    if let Err(error) = protocol_usage.record(
        crate::protocol_usage::TransportKind::ApiV1,
        Some(&client.class),
        Some(&client.version),
    ) {
        tracing::warn!(error = %error, "protocol usage persistence failed");
    }
    json_response(StatusCode::OK, response)
}
