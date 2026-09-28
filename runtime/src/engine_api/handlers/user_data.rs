// Persisted wire contract with pinned components — see
// docs/brand-legacy-identifiers.md.
const USER_DATA_ROOT_HEADER: &str = "x-kosmos-user-data-root";
const USER_DATA_APP_HEADER: &str = "x-kosmos-user-data-app";
const USER_DATA_KEY_HEADER: &str = "x-kosmos-user-data-key";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UserDataOperationRequest {
    operation: String,
    root: Option<String>,
    root_id: Option<String>,
    app_id: Option<String>,
    key: Option<String>,
}

fn user_data_failure(error: crate::user_data::UserDataError) -> HttpResponse {
    use crate::user_data::UserDataError::*;
    let status = match error {
        NotFound | UnknownRoot => StatusCode::NOT_FOUND,
        InvalidKey | InvalidRequest => StatusCode::BAD_REQUEST,
        TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        Io => StatusCode::INTERNAL_SERVER_ERROR,
    };
    json_response(status, json!({ "ok": false, "error": error.code() }))
}

fn user_data_ok(data: Value) -> HttpResponse {
    json_response(StatusCode::OK, json!({ "ok": true, "data": data }))
}

fn user_data_field<'a>(value: Option<&'a str>, field: &str) -> Result<&'a str, HttpResponse> {
    value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            json_response(
                StatusCode::BAD_REQUEST,
                json!({ "ok": false, "error": format!("missing {field}") }),
            )
        })
}

fn user_data_header<'a>(request: &'a Request<Incoming>, name: &str) -> Result<&'a str, HttpResponse> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            json_response(
                StatusCode::BAD_REQUEST,
                json!({ "ok": false, "error": format!("missing {name}") }),
            )
        })
}

/// `POST /v1/user-data` — app-scoped binary user data operations. Every
/// operation resolves inside a directory handle the Host pinned once, so no
/// pathname is ever re-validated and re-opened separately.
async fn handle_user_data(
    request: Request<Incoming>,
    client: AuthenticatedClient,
    user_data: Arc<crate::user_data::UserDataRoots>,
) -> HttpResponse {
    if client.class != "desktop-host" {
        return json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "forbidden" }),
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
    let parsed = match serde_json::from_slice::<UserDataOperationRequest>(&collected.to_bytes()) {
        Ok(parsed) => parsed,
        Err(_) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({ "ok": false, "error": "malformed user data request" }),
            )
        }
    };
    match parsed.operation.as_str() {
        "open_root" => {
            let root = match user_data_field(parsed.root.as_deref(), "root") {
                Ok(root) => root,
                Err(response) => return response,
            };
            match user_data.open(std::path::Path::new(root)) {
                Ok(root_id) => user_data_ok(json!({ "root_id": root_id })),
                Err(error) => user_data_failure(error),
            }
        }
        "close_root" => {
            let root_id = match user_data_field(parsed.root_id.as_deref(), "root_id") {
                Ok(root_id) => root_id,
                Err(response) => return response,
            };
            match user_data.close(root_id) {
                Ok(()) => user_data_ok(json!({ "closed": true })),
                Err(error) => user_data_failure(error),
            }
        }
        "read" | "stat" | "delete" => {
            let root_id = match user_data_field(parsed.root_id.as_deref(), "root_id") {
                Ok(root_id) => root_id,
                Err(response) => return response,
            };
            let app_id = match user_data_field(parsed.app_id.as_deref(), "app_id") {
                Ok(app_id) => app_id,
                Err(response) => return response,
            };
            let key = match user_data_field(parsed.key.as_deref(), "key") {
                Ok(key) => key,
                Err(response) => return response,
            };
            match parsed.operation.as_str() {
                "read" => match user_data.read(root_id, app_id, key) {
                    Ok(bytes) => user_data_ok(
                        json!({ "bytes": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes) }),
                    ),
                    Err(error) => user_data_failure(error),
                },
                "stat" => match user_data.stat(root_id, app_id, key) {
                    Ok(size) => user_data_ok(json!({ "size_bytes": size })),
                    Err(error) => user_data_failure(error),
                },
                _ => match user_data.delete(root_id, app_id, key) {
                    Ok(()) => user_data_ok(json!({ "deleted": true })),
                    Err(error) => user_data_failure(error),
                },
            }
        }
        _ => json_response(
            StatusCode::BAD_REQUEST,
            json!({ "ok": false, "error": "invalid-request" }),
        ),
    }
}

/// `PUT /v1/user-data` — binary write. The payload is the raw file body; the
/// root handle id, app id, and key travel in headers so the body never needs
/// to cross a JSON or WebSocket size ceiling.
async fn handle_user_data_write(
    request: Request<Incoming>,
    client: AuthenticatedClient,
    user_data: Arc<crate::user_data::UserDataRoots>,
) -> HttpResponse {
    if client.class != "desktop-host" {
        return json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "forbidden" }),
        );
    }
    let root_id = match user_data_header(&request, USER_DATA_ROOT_HEADER).map(str::to_owned) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let app_id = match user_data_header(&request, USER_DATA_APP_HEADER).map(str::to_owned) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let key = match user_data_header(&request, USER_DATA_KEY_HEADER).map(str::to_owned) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let body = Limited::new(
        request.into_body(),
        crate::user_data::USER_DATA_MAX_BYTES,
    );
    let collected = match body.collect().await {
        Ok(collected) => collected,
        Err(_) => return user_data_failure(crate::user_data::UserDataError::TooLarge),
    };
    match user_data.write(&root_id, &app_id, &key, &collected.to_bytes()) {
        Ok(size) => user_data_ok(json!({ "size_bytes": size })),
        Err(error) => user_data_failure(error),
    }
}
