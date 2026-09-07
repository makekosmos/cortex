use crate::ark_host::ArkHost;
pub(crate) use chrono::{DateTime, Duration as ChronoDuration, Utc};
pub(crate) use serde_json::{json, Value};
pub(crate) use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

mod body;
mod config;
mod credential_envelope;
mod handler;
mod replication_consumer;

pub(crate) use body::body_snapshot;
pub(crate) use config::{mutate_config, read_config};

// ponytail: global replication lock; per-integration locks if throughput matters.
static REPLICATION_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

pub async fn handle_operation(
    subop: &str,
    params: serde_json::Value,
    ark: &ArkHost,
    data_dir: &std::path::Path,
    packages: &crate::package_service::PackageService,
) -> Result<serde_json::Value, String> {
    let _replication_guard = if subop.starts_with("replication_") {
        Some(
            REPLICATION_LOCK
                .get_or_init(|| tokio::sync::Mutex::new(()))
                .lock()
                .await,
        )
    } else {
        None
    };
    handler::handle_operation(subop, params, ark, data_dir, packages).await
}
