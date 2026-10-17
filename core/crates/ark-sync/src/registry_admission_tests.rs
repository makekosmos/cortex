use super::*;
use ark_application::registry_admission::RegistryAdmissionStorage;
use ark_application::replication_storage::RegistryPageApply;
use ark_domain::device_registry::{DeviceId, RegistryEvent, RegistryEventDraft, RegistryOperation};
use ed25519_dalek::SigningKey;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct RegistryStorage {
    apply_result: RegistryPageApply,
    apply_calls: Arc<AtomicUsize>,
    history_events: Vec<RegistryEvent>,
    page_reads: Mutex<Vec<(i64, Option<i64>, usize)>>,
}

#[async_trait::async_trait]
impl RegistryAdmissionStorage for RegistryStorage {
    async fn load_device_registry_events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<(i64, usize, Vec<RegistryEvent>), String> {
        self.page_reads.lock().expect("page read log lock").push((
            after_position,
            high_watermark,
            limit,
        ));
        let high_watermark = high_watermark.unwrap_or(self.history_events.len() as i64);
        let start = usize::try_from(after_position).map_err(|error| error.to_string())?;
        let end = start
            .saturating_add(limit)
            .min(usize::try_from(high_watermark).map_err(|error| error.to_string())?)
            .min(self.history_events.len());
        let events = self.history_events[start.min(end)..end].to_vec();
        Ok((high_watermark, self.history_events.len(), events))
    }

    async fn apply_device_registry_events(
        &self,
        _sender_device_id: &str,
        _events: &[RegistryEvent],
    ) -> Result<RegistryPageApply, String> {
        self.apply_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.apply_result.clone())
    }

    async fn sign_registry_handshake(
        &self,
        _local_device_id: &str,
        _transcript: &RegistryHandshakeTranscript,
    ) -> Result<RegistryHandshakeProof, String> {
        Err("unexpected handshake signing".into())
    }

    async fn verify_registry_handshake(
        &self,
        _peer_device_id: &str,
        _transcript: &RegistryHandshakeTranscript,
        _proof: &RegistryHandshakeProof,
    ) -> Result<(), String> {
        Err("unexpected handshake verification".into())
    }
}

fn admission() -> DirectRegistryAdmission {
    DirectRegistryAdmission {
        local_device_id: DeviceId::parse("local-device").expect("valid local id"),
        peer_device_id: DeviceId::parse("remote-device").expect("valid peer id"),
        space_id: "space-test".into(),
        protocol: NegotiatedProtocol {
            version: 1,
            capabilities: BTreeSet::from([ProtocolCapability::RegistryHistoryV1]),
        },
        session_id: Some("session-test".into()),
        session_revoked: false,
        phase: AdmissionPhase::Exchanging {
            incoming_page_count: None,
            next_page_index: 0,
            local_digest: None,
            remote_digest: None,
            start_received: true,
        },
    }
}

fn event() -> RegistryEvent {
    RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("remote-device").expect("valid actor"),
            actor_sequence: 1,
            previous_event_hash: None,
            target_device_id: DeviceId::parse("local-device").expect("valid target"),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: ark_domain::hlc::HLC::now("remote-device").to_string(),
            operation: RegistryOperation::Disable,
        },
        &SigningKey::from_bytes(&[17; 32]),
    )
    .expect("sign a valid registry update")
}

fn storage(applied_events: Vec<RegistryEvent>, local_device_is_admitted: bool) -> RegistryStorage {
    RegistryStorage {
        apply_result: RegistryPageApply {
            digest: "a".repeat(64),
            sender_is_admitted: true,
            local_device_is_admitted,
            disabled_device_ids: if local_device_is_admitted {
                Vec::new()
            } else {
                vec!["local-device".into()]
            },
            applied_events,
        },
        apply_calls: Arc::new(AtomicUsize::new(0)),
        history_events: Vec::new(),
        page_reads: Mutex::new(Vec::new()),
    }
}

#[tokio::test]
async fn registry_history_pages_use_one_bounded_snapshot() {
    let mut backend = storage(Vec::new(), true);
    backend.history_events = vec![event(); REGISTRY_EVENT_PAGE_SIZE + 1];

    let pages = load_registry_event_pages(&backend, "paging-session")
        .await
        .expect("build registry history pages");

    assert_eq!(pages.len(), 2);
    assert!(matches!(
        &pages[0],
        LanSyncMessage::RegistryEventsPage {
            session_id,
            page_index: 0,
            page_count: 2,
            events,
        } if session_id == "paging-session" && events.len() == REGISTRY_EVENT_PAGE_SIZE
    ));
    assert!(matches!(
        &pages[1],
        LanSyncMessage::RegistryEventsPage {
            session_id,
            page_index: 1,
            page_count: 2,
            events,
        } if session_id == "paging-session" && events.len() == 1
    ));
    assert_eq!(
        *backend.page_reads.lock().expect("page read log lock"),
        vec![
            (0, None, REGISTRY_EVENT_PAGE_SIZE),
            (
                REGISTRY_EVENT_PAGE_SIZE as i64,
                Some(33),
                REGISTRY_EVENT_PAGE_SIZE
            ),
        ]
    );
}

#[tokio::test]
async fn rejects_page_after_declared_count_without_applying_its_event() {
    let event = event();
    let backend = storage(Vec::new(), true);
    let apply_calls = backend.apply_calls.clone();
    let mut admission = admission();
    if let AdmissionPhase::Exchanging {
        incoming_page_count,
        next_page_index,
        ..
    } = &mut admission.phase
    {
        *incoming_page_count = Some(1);
        *next_page_index = 1;
    }

    let result = admission
        .handle(
            &backend,
            LanSyncMessage::RegistryEventsPage {
                session_id: "session-test".into(),
                page_index: 1,
                page_count: 1,
                events: vec![event],
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(error) if error == "registry_sync_page_bounds_or_order"
    ));
    assert_eq!(apply_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn does_not_admit_local_device_disabled_by_registry_page() {
    let disable_local = event();
    let backend = storage(vec![disable_local.clone()], false);
    let apply_calls = backend.apply_calls.clone();
    let mut admission = admission();

    let result = admission
        .handle(
            &backend,
            LanSyncMessage::RegistryEventsPage {
                session_id: "session-test".into(),
                page_index: 0,
                page_count: 1,
                events: vec![disable_local],
            },
        )
        .await;
    let update = result.expect("signed registry event should be applied");
    assert!(update.close_session);
    assert!(!update.became_ready);
    assert_eq!(update.applied_events.len(), 1);
    assert_eq!(apply_calls.load(Ordering::SeqCst), 1);
}
