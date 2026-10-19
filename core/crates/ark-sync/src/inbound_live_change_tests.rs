use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;

use ark_application::replication_storage::AuthenticatedEntityBackend;
use ark_application::sync_policy::{PolicyIdentity, SyncProfile};
use ark_application::sync_storage::{InboundApplyOutcome, PeerProgressPort};
use ark_domain::types::{PeerRecord, SyncEntity, VersionVector};

use super::{
    apply_live_change, prepare_live_change, IncomingLiveChange, LiveChangeDisposition,
    VersionVectorWriteMode,
};
use crate::protocol::{NegotiatedProtocol, ProtocolCapability};

struct FakeStorage {
    trace: Mutex<Vec<&'static str>>,
    vector: Mutex<VersionVector>,
    write_during_apply: Mutex<Option<(String, String)>>,
    apply_outcome: InboundApplyOutcome,
}

impl FakeStorage {
    fn new(apply_outcome: InboundApplyOutcome) -> Self {
        Self {
            trace: Mutex::new(Vec::new()),
            vector: Mutex::new(HashMap::new()),
            write_during_apply: Mutex::new(None),
            apply_outcome,
        }
    }
}

#[async_trait::async_trait]
impl AuthenticatedEntityBackend for FakeStorage {
    async fn receive_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
    ) -> Result<bool, String> {
        self.receive_authenticated_entity_with_policy(
            entity,
            authenticated_peer_id,
            message_id,
            "",
            None,
        )
        .await
    }

    async fn receive_authenticated_entity_with_policy(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
        _message_id: &str,
        _local_device_id: &str,
        _policy_identity: Option<PolicyIdentity>,
    ) -> Result<bool, String> {
        self.trace.lock().unwrap().push("receipt");
        Ok(true)
    }

    async fn apply_authenticated_entity(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
    ) -> Result<(), String> {
        self.apply_authenticated_entity_with_policy(entity, authenticated_peer_id, "", "", None)
            .await
            .map(|_| ())
    }

    async fn apply_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
    ) -> Result<InboundApplyOutcome, String> {
        self.apply_authenticated_entity_with_policy(
            entity,
            authenticated_peer_id,
            message_id,
            "",
            None,
        )
        .await
    }

    async fn apply_authenticated_entity_with_policy(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
        _message_id: &str,
        _local_device_id: &str,
        _policy_identity: Option<PolicyIdentity>,
    ) -> Result<InboundApplyOutcome, String> {
        self.trace.lock().unwrap().push("apply");
        if let Some((key, value)) = self.write_during_apply.lock().unwrap().take() {
            self.vector.lock().unwrap().insert(key, value);
        }
        Ok(self.apply_outcome)
    }
}

#[async_trait::async_trait]
impl PeerProgressPort for FakeStorage {
    async fn load_version_vector(&self) -> Result<VersionVector, String> {
        self.trace.lock().unwrap().push("load_vector");
        Ok(self.vector.lock().unwrap().clone())
    }

    async fn store_version_vector(&self, vector: &VersionVector) -> Result<(), String> {
        self.trace.lock().unwrap().push("store_vector");
        *self.vector.lock().unwrap() = vector.clone();
        Ok(())
    }

    async fn store_version_vector_with_policy(
        &self,
        _local_device_id: &str,
        _policy_identity: &PolicyIdentity,
        vector: &VersionVector,
    ) -> Result<(), String> {
        self.store_version_vector(vector).await
    }

    async fn load_known_peers(&self) -> Result<Option<Vec<PeerRecord>>, String> {
        Ok(None)
    }

    async fn store_known_peers(&self, _peers: &[PeerRecord]) -> Result<(), String> {
        Ok(())
    }

    async fn load_removed_peer_ids(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }

    async fn store_removed_peer_ids(&self, _peer_ids: &[String]) -> Result<(), String> {
        Ok(())
    }
}

fn protocol(capabilities: &[ProtocolCapability]) -> NegotiatedProtocol {
    NegotiatedProtocol {
        version: 1,
        capabilities: capabilities.iter().cloned().collect::<BTreeSet<_>>(),
    }
}

fn entity() -> SyncEntity {
    SyncEntity {
        entity_type: "todo".into(),
        id: "todo-1".into(),
        data: Default::default(),
        hlc: "2026-10-17T12:00:00.000Z-0-device".into(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    }
}

fn incoming<'a>(
    protocol: Option<&'a NegotiatedProtocol>,
    local_profile: Option<&'a SyncProfile>,
    entity: SyncEntity,
    policy_identity: Option<PolicyIdentity>,
) -> IncomingLiveChange<'a> {
    IncomingLiveChange {
        protocol,
        local_profile,
        local_device_id: "local",
        peer_device_id: "peer",
        change_id: "change-1".into(),
        entity,
        policy_identity,
    }
}

#[tokio::test]
async fn durable_receipt_can_be_acknowledged_before_apply() {
    let storage = FakeStorage::new(InboundApplyOutcome::Applied);
    let protocol = protocol(&[ProtocolCapability::DurableReceiptAckV1]);
    let prepared = prepare_live_change(&storage, incoming(Some(&protocol), None, entity(), None))
        .await
        .unwrap();
    assert!(prepared.receipt_recorded);
    storage.trace.lock().unwrap().push("receipt_ack");

    let result = apply_live_change(&storage, prepared, VersionVectorWriteMode::MergeConcurrent)
        .await
        .unwrap();

    assert_eq!(
        result.disposition,
        LiveChangeDisposition::Applied { notify_local: true }
    );
    let trace = storage.trace.lock().unwrap();
    let ack = trace
        .iter()
        .position(|event| *event == "receipt_ack")
        .unwrap();
    let apply = trace.iter().position(|event| *event == "apply").unwrap();
    assert!(ack < apply, "receipt ACK must precede apply: {trace:?}");
}

#[tokio::test]
async fn stale_policy_identity_is_rejected_before_receipt_or_apply() {
    let storage = FakeStorage::new(InboundApplyOutcome::Applied);
    let profile = SyncProfile {
        profile_id: "profile".into(),
        device_id: "local".into(),
        revision: 3,
        name: "default".into(),
        active: true,
        rules: Vec::new(),
    };
    let protocol = protocol(&[ProtocolCapability::PolicyBoundDeliveryV1]);
    let result = prepare_live_change(
        &storage,
        incoming(
            Some(&protocol),
            Some(&profile),
            entity(),
            Some(PolicyIdentity {
                revision: 2,
                digest: "0".repeat(64),
                active: true,
            }),
        ),
    )
    .await;

    assert!(matches!(
        result,
        Err(error) if error == "stale_or_unnegotiated_sync_policy"
    ));
    assert!(storage.trace.lock().unwrap().is_empty());
}

#[tokio::test]
async fn updating_vector_preserves_newer_usage_cursors() {
    let storage = FakeStorage::new(InboundApplyOutcome::Applied);
    storage
        .vector
        .lock()
        .unwrap()
        .insert("@usage:local".into(), "42".into());
    let prepared = prepare_live_change(&storage, incoming(None, None, entity(), None))
        .await
        .unwrap();

    apply_live_change(&storage, prepared, VersionVectorWriteMode::MergeConcurrent)
        .await
        .unwrap();

    let vector = storage.vector.lock().unwrap();
    assert_eq!(vector.get("@usage:local").map(String::as_str), Some("42"));
    assert!(vector.contains_key("todo-1"));
}

#[tokio::test]
async fn pending_and_stale_are_distinct_results() {
    for (storage_outcome, expected) in [
        (InboundApplyOutcome::Pending, LiveChangeDisposition::Pending),
        (InboundApplyOutcome::Stale, LiveChangeDisposition::Stale),
    ] {
        let storage = FakeStorage::new(storage_outcome);
        let prepared = prepare_live_change(&storage, incoming(None, None, entity(), None))
            .await
            .unwrap();
        let result = apply_live_change(&storage, prepared, VersionVectorWriteMode::MergeConcurrent)
            .await
            .unwrap();
        assert_eq!(result.disposition, expected);
    }
}

#[tokio::test]
async fn client_replace_mode_preserves_legacy_vector_write_semantics() {
    let storage = FakeStorage::new(InboundApplyOutcome::Applied);
    *storage.write_during_apply.lock().unwrap() =
        Some(("concurrent-local-write".into(), "newer-hlc".into()));
    let prepared = prepare_live_change(&storage, incoming(None, None, entity(), None))
        .await
        .unwrap();

    apply_live_change(&storage, prepared, VersionVectorWriteMode::Replace)
        .await
        .unwrap();

    assert!(!storage
        .vector
        .lock()
        .unwrap()
        .contains_key("concurrent-local-write"));
}

#[tokio::test]
async fn server_merge_mode_keeps_concurrent_vector_writes() {
    let storage = FakeStorage::new(InboundApplyOutcome::Applied);
    *storage.write_during_apply.lock().unwrap() =
        Some(("concurrent-local-write".into(), "newer-hlc".into()));
    let prepared = prepare_live_change(&storage, incoming(None, None, entity(), None))
        .await
        .unwrap();

    apply_live_change(&storage, prepared, VersionVectorWriteMode::MergeConcurrent)
        .await
        .unwrap();

    assert_eq!(
        storage
            .vector
            .lock()
            .unwrap()
            .get("concurrent-local-write")
            .map(String::as_str),
        Some("newer-hlc")
    );
}
