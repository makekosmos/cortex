use super::{
    apply_remote_core_task_write, decode_remote_core_task_write, RemoteCoreTaskWriteDecodeError,
};
use crate::object_write::ObjectWriteSession;
use crate::sync_policy::{sync_profile_identity, SyncProfile};
use crate::sync_storage::PolicyIdentity;
use crate::task_mutation::TaskMutationSession;
use ark_domain::hlc::HLC;
use serde_json::{Map, Value};

#[derive(Clone, Debug, serde::Serialize)]
pub struct AuthenticatedCoreTaskWrite {
    id: String,
    source_hlc: String,
    source_origin_device_id: Option<String>,
    data: Map<String, Value>,
    inbox_message_id: String,
    inbox_payload_json: String,
    local_device_id: Option<String>,
    policy_identity: Option<PolicyIdentity>,
}

impl AuthenticatedCoreTaskWrite {
    pub fn new(
        id: String,
        source_hlc: String,
        source_origin_device_id: Option<String>,
        data: Map<String, Value>,
        inbox_message_id: String,
        inbox_payload_json: String,
    ) -> Self {
        Self {
            id,
            source_hlc,
            source_origin_device_id,
            data,
            inbox_message_id,
            inbox_payload_json,
            local_device_id: None,
            policy_identity: None,
        }
    }

    pub fn with_policy_identity(
        mut self,
        local_device_id: String,
        policy_identity: PolicyIdentity,
    ) -> Self {
        self.local_device_id = Some(local_device_id);
        self.policy_identity = Some(policy_identity);
        self
    }

    pub fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboxReceiptState {
    Received,
    Pending,
    Applied,
    Rejected,
}

pub trait AuthenticatedCoreTaskWriteSession {
    fn load_sync_profile_for_sync(
        &self,
        local_device_id: &str,
    ) -> Result<Option<SyncProfile>, String>;
    fn is_peer_admitted(&self, peer_id: &str) -> Result<bool, String>;
    fn receive_inbox_payload(
        &self,
        peer_id: &str,
        message_id: &str,
        payload_json: &str,
    ) -> Result<InboxReceiptState, String>;
    fn mark_inbox_applied(&self, peer_id: &str, message_id: &str) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteCoreTaskOutcome {
    Applied,
    Duplicate,
    Stale,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AuthenticatedCoreTaskWriteError {
    MissingLocalDevice,
    StalePolicy,
    PeerNotAdmitted,
    PeerRemoved,
    IncompleteRepresentation,
    UnsupportedType,
    InvalidObject(String),
    Storage(String),
}

pub fn apply_authenticated_core_task_write(
    object_session: &dyn ObjectWriteSession,
    task_session: &dyn TaskMutationSession,
    auth_session: &dyn AuthenticatedCoreTaskWriteSession,
    peer_id: &str,
    write: AuthenticatedCoreTaskWrite,
) -> Result<RemoteCoreTaskOutcome, AuthenticatedCoreTaskWriteError> {
    if let Some(expected) = write.policy_identity.as_ref() {
        let local_device_id = write
            .local_device_id
            .as_deref()
            .ok_or(AuthenticatedCoreTaskWriteError::MissingLocalDevice)?;
        let profile = auth_session
            .load_sync_profile_for_sync(local_device_id)
            .map_err(AuthenticatedCoreTaskWriteError::Storage)?;
        if &sync_profile_identity(profile.as_ref()) != expected {
            return Err(AuthenticatedCoreTaskWriteError::StalePolicy);
        }
    }
    if !auth_session
        .is_peer_admitted(peer_id)
        .map_err(AuthenticatedCoreTaskWriteError::Storage)?
    {
        return Err(AuthenticatedCoreTaskWriteError::PeerNotAdmitted);
    }
    if auth_session
        .receive_inbox_payload(peer_id, &write.inbox_message_id, &write.inbox_payload_json)
        .map_err(AuthenticatedCoreTaskWriteError::Storage)?
        == InboxReceiptState::Applied
    {
        return Ok(RemoteCoreTaskOutcome::Duplicate);
    }
    if object_session
        .is_peer_removed(peer_id)
        .map_err(AuthenticatedCoreTaskWriteError::Storage)?
    {
        return Err(AuthenticatedCoreTaskWriteError::PeerRemoved);
    }
    if object_session
        .load_receipt(&write.id, "full", &write.source_hlc)
        .map_err(AuthenticatedCoreTaskWriteError::Storage)?
        .is_none()
        && object_session
            .current_revision(&write.id)
            .map_err(AuthenticatedCoreTaskWriteError::Storage)?
            .as_deref()
            .is_some_and(|current| {
                HLC::compare_str(&write.source_hlc, current) != std::cmp::Ordering::Greater
            })
    {
        auth_session
            .mark_inbox_applied(peer_id, &write.inbox_message_id)
            .map_err(AuthenticatedCoreTaskWriteError::Storage)?;
        return Ok(RemoteCoreTaskOutcome::Stale);
    }

    let remote = decode_remote_core_task_write(
        write.id,
        write.source_hlc,
        write.source_origin_device_id,
        write.data,
    )
    .map_err(map_decode_error)?;
    let result = apply_remote_core_task_write(
        object_session,
        task_session,
        remote,
        Some(peer_id.to_owned()),
    )
    .map_err(AuthenticatedCoreTaskWriteError::Storage)?;
    auth_session
        .mark_inbox_applied(peer_id, &write.inbox_message_id)
        .map_err(AuthenticatedCoreTaskWriteError::Storage)?;
    Ok(if result.duplicate {
        RemoteCoreTaskOutcome::Duplicate
    } else {
        RemoteCoreTaskOutcome::Applied
    })
}

fn map_decode_error(error: RemoteCoreTaskWriteDecodeError) -> AuthenticatedCoreTaskWriteError {
    match error {
        RemoteCoreTaskWriteDecodeError::IncompleteRepresentation => {
            AuthenticatedCoreTaskWriteError::IncompleteRepresentation
        }
        RemoteCoreTaskWriteDecodeError::UnsupportedType => {
            AuthenticatedCoreTaskWriteError::UnsupportedType
        }
        RemoteCoreTaskWriteDecodeError::InvalidObject(error) => {
            AuthenticatedCoreTaskWriteError::InvalidObject(error)
        }
    }
}
