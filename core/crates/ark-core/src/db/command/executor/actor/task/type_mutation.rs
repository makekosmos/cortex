use super::*;

pub(in crate::db::command) struct RegisterPackageTypesTask {
    pub registrations: Vec<ark_domain::canonical_type::TypeRegistration>,
    pub reply: oneshot::Sender<Result<Vec<ark_application::sync_storage::ReplayNotice>, DbError>>,
}

pub(in crate::db::command) struct UpsertObjectTypeTask {
    pub object_type: ark_domain::types::ObjectType,
    pub device_id: Option<String>,
    pub reply: oneshot::Sender<Result<ark_application::type_mutation::TypeMutationCommit, DbError>>,
}

pub(in crate::db::command) struct DeleteObjectTypeTask {
    pub id: String,
    pub device_id: Option<String>,
    pub reply: oneshot::Sender<Result<ark_application::type_mutation::TypeMutationCommit, DbError>>,
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
    RegisterPackageTypesTask,
    Vec<ark_application::sync_storage::ReplayNotice>,
    register_package_types,
    { registrations }
);
impl_type_mutation_task!(
    UpsertObjectTypeTask,
    ark_application::type_mutation::TypeMutationCommit,
    upsert_object_type,
    { object_type, device_id }
);
impl_type_mutation_task!(
    DeleteObjectTypeTask,
    ark_application::type_mutation::TypeMutationCommit,
    delete_object_type,
    { id, device_id }
);
