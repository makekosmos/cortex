use super::*;
use crate::db::SqliteStorageBackend;
use crate::replication_storage::{PeerIdentityBackend, SyncRuntimeStorage};
use crate::sync_transport::{IrohSessionTransport, SyncTransport, TransportEvent};
use ark_sync::relay_registry::{PeerControlSendOutcome, PeerSessionControl};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

async fn test_storage(
    device_id: &str,
) -> (
    Arc<SqliteStorageBackend>,
    ark_store::test_support::TestDatabase,
) {
    let database = ark_store::test_support::TestDatabase::create().unwrap();
    let storage = Arc::new(
        SqliteStorageBackend::open_for_test(
            database.path().to_string_lossy().into_owned(),
            device_id,
        )
        .await
        .unwrap(),
    );
    storage.set_device_id(device_id).unwrap();
    (storage, database)
}

struct FailOncePairingAcceptedTransport {
    fail_next_acceptance: AtomicBool,
    acceptance_attempts: AtomicUsize,
    data_ready: AtomicBool,
    registry_ready_digest: Mutex<Option<String>>,
}

#[async_trait::async_trait]
impl SyncTransport for FailOncePairingAcceptedTransport {
    async fn start(
        &self,
        _: tokio::sync::mpsc::UnboundedSender<TransportEvent>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn send(&self, _: LanSyncMessage) -> Result<(), String> {
        Ok(())
    }

    async fn stop(&self) {}
}

#[async_trait::async_trait]
impl IrohSessionTransport for FailOncePairingAcceptedTransport {
    async fn send_to(&self, _: &str, _: LanSyncMessage) -> Result<(), String> {
        Ok(())
    }

    fn set_outbound_authorizer(
        &self,
        _: Arc<dyn ark_sync::transport::OutboundIntegrationFrameAuthorizer>,
    ) -> Result<(), String> {
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
        if let LanSyncMessage::RegistrySyncReady { digest, .. } = &message {
            *self.registry_ready_digest.lock().unwrap() = Some(digest.clone());
        }
        if matches!(message, LanSyncMessage::PairingAccepted { .. }) {
            self.acceptance_attempts.fetch_add(1, Ordering::SeqCst);
            if self.fail_next_acceptance.swap(false, Ordering::SeqCst) {
                return Err("injected PairingAccepted send failure".into());
            }
        }
        Ok(crate::sync_transport::AddressedSendOutcome::Sent)
    }
    fn disconnect_peer(
        &self,
        _: &str,
    ) -> Result<ark_sync::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync::transport::PeerDisconnectOutcome::PeerClosed)
    }
    fn disconnect_transport_peer(
        &self,
        _: &str,
    ) -> Result<ark_sync::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync::transport::PeerDisconnectOutcome::PeerClosed)
    }
}

#[async_trait::async_trait]
impl PeerSessionControl for FailOncePairingAcceptedTransport {
    async fn send_control_to_endpoint(
        &self,
        endpoint: &str,
        message: LanSyncMessage,
    ) -> Result<PeerControlSendOutcome, String> {
        let outcome = IrohSessionTransport::send_to_transport_peer(self, endpoint, message).await?;
        Ok(match outcome {
            crate::sync_transport::AddressedSendOutcome::Sent => PeerControlSendOutcome::Sent,
            crate::sync_transport::AddressedSendOutcome::PeerOffline => {
                PeerControlSendOutcome::PeerOffline
            }
        })
    }
    fn mark_peer_data_ready(&self, peer: &str, endpoint: &str) -> Result<(), String> {
        IrohSessionTransport::mark_peer_data_ready(self, peer, endpoint)
    }
    fn unbind_peer(&self, peer: &str) -> Result<(), String> {
        IrohSessionTransport::unbind_peer(self, peer)
    }
    fn disconnect_peer(
        &self,
        peer: &str,
    ) -> Result<ark_sync::transport::PeerDisconnectOutcome, String> {
        IrohSessionTransport::disconnect_peer(self, peer)
    }
    fn disconnect_transport_endpoint(
        &self,
        endpoint: &str,
    ) -> Result<ark_sync::transport::PeerDisconnectOutcome, String> {
        IrohSessionTransport::disconnect_transport_peer(self, endpoint)
    }
}

fn hello(device_id: &str) -> LanSyncMessage {
    LanSyncMessage::Hello {
        protocol_version: ark_sync::protocol::PROTOCOL_VERSION,
        device_id: device_id.to_owned(),
        device_name: device_id.to_owned(),
        space_id: "retry-space".into(),
        addresses: None,
        auth_nonce: None,
        auth_hmac: None,
        platform: None,
        app_version: None,
        supported_protocol_versions: Some(vec![ark_sync::protocol::PROTOCOL_VERSION]),
        supported_capabilities: Some(vec!["registry_history_v1".into()]),
        negotiated_capabilities: None,
        negotiation_hmac: None,
    }
}

#[tokio::test]
async fn persisted_pairing_retries_acceptance_after_send_failure_without_duplicate_enrollment() {
    let (storage, _database) = test_storage("z-local-device").await;
    let remote_endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    let transport = Arc::new(FailOncePairingAcceptedTransport {
        fail_next_acceptance: AtomicBool::new(true),
        acceptance_attempts: AtomicUsize::new(0),
        data_ready: AtomicBool::new(false),
        registry_ready_digest: Mutex::new(None),
    });
    let relay = RelaySync::with_iroh_transport(
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
    )
    .unwrap();

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
    let (_, _, committed_events) = storage
        .load_device_registry_events_page(0, None, 32)
        .await
        .unwrap();
    assert_eq!(committed_events.len(), 1);

    relay
        .handle_event(TransportEvent::MessageReceivedFromTransport {
            from_device_id: "remote-device".into(),
            transport_public_key: remote_endpoint.clone(),
            msg: hello("remote-device"),
        })
        .await;

    assert!(!transport.data_ready.load(Ordering::SeqCst));
    assert_eq!(transport.acceptance_attempts.load(Ordering::SeqCst), 2);
    let (_, _, committed_events) = storage
        .load_device_registry_events_page(0, None, 32)
        .await
        .unwrap();
    assert_eq!(committed_events.len(), 1);

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
    let digest = transport
        .registry_ready_digest
        .lock()
        .unwrap()
        .clone()
        .expect("page completion sends registry-ready digest");
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
    storage
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn registry_ready_replay_is_idempotent_but_conflicting_digest_is_rejected() {
    let (storage, _database) = test_storage("z-local-device").await;
    let transport = Arc::new(FailOncePairingAcceptedTransport {
        fail_next_acceptance: AtomicBool::new(false),
        acceptance_attempts: AtomicUsize::new(0),
        data_ready: AtomicBool::new(false),
        registry_ready_digest: Mutex::new(None),
    });
    let relay = RelaySync::with_iroh_transport(
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
    )
    .unwrap();
    let endpoint = "verified-endpoint";
    let session_id = "registry-session";
    let digest = "a".repeat(64);
    relay
        .transport_keys
        .lock()
        .await
        .insert("a-peer".into(), endpoint.into());
    relay
        .registry
        .as_ref()
        .unwrap()
        .begin_peer("a-peer", endpoint, false)
        .await;
    relay
        .handle_registry_sync_start("a-peer", endpoint, session_id.into())
        .await
        .unwrap();

    relay
        .handle_registry_sync_ready("a-peer", endpoint, session_id.into(), digest.clone())
        .await
        .unwrap();
    relay
        .handle_registry_sync_ready("a-peer", endpoint, session_id.into(), digest.clone())
        .await
        .unwrap();
    let error = relay
        .handle_registry_sync_ready("a-peer", endpoint, session_id.into(), "b".repeat(64))
        .await
        .unwrap_err();

    assert_eq!(error, "registry_sync_ready_conflict");
    assert_eq!(
        transport.registry_ready_digest.lock().unwrap().as_deref(),
        None
    );
    storage
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn applied_registry_events_are_not_lost_when_core_reports_a_later_error() {
    use ark_domain::device_registry::{
        DeviceId, RegistryEvent, RegistryEventDraft, RegistryOperation,
    };

    let (storage, _database) = test_storage("local-device").await;
    let transport = Arc::new(FailOncePairingAcceptedTransport {
        fail_next_acceptance: AtomicBool::new(false),
        acceptance_attempts: AtomicUsize::new(0),
        data_ready: AtomicBool::new(false),
        registry_ready_digest: Mutex::new(None),
    });
    let relay = RelaySync::with_iroh_transport(
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
    )
    .unwrap();
    let event = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("local-device").unwrap(),
            actor_sequence: 1,
            previous_event_hash: None,
            target_device_id: DeviceId::parse("remote-device").unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("local-device").to_string(),
            operation: RegistryOperation::Disable,
        },
        &ed25519_dalek::SigningKey::from_bytes(&[7; 32]),
    )
    .unwrap();
    let observed_events = Arc::new(Mutex::new(Vec::new()));
    let captured = observed_events.clone();
    relay
        .set_on_registry_events(Arc::new(move |events| {
            *captured.lock().unwrap() = events;
        }))
        .await;

    let result = relay
        .finish_registry_outcome(
            ark_sync::relay_registry::RegistryOutcome {
                applied_events: vec![event.clone()],
                operation_error: Some("injected fanout failure".into()),
                ..Default::default()
            },
            true,
            None,
            false,
        )
        .await;

    assert_eq!(result.unwrap_err(), "injected fanout failure");
    assert_eq!(*observed_events.lock().unwrap(), vec![event]);
    storage
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}
