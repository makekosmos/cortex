use crate::canonical_types::definitions::canonical_type_registrations;
use crate::canonical_types::ingress::prepare_resolved_object;
use crate::domain::object_write::{ObjectWriteSnapshot, RemoteObjectWrite};
use crate::hlc::HLC;
use crate::types::{ArkObject, ArkObjectWrite, ObjectLink};
use std::cmp::Ordering;

const CORE_TASK_ID: &str = "core.task";
const CORE_TASK_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectWriteOrigin {
    Local {
        device_id: Option<String>,
        expected_snapshot: Option<ObjectWriteSnapshot>,
    },
    AuthenticatedRemote {
        peer_id: String,
    },
    TrustedRemote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReceiptOutcome {
    Applied,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredObjectReceipt {
    pub change_seq: i64,
    pub payload_sha256: String,
    pub payload_json: String,
    pub outcome: ReceiptOutcome,
    pub resulting_hlc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewObjectReceipt {
    pub entity_id: String,
    pub representation_kind: String,
    pub source_hlc: String,
    pub source_origin_device_id: Option<String>,
    pub type_id: String,
    pub payload_sha256: String,
    pub payload_json: String,
    pub outcome: ReceiptOutcome,
    pub resulting_hlc: Option<String>,
    pub authenticated_peer_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectWriteResult {
    pub receipt: Option<StoredObjectReceipt>,
    pub duplicate: bool,
}

pub trait ObjectWriteSession {
    fn resolve_type_identity(
        &self,
        type_id: &str,
        requested_version: Option<&str>,
    ) -> Result<(String, String), String>;
    fn object_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String>;
    fn current_revision(&self, id: &str) -> Result<Option<String>, String>;
    fn is_peer_removed(&self, peer_id: &str) -> Result<bool, String>;
    fn persist_object(&self, object: &ArkObject, links: &[ObjectLink]) -> Result<(), String>;
    fn record_local_revision(&self, id: &str, device_id: Option<&str>) -> Result<String, String>;
    fn record_remote_revision(&self, id: &str, source_hlc: &str) -> Result<(), String>;
    fn load_receipt(
        &self,
        entity_id: &str,
        representation_kind: &str,
        source_hlc: &str,
    ) -> Result<Option<StoredObjectReceipt>, String>;
    fn insert_receipt(&self, receipt: NewObjectReceipt) -> Result<StoredObjectReceipt, String>;
}

enum Input {
    Local {
        object: ArkObjectWrite,
        links: Vec<ObjectLink>,
    },
    Remote(RemoteObjectWrite),
}

fn apply_core_task_write(
    session: &dyn ObjectWriteSession,
    local_task_session: Option<&dyn crate::application::task_mutation::TaskMutationSession>,
    origin: ObjectWriteOrigin,
    input: Input,
) -> Result<ObjectWriteResult, String> {
    let (write, links, source_hlc, source_origin, peer_id, expected_snapshot, device_id) =
        match (origin, input) {
            (
                ObjectWriteOrigin::Local {
                    device_id,
                    expected_snapshot,
                },
                Input::Local { object, links },
            ) => (
                object,
                links,
                None,
                None,
                None,
                expected_snapshot,
                device_id,
            ),
            (ObjectWriteOrigin::AuthenticatedRemote { peer_id }, Input::Remote(remote)) => (
                remote.object,
                remote.links,
                Some(remote.source_hlc),
                remote.source_origin_device_id,
                Some(peer_id),
                None,
                None,
            ),
            (ObjectWriteOrigin::TrustedRemote, Input::Remote(remote)) => (
                remote.object,
                remote.links,
                Some(remote.source_hlc),
                remote.source_origin_device_id,
                None,
                None,
                None,
            ),
            _ => return Err("object_write_origin_mismatch".into()),
        };

    if let Some(expected) = expected_snapshot.as_ref() {
        if session.object_snapshot(&write.id)? != *expected {
            return Err("object_conflict:stale_snapshot".into());
        }
    }
    if let Some(peer_id) = peer_id.as_deref() {
        if session.is_peer_removed(peer_id)? {
            return Err("sync_peer_removed".into());
        }
    }

    let (resolved_type_id, resolved_version) = session
        .resolve_type_identity(&write.type_id, write.type_version.as_deref())
        .map_err(|_| {
            crate::canonical_types::ingress::CanonicalIngressError::invalid(
                "unknown_type_or_version",
            )
            .to_string()
        })?;
    let registration = canonical_type_registrations()
        .map_err(|_| {
            crate::canonical_types::ingress::CanonicalIngressError::invariant("canonical_registry")
                .to_string()
        })?
        .into_iter()
        .find(|registration| {
            registration.type_id == resolved_type_id && registration.version == resolved_version
        })
        .ok_or_else(|| {
            crate::canonical_types::ingress::CanonicalIngressError::invariant("canonical_registry")
                .to_string()
        })?;
    if resolved_type_id != CORE_TASK_ID || resolved_version != CORE_TASK_VERSION {
        return Err("object_write_slice_unsupported_type".into());
    }
    let object = prepare_resolved_object(
        write,
        &resolved_type_id,
        &resolved_version,
        Some(&registration),
    )
    .map_err(|error| error.to_string())?;
    let (payload_json, payload_sha256) =
        crate::domain::object_write::canonical_payload_digest(&object, &links)?;
    let representation_kind = "full";

    if let Some(source_hlc) = source_hlc.as_deref() {
        if let Some(receipt) = session.load_receipt(&object.id, representation_kind, source_hlc)? {
            if receipt.payload_sha256 != payload_sha256 {
                return Err("object_revision_payload_mismatch".into());
            }
            return Ok(ObjectWriteResult {
                receipt: Some(receipt),
                duplicate: true,
            });
        }
        let current = session.current_revision(&object.id)?;
        if current
            .as_deref()
            .is_some_and(|current| HLC::compare_str(source_hlc, current) != Ordering::Greater)
        {
            let receipt = session.insert_receipt(NewObjectReceipt {
                entity_id: object.id.clone(),
                representation_kind: representation_kind.to_owned(),
                source_hlc: source_hlc.to_owned(),
                source_origin_device_id: source_origin.clone(),
                type_id: object.type_id.clone(),
                payload_sha256: payload_sha256.clone(),
                payload_json: payload_json.clone(),
                outcome: ReceiptOutcome::Stale,
                resulting_hlc: current,
                authenticated_peer_id: peer_id.clone(),
            })?;
            return Ok(ObjectWriteResult {
                receipt: Some(receipt),
                duplicate: false,
            });
        }
        let task_session =
            local_task_session.ok_or_else(|| "object_write_origin_mismatch".to_string())?;
        let _ = crate::application::task_mutation::apply_inbound_task_mutation(
            task_session,
            crate::application::task_mutation::InboundTaskMutation::CanonicalFullSnapshot {
                object: &object,
                links: &links,
                source_hlc,
            },
        )?;
        let receipt = session.insert_receipt(NewObjectReceipt {
            entity_id: object.id.clone(),
            representation_kind: representation_kind.to_owned(),
            source_hlc: source_hlc.to_owned(),
            source_origin_device_id: source_origin.clone(),
            type_id: object.type_id.clone(),
            payload_sha256: payload_sha256.clone(),
            payload_json: payload_json.clone(),
            outcome: ReceiptOutcome::Applied,
            resulting_hlc: Some(source_hlc.to_owned()),
            authenticated_peer_id: peer_id.clone(),
        })?;
        return Ok(ObjectWriteResult {
            receipt: Some(receipt),
            duplicate: false,
        });
    }

    let task_device_id = device_id.as_deref().unwrap_or("ark-core-rpc-local");
    let local_task_session =
        local_task_session.ok_or_else(|| "object_write_origin_mismatch".to_string())?;
    let task_entities = crate::application::task_mutation::apply_task_upsert(
        local_task_session,
        &object,
        &links,
        task_device_id,
    )?;
    let source_hlc = task_entities
        .into_iter()
        .next()
        .ok_or_else(|| "task_upsert_missing_entity".to_string())?
        .hlc;
    let source_origin_device_id = HLC::from_string(&source_hlc).device_id;
    let receipt = session.insert_receipt(NewObjectReceipt {
        entity_id: object.id.clone(),
        representation_kind: representation_kind.to_owned(),
        source_hlc: source_hlc.clone(),
        source_origin_device_id: Some(source_origin_device_id),
        type_id: object.type_id.clone(),
        payload_sha256,
        payload_json,
        outcome: ReceiptOutcome::Applied,
        resulting_hlc: Some(source_hlc),
        authenticated_peer_id: None,
    })?;
    Ok(ObjectWriteResult {
        receipt: Some(receipt),
        duplicate: false,
    })
}

pub(crate) fn apply_local_core_task_write(
    session: &dyn crate::application::task_mutation::TaskMutationSession,
    object: ArkObjectWrite,
    expected_snapshot: Option<ObjectWriteSnapshot>,
    device_id: Option<String>,
) -> Result<ObjectWriteResult, String> {
    apply_core_task_write(
        session,
        Some(session),
        ObjectWriteOrigin::Local {
            device_id,
            expected_snapshot,
        },
        Input::Local {
            object,
            links: Vec::new(),
        },
    )
}

pub(crate) fn apply_remote_core_task_write(
    session: &dyn crate::application::task_mutation::TaskMutationSession,
    remote: RemoteObjectWrite,
    authenticated_peer_id: Option<String>,
) -> Result<ObjectWriteResult, String> {
    let origin = if let Some(peer_id) = authenticated_peer_id.clone() {
        ObjectWriteOrigin::AuthenticatedRemote { peer_id }
    } else {
        ObjectWriteOrigin::TrustedRemote
    };
    apply_core_task_write(session, Some(session), origin, Input::Remote(remote))
}
