#[allow(clippy::too_many_arguments, clippy::result_large_err)]
async fn handle_request(
    request: Request<Incoming>,
    expected_token: Arc<String>,
    ws_port: u16,
    protocol_usage: Arc<ProtocolUsageStore>,
    correlation_id: Arc<String>,
    package_service: Arc<PackageService>,
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    request_timeout: Duration,
    operations: HttpOperationRegistry,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    http_port: u16,
) -> Result<HttpResponse, Infallible> {
    let response = if request.method() == Method::GET
        && request.uri().path().starts_with("/v1/apps/assets/")
    {
        serve_asset(request.uri().path(), package_service, launch_leases).await
    } else {
        match authenticate(request.headers(), &expected_token) {
            Ok(client) => {
                handle_authenticated_request(
                    request,
                    client,
                    ws_port,
                    protocol_usage,
                    correlation_id,
                    package_service,
                    dispatcher,
                    request_timeout,
                    operations,
                    launch_leases,
                    http_port,
                )
                .await
            }
            Err(response) => response,
        }
    };
    Ok(response)
}

#[allow(clippy::too_many_arguments)]
async fn handle_authenticated_request(
    request: Request<Incoming>,
    client: AuthenticatedClient,
    ws_port: u16,
    protocol_usage: Arc<ProtocolUsageStore>,
    correlation_id: Arc<String>,
    package_service: Arc<PackageService>,
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    request_timeout: Duration,
    operations: HttpOperationRegistry,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    http_port: u16,
) -> HttpResponse {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    match (&method, path.as_str()) {
        (&Method::GET, "/v1/health") => json_response(
            StatusCode::OK,
            json!({ "ok": true, "status": "ready", "api_version": API_VERSION }),
        ),
        (&Method::GET, "/v1/info") => json_response(
            StatusCode::OK,
            json!({
                "ok": true,
                "api_version": API_VERSION,
                "legacy_protocol_version": PROTOCOL_VERSION,
                "pid": std::process::id(),
                "ws_port": ws_port,
                "correlation_id": correlation_id.as_str(),
                "protocol_usage": protocol_usage.snapshot(),
            }),
        ),
        (&Method::POST, "/v1/rpc") => {
            handle_rpc(request, client, correlation_id, dispatcher, request_timeout, operations,
                protocol_usage).await
        }
        (&Method::POST, "/v1/apps/resolve") => {
            handle_resolve(request, package_service).await
        }
        (&Method::POST, "/v1/apps/launch") => {
            handle_launch(request, package_service, dispatcher, launch_leases, http_port).await
        }
        (&Method::POST, route)
            if route.starts_with("/v1/apps/launch/") && route.ends_with("/ark") =>
        {
            handle_app_rpc(
                request,
                client,
                correlation_id,
                package_service,
                dispatcher,
                request_timeout,
                operations,
                launch_leases,
                route.to_owned(),
                protocol_usage,
            )
            .await
        }
        (&Method::POST, route)
            if route.starts_with("/v1/apps/launch/") && route.ends_with("/renew") =>
        {
            handle_renew(request, package_service, launch_leases, route).await
        }
        (&Method::POST, route)
            if route.starts_with("/v1/apps/launch/") && route.ends_with("/grants/directory") =>
        {
            handle_directory_grant(request, client, package_service, launch_leases, route).await
        }
        (&Method::DELETE, route) if route.starts_with("/v1/apps/launch/") => {
            handle_revoke(launch_leases, route)
        }
        (&Method::GET, "/v1/rpc")
        | (&Method::POST, "/v1/health")
        | (&Method::POST, "/v1/info") => json_response(
            StatusCode::METHOD_NOT_ALLOWED,
            json!({ "ok": false, "error": "method not allowed" }),
        ),
        _ => json_response(
            StatusCode::NOT_FOUND,
            json!({ "ok": false, "error": "not found" }),
        ),
    }
}
