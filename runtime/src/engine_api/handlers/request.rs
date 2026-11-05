#[allow(clippy::too_many_arguments, clippy::result_large_err)]
async fn handle_request(
    request: Request<Incoming>,
    expected_token: Arc<String>,
    ws_port: u16,
    protocol_usage: Arc<ProtocolUsageStore>,
    correlation_id: Arc<String>,
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    request_timeout: Duration,
    operations: HttpOperationRegistry,
    user_data: Arc<crate::user_data::UserDataRoots>,
    _http_port: u16,
) -> Result<HttpResponse, Infallible> {
    let response = match authenticate(request.headers(), &expected_token) {
        Ok(client) => {
            handle_authenticated_request(
                request,
                client,
                ws_port,
                protocol_usage,
                correlation_id,
                dispatcher,
                request_timeout,
                operations,
                user_data,
            )
            .await
        }
        Err(response) => response,
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
    dispatcher: Arc<crate::engine_dispatch::EngineDispatcher>,
    request_timeout: Duration,
    operations: HttpOperationRegistry,
    user_data: Arc<crate::user_data::UserDataRoots>,
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
                // Product metadata, not a separate Engine release line.
                // Bare Cargo versions remain usable but explicitly dev.
                "version": crate::build_info::display_version(),
                "channel": crate::build_info::channel(),
                "build": crate::build_info::engine_source_commit(),
                "pid": std::process::id(),
                "ws_port": ws_port,
                "correlation_id": correlation_id.as_str(),
                "protocol_usage": protocol_usage.snapshot(),
            }),
        ),
        (&Method::POST, "/v1/rpc") => {
            handle_rpc(
                request,
                client,
                correlation_id,
                dispatcher,
                request_timeout,
                operations,
                protocol_usage,
            )
            .await
        }
        (&Method::POST, "/v1/user-data") => handle_user_data(request, client, user_data).await,
        (&Method::PUT, "/v1/user-data") => handle_user_data_write(request, client, user_data).await,
        (&Method::GET, "/v1/rpc") | (&Method::POST, "/v1/health") | (&Method::POST, "/v1/info") => {
            json_response(
                StatusCode::METHOD_NOT_ALLOWED,
                json!({ "ok": false, "error": "method not allowed" }),
            )
        }
        _ => json_response(
            StatusCode::NOT_FOUND,
            json!({ "ok": false, "error": "not found" }),
        ),
    }
}
