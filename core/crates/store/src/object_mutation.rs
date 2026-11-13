use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde::Serialize;

/// Applies one object deletion and its optional idempotency receipt atomically.
/// Task deletion keeps its causal journal and conflict rules in task mutation.
pub fn apply_local_object_delete(
    connection: &mut Connection,
    write: LocalObjectDelete,
    principal: &str,
    operation_id: Option<&str>,
) -> Result<(ObjectDeleteCommit, bool), crate::writer::TransactionFailure<ObjectDeleteError>> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| {
            crate::writer::TransactionFailure::Begin(crate::writer::store_error_from_sqlite(error))
        })?;
    let result = apply_local_object_delete_in(&transaction, write, principal, operation_id)
        .map_err(crate::writer::TransactionFailure::Operation)?;
    transaction.commit().map_err(|error| {
        crate::writer::TransactionFailure::Commit(crate::writer::store_error_from_sqlite(error))
    })?;
    Ok(result)
}

#[derive(Serialize)]
struct DeleteCommand<'a> {
    write: &'a LocalObjectDelete,
}

pub(crate) fn apply_local_object_delete_in(
    transaction: &Transaction<'_>,
    write: LocalObjectDelete,
    principal: &str,
    operation_id: Option<&str>,
) -> Result<(ObjectDeleteCommit, bool), ObjectDeleteError> {
    const COMMAND_TYPE: &str = "object.delete.v1";
    let payload_hash = operation_id
        .map(|_| {
            crate::operation_receipt::payload_hash(COMMAND_TYPE, &DeleteCommand { write: &write })
        })
        .transpose()
        .map_err(ObjectDeleteError::Storage)?;
    if let (Some(operation_id), Some(payload_hash)) = (operation_id, payload_hash.as_deref()) {
        if let Some(receipt) = crate::operation_receipt::load(transaction, principal, operation_id)
            .map_err(|error| ObjectDeleteError::Storage(error.to_string()))?
        {
            if receipt.command_type != COMMAND_TYPE || receipt.payload_sha256 != payload_hash {
                return Err(ObjectDeleteError::OperationIdConflict);
            }
            let commit = serde_json::from_str(&receipt.result_json)
                .map_err(|error| ObjectDeleteError::Storage(error.to_string()))?;
            return Ok((commit, true));
        }
    }

    let identity = crate::objects::object_type_identity(transaction, &write.id)
        .map_err(ObjectDeleteError::Storage)?;
    if identity
        .as_ref()
        .is_some_and(|identity| identity.type_id == "core.task")
    {
        let session = crate::task_merge::SqliteTaskMutationSession::new(transaction);
        let commit = crate::task_mutation::apply_task_delete(
            &session,
            crate::task_mutation::LocalTaskDelete {
                id: write.id,
                expected_snapshot: write.expected_snapshot,
                device_id: write.device_id,
                mode: crate::task_mutation::TaskDeleteMode::Hard,
            },
        )
        .map_err(|error| ObjectDeleteError::TaskMutation(error.to_string()))?;
        let commit = ObjectDeleteCommit {
            entity: commit.entity,
            id: commit.id,
        };
        insert_receipt(
            transaction,
            principal,
            operation_id,
            payload_hash.as_deref(),
            &commit,
        )?;
        return Ok((commit, false));
    }

    let current_snapshot = crate::object_write::read_object_write_snapshot(transaction, &write.id)
        .map_err(ObjectDeleteError::Storage)?;
    let plan = plan_local_object_delete(
        write,
        LocalObjectDeleteFacts {
            current_snapshot,
            type_id: identity.map(|identity| identity.type_id),
        },
    )
    .map_err(ObjectDeleteError::Conflict)?;

    crate::objects::delete_object(transaction, &plan.id).map_err(ObjectDeleteError::Storage)?;
    let hlc = crate::sync_versions::bump_sync_version_vector(
        transaction,
        ObjectRevisionKind::Object.as_str(),
        &plan.id,
        &plan.device_id,
        true,
    )
    .map_err(ObjectDeleteError::Storage)?;
    crate::sync_tombstones::record(
        transaction,
        ObjectRevisionKind::Object.as_str(),
        &plan.id,
        plan.type_id.as_deref(),
        &hlc,
    )
    .map_err(ObjectDeleteError::Storage)?;
    if crate::sync_versions::write_object_sync_version(transaction, &plan.id, &hlc, true)
        .map_err(ObjectDeleteError::Storage)?
        != 1
    {
        return Err(ObjectDeleteError::Conflict(
            ObjectMutationError::StaleRevision,
        ));
    }
    let commit = build_local_object_delete_commit(plan, hlc);
    insert_receipt(
        transaction,
        principal,
        operation_id,
        payload_hash.as_deref(),
        &commit,
    )?;
    Ok((commit, false))
}

fn insert_receipt(
    transaction: &Transaction<'_>,
    principal: &str,
    operation_id: Option<&str>,
    payload_hash: Option<&str>,
    commit: &ObjectDeleteCommit,
) -> Result<(), ObjectDeleteError> {
    let (Some(operation_id), Some(payload_hash)) = (operation_id, payload_hash) else {
        return Ok(());
    };
    let result_json = serde_json::to_string(commit)
        .map_err(|error| ObjectDeleteError::Storage(error.to_string()))?;
    crate::operation_receipt::insert(
        transaction,
        principal,
        operation_id,
        &crate::operation_receipt::OperationReceipt {
            command_type: "object.delete.v1".into(),
            payload_sha256: payload_hash.into(),
            result_json,
        },
    )
    .map_err(|error| ObjectDeleteError::Storage(error.to_string()))
}

use domain::object_write::ObjectWriteSnapshot;
use domain::types::{ArkObject, ArkObjectWrite, SyncEntity};
use serde_json::json;
use std::fmt;
#[derive(Debug, Clone, serde::Serialize)]
pub struct LocalObjectUpsert {
    pub object: ArkObjectWrite,
    pub expected_snapshot: Option<ObjectWriteSnapshot>,
    pub device_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LocalObjectDelete {
    pub id: String,
    pub expected_snapshot: Option<ObjectWriteSnapshot>,
    pub device_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectUpsertCommit {
    pub entity: SyncEntity,
    pub id: String,
    pub type_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectUpsertOutcome {
    pub value: ObjectUpsertCommit,
    pub replayed: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectDeleteCommit {
    pub entity: SyncEntity,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalObjectDeleteFacts {
    pub current_snapshot: ObjectWriteSnapshot,
    pub type_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalObjectDeletePlan {
    pub id: String,
    pub device_id: String,
    pub type_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectMutationError {
    UnsupportedType,
    StaleSnapshot,
    StaleRevision,
}

#[derive(Debug, Clone, Copy)]
pub enum ObjectRevisionKind {
    Object,
    ObjectLink,
}

impl ObjectRevisionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::ObjectLink => "object_link",
        }
    }
}

impl fmt::Display for ObjectMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedType => "object_write_slice_unsupported_type",
            Self::StaleSnapshot => "object_conflict:stale_snapshot",
            Self::StaleRevision => "object_conflict:stale_revision",
        })
    }
}

#[derive(Debug)]
pub enum ObjectDeleteError {
    Storage(String),
    Conflict(ObjectMutationError),
    TaskMutation(String),
    OperationIdConflict,
}

#[derive(Debug)]
pub enum ObjectUpsertError {
    Storage(String),
    Conflict(ObjectMutationError),
    OperationIdConflict,
}

impl fmt::Display for ObjectUpsertError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => formatter.write_str(error),
            Self::Conflict(error) => fmt::Display::fmt(error, formatter),
            Self::OperationIdConflict => {
                formatter.write_str("operationId was already used with a different command payload")
            }
        }
    }
}

impl std::error::Error for ObjectUpsertError {}

impl fmt::Display for ObjectDeleteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) | Self::TaskMutation(error) => formatter.write_str(error),
            Self::Conflict(error) => fmt::Display::fmt(error, formatter),
            Self::OperationIdConflict => {
                formatter.write_str("operationId was already used with a different command payload")
            }
        }
    }
}

impl std::error::Error for ObjectDeleteError {}

impl std::error::Error for ObjectMutationError {}

pub fn build_local_object_upsert_commit(object: ArkObject, hlc: String) -> ObjectUpsertCommit {
    let id = object.id.clone();
    let type_id = object.type_id.clone();
    let mut data = serde_json::to_value(&object)
        .unwrap_or_else(|_| json!({}))
        .as_object()
        .cloned()
        .unwrap_or_default();
    data.remove("id");

    ObjectUpsertCommit {
        entity: SyncEntity {
            entity_type: "object".to_owned(),
            id: id.clone(),
            data,
            hlc,
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        },
        id,
        type_id,
    }
}

pub fn validate_local_object_upsert(
    write: &LocalObjectUpsert,
    current_snapshot: Option<&ObjectWriteSnapshot>,
) -> Result<(), ObjectMutationError> {
    validate_local_object_upsert_type(write)?;
    if write
        .expected_snapshot
        .as_ref()
        .is_some_and(|expected| Some(expected) != current_snapshot)
    {
        return Err(ObjectMutationError::StaleSnapshot);
    }
    Ok(())
}

pub fn validate_local_object_upsert_type(
    write: &LocalObjectUpsert,
) -> Result<(), ObjectMutationError> {
    if write.object.type_id == "core.task" {
        return Err(ObjectMutationError::UnsupportedType);
    }
    Ok(())
}

pub fn plan_local_object_delete(
    write: LocalObjectDelete,
    facts: LocalObjectDeleteFacts,
) -> Result<LocalObjectDeletePlan, ObjectMutationError> {
    if write
        .expected_snapshot
        .as_ref()
        .is_some_and(|expected| expected != &facts.current_snapshot)
    {
        return Err(ObjectMutationError::StaleSnapshot);
    }
    Ok(LocalObjectDeletePlan {
        id: write.id,
        device_id: write.device_id,
        type_id: facts.type_id,
    })
}

pub fn build_local_object_delete_commit(
    plan: LocalObjectDeletePlan,
    hlc: String,
) -> ObjectDeleteCommit {
    let mut data = serde_json::Map::new();
    if let Some(type_id) = plan.type_id {
        data.insert("typeId".into(), type_id.into());
    }
    ObjectDeleteCommit {
        entity: SyncEntity {
            entity_type: "object".into(),
            id: plan.id.clone(),
            data,
            hlc,
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        },
        id: plan.id,
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_local_object_upsert, LocalObjectUpsert, ObjectMutationError};
    use domain::object_write::ObjectWriteSnapshot;
    use domain::types::ArkObjectWrite;
    use serde_json::json;

    fn write(type_id: &str) -> LocalObjectUpsert {
        LocalObjectUpsert {
            object: ArkObjectWrite {
                id: "object-1".into(),
                type_id: type_id.into(),
                type_version: None,
                title: "Object".into(),
                content_json: json!({}),
                props_json: json!({}),
                created_at: "2026-10-19T00:00:00Z".into(),
                updated_at: "2026-10-19T00:00:00Z".into(),
                deleted_at: None,
            },
            expected_snapshot: None,
            device_id: "device-1".into(),
        }
    }

    #[test]
    fn local_upsert_refuses_core_task_before_storage_work() {
        assert_eq!(
            validate_local_object_upsert(&write("core.task"), None),
            Err(ObjectMutationError::UnsupportedType)
        );
    }

    #[test]
    fn local_upsert_refuses_a_stale_expected_snapshot() {
        let mut write = write("note");
        write.expected_snapshot = Some(ObjectWriteSnapshot {
            exists: true,
            type_id: Some("note".into()),
            type_version: Some("1".into()),
            revision: Some("revision-1".into()),
        });
        let current = ObjectWriteSnapshot {
            exists: true,
            type_id: Some("note".into()),
            type_version: Some("1".into()),
            revision: Some("revision-2".into()),
        };

        assert_eq!(
            validate_local_object_upsert(&write, Some(&current)),
            Err(ObjectMutationError::StaleSnapshot)
        );
    }
}

#[cfg(test)]
#[path = "object_delete_tests.rs"]
mod local_object_delete_tests;
