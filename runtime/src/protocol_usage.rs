use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::lock_file::{self, LockFileError};

const FORMAT_VERSION: u32 = 1;
const MAX_CLIENT_BUCKETS: usize = 64;
pub const FILE_NAME: &str = "protocol-usage.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    ApiV1,
}

impl TransportKind {
    fn key(self) -> &'static str {
        match self {
            Self::ApiV1 => "api_v1",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageCounter {
    pub connections: u64,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageFile {
    format_version: u32,
    tracking_started_at: String,
    api_v1: UsageCounter,
    clients: BTreeMap<String, UsageCounter>,
}

impl UsageFile {
    fn new(now: DateTime<Utc>) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            tracking_started_at: now.to_rfc3339(),
            api_v1: UsageCounter::default(),
            clients: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProtocolUsageSnapshot {
    pub tracking_started_at: String,
    pub api_v1: UsageCounter,
    pub clients: BTreeMap<String, UsageCounter>,
}

pub struct ProtocolUsageStore {
    path: PathBuf,
    state: Mutex<UsageFile>,
}

impl ProtocolUsageStore {
    pub fn open(data_dir: &Path) -> Result<Self, LockFileError> {
        let path = data_dir.join(FILE_NAME);
        let state = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<UsageFile>(&bytes).ok())
            .filter(|file| file.format_version == FORMAT_VERSION)
            .unwrap_or_else(|| UsageFile::new(Utc::now()));
        lock_file::write_owner_only_json(&path, &state)?;
        Ok(Self {
            path,
            state: Mutex::new(state),
        })
    }

    pub fn record(
        &self,
        transport: TransportKind,
        client_class: Option<&str>,
        client_version: Option<&str>,
    ) -> Result<(), LockFileError> {
        let now = Utc::now().to_rfc3339();
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let counter = match transport {
            TransportKind::ApiV1 => &mut state.api_v1,
        };
        counter.connections = counter.connections.saturating_add(1);
        counter.last_seen = Some(now.clone());

        let class = sanitize_client_class(client_class.unwrap_or("unknown"));
        let version = sanitize_client_version(client_version.unwrap_or("unknown"));
        let mut bucket = format!("{}:{class}@{version}", transport.key());
        if !state.clients.contains_key(&bucket) && state.clients.len() >= MAX_CLIENT_BUCKETS {
            bucket = format!("{}:other@other", transport.key());
        }
        let client = state.clients.entry(bucket).or_default();
        client.connections = client.connections.saturating_add(1);
        client.last_seen = Some(now);

        lock_file::write_owner_only_json(&self.path, &*state)
    }

    pub fn snapshot(&self) -> ProtocolUsageSnapshot {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        ProtocolUsageSnapshot {
            tracking_started_at: state.tracking_started_at.clone(),
            api_v1: state.api_v1.clone(),
            clients: state.clients.clone(),
        }
    }
}

fn sanitize_client_class(value: &str) -> String {
    match value {
        "@kosmos/ark" | "desktop-host" | "engine-http" | "engine-manager" | "kosmos-desktop" => {
            value.to_owned()
        }
        _ => "unknown".into(),
    }
}

fn sanitize_client_version(value: &str) -> String {
    crate::protocol_version::ProtocolVersion::parse(value)
        .map(|_| value.to_owned())
        .unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_only_bounded_aggregate_client_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProtocolUsageStore::open(dir.path()).unwrap();
        store
            .record(
                TransportKind::ApiV1,
                Some("pid=4242 secret-token raw-client-id payload-body"),
                Some("secret-token"),
            )
            .unwrap();

        let raw = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        for sensitive in ["pid=4242", "secret-token", "raw-client-id", "payload-body"] {
            assert!(
                !raw.contains(sensitive),
                "persisted sensitive value: {sensitive}"
            );
        }
        let reopened = ProtocolUsageStore::open(dir.path()).unwrap();
        let snapshot = reopened.snapshot();
        assert_eq!(snapshot.api_v1.connections, 1);
        assert!(snapshot.clients.contains_key("api_v1:unknown@unknown"));
    }

    #[test]
    fn records_engine_manager_as_first_party_api_v1_client() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProtocolUsageStore::open(dir.path()).unwrap();
        store
            .record(TransportKind::ApiV1, Some("engine-manager"), Some("1.0.0"))
            .unwrap();
        assert!(store
            .snapshot()
            .clients
            .contains_key("api_v1:engine-manager@1.0.0"));
    }

    #[test]
    fn records_desktop_host_and_sanitizes_unknown_api_v1_clients() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProtocolUsageStore::open(dir.path()).unwrap();
        store
            .record(TransportKind::ApiV1, Some("desktop-host"), Some("1.2.3"))
            .unwrap();
        store
            .record(
                TransportKind::ApiV1,
                Some("untrusted-client"),
                Some("1.2.3"),
            )
            .unwrap();

        let snapshot = store.snapshot();
        assert!(snapshot.clients.contains_key("api_v1:desktop-host@1.2.3"));
        assert!(snapshot.clients.contains_key("api_v1:unknown@1.2.3"));
        assert!(!snapshot
            .clients
            .contains_key("api_v1:untrusted-client@1.2.3"));
    }

    #[test]
    fn records_desktop_and_manager_in_separate_v1_buckets_without_sensitive_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProtocolUsageStore::open(dir.path()).unwrap();
        store
            .record(TransportKind::ApiV1, Some("kosmos-desktop"), Some("1.0.0"))
            .unwrap();
        store
            .record(TransportKind::ApiV1, Some("engine-manager"), Some("1.0.0"))
            .unwrap();

        let snapshot = store.snapshot();
        assert_eq!(snapshot.api_v1.connections, 2);
        assert_eq!(
            snapshot
                .clients
                .get("api_v1:kosmos-desktop@1.0.0")
                .map(|counter| counter.connections),
            Some(1)
        );
        assert_eq!(
            snapshot
                .clients
                .get("api_v1:engine-manager@1.0.0")
                .map(|counter| counter.connections),
            Some(1)
        );
        assert!(!snapshot.clients.contains_key("legacy:kosmos-desktop@1.0.0"));

        let raw = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(!raw.contains("\"legacy\""));
        for sensitive in ["pid=4242", "secret-token", "raw-client-id", "payload-body"] {
            assert!(
                !raw.contains(sensitive),
                "persisted sensitive value: {sensitive}"
            );
        }
    }

    #[test]
    fn tolerates_legacy_key_in_files_written_before_kos_333() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE_NAME),
            serde_json::json!({
                "format_version": FORMAT_VERSION,
                "tracking_started_at": "2026-06-01T00:00:00Z",
                "api_v1": {"connections": 3, "last_seen": "2026-06-02T00:00:00Z"},
                "legacy": {"connections": 7, "last_seen": "2026-06-20T00:00:00Z"},
                "clients": {},
            })
            .to_string(),
        )
        .unwrap();

        let store = ProtocolUsageStore::open(dir.path()).unwrap();
        let snapshot = store.snapshot();
        assert_eq!(snapshot.api_v1.connections, 3);

        let raw = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(!raw.contains("\"legacy\""));
    }
}
