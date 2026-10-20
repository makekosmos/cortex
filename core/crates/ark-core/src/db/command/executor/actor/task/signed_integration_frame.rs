use super::task::{settle, Publish, WriterTask};
use crate::db::command::{ApplySignedIntegrationFrameCommand, CommandContext};
use crate::db::DbError;
use ark_store::writer::SqliteWriter;
use tokio::sync::oneshot;

pub(in crate::db::command) struct SignedIntegrationFrameTask {
    pub context: CommandContext,
    pub command: ApplySignedIntegrationFrameCommand,
    pub reply: oneshot::Sender<Result<(), DbError>>,
}

impl WriterTask for SignedIntegrationFrameTask {
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
        settle(command.apply_in_store(&context, writer), reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}
