// Service-level tests — the former `main/tests.rs` suite, now driving
// `handle_request` against a per-test `ServiceState` (no process-wide
// statics, so tests run in parallel without mutexes).

use super::runtime::get_db_handle;
use super::*;
use ark_application::sync_storage::SyncEntityMutationPort;

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
    pub(crate) dir: crate::test_support::TestTempDir,
}

pub(super) fn service_fixture() -> ServiceFixture {
    ServiceFixture {
        state: test_state(),
        dir: crate::test_support::temp_dir(),
    }
}

pub(super) async fn initialize_test_database(
    state: &Arc<ServiceState>,
    db_path: String,
) -> Result<(), String> {
    handle_stop_sync(state).await;
    *state.db.lock().unwrap_or_else(|error| error.into_inner()) = None;
    let owner = state
        .db_owner
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take();
    if let Some(mut owner) = owner {
        owner
            .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(30))
            .await
            .map_err(|error| format!("database owner shutdown failed: {error}"))?;
    }
    let (db, owner) = db::DbHandle::open(db_path.clone(), "ark-core".to_string())
        .await
        .map_err(|error| error.to_string())?;
    *state
        .db_owner
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(owner);
    *state.db.lock().unwrap_or_else(|error| error.into_inner()) = Some(db);
    *state
        .db_path
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(db_path);
    Ok(())
}

pub(super) fn received_upsert_for(
    receiver: &mut tokio::sync::broadcast::Receiver<serde_json::Value>,
    object_id: &str,
) -> bool {
    loop {
        match receiver.try_recv() {
            Ok(event) if event["event"] == "object_upserted" && event["id"] == object_id => {
                return true;
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => return false,
            Err(error) => panic!("unexpected event receiver error: {error}"),
        }
    }
}

impl ServiceFixture {
    pub(crate) fn db_path(&self) -> std::path::PathBuf {
        self.dir.path().join("ark.db")
    }

    pub(crate) async fn shutdown(&self) {
        shutdown_test_state(&self.state).await;
    }
}

pub(super) async fn shutdown_test_state(state: &Arc<ServiceState>) {
    if let Ok(mut db) = state.db.lock() {
        *db = None;
    }
    let owner = state
        .db_owner
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take();
    if let Some(mut owner) = owner {
        owner
            .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(5))
            .await
            .expect("database actor shuts down before fixture directory removal");
    }
}

impl Drop for ServiceFixture {
    fn drop(&mut self) {
        // KOS-398: drop the shared SQLite handle before TempDir tries to
        // delete ark.db. A forgotten StopSync (or a transport task that still
        // holds its own Arc) keeps the file open on Windows; TestTempDir then
        // panics with the test name instead of leaving a silent leftover.
        if let Ok(mut guard) = self.state.db.lock() {
            *guard = None;
        }
        let owner = self
            .state
            .db_owner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(owner) = owner {
            if let Err(error) = owner
                .shutdown_blocking(std::time::Instant::now() + std::time::Duration::from_secs(5))
            {
                if !std::thread::panicking() {
                    panic!("database actor shuts down before fixture directory removal: {error}");
                }
            }
        }
    }
}

mod api_wire_compat;
mod data_volume;
mod local_writes;
mod object_revision_compat;
mod object_validation;
mod object_write_snapshot;
mod pairing_iroh;
mod pairing_proto;
mod replace_object_link;
mod request_config;
/// KOS-51: atomic ARK snapshot restore RPC (list/validate/restore).
mod snapshot_restore;
mod snapshot_restore_identity;
mod sync_lifecycle;
mod sync_ticket_pending;
mod task_merge;
mod type_mutation_commands;
mod usage_commands;
mod usage_read_queries;

/// Выставляет capturing-транспорт и выставляет SyncRuntime в `state.sync`.
/// Возвращает буфер перехваченных сообщений.
async fn setup_sync_with_capturing_transport(
    state: &Arc<ServiceState>,
) -> Arc<TokioMutex<Vec<ark_sync_contracts::wire::LanSyncMessage>>> {
    use crate::relay_sync::{RelaySync, RelaySyncConfig};
    use crate::sync_server::SyncRuntimeStorage;

    let captured: Arc<TokioMutex<Vec<ark_sync_contracts::wire::LanSyncMessage>>> =
        Arc::new(TokioMutex::new(Vec::new()));
    let transport = Arc::new(CapturingSyncTransport {
        sent: captured.clone(),
    });

    let backend = Arc::new(crate::db::SqliteStorageBackend::with_db_handle(
        get_db_handle(state).unwrap(),
    ));
    backend.set_device_id("test-device").unwrap();

    let relay = RelaySync::with_iroh_transport(
        backend.clone() as Arc<dyn SyncRuntimeStorage>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "test-space".to_string(),
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            auth_secret: None,
        },
        transport,
    )
    .expect("capturing Iroh session is configured before startup");
    relay.start().await.unwrap();

    let server = Arc::new(crate::sync_server::SyncServer::new(
        backend.clone() as Arc<dyn SyncRuntimeStorage>
    ));

    let runtime = SyncRuntime {
        server,
        storage: backend,
        clients: Arc::new(TokioMutex::new(std::collections::HashMap::new())),
        relay: Some(relay),
        transport_choice: Some(TransportChoice::Iroh),
        start_params: SyncStartParams {
            space_id: "test-space".to_string(),
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            port: None,
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: true,
            iroh_peer_ticket: None,
            iroh_relay_url: None,
            pairing_connect: false,
            discovery_enabled: true,
            bind: SyncBind::AllInterfaces,
            app_version: None,
        },
        iroh_our_ticket: watch::channel(None).1,
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

struct CapturingSyncTransport {
    sent: Arc<TokioMutex<Vec<ark_sync_contracts::wire::LanSyncMessage>>>,
}

#[async_trait::async_trait]
impl crate::sync_transport::SyncTransport for CapturingSyncTransport {
    async fn start(
        &self,
        _event_tx: tokio::sync::mpsc::UnboundedSender<crate::sync_transport::TransportEvent>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn send(&self, message: ark_sync_contracts::wire::LanSyncMessage) -> Result<(), String> {
        self.sent
            .try_lock()
            .expect("no concurrent capture lock")
            .push(message);
        Ok(())
    }

    async fn stop(&self) {}
}

#[async_trait::async_trait]
impl crate::sync_transport::IrohSessionTransport for CapturingSyncTransport {
    async fn send_to(
        &self,
        _device_id: &str,
        message: ark_sync_contracts::wire::LanSyncMessage,
    ) -> Result<(), String> {
        crate::sync_transport::SyncTransport::send(self, message)
    }

    fn set_outbound_authorizer(
        &self,
        _authorizer: Arc<dyn ark_sync_contracts::transport::OutboundIntegrationFrameAuthorizer>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn bind_peer_for_control(&self, _device_id: &str, _key: &str) -> Result<(), String> {
        Ok(())
    }

    fn mark_peer_data_ready(&self, _device_id: &str, _key: &str) -> Result<(), String> {
        Ok(())
    }

    fn unbind_peer(&self, _device_id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn send_to_transport_peer(
        &self,
        _key: &str,
        message: ark_sync_contracts::wire::LanSyncMessage,
    ) -> Result<ark_sync_contracts::transport::AddressedSendOutcome, String> {
        crate::sync_transport::SyncTransport::send(self, message)?;
        Ok(ark_sync_contracts::transport::AddressedSendOutcome::Sent)
    }

    fn disconnect_peer(
        &self,
        _device_id: &str,
    ) -> Result<ark_sync_contracts::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync_contracts::transport::PeerDisconnectOutcome::PeerClosed)
    }

    fn disconnect_transport_peer(
        &self,
        _key: &str,
    ) -> Result<ark_sync_contracts::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync_contracts::transport::PeerDisconnectOutcome::PeerClosed)
    }
}

#[async_trait::async_trait]
impl ark_sync_contracts::peer_control::PeerSessionControl for CapturingSyncTransport {
    async fn send_control_to_endpoint(
        &self,
        _endpoint: &str,
        message: ark_sync_contracts::wire::LanSyncMessage,
    ) -> Result<ark_sync_contracts::peer_control::PeerControlSendOutcome, String> {
        crate::sync_transport::SyncTransport::send(self, message)?;
        Ok(ark_sync_contracts::peer_control::PeerControlSendOutcome::Sent)
    }

    fn mark_peer_data_ready(&self, _device_id: &str, _key: &str) -> Result<(), String> {
        Ok(())
    }

    fn unbind_peer(&self, _device_id: &str) -> Result<(), String> {
        Ok(())
    }

    fn disconnect_peer(
        &self,
        _device_id: &str,
    ) -> Result<ark_sync_contracts::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync_contracts::transport::PeerDisconnectOutcome::PeerClosed)
    }

    fn disconnect_transport_endpoint(
        &self,
        _endpoint: &str,
    ) -> Result<ark_sync_contracts::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync_contracts::transport::PeerDisconnectOutcome::PeerClosed)
    }
}

fn canonical_task_object(type_version: Option<&str>, props_json: Value) -> ArkObjectWrite {
    ArkObjectWrite {
        id: "canonical-task-ingress".to_string(),
        type_id: "core.task".to_string(),
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
        {"state":{"kind":"open","at":null},
        "planning":{"kind":"anytime","period":"any","date":null},
        "deadline":null,
        "reminders":[],
        "recurrence":null,
        "priority":"medium",
        "checklist":[],
        "extensions":{"vendor":{"opaque":true}}})
}

mod canonical_rpc;
mod device_registry;
mod external_ref_commands;
mod integration_rpc;
mod object_mutation_commands;
mod operation_receipts;
mod sync_policy;
mod system_commands;
