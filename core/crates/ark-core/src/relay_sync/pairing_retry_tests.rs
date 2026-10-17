use super::*;
use crate::db::{init_schema, SqliteStorageBackend};
use crate::replication_storage::{PeerIdentityBackend, SyncRuntimeStorage};
use crate::sync_transport::{SyncTransport, TransportEvent};
use rusqlite::Connection;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

struct FailOncePairingAcceptedTransport {
    fail_next_acceptance: AtomicBool,
    acceptance_attempts: AtomicUsize,
    data_ready: AtomicBool,
}

#[async_trait::async_trait]
impl SyncTransport for FailOncePairingAcceptedTransport {
    fn supports_registry_admission(&self) -> bool {
        true
    }

    async fn start(
        &self,
        _: tokio::sync::mpsc::UnboundedSender<TransportEvent>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn send(&self, _: LanSyncMessage) -> Result<(), String> {
        Ok(())
    }

    fn bind_peer_for_control(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }

    fn mark_peer_data_ready(&self, _: &str, _: &str) -> Result<(), String> {
        self.data_ready.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn unbind_peer(&self, _: &str) -> Result<(), String> {
        self.data_ready.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send_to_transport_peer(
        &self,
        _: &str,
        message: LanSyncMessage,
    ) -> Result<crate::sync_transport::AddressedSendOutcome, String> {
        if matches!(message, LanSyncMessage::PairingAccepted { .. }) {
            self.acceptance_attempts.fetch_add(1, Ordering::SeqCst);
            if self.fail_next_acceptance.swap(false, Ordering::SeqCst) {
                return Err("injected PairingAccepted send failure".into());
            }
        }
        Ok(crate::sync_transport::AddressedSendOutcome::Sent)
    }

    async fn stop(&self) {}
}

fn hello(device_id: &str) -> LanSyncMessage {
    LanSyncMessage::Hello {
        protocol_version: crate::protocol::PROTOCOL_VERSION,
        device_id: device_id.to_owned(),
        device_name: device_id.to_owned(),
        space_id: "retry-space".into(),
        addresses: None,
        auth_nonce: None,
        auth_hmac: None,
        platform: None,
        app_version: None,
        supported_protocol_versions: Some(vec![crate::protocol::PROTOCOL_VERSION]),
        supported_capabilities: Some(vec!["registry_history_v1".into()]),
        negotiated_capabilities: None,
        negotiation_hmac: None,
    }
}

#[tokio::test]
async fn persisted_pairing_retries_acceptance_after_send_failure_without_duplicate_enrollment() {
    let connection = Connection::open_in_memory().unwrap();
    init_schema(&connection).unwrap();
    let shared_connection = Arc::new(Mutex::new(connection));
    let storage = Arc::new(SqliteStorageBackend::for_test(shared_connection.clone()));
    storage.set_device_id("z-local-device").unwrap();
    let remote_endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    let transport = Arc::new(FailOncePairingAcceptedTransport {
        fail_next_acceptance: AtomicBool::new(true),
        acceptance_attempts: AtomicUsize::new(0),
        data_ready: AtomicBool::new(false),
    });
    let relay = RelaySync::with_transport(
        storage.clone() as Arc<dyn SyncRuntimeStorage>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "retry-space".into(),
            device_id: "z-local-device".into(),
            device_name: "Local device".into(),
            auth_secret: None,
        },
        transport.clone(),
    );

    relay
        .handle_event(TransportEvent::MessageReceivedFromTransport {
            from_device_id: "remote-device".into(),
            transport_public_key: remote_endpoint.clone(),
            msg: hello("remote-device"),
        })
        .await;
    relay.accept_pairing("remote-device").await.unwrap_err();

    assert!(!transport.data_ready.load(Ordering::SeqCst));
    assert_eq!(transport.acceptance_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        storage
            .load_peer_transport_keys()
            .await
            .unwrap()
            .get("remote-device"),
        Some(&remote_endpoint)
    );
    let committed_events: i64 = shared_connection
        .lock()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM device_registry_events", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(committed_events, 1);

    relay
        .handle_event(TransportEvent::MessageReceivedFromTransport {
            from_device_id: "remote-device".into(),
            transport_public_key: remote_endpoint.clone(),
            msg: hello("remote-device"),
        })
        .await;

    assert!(!transport.data_ready.load(Ordering::SeqCst));
    assert_eq!(transport.acceptance_attempts.load(Ordering::SeqCst), 2);
    let committed_events: i64 = shared_connection
        .lock()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM device_registry_events", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(committed_events, 1);

    relay
        .handle_registry_sync_start(
            "remote-device",
            &remote_endpoint,
            "test-registry-session".into(),
        )
        .await
        .unwrap();
    let (_, _, events) = storage
        .load_device_registry_events_page(0, None, 32)
        .await
        .unwrap();
    relay
        .handle_registry_events_page(
            "remote-device",
            &remote_endpoint,
            "test-registry-session".into(),
            0,
            1,
            events,
        )
        .await
        .unwrap();
    let digest = relay
        .peers
        .lock()
        .await
        .get("remote-device")
        .unwrap()
        .registry_admission
        .clone();
    let RelayRegistryAdmission::Exchanging(state) = digest else {
        panic!("peer should still be in registry exchange before readiness proof");
    };
    let digest = state.local_digest.unwrap();
    relay
        .handle_registry_sync_ready(
            "remote-device",
            &remote_endpoint,
            "test-registry-session".into(),
            digest,
        )
        .await
        .unwrap();
    assert!(transport.data_ready.load(Ordering::SeqCst));
}

#[tokio::test]
async fn registry_ready_replay_is_idempotent_but_conflicting_digest_is_rejected() {
    let connection = Connection::open_in_memory().unwrap();
    init_schema(&connection).unwrap();
    let storage = Arc::new(SqliteStorageBackend::for_test(Arc::new(Mutex::new(
        connection,
    ))));
    let transport = Arc::new(FailOncePairingAcceptedTransport {
        fail_next_acceptance: AtomicBool::new(false),
        acceptance_attempts: AtomicUsize::new(0),
        data_ready: AtomicBool::new(false),
    });
    let relay = RelaySync::with_transport(
        storage.clone() as Arc<dyn SyncRuntimeStorage>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "retry-space".into(),
            device_id: "local-device".into(),
            device_name: "Local device".into(),
            auth_secret: None,
        },
        transport,
    );
    let endpoint = "verified-endpoint";
    let session_id = "registry-session";
    let digest = "a".repeat(64);
    relay
        .transport_keys
        .lock()
        .await
        .insert("remote-device".into(), endpoint.into());
    relay.peers.lock().await.insert(
        "remote-device".into(),
        RelayPeerState {
            device_name: "Remote device".into(),
            platform: None,
            app_version: None,
            authenticated: true,
            negotiated_protocol: None,
            registry_admission: RelayRegistryAdmission::Exchanging(RegistryExchangeState {
                session_id: session_id.into(),
                transport_public_key: endpoint.into(),
                retry_count: 0,
                incoming_page_count: None,
                next_incoming_page: 0,
                local_digest: None,
                remote_digest: None,
            }),
        },
    );

    relay
        .handle_registry_sync_ready("remote-device", endpoint, session_id.into(), digest.clone())
        .await
        .unwrap();
    relay
        .handle_registry_sync_ready("remote-device", endpoint, session_id.into(), digest.clone())
        .await
        .unwrap();
    let error = relay
        .handle_registry_sync_ready("remote-device", endpoint, session_id.into(), "b".repeat(64))
        .await
        .unwrap_err();

    assert_eq!(error, "registry_sync_ready_conflict");
    assert_eq!(
        match &relay.peers.lock().await["remote-device"].registry_admission {
            RelayRegistryAdmission::Exchanging(state) => state.remote_digest.clone(),
            _ => panic!("peer should remain in registry exchange until local digest is known"),
        },
        Some(digest),
    );
}
