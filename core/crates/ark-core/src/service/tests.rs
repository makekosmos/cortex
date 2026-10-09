// Service-level tests — the former `main/tests.rs` suite, now driving
// `handle_request` against a per-test `ServiceState` (no process-wide
// statics, so tests run in parallel without mutexes).

use super::runtime::{get_shared_conn, with_conn, with_write_tx};
use super::*;

/// Fresh per-test service state — replaces the old global `DB`/`SYNC`
/// statics and the `TEST_DB_MUTEX` serialization they forced.
pub(super) fn test_state() -> Arc<ServiceState> {
    Arc::new(ServiceState::new(ArkHostConfig::default()))
}

/// Per-test fixture for tests that open `ark.db` in a tempdir. Field order
/// is teardown order: `state` (holding the open SQLite connection) drops
/// before `dir`, so the db file is closed before the directory is removed.
/// Two separate `let` locals would drop in reverse declaration order and a
/// `tempdir()` declared after `test_state()` leaked its dir on Windows,
/// where an open file cannot be deleted (KOS-270).
pub(super) struct ServiceFixture {
    pub(crate) state: Arc<ServiceState>,
    pub(crate) dir: tempfile::TempDir,
}

pub(super) fn service_fixture() -> ServiceFixture {
    ServiceFixture {
        state: test_state(),
        dir: tempfile::tempdir().unwrap(),
    }
}

impl ServiceFixture {
    pub(crate) fn db_path(&self) -> std::path::PathBuf {
        self.dir.path().join("ark.db")
    }
}

mod legacy_writes;
mod local_writes;
mod object_revision_compat;
mod object_validation;
mod object_write_snapshot;
mod pairing_iroh;
mod request_config;
/// KOS-51: atomic ARK snapshot restore RPC (list/validate/restore).
mod snapshot_restore;
mod sync_lifecycle;

/// Выставляет capturing-транспорт и выставляет SyncRuntime в `state.sync`.
/// Возвращает буфер перехваченных сообщений.
async fn setup_sync_with_capturing_transport(
    state: &Arc<ServiceState>,
) -> Arc<TokioMutex<Vec<crate::protocol::LanSyncMessage>>> {
    use crate::relay_sync::{RelaySync, RelaySyncConfig};
    use crate::sync_server::StorageBackend;

    let captured: Arc<TokioMutex<Vec<crate::protocol::LanSyncMessage>>> =
        Arc::new(TokioMutex::new(Vec::new()));
    let transport = Arc::new(legacy_writes::CapturingTransport {
        sent: captured.clone(),
    });

    let backend = Arc::new(crate::db::SqliteStorageBackend::new(
        get_shared_conn(state).unwrap(),
    ));
    backend.set_device_id("test-device").unwrap();

    let relay = RelaySync::with_transport(
        backend.clone() as Arc<dyn StorageBackend>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "test-space".to_string(),
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            auth_secret: None,
        },
        transport as Arc<dyn crate::sync_transport::SyncTransport>,
    );
    relay.start().await.unwrap();

    let server = Arc::new(crate::sync_server::SyncServer::new(
        backend.clone() as Arc<dyn StorageBackend>
    ));

    let runtime = SyncRuntime {
        server,
        storage: backend,
        clients: Arc::new(TokioMutex::new(std::collections::HashMap::new())),
        relay: Some(relay),
        transport_choice: Some(TransportChoice::Relay),
        start_params: SyncStartParams {
            space_id: "test-space".to_string(),
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            port: None,
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: false,
            iroh_peer_ticket: None,
            discovery_enabled: true,
            bind: SyncBind::AllInterfaces,
            app_version: None,
        },
        iroh_our_ticket: None,
        beacon: Arc::new(crate::beacon::BroadcastDiscovery::new()),
        space_id: "test-space".to_string(),
        device_id: "test-device".to_string(),
        device_name: "Test Device".to_string(),
        auth_secret: None,
        own_addresses: Arc::new(TokioMutex::new(Vec::new())),
    };
    *state.sync.lock().await = Some(Arc::new(runtime));

    captured
}

fn canonical_task_object(type_version: Option<&str>, props_json: Value) -> ArkObjectWrite {
    ArkObjectWrite {
        id: "canonical-task-ingress".to_string(),
        type_id: "com.kosmos.task".to_string(),
        type_version: type_version.map(str::to_owned),
        title: "Canonical task".to_string(),
        content_json: json!(
            {"type":"doc",
            "content":[{"type":"paragraph",
            "content":[{"type":"text",
            "text":"hello"}]}]}),
        props_json,
        created_at: "2026-08-11T00:00:00.000Z".to_string(),
        updated_at: "2026-08-11T00:00:00.000Z".to_string(),
        deleted_at: None,
    }
}

fn canonical_task_props() -> Value {
    json!(
        {"status":"todo",
        "priority":"medium",
        "scheduledAt":null,
        "dueAt":null,
        "reminderAt":null,
        "completedAt":null,
        "canceledAt":null,
        "recurrence":null,
        "checklist":[],
        "extensions":{"vendor":{"opaque":true}}})
}

mod canonical_rpc;
mod integration_rpc;
