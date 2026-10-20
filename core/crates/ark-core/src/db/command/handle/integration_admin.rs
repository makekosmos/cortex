use super::*;
use crate::db::command::{IntegrationAdminCommand, IntegrationAdminOutput};

impl DbHandle {
    pub(crate) async fn submit_integration_admin(
        &self,
        context: CommandContext,
        command: IntegrationAdminCommand,
    ) -> Result<IntegrationAdminOutput, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(executor::actor::IntegrationAdminTask {
                    context: context.clone(),
                    command,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(test)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                tokio::task::spawn_blocking({
                    let connection = Arc::clone(connection);
                    move || {
                        let _admission = admission;
                        let mut connection =
                            connection.lock().unwrap_or_else(|error| error.into_inner());
                        command.execute_on_connection(&context, &mut connection)
                    }
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}
