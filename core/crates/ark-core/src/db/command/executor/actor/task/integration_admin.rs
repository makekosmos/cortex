use super::task::{settle, Publish, WriterTask};
use crate::db::command::{
    commands::IntegrationAdminCommand, commands::IntegrationAdminOutput, CommandContext,
};
use crate::db::DbError;
use ark_store::writer::SqliteWriter;
use tokio::sync::oneshot;

pub(in crate::db::command) struct IntegrationAdminTask {
    pub context: CommandContext,
    pub command: IntegrationAdminCommand,
    pub reply: oneshot::Sender<Result<IntegrationAdminOutput, DbError>>,
}

impl WriterTask for IntegrationAdminTask {
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
