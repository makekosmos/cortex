use super::super::super::super::commands::{SyncPolicyWrite, SyncPolicyWriteOutput};
use super::super::super::super::{CommandContext, DbError};
use super::{settle, Publish, WriterTask};
use ark_store::writer::SqliteWriter;
use tokio::sync::oneshot;

pub(in crate::db::command) struct SyncPolicyWriteTask {
    pub context: CommandContext,
    pub write: SyncPolicyWrite,
    pub reply: oneshot::Sender<Result<SyncPolicyWriteOutput, DbError>>,
}

impl WriterTask for SyncPolicyWriteTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            context,
            write,
            reply,
        } = *self;
        settle(write.execute(&context, writer), reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}
