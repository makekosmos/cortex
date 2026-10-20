use super::super::executor::actor::{LegacyDeleteTask, LegacyGraphWriteTask, WriterTask};
use super::super::{LegacyDeleteCommand, LegacyGraphWriteCommand};
use super::*;
#[cfg(debug_assertions)]
use ark_store::writer::SqliteWriter;

impl DbHandle {
    pub(crate) async fn write_legacy_graph(
        &self,
        context: CommandContext,
        command: LegacyGraphWriteCommand,
    ) -> Result<Vec<crate::types::SyncEntity>, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        let weight = command.weight();
        validate_legacy_write_weight(weight)?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(LegacyGraphWriteTask { command, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                let connection = Arc::clone(connection);
                tokio::task::spawn_blocking(move || {
                    let _admission = admission;
                    let mut connection =
                        connection.lock().unwrap_or_else(|error| error.into_inner());
                    let crate::application::legacy_mutation::LegacyGraphWrite {
                        records,
                        device_id,
                    } = command.write;
                    SqliteWriter::apply_local_legacy_graph_write_on_connection(
                        &mut connection,
                        &records,
                        "rpc",
                        device_id,
                    )
                    .map_err(super::super::executor::actor::map_legacy_transaction_failure)
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }

    pub(crate) async fn delete_legacy_object(
        &self,
        context: CommandContext,
        command: LegacyDeleteCommand,
    ) -> Result<crate::types::SyncEntity, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        let weight = command.weight();
        validate_legacy_write_weight(weight)?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(LegacyDeleteTask { command, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                let connection = Arc::clone(connection);
                tokio::task::spawn_blocking(move || {
                    let _admission = admission;
                    let mut connection =
                        connection.lock().unwrap_or_else(|error| error.into_inner());
                    let crate::application::legacy_mutation::LegacyDelete {
                        id,
                        kind,
                        device_id,
                    } = command.write;
                    SqliteWriter::apply_local_legacy_delete_on_connection(
                        &mut connection,
                        &id,
                        kind.type_id(),
                        device_id,
                    )
                    .map_err(super::super::executor::actor::map_legacy_transaction_failure)
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

fn validate_legacy_write_weight(weight: usize) -> Result<(), DbError> {
    if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
        return Err(DbError::TooLarge);
    }
    Ok(())
}
