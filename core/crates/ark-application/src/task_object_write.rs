use crate::object_write::{
    NewObjectReceipt, ObjectWriteOrigin, ObjectWriteResult, ObjectWriteSession, ReceiptOutcome,
};
use crate::task_mutation::{self, TaskMutationSession};
use ark_domain::hlc::HLC;
use ark_domain::object_write::RemoteObjectWrite;
use ark_domain::types::{ArkObjectWrite, ObjectLink};
use std::cmp::Ordering;

mod authenticated;
mod decoder;
mod local;
pub use authenticated::{
    apply_authenticated_core_task_write, AuthenticatedCoreTaskWrite,
    AuthenticatedCoreTaskWriteError, AuthenticatedCoreTaskWriteSession, InboxReceiptState,
    RemoteCoreTaskOutcome,
};
pub use decoder::{decode_remote_core_task_write, RemoteCoreTaskWriteDecodeError};
pub use local::{LocalCoreTaskWrite, LocalCoreTaskWriteCommit, TaskWriteOperationResult};

const CORE_TASK_ID: &str = "core.task";

enum Input {
    Local {
        object: ArkObjectWrite,
        links: Vec<ObjectLink>,
    },
    Remote(RemoteObjectWrite),
}

fn apply_core_task_write(
    object_session: &dyn ObjectWriteSession,
    task_session: &dyn TaskMutationSession,
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
    if resolved_type_id != CORE_TASK_ID
        || !ark_domain::canonical_type::definitions::is_supported_core_task_version(
            &resolved_version,
        )
    {
        return Err("object_write_slice_unsupported_type".into());
    }
    let object = ark_domain::canonical_type::prepare_resolved_object(
        write,
        &resolved_type_id,
        &resolved_version,
        Some(&registration),
    )
    .map_err(|error| error.to_string())?;
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
    let task_entities = task_mutation::apply_task_upsert(
        task_session,
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
        task_session,
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
    apply_core_task_write(object_session, task_session, origin, Input::Remote(remote))
}
