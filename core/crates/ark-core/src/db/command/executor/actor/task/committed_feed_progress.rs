use super::{settle, Publish, WriterTask};
use crate::db::command::CommandContext;
use crate::db::{CommittedFeedProgress, CommittedFeedProgressCommand, DbError};
use ark_store::writer::SqliteWriter;
use tokio::sync::oneshot;

pub(in crate::db::command) struct CommittedFeedProgressTask {
    pub context: CommandContext,
    pub command: CommittedFeedProgressCommand,
    pub reply: oneshot::Sender<Result<CommittedFeedProgress, DbError>>,
}

impl WriterTask for CommittedFeedProgressTask {
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
        settle(command.execute(&context, writer), reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}
