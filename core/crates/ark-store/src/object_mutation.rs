use ark_application::object_mutation::{
    build_local_object_delete_commit, plan_local_object_delete, LocalObjectDelete,
    LocalObjectDeleteFacts, ObjectDeleteCommit, ObjectDeleteError, ObjectMutationError,
    ObjectRevisionKind,
};
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
        .map_err(crate::writer::TransactionFailure::Begin)?;
    let result = apply_local_object_delete_in(&transaction, write, principal, operation_id)
        .map_err(crate::writer::TransactionFailure::Operation)?;
    transaction
        .commit()
        .map_err(crate::writer::TransactionFailure::Commit)?;
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
        let commit = ark_application::task_mutation::apply_task_delete(
            &session,
            ark_application::task_mutation::LocalTaskDelete {
                id: write.id,
                expected_snapshot: write.expected_snapshot,
                device_id: write.device_id,
                mode: ark_application::task_mutation::TaskDeleteMode::Hard,
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
