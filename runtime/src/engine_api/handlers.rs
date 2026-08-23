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
    let is_asset =
        request.method() == Method::GET && request.uri().path().starts_with("/v1/apps/assets/");
    let response = if is_asset {
        serve_asset(request.uri().path(), package_service, launch_leases).await
    } else {
        match authenticate(request.headers(), &expected_token) {
            Ok(client) => match (request.method(), request.uri().path()) {
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
                    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
                    match body.collect().await {
                        Ok(collected) => {
                            match serde_json::from_slice::<Value>(&collected.to_bytes()) {
                                Ok(value)
                                    if value.get("operation").and_then(Value::as_str).is_some()
                                        && value.is_object() =>
                                {
                                    let mut request =
                                        match crate::engine_dispatch::DispatchRequest::from_wire(
                                            value,
                                        ) {
                                            Ok(request) => request,
                                            Err(error) => {
                                                return Ok(json_response(
                                                    StatusCode::BAD_REQUEST,
                                                    json!({ "ok": false, "error": error.to_string() }),
                                                ));
                                            }
                                        };
                                    if request.request_id.is_none() {
                                        request = request.with_request_id(request_id());
                                    }
                                    let owner = match dispatcher.allocate_owner() {
                                        Ok(owner) => owner,
                                        Err(error) => {
                                            return Ok(json_response(
                                                StatusCode::SERVICE_UNAVAILABLE,
                                                json!({ "ok": false, "error": error.to_string() }),
                                            ));
                                        }
                                    };
                                    let owner_id = owner.id();
                                    let request = request.with_client(
                                        crate::engine_dispatch::DispatchClient {
                                            pid: Some(client.pid),
                                            class: Some(client.class.clone()),
                                            version: Some(client.version.clone()),
                                            correlation_id: Some(correlation_id.as_ref().clone()),
                                            connection_id: Some(owner_id),
                                            desktop_authorized: false,
                                        },
                                    );
                                    let Some((_operation_id, receiver, mut response_guard)) =
                                        operations.start(request, dispatcher.clone(), owner).await
                                    else {
                                        return Ok(json_response(
                                            StatusCode::SERVICE_UNAVAILABLE,
                                            json!({ "ok": false, "error": "HTTP operation capacity exhausted or server shutting down" }),
                                        ));
                                    };
                                    if let Err(error) = protocol_usage.record(
                                        crate::protocol_usage::TransportKind::ApiV1,
                                        Some(&client.class),
                                        Some(&client.version),
                                    ) {
                                        tracing::warn!(error = %error, "protocol usage persistence failed");
                                    }
                                    let response = match tokio::time::timeout(
                                        request_timeout,
                                        receiver,
                                    )
                                    .await
                                    {
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
                                    };
                                    response
                                }
                                Ok(_) => json_response(
                                    StatusCode::BAD_REQUEST,
                                    json!({ "ok": false, "error": "body must contain string operation" }),
                                ),
                                Err(_) => json_response(
                                    StatusCode::BAD_REQUEST,
                                    json!({ "ok": false, "error": "malformed JSON body" }),
                                ),
                            }
                        }
                        Err(_) => json_response(
                            StatusCode::PAYLOAD_TOO_LARGE,
                            json!({ "ok": false, "error": "request body exceeds 1 MiB" }),
                        ),
                    }
                }
                (&Method::POST, "/v1/apps/resolve") => {
                    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
                    match body.collect().await {
                        Ok(collected) => {
                            match serde_json::from_slice::<LaunchRequest>(&collected.to_bytes()) {
                                Ok(resolve) => match package_service
                                    .resolve_app(&resolve.id, resolve.version.as_deref())
                                {
                                    Ok(resolved) => json_response(
                                        StatusCode::OK,
                                        resolve_payload(&resolved.package),
                                    ),
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
                        Err(_) => json_response(
                            StatusCode::PAYLOAD_TOO_LARGE,
                            json!({ "ok": false, "error": "request body exceeds 1 MiB" }),
                        ),
                    }
                }
                (&Method::POST, "/v1/apps/launch") => {
                    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
                    match body.collect().await {
                        Ok(collected) => {
                            match serde_json::from_slice::<LaunchRequest>(&collected.to_bytes()) {
                                Ok(launch) => match register_package_definitions(
                                    &package_service,
                                    &dispatcher,
                                )
                                .await
                                {
                                    Err(_) => json_response(
                                        StatusCode::SERVICE_UNAVAILABLE,
                                        json!({ "ok": false, "error": "package definition registration failed" }),
                                    ),
                                    Ok(()) => match package_service
                                        .resolve_app(&launch.id, launch.version.as_deref())
                                    {
                                        Ok(resolved) => {
                                            let package = resolved.package;
                                            let mut leases = launch_leases
                                                .lock()
                                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                                            let ttl = leases.ttl;
                                            let grant = resolved.grant;
                                            let lease_result = leases.create_with_typed_grant(
                                                AssetGrant {
                                                    id: package.id.clone(),
                                                    version: package.version.clone(),
                                                    hash: package.hash.clone(),
                                                },
                                                grant,
                                            );
                                            match lease_result {
                                                Ok(lease) => json_response(
                                                    StatusCode::OK,
                                                    launch_payload(
                                                        http_port, &lease, &package, ttl,
                                                    ),
                                                ),
                                                Err(LeaseCapacityError) => json_response(
                                                    StatusCode::TOO_MANY_REQUESTS,
                                                    json!({ "ok": false, "error": "launch capacity reached" }),
                                                ),
                                            }
                                        }
                                        Err(_) => json_response(
                                            StatusCode::NOT_FOUND,
                                            json!({ "ok": false, "error": "app not found" }),
                                        ),
                                    },
                                },
                                Err(_) => json_response(
                                    StatusCode::BAD_REQUEST,
                                    json!({ "ok": false, "error": "malformed launch request" }),
                                ),
                            }
                        }
                        Err(_) => json_response(
                            StatusCode::PAYLOAD_TOO_LARGE,
                            json!({ "ok": false, "error": "request body exceeds 1 MiB" }),
                        ),
                    }
                }
                (&Method::POST, path)
                    if path.starts_with("/v1/apps/launch/") && path.ends_with("/ark") =>
                {
                    let launch_id = path
                        .strip_prefix("/v1/apps/launch/")
                        .and_then(|value| value.strip_suffix("/ark"))
                        .filter(|value| uuid::Uuid::parse_str(value).is_ok());
                    let Some(launch_id) = launch_id else {
                        return Ok(json_response(
                            StatusCode::NOT_FOUND,
                            json!({ "ok": false, "error": "launch not found" }),
                        ));
                    };
                    let Some(launch_token) = request
                        .headers()
                        .get(APP_LAUNCH_TOKEN_HEADER)
                        .and_then(|value| value.to_str().ok())
                    else {
                        return Ok(json_response(
                            StatusCode::UNAUTHORIZED,
                            json!({ "ok": false, "error": "missing launch token" }),
                        ));
                    };
                    let Some((asset_grant, typed_grant)) = ({
                        let mut leases = launch_leases
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        leases.typed_grant(launch_id, launch_token)
                    }) else {
                        return Ok(json_response(
                            StatusCode::FORBIDDEN,
                            json!({ "ok": false, "error": "launch authority denied" }),
                        ));
                    };
                    if typed_grant.package_id != asset_grant.id
                        || typed_grant.package_version != asset_grant.version
                        || !launch_binding_current(&package_service, &asset_grant, &typed_grant)
                    {
                        return Ok(json_response(
                            StatusCode::FORBIDDEN,
                            json!({ "ok": false, "error": "launch authority denied" }),
                        ));
                    }
                    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
                    let collected = match body.collect().await {
                        Ok(collected) => collected,
                        Err(_) => {
                            return Ok(json_response(
                                StatusCode::PAYLOAD_TOO_LARGE,
                                json!({ "ok": false, "error": "request body exceeds 1 MiB" }),
                            ));
                        }
                    };
                    let (wire_request_id, operation, params) = match parse_app_rpc(
                        serde_json::from_slice(&collected.to_bytes()).unwrap_or(Value::Null),
                        &typed_grant,
                    ) {
                        Ok(value) => value,
                        Err(error) => {
                            return Ok(json_response(
                                StatusCode::BAD_REQUEST,
                                json!({ "ok": false, "error": error }),
                            ));
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
                            return Ok(json_response(
                                StatusCode::FORBIDDEN,
                                json!({ "ok": false, "error": error }),
                            ));
                        }
                    };
                    let owner = match dispatcher.allocate_owner() {
                        Ok(owner) => owner,
                        Err(error) => {
                            return Ok(json_response(
                                StatusCode::SERVICE_UNAVAILABLE,
                                json!({ "ok": false, "error": error.to_string() }),
                            ));
                        }
                    };
                    let owner_id = owner.id();
                    let request = DispatchRequest {
                        request_id: wire_request_id.or_else(|| Some(request_id())),
                        operation: Operation::Named(operation.clone()),
                        params,
                        client: DispatchClient {
                            connection_id: Some(owner_id),
                            ..app_client.clone()
                        },
                    };
                    let Some((_operation_id, receiver, mut response_guard)) =
                        operations.start(request, dispatcher.clone(), owner).await
                    else {
                        return Ok(json_response(
                            StatusCode::SERVICE_UNAVAILABLE,
                            json!({ "ok": false, "error": "HTTP operation capacity exhausted or server shutting down" }),
                        ));
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
                            )
                            .await
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
                (&Method::POST, path)
                    if path.starts_with("/v1/apps/launch/") && path.ends_with("/renew") =>
                {
                    let launch_id = path
                        .strip_prefix("/v1/apps/launch/")
                        .and_then(|value| value.strip_suffix("/renew"))
                        .filter(|value| uuid::Uuid::parse_str(value).is_ok());
                    let Some(launch_id) = launch_id else {
                        return Ok(json_response(
                            StatusCode::NOT_FOUND,
                            json!({ "ok": false, "error": "launch not found" }),
                        ));
                    };
                    let Some(launch_token) = request
                        .headers()
                        .get(APP_LAUNCH_TOKEN_HEADER)
                        .and_then(|value| value.to_str().ok())
                    else {
                        return Ok(json_response(
                            StatusCode::UNAUTHORIZED,
                            json!({ "ok": false, "error": "missing launch token" }),
                        ));
                    };
                    let current = launch_leases
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .typed_grant(launch_id, launch_token)
                        .filter(|(asset, grant)| {
                            launch_binding_current(&package_service, asset, grant)
                        });
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
                (&Method::DELETE, path) if path.starts_with("/v1/apps/launch/") => {
                    if let Some(launch_id) = path
                        .strip_prefix("/v1/apps/launch/")
                        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
                    {
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
                    } else {
                        json_response(
                            StatusCode::NOT_FOUND,
                            json!({ "ok": false, "error": "launch not found" }),
                        )
                    }
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
            },
            Err(response) => response,
        }
    };
    Ok(response)
}

async fn serve_asset(
    path: &str,
    package_service: Arc<PackageService>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
) -> HttpResponse {
    let Some(rest) = path.strip_prefix("/v1/apps/assets/") else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let Some((token, raw_asset)) = rest.split_once('/') else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    if token.is_empty()
        || token.len() > 128
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return asset_error(StatusCode::NOT_FOUND);
    }
    let Some(asset) = percent_decode(raw_asset) else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let grant = launch_leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .asset(token);
    let Some(grant) = grant else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let bytes = match package_service.read_app_asset(&grant.id, &grant.version, &grant.hash, &asset)
    {
        Ok(bytes) => bytes,
        Err(_) => return asset_error(StatusCode::NOT_FOUND),
    };
    asset_response(&asset, bytes)
}

fn asset_response(asset: &str, bytes: Vec<u8>) -> HttpResponse {
    let html = asset.rsplit_once('.').is_some_and(|(_, ext)| {
        ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm")
    });
    let content_type = mime_type(asset);
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header("cache-control", "no-store")
        .header("referrer-policy", "no-referrer")
        .header("x-content-type-options", "nosniff");
    if html {
        response = response.header("content-security-policy", "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'");
    }
    response
        .body(Full::new(Bytes::from(bytes)))
        .unwrap_or_else(|_| asset_error(StatusCode::INTERNAL_SERVER_ERROR))
}

fn asset_error(status: StatusCode) -> HttpResponse {
    Response::builder()
        .status(status)
        .header("cache-control", "no-store")
        .header("referrer-policy", "no-referrer")
        .body(Full::new(Bytes::new()))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
}

#[derive(Debug, Deserialize)]
struct AppRpcEnvelope {
    #[serde(rename = "_req_id", alias = "id", default)]
    request_id: Option<String>,
    operation: String,
    #[serde(default)]
    params: Value,
}

const APP_GRAPH_READS: &[&str] = &[
    "list_object_types",
    "list_objects",
    "list_objects_by_type",
    "list_object_summaries",
    "list_object_summaries_by_type",
    "get_object",
    "get_object_type",
    "search_objects",
    "list_object_links",
];
const APP_GRAPH_WRITES: &[&str] = &[
    "upsert_object",
    "delete_object",
    "upsert_object_link",
    "delete_object_link",
];
const APP_DICTATION_OPERATIONS: &[&str] = &[
    "dictation.get_state",
    "dictation.get_config",
    "dictation.start_recording",
    "dictation.cancel",
];

fn parse_app_rpc(
    value: Value,
    grant: &LaunchGrant,
) -> Result<(Option<String>, String, Value), &'static str> {
    let envelope: AppRpcEnvelope = serde_json::from_value(value)
        .map_err(|_| "app request must contain operation and params")?;
    if !APP_GRAPH_READS.contains(&envelope.operation.as_str())
        && !APP_GRAPH_WRITES.contains(&envelope.operation.as_str())
        && !(APP_DICTATION_OPERATIONS.contains(&envelope.operation.as_str())
            && grant.allows_dictation_operation(&envelope.operation))
    {
        return Err("unsupported app operation");
    }
    let params = if envelope.params.is_null() {
        Value::Object(serde_json::Map::new())
    } else if envelope.params.is_object() {
        envelope.params
    } else {
        return Err("app params must be an object");
    };
    Ok((envelope.request_id, envelope.operation, params))
}

fn canonical_type_id(type_id: &str) -> String {
    ark_core::canonical_types::definitions::canonical_type_registrations()
        .ok()
        .and_then(|registrations| {
            registrations.into_iter().find_map(|registration| {
                (registration.type_id == type_id
                    || registration
                        .aliases
                        .iter()
                        .any(|alias| alias.alias == type_id))
                .then_some(registration.type_id)
            })
        })
        .unwrap_or_else(|| type_id.to_owned())
}

fn param_str<'a>(params: &'a Value, snake: &str, camel: &str) -> Option<&'a str> {
    params
        .get(snake)
        .or_else(|| params.get(camel))
        .and_then(Value::as_str)
}

fn grant_rule_matches<'a>(
    grant: &'a LaunchGrant,
    type_id: &str,
    type_version: Option<&str>,
    action: &str,
) -> Option<&'a crate::runtime_grants::GrantRule> {
    grant.rules.iter().find(|rule| {
        rule.type_id == type_id
            && rule.actions.contains(action)
            && type_version.is_none_or(|version| {
                semver::Version::parse(version).ok().is_some_and(|version| {
                    rule.versions.iter().any(|requirement| {
                        semver::VersionReq::parse(requirement)
                            .ok()
                            .is_some_and(|requirement| requirement.matches(&version))
                    })
                })
            })
    })
}

fn grant_authorizes(
    grant: &LaunchGrant,
    type_id: &str,
    type_version: Option<&str>,
    action: &str,
    fields: &[String],
    relations: &[String],
) -> Result<(), &'static str> {
    let rule =
        grant_rule_matches(grant, type_id, type_version, action).ok_or("data grant denied")?;
    let allowed_fields = if matches!(action, "read" | "subscribe") {
        &rule.fields_read
    } else {
        &rule.fields_write
    };
    let allowed_relations = if matches!(action, "read" | "subscribe") {
        &rule.relations_read
    } else {
        &rule.relations_write
    };
    if fields.iter().any(|field| !allowed_fields.contains(field))
        || relations
            .iter()
            .any(|relation| !allowed_relations.contains(relation))
    {
        return Err("data grant denied");
    }
    Ok(())
}

fn broad_read_authorized(grant: &LaunchGrant) -> bool {
    grant.rules.iter().any(|rule| rule.actions.contains("read"))
}

fn object_field_inputs(object: &Value) -> Result<(Vec<FieldInput>, Vec<String>), &'static str> {
    let map = object.as_object().ok_or("object must be an object")?;
    let mut fields = Vec::new();
    let mut names = Vec::new();
    for (key, field_id) in [
        ("title", "title"),
        ("contentJson", "content"),
        ("content_json", "content"),
    ] {
        if let Some(value) = map.get(key) {
            if key == "content_json" && map.contains_key("contentJson") {
                continue;
            }
            fields.push(FieldInput {
                field_id: field_id.to_owned(),
                value: serde_json::from_value(value.clone()).map_err(|_| "invalid object field")?,
            });
            names.push(field_id.to_owned());
        }
    }
    let props = map.get("propsJson").or_else(|| map.get("props_json"));
    if let Some(Value::Object(props)) = props {
        for (key, value) in props {
            let field_id = format!("props.{key}");
            fields.push(FieldInput {
                field_id: field_id.clone(),
                value: serde_json::from_value(value.clone()).map_err(|_| "invalid object field")?,
            });
            names.push(field_id);
        }
    } else if props.is_some() {
        return Err("propsJson must be an object");
    }
    Ok((fields, names))
}

fn object_type_and_version(object: &Value) -> Result<(String, String), &'static str> {
    let type_id = param_str(object, "type_id", "typeId").ok_or("object type is required")?;
    let canonical = canonical_type_id(type_id);
    let type_version = param_str(object, "type_version", "typeVersion")
        .map(str::to_owned)
        .or_else(|| {
            ark_core::canonical_types::definitions::canonical_type_registrations()
                .ok()
                .and_then(|registrations| {
                    registrations
                        .into_iter()
                        .find(|registration| registration.type_id == canonical)
                        .map(|registration| registration.version)
                })
        })
        .ok_or("object type version is required")?;
    Ok((canonical, type_version))
}

fn object_type_and_version_for_write(
    object: &Value,
    grant: &LaunchGrant,
) -> Result<(String, String), &'static str> {
    let type_id =
        canonical_type_id(param_str(object, "type_id", "typeId").ok_or("object type is required")?);
    if let Some(version) = param_str(object, "type_version", "typeVersion") {
        return Ok((type_id, version.to_owned()));
    }
    let version = grant
        .rules
        .iter()
        .find(|rule| rule.type_id == type_id)
        .and_then(|rule| (rule.versions.len() == 1).then(|| &rule.versions[0]))
        .and_then(|requirement| requirement.strip_prefix('='))
        .filter(|version| semver::Version::parse(version).is_ok())
        .ok_or("object type version is required")?;
    Ok((type_id, version.to_owned()))
}

fn data_request_allowed(grant: &LaunchGrant, request: DataRequest) -> Result<(), &'static str> {
    grant
        .authorize_request(&request)
        .map_err(|_| "data grant denied")
}

async fn internal_app_lookup(
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
    operation: &str,
    params: Value,
) -> Option<Value> {
    let request = DispatchRequest {
        request_id: Some(request_id()),
        operation: Operation::Named(operation.to_owned()),
        params,
        client: client.clone(),
    };
    let response = dispatcher.dispatch(request).await.ok()?;
    if response.get("ok").is_some() {
        response
            .get("ok")
            .and_then(Value::as_bool)
            .filter(|ok| *ok)
            .and_then(|_| response.get("data").cloned())
    } else {
        Some(response)
    }
}

fn launch_binding_current(
    package_service: &PackageService,
    asset: &AssetGrant,
    grant: &LaunchGrant,
) -> bool {
    package_service
        .resolve_app(&asset.id, Some(&asset.version))
        .ok()
        .and_then(|resolved| {
            (resolved.package.hash.eq_ignore_ascii_case(&asset.hash)
                && resolved.grant.package_id == grant.package_id
                && resolved.grant.package_version == grant.package_version
                && resolved.grant.manifest_digest == grant.manifest_digest)
                .then_some(())
        })
        .is_some()
}

async fn authorize_app_request(
    operation: &str,
    mut params: Value,
    grant: &LaunchGrant,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
) -> Result<Value, &'static str> {
    let map = params
        .as_object_mut()
        .ok_or("app params must be an object")?;
    match operation {
        "dictation.get_state"
        | "dictation.get_config"
        | "dictation.start_recording"
        | "dictation.cancel" => {
            if !grant.allows_dictation_operation(operation) {
                return Err("dictation grant denied");
            }
        }
        "list_objects_by_type" | "list_object_summaries_by_type" => {
            let raw_type = map
                .get("type_id")
                .or_else(|| map.get("typeId"))
                .and_then(Value::as_str)
                .ok_or("type_id is required")?;
            let canonical = canonical_type_id(raw_type);
            grant_authorizes(grant, &canonical, None, "read", &[], &[])?;
            map.insert("type_id".into(), Value::String(canonical));
            map.remove("typeId");
        }
        "list_objects" | "list_object_summaries" | "list_object_types" => {
            if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "get_object" => {
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object id is required")?;
            let object = internal_app_lookup(dispatcher, client, "get_object", json!({ "id": id }))
                .await
                .ok_or("data grant denied")?;
            if let Some(object) = object.as_object() {
                let (type_id, version) = object_type_and_version(&Value::Object(object.clone()))?;
                grant_authorizes(grant, &type_id, Some(&version), "read", &[], &[])?;
            } else if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "get_object_type" => {
            let raw_type = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object type id is required")?;
            let canonical = canonical_type_id(raw_type);
            grant_authorizes(grant, &canonical, None, "read", &[], &[])?;
            map.insert("id".into(), Value::String(canonical));
        }
        "search_objects" => {
            if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "list_object_links" => {
            if !broad_read_authorized(grant) {
                return Err("data grant denied");
            }
        }
        "upsert_object" => {
            let object = map.get("object").ok_or("object is required")?;
            let (type_id, type_version) = object_type_and_version_for_write(object, grant)?;
            let (fields, _) = object_field_inputs(object)?;
            let object_id = param_str(object, "id", "id").map(str::to_owned);
            let create_request = || DataRequest::CreateObject {
                type_id: type_id.clone(),
                type_version: type_version.clone(),
                object_id: object_id.clone(),
                fields: fields.clone(),
                links: vec![],
            };
            let update_request = |object_id: String| DataRequest::UpdateObject {
                type_id: type_id.clone(),
                type_version: type_version.clone(),
                object_id,
                expected_hlc: None,
                fields: fields.clone(),
                links: vec![],
            };
            let (create, update) = match object_id.as_deref() {
                None => (data_request_allowed(grant, create_request()).is_ok(), false),
                Some(object_id) => {
                    let existing = internal_app_lookup(
                        dispatcher,
                        client,
                        "get_object",
                        json!({ "id": object_id }),
                    )
                    .await
                    .ok_or("data grant denied")?;
                    if existing.is_null() {
                        (data_request_allowed(grant, create_request()).is_ok(), false)
                    } else {
                        let (existing_type, existing_version) = object_type_and_version(&existing)?;
                        if existing_type != type_id || existing_version != type_version {
                            return Err("data grant denied");
                        }
                        (
                            false,
                            data_request_allowed(grant, update_request(object_id.to_owned()))
                                .is_ok(),
                        )
                    }
                }
            };
            if !create && !update {
                return Err("data grant denied");
            }
            if let Some(object) = map.get_mut("object").and_then(Value::as_object_mut) {
                object.retain(|key, _| {
                    matches!(
                        key.as_str(),
                        "id" | "typeId"
                            | "type_id"
                            | "typeVersion"
                            | "type_version"
                            | "title"
                            | "contentJson"
                            | "content_json"
                            | "propsJson"
                            | "props_json"
                    )
                });
                object.insert("typeId".into(), Value::String(type_id));
                object.insert("typeVersion".into(), Value::String(type_version));
                object.remove("type_id");
                object.remove("type_version");
            }
        }
        "delete_object" => {
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object id is required")?;
            let object = internal_app_lookup(dispatcher, client, "get_object", json!({ "id": id }))
                .await
                .ok_or("data grant denied")?;
            let (type_id, version) = object_type_and_version(&object)?;
            data_request_allowed(
                grant,
                DataRequest::DeleteObject {
                    type_id,
                    type_version: version,
                    object_id: id.to_owned(),
                    expected_hlc: None,
                },
            )?;
        }
        "upsert_object_link" => {
            let link = map
                .get("object_link")
                .or_else(|| map.get("objectLink"))
                .ok_or("object_link is required")?;
            let (source_id, target_id, relation) = link_parts(link)?;
            let source =
                internal_app_lookup(dispatcher, client, "get_object", json!({ "id": source_id }))
                    .await
                    .ok_or("data grant denied")?;
            let target =
                internal_app_lookup(dispatcher, client, "get_object", json!({ "id": target_id }))
                    .await
                    .ok_or("data grant denied")?;
            authorize_link(grant, &source, &target, relation, true)?;
        }
        "delete_object_link" => {
            let id = map
                .get("id")
                .and_then(Value::as_str)
                .ok_or("object link id is required")?;
            let links = internal_app_lookup(dispatcher, client, "list_object_links", json!({}))
                .await
                .ok_or("data grant denied")?;
            let link = links
                .as_array()
                .and_then(|links| {
                    links
                        .iter()
                        .find(|link| link.get("id").and_then(Value::as_str) == Some(id))
                })
                .ok_or("data grant denied")?;
            let (source_id, target_id, relation) = link_parts(link)?;
            let source =
                internal_app_lookup(dispatcher, client, "get_object", json!({ "id": source_id }))
                    .await
                    .ok_or("data grant denied")?;
            let target =
                internal_app_lookup(dispatcher, client, "get_object", json!({ "id": target_id }))
                    .await
                    .ok_or("data grant denied")?;
            authorize_link(grant, &source, &target, relation, true)?;
        }
        _ => return Err("unsupported app operation"),
    }
    Ok(params)
}

fn link_parts(link: &Value) -> Result<(&str, &str, &str), &'static str> {
    let source = param_str(link, "source_object_id", "sourceObjectId")
        .ok_or("source_object_id is required")?;
    let target = param_str(link, "target_object_id", "targetObjectId")
        .ok_or("target_object_id is required")?;
    let relation = param_str(link, "link_type", "linkType").ok_or("link_type is required")?;
    Ok((source, target, relation))
}

fn authorize_link(
    grant: &LaunchGrant,
    source: &Value,
    target: &Value,
    relation: &str,
    write: bool,
) -> Result<(), &'static str> {
    let (source_type, source_version) = object_type_and_version(source)?;
    let (target_type, target_version) = object_type_and_version(target)?;
    let source_rule = grant_rule_matches(
        grant,
        &source_type,
        Some(&source_version),
        if write { "link" } else { "read" },
    )
    .ok_or("data grant denied")?;
    let relations = if write {
        &source_rule.relations_write
    } else {
        &source_rule.relations_read
    };
    if !relations.iter().any(|value| value == relation) {
        return Err("data grant denied");
    }
    grant_authorizes(grant, &target_type, Some(&target_version), "read", &[], &[])
}

fn filter_object_value(value: &Value, grant: &LaunchGrant) -> Option<Value> {
    let mut object = value.as_object()?.clone();
    let (type_id, version) = object_type_and_version(value).ok()?;
    let rule = grant_rule_matches(grant, &type_id, Some(&version), "read")?;
    if !rule.fields_read.iter().any(|field| field == "title") {
        object.remove("title");
    }
    if !rule.fields_read.iter().any(|field| field == "content") {
        object.remove("contentJson");
        object.remove("content_json");
    }
    let props_key = if object.contains_key("propsJson") {
        Some("propsJson")
    } else if object.contains_key("props_json") {
        Some("props_json")
    } else {
        None
    };
    if let Some(props) = props_key
        .and_then(|key| object.get_mut(key))
        .and_then(Value::as_object_mut)
    {
        props.retain(|key, _| {
            rule.fields_read
                .iter()
                .any(|field| field == &format!("props.{key}"))
        });
    }
    Some(Value::Object(object))
}

fn filter_object_array(value: &mut Value, grant: &LaunchGrant) {
    let Some(objects) = value.as_array_mut() else {
        return;
    };
    objects.retain_mut(|object| {
        let Some(filtered) = filter_object_value(object, grant) else {
            return false;
        };
        *object = filtered;
        true
    });
}

fn filter_type_value(value: &Value, grant: &LaunchGrant) -> Option<Value> {
    let mut object = value.as_object()?.clone();
    let id = object
        .get("id")
        .or_else(|| object.get("typeId"))
        .and_then(Value::as_str)?;
    let canonical = canonical_type_id(id);
    let rule = grant_rule_matches(grant, &canonical, None, "read")?;
    for key in ["schemaJson", "uiSchemaJson"] {
        let Some(document) = object.get_mut(key) else {
            continue;
        };
        let Some(mut document_json) = document
            .as_str()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        else {
            continue;
        };
        let allowed_props = rule
            .fields_read
            .iter()
            .filter_map(|field| field.strip_prefix("props."))
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(properties) = document_json
            .get_mut("properties")
            .and_then(Value::as_object_mut)
        {
            properties.retain(|field, _| allowed_props.contains(field.as_str()));
        }
        if let Some(required) = document_json
            .get_mut("required")
            .and_then(Value::as_array_mut)
        {
            required.retain(|field| {
                field
                    .as_str()
                    .is_some_and(|field| allowed_props.contains(field))
            });
        }
        for field in [
            "visibleFields",
            "hiddenFields",
            "featuredFields",
            "readOnlyFields",
            "fieldOrder",
        ] {
            if let Some(values) = document_json.get_mut(field).and_then(Value::as_array_mut) {
                values.retain(|value| {
                    value
                        .as_str()
                        .is_some_and(|value| allowed_props.contains(value))
                });
            }
        }
        if let Ok(raw) = serde_json::to_string(&document_json) {
            *document = Value::String(raw);
        }
    }
    Some(Value::Object(object))
}

async fn filter_link_array(
    value: &mut Value,
    grant: &LaunchGrant,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
) {
    let Some(links) = value.as_array_mut() else {
        return;
    };
    let mut filtered = Vec::with_capacity(links.len());
    for link in links.iter() {
        let Ok((source_id, target_id, relation)) = link_parts(link) else {
            continue;
        };
        let Some(source) =
            internal_app_lookup(dispatcher, client, "get_object", json!({ "id": source_id })).await
        else {
            continue;
        };
        let Some(target) =
            internal_app_lookup(dispatcher, client, "get_object", json!({ "id": target_id })).await
        else {
            continue;
        };
        if authorize_link(grant, &source, &target, relation, false).is_ok() {
            filtered.push(link.clone());
        }
    }
    *links = filtered;
}

async fn filter_app_response(
    operation: &str,
    response: Value,
    grant: &LaunchGrant,
    dispatcher: &crate::engine_dispatch::EngineDispatcher,
    client: &DispatchClient,
) -> Value {
    if response.get("ok").and_then(Value::as_bool) == Some(false) {
        return response;
    }
    let mut response = response;
    let data_is_wrapped = response.get("data").is_some();
    let data = if data_is_wrapped {
        response.get_mut("data").expect("data exists")
    } else {
        &mut response
    };
    match operation {
        "list_objects"
        | "list_objects_by_type"
        | "list_object_summaries"
        | "list_object_summaries_by_type" => {
            filter_object_array(data, grant);
        }
        "get_object" => {
            if let Some(filtered) = filter_object_value(data, grant) {
                *data = filtered;
            } else {
                *data = Value::Null;
            }
        }
        "list_object_types" => {
            if let Some(types) = data.as_array_mut() {
                types.retain_mut(|item| {
                    let Some(filtered) = filter_type_value(item, grant) else {
                        return false;
                    };
                    *item = filtered;
                    true
                });
            }
        }
        "get_object_type" => {
            if let Some(filtered) = filter_type_value(data, grant) {
                *data = filtered;
            } else {
                *data = Value::Null;
            }
        }
        "search_objects" => {
            if let Some(results) = data.as_array_mut() {
                let mut filtered = Vec::with_capacity(results.len());
                for result in results.iter() {
                    let Some(id) = result.get("entryId").and_then(Value::as_str) else {
                        continue;
                    };
                    let Some(object) =
                        internal_app_lookup(dispatcher, client, "get_object", json!({ "id": id }))
                            .await
                    else {
                        continue;
                    };
                    if filter_object_value(&object, grant).is_some() {
                        let mut result = result.clone();
                        // ARK search snippets can combine multiple indexed fields, so their
                        // provenance cannot be filtered safely at this compatibility boundary.
                        if let Some(map) = result.as_object_mut() {
                            map.insert("text".into(), Value::String(String::new()));
                        }
                        filtered.push(result);
                    }
                }
                *results = filtered;
            }
        }
        "list_object_links" => filter_link_array(data, grant, dispatcher, client).await,
        _ => {}
    }
    response
}

fn resolve_payload(package: &crate::package_store::InstalledPackage) -> Value {
    json!({
        "ok": true,
        "data": {
            "id": package.id,
            "version": package.version,
            "name": package.manifest.name(),
            "permissions": package.manifest.permissions(),
            "enabled": package.enabled,
            "revoked": package.revoked,
        }
    })
}

fn launch_payload(
    http_port: u16,
    lease: &LaunchLease,
    package: &crate::package_store::InstalledPackage,
    ttl: Duration,
) -> Value {
    let mut data = json!({
        "id": package.id,
        "version": package.version,
        "name": package.manifest.name(),
        "launch_url": format!("http://127.0.0.1:{http_port}/v1/apps/assets/{}/{}", lease.asset_token, package.manifest.entrypoint()),
        "permissions": package.manifest.permissions(),
        "launch_id": lease.launch_id,
        "asset_token": lease.asset_token,
        "ttl_seconds": ttl.as_secs(),
        "expires_at": lease.expires_at_rfc3339,
    });
    if let Some(token) = lease.launch_token.as_ref() {
        data["broker_token"] = Value::String(token.clone());
        data["data_api"] = Value::String(format!(
            "http://127.0.0.1:{http_port}/v1/apps/launch/{}/ark",
            lease.launch_id
        ));
        data["ttl_seconds"] = Value::from(DATA_GRANT_TTL.as_secs());
        if let Some(expires_at) = lease.grant_expires_at_rfc3339.as_ref() {
            data["expires_at"] = Value::String(expires_at.clone());
        }
        data["manifest_schema_version"] = Value::from(2);
        let effective_read_types = lease
            .typed_grant
            .as_ref()
            .map(|grant| {
                let mut ids = std::collections::BTreeSet::new();
                for rule in grant
                    .rules
                    .iter()
                    .filter(|rule| rule.actions.contains("subscribe"))
                {
                    ids.insert(rule.type_id.clone());
                    if let Ok(registrations) =
                        ark_core::canonical_types::definitions::canonical_type_registrations()
                    {
                        if let Some(registration) = registrations
                            .into_iter()
                            .find(|registration| registration.type_id == rule.type_id)
                        {
                            ids.extend(registration.aliases.into_iter().map(|alias| alias.alias));
                        }
                    }
                }
                ids.into_iter().map(Value::String).collect::<Vec<_>>()
            })
            .unwrap_or_default();
        data["effective_read_types"] = Value::Array(effective_read_types);
    }
    json!({
        "ok": true,
        "data": data
    })
}

fn new_asset_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn percent_decode(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let input = value.as_bytes();
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' {
            if index + 2 >= input.len() {
                return None;
            }
            let high = (input[index + 1] as char).to_digit(16)? as u8;
            let low = (input[index + 2] as char).to_digit(16)? as u8;
            bytes.push((high << 4) | low);
            index += 3;
        } else {
            bytes.push(input[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).ok()
}

fn mime_type(path: &str) -> &'static str {
    match path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[derive(Debug)]
struct AuthenticatedClient {
    pid: u32,
    class: String,
    version: String,
}

fn authenticate(
    headers: &HeaderMap,
    expected_token: &str,
) -> Result<AuthenticatedClient, HttpResponse> {
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if !bearer.is_some_and(|token| auth::validate_token(token, expected_token)) {
        return Err(json_response(
            StatusCode::UNAUTHORIZED,
            json!({ "ok": false, "error": "invalid bearer token" }),
        ));
    }

    let pid = headers
        .get(CLIENT_PID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u32>().ok());
    if pid.is_none_or(|pid| auth::validate_pid_belongs_to_current_user(pid).is_err()) {
        return Err(json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "invalid client PID" }),
        ));
    }

    let compatible = headers
        .get(API_VERSION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| ProtocolVersion::parse(value).ok())
        .is_some_and(|version| {
            !matches!(
                version.is_compatible_with_server(&API_VERSION_CURRENT),
                Compatibility::Incompatible
            )
        });
    if !compatible {
        return Err(json_response(
            StatusCode::UPGRADE_REQUIRED,
            json!({ "ok": false, "error": "missing or incompatible API version" }),
        ));
    }

    Ok(AuthenticatedClient {
        pid: pid.expect("validated client PID"),
        class: header_string(headers, CLIENT_CLASS_HEADER)
            .unwrap_or_else(|| "engine-http".to_string()),
        version: header_string(headers, CLIENT_VERSION_HEADER)
            .unwrap_or_else(|| API_VERSION.to_string()),
    })
}

fn header_string(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn request_id() -> String {
    format!("engine-http-{}", uuid::Uuid::new_v4())
}

fn json_response(status: StatusCode, value: Value) -> HttpResponse {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .header("cache-control", "no-store")
        .body(Full::new(Bytes::from(value.to_string())))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::from_static(b"{\"ok\":false}"))))
}
