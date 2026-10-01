fn public_app_error(error: &str) -> &'static str {
    let code = error.strip_prefix("package worker: ").unwrap_or(error);
    match code {
        "forbidden" => "forbidden",
        "invalid-request" => "invalid-request",
        "not-found" => "not-found",
        "conflict" => "conflict",
        "timeout" => "timeout",
        "unavailable" => "unavailable",
        _ => "unavailable",
    }
}

fn public_app_error_response(response: &Value) -> Option<Value> {
    (response.get("ok").and_then(Value::as_bool) == Some(false)).then(|| {
        let error = response
            .get("error")
            .and_then(Value::as_str)
            .map(public_app_error)
            .unwrap_or("unavailable");
        json!({ "ok": false, "error": error })
    })
}

#[cfg(test)]
mod app_rpc_error_tests {
    use super::{public_app_error, public_app_error_response};
    use serde_json::json;

    #[test]
    fn exposes_only_fixed_app_error_classes() {
        assert_eq!(public_app_error("package worker: forbidden"), "forbidden");
        assert_eq!(public_app_error("timeout"), "timeout");
        assert_eq!(public_app_error("C:\\private\\path"), "unavailable");
    }

    #[test]
    fn redacts_dispatch_error_envelopes() {
        assert_eq!(
            public_app_error_response(&json!({
                "ok": false,
                "error": "C:\\private\\secret",
                "details": "must not cross the boundary"
            })),
            Some(json!({ "ok": false, "error": "unavailable" }))
        );
    }
}

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
    let body = serde_json::from_slice(&collected.to_bytes()).unwrap_or(Value::Null);
    let (wire_request_id, operation, params) = match parse_app_rpc(body.clone(), &typed_grant) {
        Ok(value) => value,
        Err(reason) => {
            crate::observability::log_app_rpc_rejection(
                &client.class,
                body.get("operation").and_then(Value::as_str).unwrap_or("-"),
                None,
                reason,
            );
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({ "ok": false, "error": "invalid-request" }),
            );
        }
    };
    // Metadata for the rejection log — read before authorize rewrites params.
    let type_id = crate::observability::app_rpc_type_id(&params).map(str::to_owned);
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
        Err(reason) => {
            crate::observability::log_app_rpc_rejection(
                &client.class,
                &operation,
                type_id.as_deref(),
                reason,
            );
            return json_response(
                StatusCode::FORBIDDEN,
                json!({ "ok": false, "error": "forbidden" }),
            );
        }
    };
    let owner = match dispatcher.allocate_owner() {
        Ok(owner) => owner,
        Err(_) => {
            crate::observability::log_app_rpc_rejection(
                &client.class,
                &operation,
                type_id.as_deref(),
                "dispatcher has no free owner",
            );
            return json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({ "ok": false, "error": "unavailable" }),
            );
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
        crate::observability::log_app_rpc_rejection(
            &client.class,
            &operation,
            type_id.as_deref(),
            "HTTP operation capacity exhausted or server shutting down",
        );
        return json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "ok": false, "error": "HTTP operation capacity exhausted or server shutting down" }),
        );
    };
    let response = match tokio::time::timeout(request_timeout, receiver).await {
        Ok(Ok(Ok(value))) => {
            response_guard.disarm();
            filter_app_response(
                &operation,
                value,
                &typed_grant,
                &dispatcher,
                &app_client,
                type_id.as_deref(),
            )
            .await
        }
        Ok(Ok(Err(error))) => {
            response_guard.disarm();
            let reason = error.to_string();
            let public = public_app_error(&reason);
            crate::observability::log_app_rpc_rejection(
                &client.class,
                &operation,
                type_id.as_deref(),
                &reason,
            );
            json!({ "ok": false, "error": public })
        }
        Ok(Err(_)) => {
            response_guard.disarm();
            crate::observability::log_app_rpc_rejection(
                &client.class,
                &operation,
                type_id.as_deref(),
                "dispatch task failed",
            );
            json!({ "ok": false, "error": "unavailable" })
        }
        Err(_) => {
            crate::observability::log_app_rpc_rejection(
                &client.class,
                &operation,
                type_id.as_deref(),
                "timeout",
            );
            json!({ "ok": false, "error": "timeout" })
        }
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
