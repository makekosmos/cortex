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

async fn handle_resolve(
    request: Request<Incoming>,
    package_service: Arc<PackageService>,
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
    match serde_json::from_slice::<LaunchRequest>(&collected.to_bytes()) {
        Ok(resolve) => match package_service.resolve_app(&resolve.id, resolve.version.as_deref()) {
            Ok(resolved) => json_response(StatusCode::OK, resolve_payload(&resolved.package)),
            Err(_) => json_response(
                StatusCode::NOT_FOUND,
                json!({ "ok": false, "error": "app not found" }),
            ),
        },
        Err(_) => json_response(
            StatusCode::BAD_REQUEST,
            json!({ "ok": false, "error": "malformed resolve request" }),
        ),
    }
}

async fn handle_launch(
    request: Request<Incoming>,
    package_service: Arc<PackageService>,
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    http_port: u16,
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
    let launch = match serde_json::from_slice::<LaunchRequest>(&collected.to_bytes()) {
        Ok(launch) => launch,
        Err(_) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({ "ok": false, "error": "malformed launch request" }),
            )
        }
    };
    if register_package_definitions(&package_service, &dispatcher)
        .await
        .is_err()
    {
        return json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "ok": false, "error": "package definition registration failed" }),
        );
    }
    let resolved = match package_service.resolve_app(&launch.id, launch.version.as_deref()) {
        Ok(resolved) => resolved,
        Err(_) => {
            return json_response(
                StatusCode::NOT_FOUND,
                json!({ "ok": false, "error": "app not found" }),
            )
        }
    };
    let package = resolved.package;
    let mut leases = launch_leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let ttl = leases.ttl;
    let result = leases.create_with_typed_grant(
        AssetGrant {
            id: package.id.clone(),
            version: package.version.clone(),
            hash: package.hash.clone(),
        },
        resolved.grant,
    );
    match result {
        Ok(lease) => json_response(
            StatusCode::OK,
            launch_payload(http_port, &lease, &package, ttl),
        ),
        Err(LeaseCapacityError) => json_response(
            StatusCode::TOO_MANY_REQUESTS,
            json!({ "ok": false, "error": "launch capacity reached" }),
        ),
    }
}

async fn handle_renew(
    request: Request<Incoming>,
    package_service: Arc<PackageService>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    path: &str,
) -> HttpResponse {
    let launch_id = path
        .strip_prefix("/v1/apps/launch/")
        .and_then(|value| value.strip_suffix("/renew"))
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
    let current = launch_leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .typed_grant(launch_id, launch_token)
        .filter(|(asset, grant)| launch_binding_current(&package_service, asset, grant));
    let renewed = current.and_then(|_| {
        launch_leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .renew(launch_id, launch_token)
    });
    match renewed {
        Some(lease) => json_response(
            StatusCode::OK,
            json!({
                "ok": true,
                "data": {
                    "launch_id": launch_id,
                    "ttl_seconds": DATA_GRANT_TTL.as_secs(),
                    "expires_at": lease.grant_expires_at_rfc3339,
                }
            }),
        ),
        None => json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "launch authority denied" }),
        ),
    }
}

fn handle_revoke(launch_leases: Arc<Mutex<LaunchLeaseRegistry>>, path: &str) -> HttpResponse {
    let Some(launch_id) = path
        .strip_prefix("/v1/apps/launch/")
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
    else {
        return json_response(
            StatusCode::NOT_FOUND,
            json!({ "ok": false, "error": "launch not found" }),
        );
    };
    let revoked = launch_leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .revoke(launch_id);
    if revoked {
        json_response(
            StatusCode::OK,
            json!({ "ok": true, "data": { "launch_id": launch_id, "revoked": true } }),
        )
    } else {
        json_response(
            StatusCode::NOT_FOUND,
            json!({ "ok": false, "error": "launch not found" }),
        )
    }
}
