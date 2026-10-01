//! Periodic storage maintenance (KOS-302).
//!
//! Two duties, both off the hot path:
//!   * `usage_sync_log` compaction in ark.db — the superseded-ref rule lives
//!     in ark-core (`compact_usage_sync_log` op); here we only drive it in
//!     bounded batches with a pause between calls, so the serial ARK worker
//!     stays responsive and no write lock is held across the sweep;
//!   * `file-index.db` prune + bounded VACUUM (policy in
//!     `file_index::maintenance`), on a blocking thread.
//!
//! The pass runs at most once per `MUNDUS_STORAGE_MAINTENANCE_HOURS`
//! (default 24): the last-run timestamp lives in sync_kv under
//! `mundus.last_storage_maintenance_ts`, so restarts don't re-run it.
//! Failures are logged and retried on the next interval — like db_backup,
//! nothing here gates startup.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::json;
use tokio::task::JoinHandle;

use crate::ark_host::ArkHost;
use crate::file_index::FileIndex;

const LAST_RUN_KEY: &str = "mundus.last_storage_maintenance_ts";
const COMPACT_BATCH_LIMIT: i64 = 2_000;
/// Hard bound on batches per pass: a pathological log that never drains
/// yields to the next interval instead of looping forever.
const COMPACT_MAX_BATCHES: usize = 500;
const COMPACT_BATCH_PAUSE_MS: u64 = 200;

fn interval_hours() -> u64 {
    std::env::var("MUNDUS_STORAGE_MAINTENANCE_HOURS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(24)
}

fn initial_delay_ms() -> u64 {
    std::env::var("MUNDUS_STORAGE_MAINTENANCE_DELAY_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(180_000)
}

async fn read_last_run(ark: &ArkHost) -> Option<DateTime<Utc>> {
    let resp = ark
        .request("get_sync_kv", json!({ "key": LAST_RUN_KEY }))
        .await
        .ok()?;
    DateTime::parse_from_rfc3339(resp.data.as_str()?)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

async fn write_last_run(ark: &ArkHost) {
    let _ = ark
        .request(
            "set_sync_kv",
            json!({ "key": LAST_RUN_KEY, "value": Utc::now().to_rfc3339() }),
        )
        .await;
}

async fn compact_usage_sync_log(ark: &ArkHost) -> Result<u64, String> {
    let mut total = 0_u64;
    for _ in 0..COMPACT_MAX_BATCHES {
        let resp = ark
            .request(
                "compact_usage_sync_log",
                json!({ "batch_limit": COMPACT_BATCH_LIMIT }),
            )
            .await
            .map_err(|e| e.to_string())?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "compact failed".into()));
        }
        total += resp.data["deleted"].as_u64().unwrap_or(0);
        if !resp.data["has_more"].as_bool().unwrap_or(false) {
            return Ok(total);
        }
        tokio::time::sleep(std::time::Duration::from_millis(COMPACT_BATCH_PAUSE_MS)).await;
    }
    Err(format!(
        "usage_sync_log compaction hit the {COMPACT_MAX_BATCHES}-batch bound"
    ))
}

pub fn spawn(ark: Arc<ArkHost>, file_index: Arc<FileIndex>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(interval_hours() * 3600);
        let mut wait = std::time::Duration::from_millis(initial_delay_ms());
        loop {
            tokio::time::sleep(wait).await;
            wait = interval;
            let due = match read_last_run(&ark).await {
                Some(last) => {
                    Utc::now().signed_duration_since(last)
                        >= chrono::Duration::hours(interval_hours() as i64)
                }
                None => true,
            };
            if !due {
                continue;
            }
            match compact_usage_sync_log(&ark).await {
                Ok(0) => {}
                Ok(deleted) => {
                    tracing::info!(deleted, "usage_sync_log compaction done")
                }
                Err(e) => tracing::warn!(error = %e, "usage_sync_log compaction failed"),
            }
            let index = file_index.clone();
            match tokio::task::spawn_blocking(move || index.run_maintenance()).await {
                Ok(Ok(report))
                    if report.removed_stale
                        + report.removed_out_of_roots
                        + report.removed_orphan_fts
                        > 0
                        || report.vacuumed =>
                {
                    tracing::info!(
                        removed_stale = report.removed_stale,
                        removed_out_of_roots = report.removed_out_of_roots,
                        removed_orphan_fts = report.removed_orphan_fts,
                        vacuumed = report.vacuumed,
                        "file-index maintenance done"
                    );
                }
                Ok(Ok(_)) => {}
                Ok(Err(e)) => tracing::warn!(error = %e, "file-index maintenance failed"),
                Err(e) => tracing::warn!(error = %e, "file-index maintenance task failed"),
            }
            write_last_run(&ark).await;
        }
    })
}
