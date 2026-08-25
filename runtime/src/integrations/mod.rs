use crate::ark_host::ArkHost;
pub(crate) use chrono::{DateTime, Duration as ChronoDuration, Utc};
pub(crate) use serde::Deserialize;
pub(crate) use serde_json::{json, Value};
pub(crate) use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
pub(crate) use std::sync::Arc;
pub(crate) use std::time::Duration;
#[path = "../integration-codewars.rs"]
mod integration_codewars;

mod body;
mod config;
mod credentials;
mod handler;
mod hevy;
mod http;
mod leetcode_mapping;
mod leetcode_sync;
mod ops;
mod sync;
mod toggl;
mod web_progress;
mod web_progress_fetch;

pub(crate) use body::*;
pub(crate) use config::{
    mutate_config, read_config, sync_lock, ALLOWED_INTERVALS, CODEWARS_BASE_URL,
    CODING_PROFILE_TYPE_ID, CODING_SUBMISSION_TYPE_ID, HEVY_BASE_URL, LEETCODE_GRAPHQL_URL,
    TIME_ENTRY_TYPE_ID, TOGGL_BASE_URL, WORKOUT_TYPE_ID,
};
pub use config::{IntegrationsConfig, Provider, ProviderSettings};
pub(crate) use credentials::*;
pub(crate) use hevy::*;
pub(crate) use http::*;
pub(crate) use leetcode_mapping::*;
pub(crate) use leetcode_sync::*;
pub(crate) use ops::*;
pub(crate) use sync::sync_provider;
pub(crate) use toggl::*;
use web_progress::*;
use web_progress_fetch::*;

#[cfg(test)]
mod tests_mappings;
#[cfg(test)]
mod tests_scheduler;

pub async fn handle_operation(
    subop: &str,
    params: serde_json::Value,
    ark: &ArkHost,
    data_dir: &std::path::Path,
) -> Result<serde_json::Value, String> {
    handler::handle_operation(subop, params, ark, data_dir).await
}

pub fn spawn_scheduler(ark: std::sync::Arc<ArkHost>, data_dir: std::path::PathBuf) {
    sync::spawn_scheduler(ark, data_dir);
}
