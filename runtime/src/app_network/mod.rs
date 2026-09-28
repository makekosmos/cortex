//! Engine-owned app network operations (KOS-152 / KOS-157): Open Library book
//! metadata lookup, public-HTTPS page fetch and remote/local image helpers.
//! Apps never touch the network or the filesystem directly — they call these
//! ops and receive normalized results. Packaged launches additionally gate
//! every op behind the manifest `network` scope that owns it
//! (`bookMetadata` / `images`, see `runtime_grants`).
//!
//! Errors collapse into the fixed public classes that the app-RPC boundary
//! already exposes (`invalid-request` / `forbidden` / `not-found` /
//! `timeout` / `unavailable`) — internal details are logged, never leaked.

mod address;
mod browser_page;
mod color;
mod fetch;
mod image_ops;
mod open_library;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_ops;

use std::path::PathBuf;

use serde_json::{json, Value};

pub(crate) use address::is_public_network_address;

/// Ops exported under the `network` manifest scope `bookMetadata` /
/// `images`. `runtime_grants::app_network_operation_scope` is the single
/// source of truth; `handle` re-checks it so an unknown `bookMetadata.*`
/// name can never silently reach a handler.
pub(crate) fn is_app_network_op(operation: &str) -> bool {
    crate::runtime_grants::app_network_operation_scope(operation).is_some()
}

/// Runtime context for the ops. Production uses `engine()`; tests build a
/// `test` context pointing at a tempdir with plain-HTTP loopback fetch
/// enabled (mirrors the `package-worker-fixture` escape hatch, but scoped to
/// this module's `#[cfg(test)]` helpers only).
pub(crate) struct AppNetworkCtx {
    /// Mundus data dir — image writes land in `extension-data/<app>/…`.
    pub data_dir: PathBuf,
    /// Test-only: allow `http://` + non-public addresses in the fetch layer.
    pub allow_private_http: bool,
    /// Test-only: substitute the pinned Open Library origin.
    pub open_library_origin: Option<String>,
}

impl AppNetworkCtx {
    pub(crate) fn engine() -> Result<Self, &'static str> {
        Ok(Self {
            data_dir: crate::lock_file::mundus_data_dir().map_err(|_| "unavailable")?,
            allow_private_http: false,
            open_library_origin: None,
        })
    }
}

/// `handle_app_network_op` — normalize params, run the op, return data.
pub(crate) async fn handle(
    operation: &str,
    params: &Value,
    ctx: &AppNetworkCtx,
) -> Result<Value, &'static str> {
    if crate::runtime_grants::app_network_operation_scope(operation).is_none() {
        return Err("invalid-request");
    }
    let param_str = |key: &str| -> Result<&str, &'static str> {
        params
            .get(key)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .ok_or("invalid-request")
    };
    match operation {
        "bookMetadata.lookupIsbn" => {
            let metadata = open_library::lookup_isbn(param_str("isbn")?, ctx).await?;
            Ok(serde_json::to_value(metadata).unwrap_or(Value::Null))
        }
        "bookMetadata.fetchPage" => {
            let page = browser_page::fetch_page(param_str("url")?, ctx).await?;
            Ok(json!({ "finalUrl": page.final_url, "html": page.html }))
        }
        "images.fetch" => {
            let fetched =
                image_ops::fetch_remote(param_str("url")?, param_str("appId")?, ctx).await?;
            Ok(json!({ "path": fetched.path, "color": fetched.color }))
        }
        "images.dominantColor" => {
            let color = image_ops::dominant_color(param_str("src")?, ctx).await?;
            Ok(json!({ "color": color }))
        }
        "images.storeCover" => {
            let path = image_ops::store_cover(
                param_str("sourcePath")?,
                param_str("entryId")?,
                param_str("appId")?,
                ctx,
            )
            .await?;
            Ok(json!({ "path": path }))
        }
        _ => Err("invalid-request"),
    }
}
