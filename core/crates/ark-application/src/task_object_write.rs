use crate::object_write::{
    NewObjectReceipt, ObjectWriteOrigin, ObjectWriteResult, ObjectWriteSession, ReceiptOutcome,
    StoredObjectReceipt,
};
use crate::task_mutation::{self, TaskMutationSession};
use ark_domain::hlc::HLC;
use ark_domain::object_write::{ObjectWriteSnapshot, RemoteObjectWrite};
use ark_domain::types::{ArkObject, ArkObjectWrite, ObjectLink};
use serde_json::{Map, Value};
use std::cmp::Ordering;

const CORE_TASK_ID: &str = "core.task";
const CORE_TASK_VERSION: &str = "0.1.0";

#[derive(Debug, PartialEq, Eq)]
pub enum RemoteCoreTaskWriteDecodeError {
    IncompleteRepresentation,
    UnsupportedType,
    InvalidObject(String),
}

pub fn decode_remote_core_task_write(
    id: String,
    source_hlc: String,
    source_origin_device_id: Option<String>,
    data: Map<String, Value>,
) -> Result<RemoteObjectWrite, RemoteCoreTaskWriteDecodeError> {
    for key in [
        "typeId",
        "typeVersion",
        "title",
        "createdAt",
        "updatedAt",
        "deletedAt",
        "contentJson",
        "propsJson",
    ] {
        if !data.contains_key(key) {
            return Err(RemoteCoreTaskWriteDecodeError::IncompleteRepresentation);
        }
    }
    if data.get("typeId").and_then(Value::as_str) != Some(CORE_TASK_ID)
        || data.get("typeVersion").and_then(Value::as_str) != Some(CORE_TASK_VERSION)
    {
        return Err(RemoteCoreTaskWriteDecodeError::UnsupportedType);
    }
    let mut data = data;
    data.insert("id".into(), Value::String(id));
    let links: Vec<ObjectLink> = data
        .remove("links")
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| RemoteCoreTaskWriteDecodeError::InvalidObject(error.to_string()))?
        .unwrap_or_default();
    let object = serde_json::from_value(Value::Object(data))
        .map_err(|error| RemoteCoreTaskWriteDecodeError::InvalidObject(error.to_string()))?;
    Ok(RemoteObjectWrite {
        object,
        links,
        source_hlc,
        source_origin_device_id,
    })
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LocalCoreTaskWrite {
    object: ArkObjectWrite,
    expected_snapshot: Option<ObjectWriteSnapshot>,
    device_id: Option<String>,
}

impl LocalCoreTaskWrite {
    pub fn new(
        object: ArkObjectWrite,
        expected_snapshot: Option<ObjectWriteSnapshot>,
        device_id: Option<String>,
    ) -> Self {
        Self {
            object,
            expected_snapshot,
            device_id,
        }
    }

    pub fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }

    pub fn object_type_id(&self) -> &str {
        &self.object.type_id
    }

    pub fn into_parts(self) -> (ArkObjectWrite, Option<ObjectWriteSnapshot>, Option<String>) {
        (self.object, self.expected_snapshot, self.device_id)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LocalCoreTaskWriteCommit {
    pub object: ArkObject,
    pub receipt: StoredObjectReceipt,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TaskWriteOperationResult {
    pub value: LocalCoreTaskWriteCommit,
    pub replayed: bool,
}

enum Input {
    Local {
        object: ArkObjectWrite,
        links: Vec<ObjectLink>,
    },
    Remote(RemoteObjectWrite),
}

fn apply_core_task_write(
    object_session: &dyn ObjectWriteSession,
    local_task_session: Option<&dyn TaskMutationSession>,
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
        let current = object_session.object_snapshot(&write.id)?;
        if current != *expected {
            return Err("object_conflict:stale_snapshot".into());
        }
    }
    if let Some(peer_id) = peer_id.as_deref() {
        if object_session.is_peer_removed(peer_id)? {
            return Err("sync_peer_removed".into());
        }
    }

    let (resolved_type_id, resolved_version) = object_session
        .resolve_type_identity(&write.type_id, write.type_version.as_deref())
        .map_err(|_| {
            ark_domain::canonical_type::CanonicalIngressError::invalid("unknown_type_or_version")
                .to_string()
        })?;
    let registration = ark_domain::canonical_type::definitions::canonical_type_registrations()
        .map_err(|_| {
            ark_domain::canonical_type::CanonicalIngressError::invariant("canonical_registry")
                .to_string()
        })?
        .into_iter()
        .find(|registration| {
            registration.type_id == resolved_type_id && registration.version == resolved_version
        })
        .ok_or_else(|| {
            ark_domain::canonical_type::CanonicalIngressError::invariant("canonical_registry")
                .to_string()
        })?;
    if resolved_type_id != CORE_TASK_ID || resolved_version != CORE_TASK_VERSION {
        return Err("object_write_slice_unsupported_type".into());
    }
    let object = ark_domain::canonical_type::prepare_resolved_object(
        write,
        &resolved_type_id,
        &resolved_version,
        Some(&registration),
    )
    .map_err(|error| error.to_string())?;
    if source_hlc.is_some() {
        local_task_session
            .ok_or_else(|| "object_write_origin_mismatch".to_string())?
            .ensure_legacy_task_write_allowed(&object.id)?;
    }
    let (payload_json, payload_sha256) =
        ark_domain::object_write::canonical_payload_digest(&object, &links)?;
    let representation_kind = "full";

    if let Some(source_hlc) = source_hlc.as_deref() {
        if let Some(receipt) =
            object_session.load_receipt(&object.id, representation_kind, source_hlc)?
        {
            if receipt.payload_sha256 != payload_sha256 {
                return Err("object_revision_payload_mismatch".into());
            }
            return Ok(ObjectWriteResult {
                receipt: Some(receipt),
                duplicate: true,
            });
        }
        let current = object_session.current_revision(&object.id)?;
        if current
            .as_deref()
            .is_some_and(|current| HLC::compare_str(source_hlc, current) != Ordering::Greater)
        {
            let receipt = object_session.insert_receipt(NewObjectReceipt {
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
        let _ = task_mutation::apply_inbound_task_mutation(
            task_session,
            task_mutation::InboundTaskMutation::CanonicalFullSnapshot {
                object: &object,
                links: &links,
                source_hlc,
            },
        )?;
        let receipt = object_session.insert_receipt(NewObjectReceipt {
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
    let task_entities = task_mutation::apply_task_upsert(
        local_task_session,
        &object,
        &links,
        task_device_id,
        expected_snapshot.as_ref(),
    )?;
    let source_hlc = task_entities
        .into_iter()
        .next()
        .ok_or_else(|| "task_upsert_missing_entity".to_string())?
        .hlc;
    let source_origin_device_id = HLC::from_string(&source_hlc).device_id;
    let receipt = object_session.insert_receipt(NewObjectReceipt {
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

pub fn apply_local_core_task_write(
    object_session: &dyn ObjectWriteSession,
    task_session: &dyn TaskMutationSession,
    write: LocalCoreTaskWrite,
) -> Result<ObjectWriteResult, String> {
    let (object, expected_snapshot, device_id) = write.into_parts();
    apply_core_task_write(
        object_session,
        Some(task_session),
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

pub fn apply_remote_core_task_write(
    object_session: &dyn ObjectWriteSession,
    task_session: &dyn TaskMutationSession,
    remote: RemoteObjectWrite,
    authenticated_peer_id: Option<String>,
) -> Result<ObjectWriteResult, String> {
    let origin = if let Some(peer_id) = authenticated_peer_id.clone() {
        ObjectWriteOrigin::AuthenticatedRemote { peer_id }
    } else {
        ObjectWriteOrigin::TrustedRemote
    };
    apply_core_task_write(
        object_session,
        Some(task_session),
        origin,
        Input::Remote(remote),
    )
}

#[cfg(test)]
mod decode_tests {
    use super::{decode_remote_core_task_write, RemoteCoreTaskWriteDecodeError};
    use serde_json::{json, Map, Value};

    fn task_data() -> Map<String, Value> {
        json!({
            "typeId": "core.task",
            "typeVersion": "0.1.0",
            "title": "Task",
            "createdAt": "created",
            "updatedAt": "updated",
            "deletedAt": null,
            "contentJson": {},
            "propsJson": {},
            "links": []
        })
        .as_object()
        .expect("task fixture is a JSON object")
        .clone()
    }

    #[test]
    fn remote_task_decoder_requires_full_representation() {
        let mut data = task_data();
        data.remove("deletedAt");
        assert!(matches!(
            decode_remote_core_task_write("id".into(), "hlc".into(), None, data),
            Err(RemoteCoreTaskWriteDecodeError::IncompleteRepresentation)
        ));
    }

    #[test]
    fn remote_task_decoder_rejects_other_types() {
        let mut data = task_data();
        data.insert("typeId".into(), Value::String("note".into()));
        assert!(matches!(
            decode_remote_core_task_write("id".into(), "hlc".into(), None, data),
            Err(RemoteCoreTaskWriteDecodeError::UnsupportedType)
        ));
    }

    #[test]
    fn remote_task_decoder_preserves_identity_and_links() {
        let remote = decode_remote_core_task_write(
            "task-id".into(),
            "hlc".into(),
            Some("origin".into()),
            task_data(),
        )
        .expect("valid core.task full representation decodes");
        assert_eq!(remote.object.id, "task-id");
        assert_eq!(remote.object.type_id, "core.task");
        assert!(remote.links.is_empty());
        assert_eq!(remote.source_hlc, "hlc");
        assert_eq!(remote.source_origin_device_id.as_deref(), Some("origin"));
    }
}
