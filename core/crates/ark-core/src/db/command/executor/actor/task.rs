use super::super::super::{CommandContext, DbCommand, DbError, DbMaintenanceCommand};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use tokio::sync::oneshot;

/// Delivers a reply. The writer calls it only after the slot is released, so a
/// caller that receives the reply can submit again without hitting the barrier.
pub(in crate::db::command) type Publish = Box<dyn FnOnce()>;

pub(in crate::db::command) trait WriterTask: Send {
    fn run(
        self: Box<Self>,
        connection: &mut Connection,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish);
    fn cancel_before_start(self: Box<Self>);
    fn receiver_closed(&self) -> bool;
}

fn settle<T: 'static>(
    result: Result<T, DbError>,
    reply: oneshot::Sender<Result<T, DbError>>,
    on_error: &mut dyn FnMut(&DbError),
) -> (Result<(), DbError>, Publish) {
    let status = match &result {
        Ok(_) => Ok(()),
        Err(error) => {
            on_error(error);
            Err(error.clone())
        }
    };
    let publish: Publish = Box::new(move || {
        let _ = reply.send(result);
    });
    (status, publish)
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
    ) -> (Result<(), DbError>, Publish) {
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
        settle(result, reply, on_error)
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
    ) -> (Result<(), DbError>, Publish) {
        let Self { command, reply } = *self;
        settle(command.execute(connection), reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}
