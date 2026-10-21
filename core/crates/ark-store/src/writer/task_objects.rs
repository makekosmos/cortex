use super::*;

pub struct TaskObjectWrites<'scope, 'connection> {
    pub(super) transaction: &'scope Transaction<'connection>,
}

#[derive(Debug)]
pub enum TaskObjectWriteError {
    Storage(String),
    OperationIdConflict,
}

impl std::fmt::Display for TaskObjectWriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(error) => formatter.write_str(error),
            Self::OperationIdConflict => {
                formatter.write_str("operationId was already used with a different command payload")
            }
        }
    }
}

impl TaskObjectWrites<'_, '_> {
    pub fn apply_local_core_task_write(
        &mut self,
        write: ark_application::task_object_write::LocalCoreTaskWrite,
        principal: &str,
        operation_id: Option<&str>,
    ) -> Result<ark_application::task_object_write::TaskWriteOperationResult, TaskObjectWriteError>
    {
        apply_local_core_task_write_in(self.transaction, write, principal, operation_id)
    }
}

pub(super) fn apply_local_core_task_write_in(
    transaction: &Transaction<'_>,
    write: ark_application::task_object_write::LocalCoreTaskWrite,
    principal: &str,
    operation_id: Option<&str>,
) -> Result<ark_application::task_object_write::TaskWriteOperationResult, TaskObjectWriteError> {
    const COMMAND_TYPE: &str = "object.upsert.v1";
    let payload_sha256 = operation_id
        .map(|_| crate::operation_receipt::payload_hash(COMMAND_TYPE, &write))
        .transpose()
        .map_err(TaskObjectWriteError::Storage)?;
    if let (Some(operation_id), Some(payload_sha256)) = (operation_id, payload_sha256.as_deref()) {
        let stored = crate::operation_receipt::load(transaction, principal, operation_id)
            .map_err(|error| TaskObjectWriteError::Storage(error.to_string()))?;
        if let Some(stored) = stored {
            if stored.command_type != COMMAND_TYPE || stored.payload_sha256 != payload_sha256 {
                return Err(TaskObjectWriteError::OperationIdConflict);
            }
            let value = serde_json::from_str(&stored.result_json)
                .map_err(|error| TaskObjectWriteError::Storage(error.to_string()))?;
            return Ok(
                ark_application::task_object_write::TaskWriteOperationResult {
                    value,
                    replayed: true,
                },
            );
        }
    }

    let (object_write, expected_snapshot, device_id) = write.into_parts();
    let (object, result) =
        apply_local_core_task_write_body(transaction, object_write, expected_snapshot, device_id)?;
    let receipt = result
        .receipt
        .ok_or_else(|| TaskObjectWriteError::Storage("object_write_receipt_missing".into()))?;
    let value = ark_application::task_object_write::LocalCoreTaskWriteCommit { object, receipt };
    if let (Some(operation_id), Some(payload_sha256)) = (operation_id, payload_sha256) {
        let result_json = serde_json::to_string(&value)
            .map_err(|error| TaskObjectWriteError::Storage(error.to_string()))?;
        crate::operation_receipt::insert(
            transaction,
            principal,
            operation_id,
            &crate::operation_receipt::OperationReceipt {
                command_type: COMMAND_TYPE.to_owned(),
                payload_sha256,
                result_json,
            },
        )
        .map_err(|error| TaskObjectWriteError::Storage(error.to_string()))?;
    }
    Ok(
        ark_application::task_object_write::TaskWriteOperationResult {
            value,
            replayed: false,
        },
    )
}

fn apply_local_core_task_write_body(
    transaction: &Transaction<'_>,
    object: ark_domain::types::ArkObjectWrite,
    expected_snapshot: Option<ark_domain::object_write::ObjectWriteSnapshot>,
    device_id: Option<String>,
) -> Result<
    (
        ark_domain::types::ArkObject,
        ark_application::object_write::ObjectWriteResult,
    ),
    TaskObjectWriteError,
> {
    let object_id = object.id.clone();
    let object_session = crate::object_write::SqliteObjectWriteSession::new(transaction);
    let task_session = crate::task_mutation::SqliteTaskMutationSession::new(transaction);
    let result = ark_application::task_object_write::apply_local_core_task_write(
        &object_session,
        &task_session,
        ark_application::task_object_write::LocalCoreTaskWrite::new(
            object,
            expected_snapshot,
            device_id,
        ),
    )
    .map_err(TaskObjectWriteError::Storage)?;
    let persisted = crate::objects::get_object(transaction, &object_id)
        .map_err(TaskObjectWriteError::Storage)?
        .ok_or_else(|| TaskObjectWriteError::Storage("object_write_missing_after_commit".into()))?;
    Ok((persisted, result))
}
