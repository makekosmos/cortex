use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::types::{ArkObject, ArkObjectWrite, ObjectLink};
use ark_application::object_write::{ObjectWriteResult, ObjectWriteSession, StoredObjectReceipt};
use ark_domain::object_write::{ObjectWriteSnapshot, RemoteObjectWrite};
use rusqlite::Transaction;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct LocalCoreTaskWrite {
    object: ArkObjectWrite,
    expected_snapshot: Option<ObjectWriteSnapshot>,
    device_id: Option<String>,
}

impl LocalCoreTaskWrite {
    pub(crate) fn new(
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
}

impl sealed::Sealed for LocalCoreTaskWrite {}

impl DbCommand for LocalCoreTaskWrite {
    type Output = CoreTaskWriteCommit;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        if self.object.type_id != "core.task" {
            return Err(DbError::Unsupported(
                "object_write_slice_unsupported_type".into(),
            ));
        }
        let id = self.object.id.clone();
        let object_session = ark_store::object_write::SqliteObjectWriteSession::new(transaction);
        let task_session = ark_store::task_merge::SqliteTaskMutationSession::new(transaction);
        let result = ark_application::task_object_write::apply_local_core_task_write(
            &object_session,
            &task_session,
            self.object,
            self.expected_snapshot,
            self.device_id,
        )
        .map_err(DbError::from_application_error)?;
        let receipt = required_receipt(result)?;
        let object = ark_store::objects::get_object(transaction, &id)
            .map_err(|error| DbError::Storage(error.to_string()))?
            .ok_or_else(|| DbError::Storage("object_write_missing_after_commit".into()))?;
        Ok(CoreTaskWriteCommit { object, receipt })
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AuthenticatedCoreTaskWrite {
    id: String,
    source_hlc: String,
    source_origin_device_id: Option<String>,
    data: Map<String, Value>,
    inbox_message_id: String,
    inbox_payload_json: String,
    local_device_id: Option<String>,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
}

impl AuthenticatedCoreTaskWrite {
    pub(crate) fn new(
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_policy_identity(
        id: String,
        source_hlc: String,
        source_origin_device_id: Option<String>,
        data: Map<String, Value>,
        inbox_message_id: String,
        inbox_payload_json: String,
        local_device_id: String,
        policy_identity: ark_application::sync_storage::PolicyIdentity,
    ) -> Self {
        let mut command = Self::new(
            id,
            source_hlc,
            source_origin_device_id,
            data,
            inbox_message_id,
            inbox_payload_json,
        );
        command.local_device_id = Some(local_device_id);
        command.policy_identity = Some(policy_identity);
        command
    }
}

impl sealed::Sealed for AuthenticatedCoreTaskWrite {}

impl DbCommand for AuthenticatedCoreTaskWrite {
    type Output = RemoteCoreTaskOutcome;

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        let peer_id = context.authenticated_peer_id()?;
        super::require_sync_policy_identity(
            transaction,
            self.local_device_id.as_deref(),
            self.policy_identity.as_ref(),
        )?;
        super::device_registry_sync::require_registry_peer_admitted(transaction, peer_id)?;
        let inbox_state = crate::db::receive_inbox_payload(
            transaction,
            peer_id,
            &self.inbox_message_id,
            &self.inbox_payload_json,
        )
        .map_err(DbError::from_application_error)?;
        if inbox_state == crate::db::InboxState::Applied {
            return Ok(RemoteCoreTaskOutcome::Duplicate);
        }
        let removed: Vec<String> =
            ark_store::sync_kv::get_sync_kv(transaction, "sync.removed_peers")
                .map_err(DbError::from_application_error)?
                .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
                .transpose()
                .map_err(DbError::from_application_error)?
                .unwrap_or_default();
        if removed.iter().any(|removed_id| removed_id == peer_id) {
            return Err(DbError::Permission("sync_peer_removed".into()));
        }

        let object_session = ark_store::object_write::SqliteObjectWriteSession::new(transaction);
        let task_session = ark_store::task_merge::SqliteTaskMutationSession::new(transaction);
        if object_session
            .load_receipt(&self.id, "full", &self.source_hlc)
            .map_err(DbError::from_application_error)?
            .is_none()
            && object_session
                .current_revision(&self.id)
                .map_err(DbError::from_application_error)?
                .as_deref()
                .is_some_and(|current| {
                    crate::hlc::HLC::compare_str(&self.source_hlc, current)
                        != std::cmp::Ordering::Greater
                })
        {
            // Preserve the existing authenticated-sync stale no-op: it does
            // not decode malformed stale payloads or write a receipt.
            crate::db::set_inbox_state(
                transaction,
                peer_id,
                &self.inbox_message_id,
                crate::db::InboxState::Applied,
            )
            .map_err(DbError::from_application_error)?;
            return Ok(RemoteCoreTaskOutcome::Stale);
        }

        let inbox_message_id = self.inbox_message_id.clone();
        let remote = decode_remote(self)?;
        let result = ark_application::task_object_write::apply_remote_core_task_write(
            &object_session,
            &task_session,
            remote,
            Some(peer_id.to_owned()),
        )
        .map_err(DbError::from_application_error)?;
        crate::db::set_inbox_state(
            transaction,
            peer_id,
            &inbox_message_id,
            crate::db::InboxState::Applied,
        )
        .map_err(DbError::from_application_error)?;
        Ok(if result.duplicate {
            RemoteCoreTaskOutcome::Duplicate
        } else {
            RemoteCoreTaskOutcome::Applied
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteCoreTaskOutcome {
    Applied,
    Duplicate,
    Stale,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct CoreTaskWriteCommit {
    pub(crate) object: ArkObject,
    pub(crate) receipt: StoredObjectReceipt,
}

fn required_receipt(result: ObjectWriteResult) -> Result<StoredObjectReceipt, DbError> {
    result
        .receipt
        .ok_or_else(|| DbError::Storage("object_write_receipt_missing".into()))
}

fn serialized_weight(value: &impl Serialize) -> usize {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
}

fn decode_remote(input: AuthenticatedCoreTaskWrite) -> Result<RemoteObjectWrite, DbError> {
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
        if !input.data.contains_key(key) {
            return Err(DbError::Unsupported(
                "object_write_slice_requires_full_representation".into(),
            ));
        }
    }
    if input.data.get("typeId").and_then(Value::as_str) != Some("core.task")
        || input.data.get("typeVersion").and_then(Value::as_str) != Some("0.1.0")
    {
        return Err(DbError::Unsupported(
            "object_write_slice_unsupported_type".into(),
        ));
    }
    let mut data = input.data;
    data.insert("id".into(), Value::String(input.id));
    let links: Vec<ObjectLink> = data
        .remove("links")
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| DbError::Invalid(error.to_string()))?
        .unwrap_or_default();
    let object = serde_json::from_value(Value::Object(data))
        .map_err(|error| DbError::Invalid(error.to_string()))?;
    Ok(RemoteObjectWrite {
        object,
        links,
        source_hlc: input.source_hlc,
        source_origin_device_id: input.source_origin_device_id,
    })
}
