#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectoryGrantRequest {
    selected_root: String,
}

async fn handle_directory_grant(
    request: Request<Incoming>,
    client: AuthenticatedClient,
    package_service: Arc<PackageService>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
    path: &str,
) -> HttpResponse {
    if client.class != "desktop-host" {
        return json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "native host required" }),
        );
    }
    let Some(launch_id) = path
        .strip_prefix("/v1/apps/launch/")
        .and_then(|value| value.strip_suffix("/grants/directory"))
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
    else {
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
    let current = launch_leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .typed_grant(launch_id, &launch_token)
        .filter(|(asset, grant)| launch_binding_current(&package_service, asset, grant));
    let Some((asset, _)) = current else {
        return json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "launch authority denied" }),
        );
    };
    let allowed = package_service
        .resolve_app(&asset.id, Some(&asset.version))
        .is_ok_and(|resolved| {
            resolved
                .package
                .manifest
                .permissions()
                .iter()
                .any(|permission| {
                    matches!(
                        permission.capability.as_str(),
                        "filesystem.read" | "filesystem.write"
                    )
                })
        });
    if !allowed {
        return json_response(
            StatusCode::FORBIDDEN,
            json!({ "ok": false, "error": "filesystem grant denied" }),
        );
    }
    let body = Limited::new(request.into_body(), MAX_HTTP_BODY_BYTES);
    let Ok(collected) = body.collect().await else {
        return json_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            json!({ "ok": false, "error": "request body exceeds 1 MiB" }),
        );
    };
    let Ok(body) = serde_json::from_slice::<DirectoryGrantRequest>(&collected.to_bytes()) else {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({ "ok": false, "error": "malformed directory grant request" }),
        );
    };
    let selected_root = std::path::PathBuf::from(body.selected_root);
    let Some(label) = selected_root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
    else {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({ "ok": false, "error": "invalid selected directory" }),
        );
    };
    let owner = crate::grant_authority::GrantOwner {
        session_id: launch_id.to_owned(),
        generation: 1,
        connection_id: u64::from(client.pid),
    };
    let grants = package_service.grant_authority();
    let result = grants.register(
        &owner,
        &asset.id,
        &selected_root,
        false,
        crate::grant_authority::GrantProvenance::NativeDialog,
        None,
    );
    grants.close_owner(owner);
    match result {
        Ok((_, _, Some(persistent_grant_id))) => json_response(
            StatusCode::OK,
            json!(
                { "ok": true,
                "data": { "persistentGrantId": persistent_grant_id,
                "label": label } }),
        ),
        _ => json_response(
            StatusCode::BAD_REQUEST,
            json!({ "ok": false, "error": "selected directory unavailable" }),
        ),
    }
}
