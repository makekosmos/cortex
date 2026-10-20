use super::super::super::super::commands::CompactUsageSyncLogCommand;
use super::super::super::super::{CommandContext, DbError};
use super::{settle, Publish, WriterTask};
use ark_store::writer::SqliteWriter;
use tokio::sync::oneshot;

pub(in crate::db::command) struct UsageCompactionTask {
    pub context: CommandContext,
    pub command: CompactUsageSyncLogCommand,
    pub reply: oneshot::Sender<Result<usize, DbError>>,
}

impl WriterTask for UsageCompactionTask {
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
