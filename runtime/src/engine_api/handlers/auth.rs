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
