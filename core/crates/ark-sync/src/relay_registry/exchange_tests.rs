use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

use ark_application::replication_storage::{PeerIdentityBackend, RegistryPageApply};
use ark_application::{
    registry_admission::RegistryAdmissionStorage, sync_storage::SyncRuntimeStorage,
};
use ark_domain::device_registry::{DeviceId, RegistryEvent, RegistryEventDraft, RegistryOperation};
use ed25519_dalek::SigningKey;

use ark_sync_contracts::peer_control::{PeerControlSendOutcome, PeerSessionControl};

use crate::relay_registry::{RegistryFrame, RelayRegistryCoordinator};
use ark_sync_contracts::wire::LanSyncMessage;

#[derive(Default)]
struct FakeStorage {
    apply_count: AtomicUsize,
}

#[async_trait::async_trait]
impl PeerIdentityBackend for FakeStorage {
    async fn load_local_sync_profile(
        &self,
        _: &str,
    ) -> Result<Option<ark_application::sync_policy::SyncProfile>, String> {
        Ok(None)
    }

    async fn accept_peer_sync_policy(
        &self,
        _: &str,
        _: i64,
        _: &str,
        _: bool,
        _: &[ark_application::sync_policy::SyncProfileRule],
    ) -> Result<ark_application::sync_policy::AcceptedPeerSyncPolicy, String> {
        Err("test_sync_policy_unsupported".into())
    }

    async fn load_accepted_peer_sync_policy(
        &self,
        _: &str,
    ) -> Result<Option<ark_application::sync_policy::AcceptedPeerSyncPolicy>, String> {
        Ok(None)
    }

    async fn sync_session_is_admitted(&self, _: &str) -> Result<bool, String> {
        Err("test_session_admission_unsupported".into())
    }

    async fn registry_device_is_enabled(&self, _: &str) -> Result<Option<bool>, String> {
        Err("test_device_registry_unsupported".into())
    }

    async fn authorized_transport_public_key(&self, _: &str) -> Result<Option<String>, String> {
        Ok(None)
    }

    async fn load_peer_transport_keys(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, String> {
        Ok(Default::default())
    }

    async fn store_peer_transport_keys(
        &self,
        _: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn enroll_paired_device(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
        Err("test_device_enrollment_unsupported".into())
    }

    async fn load_device_registry_events_page(
        &self,
        _: i64,
        _: Option<i64>,
        _: usize,
    ) -> Result<(i64, usize, Vec<RegistryEvent>), String> {
        Ok((0, 0, Vec::new()))
    }

    async fn apply_device_registry_events(
        &self,
        _: &str,
        events: &[RegistryEvent],
    ) -> Result<RegistryPageApply, String> {
        self.apply_count.fetch_add(1, Ordering::SeqCst);
        Ok(RegistryPageApply {
            digest: "a".repeat(64),
            sender_is_admitted: true,
            local_device_is_admitted: true,
            disabled_device_ids: Vec::new(),
            applied_events: events.to_vec(),
        })
    }

    async fn sign_registry_handshake(
        &self,
        _: &str,
        _: &ark_domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<ark_domain::device_registry::RegistryHandshakeProof, String> {
        Err("test_registry_handshake_unsupported".into())
    }

    async fn verify_registry_handshake(
        &self,
        _: &str,
        _: &ark_domain::device_registry::RegistryHandshakeTranscript,
        _: &ark_domain::device_registry::RegistryHandshakeProof,
    ) -> Result<(), String> {
        Err("test_registry_handshake_unsupported".into())
    }
}

#[derive(Default)]
struct FakeTransport {
    sends: Mutex<Vec<(String, LanSyncMessage)>>,
    unbound: Mutex<Vec<String>>,
    disconnected: Mutex<Vec<String>>,
    ready: Mutex<Vec<(String, String)>>,
    trace: Mutex<Vec<String>>,
    fail_sends: AtomicBool,
}

#[async_trait::async_trait]
impl PeerSessionControl for FakeTransport {
    fn mark_peer_data_ready(&self, peer: &str, endpoint: &str) -> Result<(), String> {
        self.ready
            .lock()
            .unwrap()
            .push((peer.to_owned(), endpoint.to_owned()));
        Ok(())
    }

    fn unbind_peer(&self, peer: &str) -> Result<(), String> {
        self.unbound.lock().unwrap().push(peer.to_owned());
        self.trace.lock().unwrap().push(format!("unbind:{peer}"));
        Ok(())
    }

    async fn send_control_to_endpoint(
        &self,
        endpoint: &str,
        message: LanSyncMessage,
    ) -> Result<PeerControlSendOutcome, String> {
        if self.fail_sends.load(Ordering::SeqCst) {
            return Err("injected send failure".into());
        }
        self.trace.lock().unwrap().push(format!("send:{endpoint}"));
        self.sends
            .lock()
            .unwrap()
            .push((endpoint.to_owned(), message));
        Ok(PeerControlSendOutcome::Sent)
    }

    fn disconnect_peer(
        &self,
        peer: &str,
    ) -> Result<ark_sync_contracts::transport::PeerDisconnectOutcome, String> {
        self.trace
            .lock()
            .unwrap()
            .push(format!("disconnect-peer:{peer}"));
        Ok(ark_sync_contracts::transport::PeerDisconnectOutcome::PeerClosed)
    }

    fn disconnect_transport_endpoint(
        &self,
        endpoint: &str,
    ) -> Result<ark_sync_contracts::transport::PeerDisconnectOutcome, String> {
        self.disconnected.lock().unwrap().push(endpoint.to_owned());
        self.trace
            .lock()
            .unwrap()
            .push(format!("disconnect:{endpoint}"));
        Ok(ark_sync_contracts::transport::PeerDisconnectOutcome::PeerClosed)
    }
}

type Coordinator = RelayRegistryCoordinator<FakeStorage>;

#[test]
fn aggregate_sync_storage_implements_the_narrow_registry_port() {
    fn assert_port<T: RegistryAdmissionStorage + ?Sized>() {}
    assert_port::<dyn SyncRuntimeStorage>();
}

fn coordinator(transport: Arc<FakeTransport>) -> (Coordinator, Arc<FakeStorage>) {
    let storage = Arc::new(FakeStorage::default());
    (
        Coordinator::new("local-device".into(), storage.clone(), transport),
        storage,
    )
}

fn event(target: &str, operation: RegistryOperation) -> RegistryEvent {
    event_from("local-device", target, operation)
}

fn event_from(actor: &str, target: &str, operation: RegistryOperation) -> RegistryEvent {
    RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse(actor).unwrap(),
            actor_sequence: 1,
            previous_event_hash: None,
            target_device_id: DeviceId::parse(target).unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: ark_domain::hlc::HLC::now(actor).to_string(),
            operation,
        },
        &SigningKey::from_bytes(&[7; 32]),
    )
    .unwrap()
}

#[tokio::test]
async fn exchange_requires_current_endpoint_and_pages_in_order() {
    let transport = Arc::new(FakeTransport::default());
    let (coordinator, storage) = coordinator(transport);
    coordinator
        .begin_peer("device-a", "endpoint-a", false)
        .await;

    let stale = coordinator
        .handle_registry_frame(
            "device-a",
            "endpoint-b",
            RegistryFrame::SyncStart {
                session_id: "session".into(),
            },
            &[],
        )
        .await;
    assert_eq!(stale.unwrap_err(), "registry_sync_start_identity_mismatch");

    coordinator
        .handle_registry_frame(
            "device-a",
            "endpoint-a",
            RegistryFrame::SyncStart {
                session_id: "session".into(),
            },
            &[],
        )
        .await
        .unwrap();
    let out_of_order = coordinator
        .handle_registry_frame(
            "device-a",
            "endpoint-a",
            RegistryFrame::EventsPage {
                session_id: "session".into(),
                page_index: 1,
                page_count: 2,
                events: Vec::new(),
            },
            &[],
        )
        .await;
    assert_eq!(out_of_order.unwrap_err(), "registry_sync_page_order");
    let invalid_bounds = coordinator
        .handle_registry_frame(
            "device-a",
            "endpoint-a",
            RegistryFrame::EventsPage {
                session_id: "session".into(),
                page_index: 0,
                page_count: 0,
                events: Vec::new(),
            },
            &[],
        )
        .await;
    assert_eq!(invalid_bounds.unwrap_err(), "registry_sync_page_bounds");
    assert_eq!(storage.apply_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn ready_requires_matching_digest_after_applied_history_page() {
    let transport = Arc::new(FakeTransport::default());
    let (coordinator, storage) = coordinator(transport.clone());
    coordinator.begin_peer("device-a", "endpoint-a", true).await;
    coordinator
        .handle_registry_frame(
            "device-a",
            "endpoint-a",
            RegistryFrame::SyncStart {
                session_id: "session".into(),
            },
            &[],
        )
        .await
        .unwrap();
    let page = coordinator
        .handle_registry_frame(
            "device-a",
            "endpoint-a",
            RegistryFrame::EventsPage {
                session_id: "session".into(),
                page_index: 0,
                page_count: 1,
                events: Vec::new(),
            },
            &[],
        )
        .await
        .unwrap();
    assert!(!page.became_ready);
    let ready = coordinator
        .handle_registry_frame(
            "device-a",
            "endpoint-a",
            RegistryFrame::SyncReady {
                session_id: "session".into(),
                digest: "a".repeat(64),
            },
            &[],
        )
        .await
        .unwrap();
    assert!(ready.became_ready);
    assert!(ready.policy_negotiated);
    assert!(coordinator.is_ready("device-a", "endpoint-a").await);
    assert_eq!(storage.apply_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        transport.ready.lock().unwrap().as_slice(),
        &[("device-a".into(), "endpoint-a".into())]
    );
}

#[tokio::test]
async fn applied_events_survive_a_later_send_failure() {
    let transport = Arc::new(FakeTransport::default());
    let (coordinator, _) = coordinator(transport.clone());
    let applied = event("other-device", RegistryOperation::Disable);
    coordinator
        .begin_peer("device-a", "endpoint-a", false)
        .await;
    coordinator
        .handle_registry_frame(
            "device-a",
            "endpoint-a",
            RegistryFrame::SyncStart {
                session_id: "session".into(),
            },
            &[],
        )
        .await
        .unwrap();
    transport.fail_sends.store(true, Ordering::SeqCst);
    let outcome = coordinator
        .handle_registry_frame(
            "device-a",
            "endpoint-a",
            RegistryFrame::EventsPage {
                session_id: "session".into(),
                page_index: 0,
                page_count: 1,
                events: vec![applied.clone()],
            },
            &[],
        )
        .await
        .unwrap();
    assert_eq!(outcome.applied_events, vec![applied]);
    assert_eq!(
        outcome.operation_error.as_deref(),
        Some("injected send failure")
    );
    assert!(!coordinator.is_ready("device-a", "endpoint-a").await);
    assert!(transport.ready.lock().unwrap().is_empty());
    assert!(!transport
        .sends
        .lock()
        .unwrap()
        .iter()
        .any(|(_, message)| { matches!(message, LanSyncMessage::RegistrySyncReady { .. }) }));
}

#[tokio::test]
async fn local_disable_revokes_all_connected_peers_including_untracked_ones() {
    let transport = Arc::new(FakeTransport::default());
    let (coordinator, _) = coordinator(transport.clone());
    coordinator.begin_peer("peer-a", "endpoint-a", false).await;
    let disabled_local = event_from("peer-a", "local-device", RegistryOperation::Disable);
    let outcome = coordinator
        .publish_delta(
            &[disabled_local],
            &["local-device".into()],
            None,
            &[
                ("peer-a".into(), "endpoint-a".into()),
                ("peer-b".into(), "endpoint-b".into()),
            ],
        )
        .await;
    assert_eq!(outcome.revoked_peer_ids, ["peer-a", "peer-b"]);
    assert_eq!(*transport.unbound.lock().unwrap(), ["peer-a", "peer-b"]);
    assert_eq!(
        *transport.disconnected.lock().unwrap(),
        ["endpoint-a", "endpoint-b"]
    );
    assert_eq!(
        transport.trace.lock().unwrap().as_slice(),
        [
            "unbind:peer-a",
            "unbind:peer-b",
            "disconnect:endpoint-a",
            "disconnect:endpoint-b"
        ]
    );
}

#[tokio::test]
async fn non_local_disable_skips_unknown_offline_peers() {
    let transport = Arc::new(FakeTransport::default());
    let (coordinator, _) = coordinator(transport.clone());
    let disabled = event_from("peer-a", "offline-peer", RegistryOperation::Disable);
    let outcome = coordinator
        .publish_delta(&[disabled], &["offline-peer".into()], None, &[])
        .await;

    assert!(outcome.revoked_peer_ids.is_empty());
    assert!(transport.unbound.lock().unwrap().is_empty());
    assert!(transport.disconnected.lock().unwrap().is_empty());
    assert!(outcome.operation_error.is_none());
}
