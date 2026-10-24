use super::*;
#[cfg(test)]
use executor::actor::TypedCommand;
use executor::actor::{ActorOwner, ActorShared, TypedMaintenance, WriterTask};
use std::time::Instant;

pub(in crate::db::command) mod authenticated_core_task_write;
mod authenticated_inbound_entity;
mod committed_feed_progress;
mod core_task_write;
mod external_ref;
mod inbound_entity;
mod inbox_receipt;
mod integration_admin;
mod iroh_identity;
mod local_sync_key_value_write;
mod object_link_mutation;
mod object_mutation;
mod registry_events;
mod registry_local_mutation;
mod signed_integration_frame;
mod sync_policy_write;
mod sync_progress_write;
mod system_mutation;
mod task_merge_append;
mod task_merge_batch;
mod task_merge_progress;
mod tracked_app_mutation;
mod type_mutation;
mod usage;
mod usage_compaction;
mod usage_sequence_migration;

#[derive(Clone)]
pub(crate) struct DbHandle {
    backend: Arc<ActorShared>,
}

pub(crate) struct DbOwner {
    actor: ActorOwner,
}

impl DbHandle {
    pub(crate) async fn open(
        db_path: String,
        legacy_task_device_id: String,
    ) -> Result<(Self, DbOwner), DbError> {
        Self::open_with_limits_inner(db_path, legacy_task_device_id, super::DB_ADMISSION_LIMITS)
            .await
    }

    #[cfg(test)]
    pub(crate) async fn open_with_limits(
        db_path: String,
        legacy_task_device_id: String,
        limits: executor::AdmissionLimits,
    ) -> Result<(Self, DbOwner), DbError> {
        Self::open_with_limits_inner(db_path, legacy_task_device_id, limits).await
    }

    async fn open_with_limits_inner(
        db_path: String,
        legacy_task_device_id: String,
        limits: executor::AdmissionLimits,
    ) -> Result<(Self, DbOwner), DbError> {
        let (shared, owner, ready) = ActorShared::spawn(db_path, legacy_task_device_id, limits)
            .map_err(|error| DbError::Storage(error.to_string()))?;
        match tokio::task::spawn_blocking(move || ready.recv()).await {
            Ok(Ok(Ok(()))) => Ok((Self { backend: shared }, DbOwner { actor: owner })),
            Ok(Ok(Err(error))) => Err(error),
            Ok(Err(_)) => Err(DbError::Unavailable),
            Err(error) => Err(DbError::Storage(error.to_string())),
        }
    }

    #[cfg(test)]
    pub(crate) async fn submit<C: DbCommand>(
        &self,
        context: CommandContext,
        command: C,
    ) -> Result<C::Output, DbError> {
        if context.class != C::CLASS {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(TypedCommand {
                context: context.clone(),
                command,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn submit_maintenance<C: DbMaintenanceCommand>(
        &self,
        context: CommandContext,
        command: C,
    ) -> Result<C::Output, DbError> {
        if context.class != CommandClass::Maintenance {
            return Err(DbError::Invalid(
                "maintenance command class required".into(),
            ));
        }
        context.require_engine()?;
        let weight = command.weight();
        if weight == 0 || weight > super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(TypedMaintenance { command, reply });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn query_with_store<Q: StoreReadQuery>(
        &self,
        context: CommandContext,
        query: Q,
    ) -> Result<Q::Output, DbError> {
        if context.class != query.class() {
            return Err(DbError::Invalid("database query class mismatch".into()));
        }
        let weight = query.weight();
        if weight == 0 || weight > super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let shared = Arc::clone(actor);
            let _admission = actor.admit_read(query.class(), &context.source, weight)?;
            tokio::task::spawn_blocking(move || {
                let _admission = _admission;
                let _snapshot = shared
                    .readers
                    .read()
                    .unwrap_or_else(|error| error.into_inner());
                let mut reader = shared
                    .database
                    .open_reader()
                    .map_err(|error| DbError::Storage(error.to_string()))?;
                reader
                    .with_read_snapshot(|capabilities| query.read(&context, &capabilities))
                    .map_err(map_store_read_failure)
            })
            .await
            .map_err(|error| DbError::Storage(error.to_string()))?
        }
    }

    #[cfg(test)]
    pub(crate) fn admitted_commands_for_test(&self) -> usize {
        self.backend.admitted_commands()
    }

    #[cfg(test)]
    pub(crate) fn actor_ready_for_test(&self) -> bool {
        self.backend.is_ready()
    }

    #[cfg(test)]
    pub(crate) fn generation_for_test(&self) -> u64 {
        self.backend.generation()
    }

    #[cfg(test)]
    pub(crate) fn maintenance_state_for_test(&self) -> (bool, bool) {
        self.backend.maintenance_state()
    }

    #[cfg(test)]
    pub(crate) fn hold_reader_barrier_for_test(&self) -> impl Drop + '_ {
        self.backend
            .readers
            .write()
            .unwrap_or_else(|error| error.into_inner())
    }

    #[cfg(test)]
    pub(super) fn hold_read_admission_for_test(
        &self,
        class: CommandClass,
        source: &CommandSource,
    ) -> impl Drop {
        self.backend
            .admit_read(class, source, 1)
            .expect("test admission slot")
    }
}

impl DbOwner {
    pub(crate) async fn shutdown(&mut self, deadline: Instant) -> Result<(), DbError> {
        self.actor.shutdown(deadline).await
    }

    #[cfg(test)]
    pub(crate) fn shutdown_blocking(self, deadline: Instant) -> Result<(), DbError> {
        std::thread::Builder::new()
            .name("ark-test-db-shutdown".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| DbError::Storage(error.to_string()))?;
                let mut owner = self;
                runtime.block_on(owner.shutdown(deadline))
            })
            .map_err(|error| DbError::Storage(error.to_string()))?
            .join()
            .map_err(|_| DbError::Storage("database shutdown thread panicked".into()))?
    }
}

fn map_store_read_failure(
    error: ark_store::read_snapshot::ReadSnapshotFailure<DbError>,
) -> DbError {
    match error {
        ark_store::read_snapshot::ReadSnapshotFailure::Begin(error)
        | ark_store::read_snapshot::ReadSnapshotFailure::Commit(error) => {
            DbError::Storage(error.to_string())
        }
        ark_store::read_snapshot::ReadSnapshotFailure::Operation(error) => error,
    }
}
