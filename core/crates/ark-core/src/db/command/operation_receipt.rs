use super::{sealed, CommandClass, CommandContext, DbCommand, DbError, DbHandle};
use rusqlite::{OptionalExtension, Transaction};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct OperationResult<T> {
    pub(crate) value: T,
    pub(crate) replayed: bool,
}

#[derive(Serialize)]
pub(super) struct OperationReceiptCommand<C> {
    operation_id: String,
    command_type: &'static str,
    command: C,
}

impl<C> OperationReceiptCommand<C> {
    pub(super) fn new(
        operation_id: String,
        command_type: &'static str,
        command: C,
    ) -> Result<Self, DbError> {
        if operation_id.trim().is_empty() || operation_id.len() > 256 {
            return Err(DbError::Invalid(
                "operationId must contain 1 to 256 bytes".into(),
            ));
        }
        Ok(Self {
            operation_id,
            command_type,
            command,
        })
    }
}

impl<C> sealed::Sealed for OperationReceiptCommand<C> where C: DbCommand + Serialize {}

impl<C> DbCommand for OperationReceiptCommand<C>
where
    C: DbCommand + Serialize,
    C::Output: Serialize + DeserializeOwned,
{
    type Output = OperationResult<C::Output>;

    const CLASS: CommandClass = C::CLASS;

    fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        let payload = serde_json::to_value(("ark-operation-v1", self.command_type, &self.command))
            .map_err(|error| DbError::Storage(error.to_string()))?;
        let canonical_payload =
            serde_json::to_vec(&payload).map_err(|error| DbError::Storage(error.to_string()))?;
        let payload_hash = format!("{:x}", Sha256::digest(canonical_payload));
        let principal = context.operation_principal();

        let stored = transaction
            .query_row(
                "SELECT command_type, payload_sha256, result_json
                 FROM ark_operation_receipts
                 WHERE principal = ?1 AND operation_id = ?2",
                rusqlite::params![principal, self.operation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| DbError::Storage(error.to_string()))?;

        if let Some((stored_type, stored_hash, result_json)) = stored {
            if stored_type != self.command_type || stored_hash != payload_hash {
                return Err(DbError::Conflict(
                    "operationId was already used with a different command payload".into(),
                ));
            }
            let value = serde_json::from_str(&result_json)
                .map_err(|error| DbError::Storage(error.to_string()))?;
            return Ok(OperationResult {
                value,
                replayed: true,
            });
        }

        let value = self.command.execute(context, transaction)?;
        let result_json =
            serde_json::to_string(&value).map_err(|error| DbError::Storage(error.to_string()))?;
        transaction
            .execute(
                "INSERT INTO ark_operation_receipts
                 (principal, operation_id, command_type, payload_sha256, result_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    principal,
                    self.operation_id,
                    self.command_type,
                    payload_hash,
                    result_json,
                ],
            )
            .map_err(|error| DbError::Storage(error.to_string()))?;
        Ok(OperationResult {
            value,
            replayed: false,
        })
    }
}

impl DbHandle {
    pub(crate) async fn submit_with_operation_id<C>(
        &self,
        context: CommandContext,
        operation_id: String,
        command_type: &'static str,
        command: C,
    ) -> Result<OperationResult<C::Output>, DbError>
    where
        C: DbCommand + Serialize,
        C::Output: Serialize + DeserializeOwned,
    {
        let command = OperationReceiptCommand::new(operation_id, command_type, command)?;
        self.submit(context, command).await
    }

    pub(crate) async fn submit_with_optional_operation_id<C>(
        &self,
        context: CommandContext,
        operation_id: Option<String>,
        command_type: &'static str,
        command: C,
    ) -> Result<OperationResult<C::Output>, DbError>
    where
        C: DbCommand + Serialize,
        C::Output: Serialize + DeserializeOwned,
    {
        match operation_id {
            Some(operation_id) => {
                self.submit_with_operation_id(context, operation_id, command_type, command)
                    .await
            }
            None => self
                .submit(context, command)
                .await
                .map(|value| OperationResult {
                    value,
                    replayed: false,
                }),
        }
    }
}
