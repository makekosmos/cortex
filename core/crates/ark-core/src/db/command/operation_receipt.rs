use super::{sealed, CommandClass, CommandContext, DbCommand, DbError, DbHandle};
use rusqlite::Transaction;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

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
        let payload_hash =
            ark_store::operation_receipt::payload_hash(self.command_type, &self.command)
                .map_err(DbError::from_application_error)?;
        let principal = context.operation_principal();

        let stored =
            ark_store::operation_receipt::load(transaction, &principal, &self.operation_id)
                .map_err(DbError::from_sqlite)?;

        if let Some(stored) = stored {
            if stored.command_type != self.command_type || stored.payload_sha256 != payload_hash {
                return Err(DbError::Conflict(
                    "operationId was already used with a different command payload".into(),
                ));
            }
            let value = serde_json::from_str(&stored.result_json)
                .map_err(|error| DbError::Storage(error.to_string()))?;
            return Ok(OperationResult {
                value,
                replayed: true,
            });
        }

        let value = self.command.execute(context, transaction)?;
        let result_json =
            serde_json::to_string(&value).map_err(|error| DbError::Storage(error.to_string()))?;
        ark_store::operation_receipt::insert(
            transaction,
            &principal,
            &self.operation_id,
            &ark_store::operation_receipt::OperationReceipt {
                command_type: self.command_type.to_owned(),
                payload_sha256: payload_hash,
                result_json,
            },
        )
        .map_err(DbError::from_sqlite)?;
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
