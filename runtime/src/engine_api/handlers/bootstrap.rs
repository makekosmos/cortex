// Launch-scoped endpoints reachable by a served package page: the page holds
// no Engine bearer, so every route here authenticates on the launch
// credential itself — the one-time bootstrap code for `/bootstrap`, and the
// launch token (`X-Kosmos-Launch-Token`, or the beacon body for `/revoke`)
// for everything else.

const BOOTSTRAP_DENIED: &str = "bootstrap denied";

fn launch_id_from(path: &str, suffix: &str) -> Option<String> {
    path.strip_prefix("/v1/apps/launch/")
        .and_then(|rest| rest.strip_suffix(suffix))
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
        .map(str::to_owned)
}

fn deny_bootstrap() -> HttpResponse {
    json_response(
        StatusCode::FORBIDDEN,
        json!({ "ok": false, "error": BOOTSTRAP_DENIED }),
    )
}

/// `POST /v1/apps/launch/<id>/bootstrap` — single-use code → launch
/// credentials. Origin-locked to the Engine's own loopback origin so a
/// foreign site cannot ride a leaked fragment; every failure collapses to
/// one `403 bootstrap denied` (no oracle for wrong/expired/reused codes).
async fn handle_bootstrap(
    request: Request<Incoming>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    http_port: u16,
    path: &str,
) -> HttpResponse {
    let Some(launch_id) = launch_id_from(path, "/bootstrap") else {
        return deny_bootstrap();
    };
    let same_origin = request
        .headers()
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|origin| origin == format!("http://127.0.0.1:{http_port}"));
    if !same_origin {
        return deny_bootstrap();
    }
    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
    let Ok(collected) = body.collect().await else {
        return deny_bootstrap();
    };
    let code = serde_json::from_slice::<Value>(&collected.to_bytes())
        .ok()
        .and_then(|body| body.get("code").and_then(Value::as_str).map(str::to_owned));
    let Some(code) = code else {
        return deny_bootstrap();
    };
    let lease = launch_leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .bootstrap(&launch_id, &code);
    let Some(lease) = lease else {
        return deny_bootstrap();
    };
    let Some(token) = lease.launch_token.as_ref() else {
        return deny_bootstrap();
    };
    json_response(
        StatusCode::OK,
        json!({
            "ok": true,
            "data": {
                "launch_id": lease.launch_id,
                "id": lease.grant.id,
                "version": lease.grant.version,
                "broker_token": token,
                "data_api": format!("http://127.0.0.1:{http_port}/v1/apps/launch/{}/ark", lease.launch_id),
                "ttl_seconds": DATA_GRANT_TTL.as_secs(),
                "expires_at": lease.grant_expires_at_rfc3339,
            }
        }),
    )
}

/// `POST /v1/apps/launch/<id>/revoke` — the `pagehide` beacon variant of
/// `DELETE /v1/apps/launch/<id>`: `navigator.sendBeacon` cannot set headers,
/// so the launch token travels in the body. Untyped leases (no launch
/// token) cannot be revoked this way — they are asset-only.
async fn handle_revoke_token(
    request: Request<Incoming>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    path: &str,
) -> HttpResponse {
    let Some(launch_id) = launch_id_from(path, "/revoke") else {
        return json_response(
            StatusCode::NOT_FOUND,
            json!({ "ok": false, "error": "launch not found" }),
        );
    };
    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
    let token = match body.collect().await {
        Ok(collected) => serde_json::from_slice::<Value>(&collected.to_bytes())
            .ok()
            .and_then(|body| body.get("token").and_then(Value::as_str).map(str::to_owned)),
        Err(_) => None,
    };
    let revoked = token.is_some_and(|token| {
        launch_leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .revoke_with_token(&launch_id, &token)
    });
    if revoked {
        json_response(
            StatusCode::OK,
            json!({ "ok": true, "data": { "launch_id": launch_id, "revoked": true } }),
        )
    } else {
        json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "launch authority denied" }),
        )
    }
}

/// Type id an ARK event carries, wherever the producer put it — mirrors the
/// deleted host's `entityType()` lookup chain (top level, `entity`, and
/// `entity.data`).
fn event_type_id(payload: &Value) -> Option<&str> {
    ["type_id", "typeId"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_str))
        .or_else(|| {
            let entity = payload.get("entity")?;
            ["type_id", "typeId"]
                .iter()
                .find_map(|key| entity.get(*key).and_then(Value::as_str))
                .or_else(|| {
                    entity
                        .get("data")
                        .and_then(|data| {
                            ["type_id", "typeId"]
                                .iter()
                                .find_map(|key| data.get(*key).and_then(Value::as_str))
                        })
                })
        })
}

fn event_allowed(
    name: &str,
    payload: &Value,
    read_types: &[String],
    event_names: &[String],
) -> bool {
    event_names.iter().any(|allowed| allowed == name)
        || event_type_id(payload).is_some_and(|type_id| {
            read_types.iter().any(|allowed| allowed == type_id)
        })
}

/// `GET /v1/apps/launch/<id>/events` — grant-filtered ARK events as SSE.
/// The lease is re-validated per frame, so a revoked or expired launch stops
/// receiving immediately instead of streaming on a dead session.
fn handle_launch_events(
    request: Request<Incoming>,
    package_service: Arc<PackageService>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    launch_events: &Option<tokio::sync::broadcast::Receiver<(String, Value)>>,
    path: &str,
) -> HttpResponse {
    let Some(launch_id) = launch_id_from(path, "/events") else {
        return json_response(
            StatusCode::NOT_FOUND,
            json!({ "ok": false, "error": "launch not found" }),
        );
    };
    let Some(launch_token) = request
        .headers()
        .get(APP_LAUNCH_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
    else {
        return json_response(
            StatusCode::UNAUTHORIZED,
            json!({ "ok": false, "error": "missing launch token" }),
        );
    };
    let authorized = launch_leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .typed_grant(&launch_id, &launch_token)
        .filter(|(asset, grant)| launch_binding_current(&package_service, asset, grant));
    let Some((_, grant)) = authorized else {
        return json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "launch authority denied" }),
        );
    };
    let Some(events) = launch_events.as_ref() else {
        return json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "ok": false, "error": "events unavailable" }),
        );
    };
    let read_types = crate::package_launch::effective_read_types(&grant);
    let event_names = crate::package_launch::effective_events(&grant);
    let rx = events.resubscribe();
    let stream = futures_util::stream::unfold(rx, {
        let launch_leases = launch_leases.clone();
        move |mut rx| {
            let launch_leases = launch_leases.clone();
            let launch_id = launch_id.clone();
            let launch_token = launch_token.clone();
            let read_types = read_types.clone();
            let event_names = event_names.clone();
            async move {
                loop {
                    let live = launch_leases
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .typed_grant(&launch_id, &launch_token)
                        .is_some();
                    if !live {
                        return None;
                    }
                    match rx.recv().await {
                        Ok((name, payload)) => {
                            if !event_allowed(&name, &payload, &read_types, &event_names) {
                                continue;
                            }
                            let frame = hyper::body::Frame::data(Bytes::from(format!(
                                "data: {}\n\n",
                                payload
                            )));
                            return Some((Ok::<_, Infallible>(frame), rx));
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                    }
                }
            }
        }
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .header("cache-control", "no-store")
        .body(
            http_body_util::StreamBody::new(stream)
                .map_err(|never: Infallible| match never {})
                .boxed(),
        )
        .unwrap_or_else(|_| asset_error(StatusCode::INTERNAL_SERVER_ERROR))
}
