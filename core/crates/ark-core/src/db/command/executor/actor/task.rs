#[cfg(test)]
use super::super::super::DbCommand;
use super::super::super::{
    AuthenticatedCoreTaskWrite, LocalCoreTaskWrite, RemoteCoreTaskOutcome, TaskWriteOperationResult,
};
use super::super::super::{CommandContext, DbError, DbMaintenanceCommand};
use ark_store::writer::{SqliteWriter, TransactionFailure};
use tokio::sync::oneshot;
mod committed_feed_progress;
mod core_task_write;
mod inbound_entity;
mod inbox_receipt;
pub(in crate::db::command) use committed_feed_progress::CommittedFeedProgressTask;
pub(in crate::db::command) use core_task_write::LocalCoreTaskWriteTask;
pub(in crate::db::command) use inbound_entity::InboundEntityTask;
pub(in crate::db::command) use inbox_receipt::InboxReceiptTask;
mod authenticated_core_task_write;
pub(in crate::db::command) use authenticated_core_task_write::AuthenticatedCoreTaskWriteTask;
mod authenticated_inbound_entity;
pub(in crate::db::command) use authenticated_inbound_entity::AuthenticatedInboundEntityTask;
mod external_ref;
pub(in crate::db::command) use external_ref::ExternalRefUpsertTask;
mod iroh_identity;
pub(in crate::db::command) use iroh_identity::IrohIdentityKeyTask;
mod local_sync_key_value_write;
pub(in crate::db::command) use local_sync_key_value_write::LocalSyncKeyValueWriteTask;
mod sync_progress_write;
pub(in crate::db::command) use sync_progress_write::SyncProgressWriteTask;
mod sync_policy_write;
pub(in crate::db::command) use sync_policy_write::SyncPolicyWriteTask;
mod usage_compaction;
pub(in crate::db::command) use usage_compaction::UsageCompactionTask;
mod usage_sequence_migration;
pub(in crate::db::command) use usage_sequence_migration::UsageSequenceMigrationTask;
mod type_mutation;
pub(in crate::db::command) use type_mutation::{
    DeleteObjectTypeTask, RegisterPackageTypesTask, UpsertObjectTypeTask,
};
pub(in crate::db::command) mod task_merge_append;
mod tracked_app_mutation;
pub(in crate::db::command) use tracked_app_mutation::TrackedAppMutationTask;
/// Delivers replies only after the writer releases its slot.
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

pub(in crate::db::command) fn settle<T: 'static>(
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

#[cfg(test)]
pub(in crate::db::command) struct TypedCommand<C: DbCommand> {
    pub context: CommandContext,
    pub command: C,
    pub reply: oneshot::Sender<Result<C::Output, DbError>>,
}

#[cfg(test)]
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
            .write_with_capabilities(|capabilities| command.execute(&context, capabilities))
            .map_err(|failure| match failure {
                TransactionFailure::Begin(error) => DbError::from_store_error(error),
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

pub(in crate::db::command) struct CanonicalGameUpsertTask {
    pub command: ark_application::canonical_game::GameUpsertCommand,
    pub fallback_device_id: String,
    pub reply:
        oneshot::Sender<Result<ark_application::canonical_game::GameMutationResult, DbError>>,
}

pub(in crate::db::command) struct LocalUsageWriteTask {
    pub device_id: String,
    pub write: ark_application::usage::LocalUsageWrite,
    pub reply: oneshot::Sender<Result<Vec<ark_application::usage::UsageMutation>, DbError>>,
}

pub(in crate::db::command) struct ClearAllTask {
    pub reply: oneshot::Sender<Result<(), DbError>>,
}


pub(in crate::db::command) struct UpsertObjectLinkTask {
    pub object_link: ark_domain::types::ObjectLink,
    pub device_id: Option<String>,
    pub reply: oneshot::Sender<Result<ark_application::type_mutation::TypeMutationCommit, DbError>>,
}

pub(in crate::db::command) struct DeleteObjectLinkTask {
    pub id: String,
    pub device_id: Option<String>,
    pub reply: oneshot::Sender<Result<ark_application::type_mutation::TypeMutationCommit, DbError>>,
}

pub(in crate::db::command) struct ReplaceObjectLinkTask {
    pub object_link: ark_domain::types::ObjectLink,
    pub device_id: Option<String>,
    pub reply:
        oneshot::Sender<Result<ark_application::type_mutation::ReplaceObjectLinkCommit, DbError>>,
}

macro_rules! impl_object_link_task {
    ($task:ty, $operation:ident) => {
        impl WriterTask for $task {
            fn run(
                self: Box<Self>,
                writer: &mut SqliteWriter,
                on_error: &mut dyn FnMut(&DbError),
            ) -> (Result<(), DbError>, Publish) {
                let Self {
                    object_link,
                    device_id,
                    reply,
                } = *self;
                let result = writer
                    .$operation(object_link, device_id)
                    .map_err(map_application_transaction_failure);
                settle(result, reply, on_error)
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

impl_object_link_task!(UpsertObjectLinkTask, upsert_object_link);
impl_object_link_task!(ReplaceObjectLinkTask, replace_object_link);

impl WriterTask for DeleteObjectLinkTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            id,
            device_id,
            reply,
        } = *self;
        let result = writer
            .delete_object_link(id, device_id)
            .map_err(map_application_transaction_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

impl WriterTask for ClearAllTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { reply } = *self;
        let result = writer.clear_all().map_err(map_storage_transaction_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

fn map_storage_transaction_failure<E: ToString>(failure: TransactionFailure<E>) -> DbError {
    match failure {
        TransactionFailure::Begin(error) => DbError::from_store_error(error),
        TransactionFailure::Operation(error) => DbError::Storage(error.to_string()),
        TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
    }
}

fn map_application_transaction_failure<E: ToString>(failure: TransactionFailure<E>) -> DbError {
    match failure {
        TransactionFailure::Begin(error) => DbError::from_store_error(error),
        TransactionFailure::Operation(error) => DbError::from_application_error(error.to_string()),
        TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
    }
}

fn map_store_application_transaction_failure<E: ToString>(
    failure: ark_store::writer::StoreTransactionFailure<E>,
) -> DbError {
    use ark_store::writer::{StoreErrorKind, StoreTransactionFailure};
    match failure {
        StoreTransactionFailure::Begin(error) => match error.kind {
            StoreErrorKind::DiskFull | StoreErrorKind::Io | StoreErrorKind::Corrupt => {
                DbError::StorageUnhealthy(error.to_string())
            }
            StoreErrorKind::Other => DbError::Storage(error.to_string()),
        },
        StoreTransactionFailure::Operation(error) => {
            DbError::from_application_error(error.to_string())
        }
        StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
    }
}

impl WriterTask for LocalUsageWriteTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            device_id,
            write,
            reply,
        } = *self;
        let result = writer
            .write_with_capabilities(|capabilities| {
                capabilities
                    .usage()
                    .apply_local_usage_write(&device_id, write)
            })
            .map_err(map_store_application_transaction_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

impl WriterTask for CanonicalGameUpsertTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            command,
            fallback_device_id,
            reply,
        } = *self;
        let result = writer.write_with_capabilities(|capabilities| {
            capabilities
                .canonical_game()
                .apply_upsert(command, &fallback_device_id)
        });
        let result = result.map_err(map_store_application_transaction_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

impl WriterTask for LocalBookCoverTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { write, reply } = *self;
        let result = writer
            .write_with_capabilities(|capabilities| {
                capabilities.book_cover().apply_local_book_cover(write)
            })
            .map_err(map_store_application_transaction_failure);
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
            .write_with_capabilities(|capabilities| {
                capabilities.objects().apply_local_object_delete(
                    write,
                    &principal,
                    operation_id.as_deref(),
                )
            })
            .map_err(map_store_application_transaction_failure);
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
            .write_with_capabilities(|capabilities| {
                capabilities.objects().apply_local_object_upsert(
                    write,
                    &principal,
                    operation_id.as_deref(),
                )
            })
            .map_err(map_store_application_transaction_failure);
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
        let result = command.execute(writer);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}
