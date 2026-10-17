use super::super::super::{CommandContext, DbCommand, DbError, DbMaintenanceCommand};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use tokio::sync::oneshot;

pub(in crate::db::command) trait WriterTask: Send {
    fn run(
        self: Box<Self>,
        connection: &mut Connection,
        on_error: &mut dyn FnMut(&DbError),
    ) -> Result<(), DbError>;
    fn cancel_before_start(self: Box<Self>);
    fn receiver_closed(&self) -> bool;
}

pub(in crate::db::command) struct TypedCommand<C: DbCommand> {
    pub context: CommandContext,
    pub command: C,
    pub reply: oneshot::Sender<Result<C::Output, DbError>>,
}

impl<C: DbCommand> WriterTask for TypedCommand<C> {
    fn run(
        self: Box<Self>,
        connection: &mut Connection,
        on_error: &mut dyn FnMut(&DbError),
    ) -> Result<(), DbError> {
        let Self {
            context,
            command,
            reply,
        } = *self;
        let result: Result<C::Output, DbError> = (|| {
            let transaction =
                Transaction::new_unchecked(connection, TransactionBehavior::Immediate)
                    .map_err(DbError::from_sqlite)?;
            let output = command.execute(&context, &transaction)?;
            transaction
                .commit()
                .map_err(|error| DbError::CommitFailed(error.to_string()))?;
            Ok(output)
        })();
        match result {
            Ok(output) => {
                let _ = reply.send(Ok(output));
                Ok(())
            }
            Err(error) => {
                on_error(&error);
                let _ = reply.send(Err(error.clone()));
                Err(error)
            }
        }
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) struct TypedMaintenance<C: DbMaintenanceCommand> {
    pub command: C,
    pub reply: oneshot::Sender<Result<C::Output, DbError>>,
}

impl<C: DbMaintenanceCommand> WriterTask for TypedMaintenance<C> {
    fn run(
        self: Box<Self>,
        connection: &mut Connection,
        on_error: &mut dyn FnMut(&DbError),
    ) -> Result<(), DbError> {
        let Self { command, reply } = *self;
        match command.execute(connection) {
            Ok(output) => {
                let _ = reply.send(Ok(output));
                Ok(())
            }
            Err(error) => {
                on_error(&error);
                let _ = reply.send(Err(error.clone()));
                Err(error)
            }
        }
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}
