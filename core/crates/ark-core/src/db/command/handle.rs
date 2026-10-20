use super::*;
use executor::actor::{ActorOwner, ActorShared, TypedCommand, TypedMaintenance, WriterTask};
use rusqlite::{OpenFlags, TransactionBehavior};
use std::time::Instant;

pub(in crate::db::command) mod authenticated_core_task_write;
mod committed_feed_progress;
mod core_task_write;
mod external_ref;
mod inbox_receipt;
mod iroh_identity;
mod local_sync_key_value_write;
mod object_link_mutation;
mod object_mutation;
mod signed_integration_frame;
mod sync_policy_write;
mod sync_progress_write;
mod system_mutation;
mod task_merge_append;
#[cfg(debug_assertions)]
mod test_writer;
mod tracked_app_mutation;
mod type_mutation;
mod usage;
mod usage_compaction;
mod usage_sequence_migration;

#[derive(Clone)]
pub(crate) struct DbHandle {
    backend: HandleBackend,
    #[cfg(debug_assertions)]
    admission: executor::AdmissionBudget,
}

#[derive(Clone)]
enum HandleBackend {
    Actor(Arc<ActorShared>),
    #[cfg(debug_assertions)]
    Test(Arc<Mutex<Connection>>),
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
            Ok(Ok(Ok(()))) => Ok((
                Self {
                    backend: HandleBackend::Actor(shared),
                    #[cfg(debug_assertions)]
                    admission: executor::AdmissionBudget::new(limits),
                },
                DbOwner { actor: owner },
            )),
            Ok(Ok(Err(error))) => Err(error),
            Ok(Err(_)) => Err(DbError::Unavailable),
            Err(error) => Err(DbError::Storage(error.to_string())),
        }
    }

    #[cfg(debug_assertions)]
    pub(crate) fn for_test(connection: Arc<Mutex<Connection>>) -> Self {
        Self {
            backend: HandleBackend::Test(connection),
            admission: executor::AdmissionBudget::new(super::DB_ADMISSION_LIMITS),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_connection<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        match &self.backend {
            HandleBackend::Test(connection) => {
                let guard = connection.lock().unwrap_or_else(|error| error.into_inner());
                f(&guard)
            }
            HandleBackend::Actor(actor) => {
                let connection = ark_store::open::open_db(&actor.path).expect("test DB connection");
                f(&connection)
            }
        }
    }

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
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(TypedCommand {
                    context: context.clone(),
                    command,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let _admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_submit(Arc::clone(connection), context, command, _admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
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
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(TypedMaintenance { command, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let _admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_maintenance(Arc::clone(connection), command, _admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }

    pub(crate) async fn query<Q: DbQuery>(
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
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let shared = Arc::clone(actor);
                let _admission = actor.admit_read(query.class(), &context.source, weight)?;
                tokio::task::spawn_blocking(move || {
                    let _admission = _admission;
                    let _snapshot = shared
                        .readers
                        .read()
                        .unwrap_or_else(|error| error.into_inner());
                    let mut connection = open_reader(&shared.path)?;
                    let transaction = connection
                        .transaction_with_behavior(TransactionBehavior::Deferred)
                        .map_err(|error| DbError::Storage(error.to_string()))?;
                    let output = query.read(&context, &transaction)?;
                    transaction
                        .commit()
                        .map_err(|error| DbError::Storage(error.to_string()))?;
                    Ok(output)
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let _admission = self
                    .admission
                    .try_acquire(query.class(), &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_query(Arc::clone(connection), context, query, _admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
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
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let shared = Arc::clone(actor);
                let _admission = actor.admit_read(query.class(), &context.source, weight)?;
                tokio::task::spawn_blocking(move || {
                    let _admission = _admission;
                    let _snapshot = shared
                        .readers
                        .read()
                        .unwrap_or_else(|error| error.into_inner());
                    let mut reader =
                        ark_store::read_snapshot::SqliteReader::open_read_only(&shared.path)
                            .map_err(|error| DbError::Storage(error.to_string()))?;
                    reader
                        .with_read_snapshot(|capabilities| query.read(&context, &capabilities))
                        .map_err(map_store_read_failure)
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let _admission = self
                    .admission
                    .try_acquire(query.class(), &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_store_query(Arc::clone(connection), context, query, _admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn shared_connection_for_test(&self) -> Arc<Mutex<Connection>> {
        match &self.backend {
            HandleBackend::Test(connection) => Arc::clone(connection),
            HandleBackend::Actor(actor) => Arc::new(Mutex::new(
                ark_store::open::open_db(&actor.path).expect("test DB connection"),
            )),
        }
    }

    #[cfg(test)]
    pub(crate) fn admitted_commands_for_test(&self) -> usize {
        match &self.backend {
            HandleBackend::Actor(actor) => actor.admitted_commands(),
            #[cfg(debug_assertions)]
            HandleBackend::Test(_) => 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn actor_ready_for_test(&self) -> bool {
        match &self.backend {
            HandleBackend::Actor(actor) => actor.is_ready(),
            #[cfg(debug_assertions)]
            HandleBackend::Test(_) => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn generation_for_test(&self) -> u64 {
        match &self.backend {
            HandleBackend::Actor(actor) => actor.generation(),
            #[cfg(debug_assertions)]
            HandleBackend::Test(_) => 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn maintenance_state_for_test(&self) -> (bool, bool) {
        match &self.backend {
            HandleBackend::Actor(actor) => actor.maintenance_state(),
            #[cfg(debug_assertions)]
            HandleBackend::Test(_) => (false, false),
        }
    }

    #[cfg(test)]
    pub(crate) fn hold_reader_barrier_for_test(&self) -> impl Drop + '_ {
        match &self.backend {
            HandleBackend::Actor(actor) => actor
                .readers
                .write()
                .unwrap_or_else(|error| error.into_inner()),
            #[cfg(debug_assertions)]
            HandleBackend::Test(_) => panic!("reader barrier requires a production actor"),
        }
    }

    #[cfg(test)]
    pub(super) fn hold_admission_for_test(
        &self,
        class: CommandClass,
        source: &CommandSource,
    ) -> impl Drop {
        self.admission
            .try_acquire(class, source, 1)
            .expect("test admission slot")
    }
}

#[cfg(debug_assertions)]
fn map_admission_error(error: executor::AdmissionError) -> DbError {
    match error {
        executor::AdmissionError::TooLarge => DbError::TooLarge,
        executor::AdmissionError::Saturated => DbError::Busy,
    }
}

#[cfg(debug_assertions)]
fn spawn_test_submit<C: DbCommand>(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    command: C,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<C::Output, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)
            .map_err(|error| DbError::Storage(error.to_string()))?;
        let output = command.execute(&context, &transaction)?;
        transaction
            .commit()
            .map_err(|error| DbError::Storage(error.to_string()))?;
        Ok(output)
    })
}

#[cfg(debug_assertions)]
fn spawn_test_maintenance<C: DbMaintenanceCommand>(
    connection: Arc<Mutex<Connection>>,
    command: C,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<C::Output, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        command.execute(&mut connection)
    })
}

#[cfg(debug_assertions)]
fn spawn_test_query<Q: DbQuery>(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    query: Q,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<Q::Output, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        query.read(&context, &connection)
    })
}

#[cfg(debug_assertions)]
fn spawn_test_store_query<Q: StoreReadQuery>(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    query: Q,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<Q::Output, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::read_snapshot::test_support::with_read_snapshot(
            &mut connection,
            |capabilities| query.read(&context, &capabilities),
        )
        .map_err(map_store_read_failure)
    })
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

fn open_reader(path: &str) -> Result<Connection, DbError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| DbError::Storage(error.to_string()))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| DbError::Storage(error.to_string()))?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|error| DbError::Storage(error.to_string()))?;
    Ok(connection)
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
