use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Default, Serialize)]
pub struct RpcOperationStats {
    pub count: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub max_ms: u64,
    pub response_bytes_p50: u64,
    pub response_bytes_p95: u64,
    pub response_bytes_max: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RpcDiagnosticsSnapshot {
    pub total: u64,
    pub by_operation: HashMap<String, RpcOperationStats>,
}

#[derive(Debug, Default)]
pub struct RpcDiagnostics {
    inner: Mutex<HashMap<String, Vec<RpcSample>>>,
}

#[derive(Debug, Clone)]
struct RpcSample {
    duration_ms: u64,
    response_bytes: u64,
}

impl RpcDiagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(&self, operation: &str, duration: Duration) {
        self.observe_response(operation, duration, 0);
    }

    pub fn observe_response(&self, operation: &str, duration: Duration, response_bytes: usize) {
        let ms = duration.as_millis().min(u128::from(u64::MAX)) as u64;
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let samples = inner.entry(operation.to_string()).or_default();
        samples.push(RpcSample {
            duration_ms: ms,
            response_bytes: response_bytes as u64,
        });
        if samples.len() > 512 {
            let overflow = samples.len() - 512;
            samples.drain(0..overflow);
        }
    }

    pub fn snapshot(&self) -> RpcDiagnosticsSnapshot {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut total = 0_u64;
        let mut by_operation = HashMap::new();
        for (operation, samples) in inner.iter() {
            total = total.saturating_add(samples.len() as u64);
            by_operation.insert(operation.clone(), stats_for(samples));
        }
        RpcDiagnosticsSnapshot {
            total,
            by_operation,
        }
    }
}

pub type SharedRpcDiagnostics = Arc<RpcDiagnostics>;

fn stats_for(samples: &[RpcSample]) -> RpcOperationStats {
    if samples.is_empty() {
        return RpcOperationStats::default();
    }
    let mut durations: Vec<u64> = samples.iter().map(|sample| sample.duration_ms).collect();
    let mut response_bytes: Vec<u64> = samples.iter().map(|sample| sample.response_bytes).collect();
    durations.sort_unstable();
    response_bytes.sort_unstable();
    RpcOperationStats {
        count: durations.len() as u64,
        p50_ms: percentile(&durations, 50),
        p95_ms: percentile(&durations, 95),
        max_ms: *durations.last().unwrap_or(&0),
        response_bytes_p50: percentile(&response_bytes, 50),
        response_bytes_p95: percentile(&response_bytes, 95),
        response_bytes_max: *response_bytes.last().unwrap_or(&0),
    }
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((sorted.len() - 1) * percentile) / 100;
    sorted[rank]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_snapshot_reports_percentiles_and_max() {
        let diagnostics = RpcDiagnostics::new();
        for ms in [1, 10, 20, 30, 40] {
            diagnostics.observe("commands.list", Duration::from_millis(ms));
        }

        let snapshot = diagnostics.snapshot();
        let stats = snapshot
            .by_operation
            .get("commands.list")
            .expect("operation stats");

        assert_eq!(snapshot.total, 5);
        assert_eq!(stats.count, 5);
        assert_eq!(stats.p50_ms, 20);
        assert_eq!(stats.p95_ms, 30);
        assert_eq!(stats.max_ms, 40);
        assert_eq!(stats.response_bytes_max, 0);
    }

    #[test]
    fn rpc_snapshot_reports_response_payload_bytes() {
        let diagnostics = RpcDiagnostics::new();
        diagnostics.observe_response("app_index.list_all", Duration::from_millis(5), 100);
        diagnostics.observe_response("app_index.list_all", Duration::from_millis(10), 20_000);
        diagnostics.observe_response("app_index.list_all", Duration::from_millis(20), 400);

        let snapshot = diagnostics.snapshot();
        let stats = snapshot
            .by_operation
            .get("app_index.list_all")
            .expect("operation stats");

        assert_eq!(stats.response_bytes_p50, 400);
        assert_eq!(stats.response_bytes_p95, 400);
        assert_eq!(stats.response_bytes_max, 20_000);
    }

    #[test]
    fn rpc_samples_are_bounded_per_operation() {
        let diagnostics = RpcDiagnostics::new();
        for index in 0..600 {
            diagnostics.observe("file_index.search", Duration::from_millis(index));
        }

        let snapshot = diagnostics.snapshot();
        let stats = snapshot
            .by_operation
            .get("file_index.search")
            .expect("operation stats");

        assert_eq!(stats.count, 512);
        assert_eq!(snapshot.total, 512);
    }
}
