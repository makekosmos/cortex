//! `apps.*` ops — the native app Store. The id resolves to its hardcoded
//! descriptor ONCE here (`native_apps::app_descriptor`); every sub-op then
//! works with the descriptor — unknown or malformed ids answer not-found
//! before they can name a path.

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use super::*;
use crate::native_apps::{self, NativeAppDescriptor};

pub(in crate::ws_server) async fn handle_apps_op(
    subop: &str,
    params: Value,
    service: &Arc<PackageService>,
) -> LocalResponse {
    match subop {
        // `refresh` revalidates each app's release over the wire; without it
        // the list serves the TTL/ETag cache.
        "list" => ok_or_err(
            subop,
            service
                .native_apps_refresh(
                    params
                        .get("refresh")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )
                .await
                .map(|apps| AppsList { apps }),
        ),
        // The install is a background job: this returns the row with
        // `state: "installing"` immediately; `apps.list` reports progress.
        "install" => match resolve_app(subop, &params) {
            Ok(desc) => ok_or_err(
                subop,
                service
                    .start_native_install(desc)
                    .map(|app| InstalledApp { app }),
            ),
            Err(response) => response,
        },
        "uninstall" => match resolve_app(subop, &params) {
            Ok(desc) => {
                // Filesystem delete is blocking — keep it off the async
                // worker like the other store mutations.
                let service = Arc::clone(service);
                ok_or_err(
                    subop,
                    package_blocking(move || service.uninstall_native_app(desc))
                        .await
                        .map(|()| Value::Bool(true)),
                )
            }
            Err(response) => response,
        },
        "open" => match resolve_app(subop, &params) {
            Ok(desc) => ok_or_err(
                subop,
                service.open_native_app(desc).map(|()| Value::Bool(true)),
            ),
            Err(response) => response,
        },
        _ => LocalResponse::err(format!("apps.{subop}: unknown sub-operation")),
    }
}

#[derive(Serialize)]
struct AppsList {
    apps: Vec<crate::package_service::NativeAppSummary>,
}

#[derive(Serialize)]
struct InstalledApp {
    app: crate::package_service::NativeAppSummary,
}

/// The single app-id resolver: `id` must name a hardcoded descriptor —
/// anything else, including `.`, `..` and unknown ids, is `not-found`.
fn resolve_app(subop: &str, params: &Value) -> Result<&'static NativeAppDescriptor, LocalResponse> {
    params
        .get("id")
        .and_then(Value::as_str)
        .and_then(native_apps::app_descriptor)
        .ok_or_else(|| LocalResponse::err(format!("apps.{subop}: not-found")))
}

/// One response/error shape for every sub-op — `apps.<subop>: <code>`, and
/// every failure logged once here.
fn ok_or_err<T: Serialize>(
    subop: &str,
    result: Result<T, crate::package_service::PackageError>,
) -> LocalResponse {
    match result {
        Ok(value) => match serde_json::to_value(value) {
            Ok(value) => LocalResponse::ok(value),
            Err(error) => {
                LocalResponse::err(format!("apps.{subop}: serialization-failed ({error})"))
            }
        },
        Err(error) => {
            tracing::warn!(
                target: "native_apps",
                op = %format!("apps.{subop}"),
                %error,
                "apps op failed"
            );
            LocalResponse::err(format!(
                "apps.{subop}: {}",
                crate::package_service::native_error_code(&error)
            ))
        }
    }
}

#[cfg(test)]
#[path = "apps_tests.rs"]
mod tests;
