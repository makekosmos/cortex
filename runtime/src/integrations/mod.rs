use crate::ark_host::ArkHost;
pub(crate) use chrono::{DateTime, Duration as ChronoDuration, Utc};
pub(crate) use serde_json::{json, Value};
pub(crate) use std::collections::HashMap;
use std::path::Path;

mod body;
mod config;
mod credential_envelope;
mod handler;
mod replication_consumer;

pub(crate) use body::body_snapshot;
pub(crate) use config::{mutate_config, read_config};

pub async fn handle_operation(
    subop: &str,
    params: serde_json::Value,
    ark: &ArkHost,
    data_dir: &std::path::Path,
    packages: &crate::package_service::PackageService,
) -> Result<serde_json::Value, String> {
    handler::handle_operation(subop, params, ark, data_dir, packages).await
}
