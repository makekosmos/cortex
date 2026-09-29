use super::*;

fn app_id<'a>(subop: &'a str, params: &'a serde_json::Value) -> Result<&'a str, LocalResponse> {
    params
        .get("id")
        .or_else(|| params.get("app_id"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| LocalResponse::err(format!("apps.{subop}: invalid-request")))
}

fn ok_or_err(
    subop: &str,
    result: Result<serde_json::Value, crate::package_service::PackageError>,
) -> LocalResponse {
    match result {
        Ok(data) => LocalResponse::ok(data),
        // `app-running` is the clearest refusal the UI can offer — the rest
        // collapse to the package code mapping.
        Err(crate::package_service::PackageError::Worker("app-running")) => {
            LocalResponse::err(format!("apps.{subop}: app-running"))
        }
        Err(error) => package_response::<serde_json::Value>(subop, Err(error)),
    }
}

pub(in crate::ws_server) async fn handle_apps_op(
    subop: &str,
    params: serde_json::Value,
    service: &Arc<PackageService>,
) -> LocalResponse {
    match subop {
        "list" => ok_or_err(
            subop,
            service
                .native_apps()
                .map(|apps| serde_json::json!({ "apps": apps })),
        ),
        // Update is install-at-latest: a new version dir, pointer flip, old
        // dir dropped — the store keeps the previous install live on failure.
        "install" | "update" => {
            let id = match app_id(subop, &params) {
                Ok(id) => id.to_owned(),
                Err(response) => return response,
            };
            let version = params
                .get("version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            ok_or_err(
                subop,
                service
                    .install_native_app(&id, version.as_deref())
                    .await
                    .map(|summary| serde_json::to_value(summary).unwrap_or_default()),
            )
        }
        "uninstall" => {
            let id = match app_id(subop, &params) {
                Ok(id) => id.to_owned(),
                Err(response) => return response,
            };
            ok_or_err(
                subop,
                service
                    .uninstall_native_app(&id)
                    .map(|()| serde_json::json!({ "uninstalled": true })),
            )
        }
        "open" | "launch" => {
            let id = match app_id(subop, &params) {
                Ok(id) => id.to_owned(),
                Err(response) => return response,
            };
            ok_or_err(
                subop,
                service
                    .open_native_app(&id)
                    .map(|()| serde_json::json!({ "opened": true })),
            )
        }
        "resolve" => {
            let id = match app_id(subop, &params) {
                Ok(id) => id.to_owned(),
                Err(response) => return response,
            };
            ok_or_err(
                subop,
                service
                    .native_app_executable(&id)
                    .map(|path| serde_json::json!({ "executable": path.to_string_lossy() })),
            )
        }
        other => LocalResponse::err(format!("apps.{other}: unknown sub-operation")),
    }
}
