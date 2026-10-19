use super::super::super::{CommandContext, DbCommand, DbError, DbMaintenanceCommand};
use ark_store::writer::{SqliteWriter, TransactionFailure};
use tokio::sync::oneshot;

/// Delivers a reply. The writer calls it only after the slot is released, so a
/// caller that receives the reply can submit again without hitting the barrier.
pub(in crate::db::command) type Publish = Box<dyn FnOnce()>;

pub(in crate::db::command) trait WriterTask: Send {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
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
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            context,
            command,
            reply,
        } = *self;
        let result = writer
            .immediate(|transaction| command.execute(&context, transaction))
            .map_err(|failure| match failure {
                TransactionFailure::Begin(error) => DbError::from_sqlite(error),
                TransactionFailure::Operation(error) => error,
                TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
            });
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) struct LocalObjectUpsertTask {
    pub write: ark_application::object_mutation::LocalObjectUpsert,
    pub principal: String,
    pub operation_id: Option<String>,
    pub reply:
        oneshot::Sender<Result<ark_application::object_mutation::ObjectUpsertOutcome, DbError>>,
}

pub(in crate::db::command) struct LocalObjectDeleteTask {
    pub write: ark_application::object_mutation::LocalObjectDelete,
    pub principal: String,
    pub operation_id: Option<String>,
    pub reply: oneshot::Sender<
        Result<(ark_application::object_mutation::ObjectDeleteCommit, bool), DbError>,
    >,
}

pub(in crate::db::command) struct LocalBookCoverTask {
    pub write: ark_application::book_cover::LocalBookCoverWrite,
    pub reply: oneshot::Sender<Result<bool, DbError>>,
}

impl WriterTask for LocalBookCoverTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { write, reply } = *self;
        let result = writer
            .apply_local_book_cover(write)
            .map_err(|failure| match failure {
                TransactionFailure::Begin(error) => DbError::from_sqlite(error),
                TransactionFailure::Operation(error) => {
                    DbError::from_application_error(error.to_string())
                }
                TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
            });
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

impl WriterTask for LocalObjectDeleteTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            write,
            principal,
            operation_id,
            reply,
        } = *self;
        let result = writer
            .apply_local_object_delete(write, &principal, operation_id.as_deref())
            .map_err(|failure| match failure {
                TransactionFailure::Begin(error) => DbError::from_sqlite(error),
                TransactionFailure::Operation(error) => {
                    DbError::from_application_error(error.to_string())
                }
                TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
            });
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

impl WriterTask for LocalObjectUpsertTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            write,
            principal,
            operation_id,
            reply,
        } = *self;
        let result = writer
            .apply_local_object_upsert(write, &principal, operation_id.as_deref())
            .map_err(|failure| match failure {
                TransactionFailure::Begin(error) => DbError::from_sqlite(error),
                TransactionFailure::Operation(error) => {
                    DbError::from_application_error(error.to_string())
                }
                TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
            });
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
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { command, reply } = *self;
        let result = writer.with_connection_mut(|connection| command.execute(connection));
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}
