use super::*;

pub(in crate::db::command) struct RegisterApplicationTypeTask {
    pub registration: ::domain::r#type::TypeRegistration,
    pub reply: oneshot::Sender<Result<Vec<::domain::sync_storage::ReplayNotice>, DbError>>,
}

macro_rules! impl_type_mutation_task {
    ($task:ty, $output:ty, $method:ident, { $($field:ident),+ $(,)? }) => {
        impl WriterTask for $task {
            fn run(
                self: Box<Self>,
                writer: &mut SqliteWriter,
                on_error: &mut dyn FnMut(&DbError),
            ) -> (Result<(), DbError>, Publish) {
                let Self { $($field,)+ reply } = *self;
                let result = writer.$method($($field),+)
                    .map_err(map_store_application_transaction_failure);
                settle::<$output>(result, reply, on_error)
            }

            fn cancel_before_start(self: Box<Self>) {
                let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
            }

            fn receiver_closed(&self) -> bool {
                self.reply.is_closed()
            }
        }
    };
}

impl_type_mutation_task!(
    RegisterApplicationTypeTask,
    Vec<::domain::sync_storage::ReplayNotice>,
    register_application_type,
    { registration }
);
