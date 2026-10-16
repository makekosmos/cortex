use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::sync::{Arc, Mutex};

mod commands;
mod executor;

pub(crate) use commands::{
    AcquireIntegrationRefreshLeaseCommand, ApplyAuthenticatedSyncEntityCommand,
    ApplySignedIntegrationFrameCommand, ApplySyncEntityCommand, AuthenticatedCoreTaskWrite,
    AuthorizedTransportKeyQuery, CanonicalGameUpsertCommand, DeleteObjectLinkCommand,
    DeleteObjectTypeCommand, GetSyncKeyValueQuery, IdentityErrorStyle, LegacyDeleteCommand,
    LegacyGraphWriteCommand, LoadEntitiesQuery, LocalBookCoverCommand, LocalCoreTaskWrite,
    LocalObjectDeleteCommand, LocalObjectUpsertCommand, LocalUsageWriteCommand,
    PersistIntegrationGrantCommand, PersistNodeAuthorizationCommand,
    PublishIntegrationCredentialEnvelopeCommand, RegisterPackageTypesCommand,
    ReplaceObjectLinkCommand, SetLocalSyncKeyValueCommand, SetSyncKeyValueCommand,
    TrackedAppDeleteCommand, TrackedAppUpsertCommand, UpsertExternalRefCommand,
    UpsertObjectLinkCommand, UpsertObjectTypeCommand, UsageCompleteThroughQuery,
    ValidateOutboundSignedFrameQuery,
};

// A full transport frame is admitted at 16 MiB. Leave bounded room for the
// sealed query/command metadata that wraps the same frame for DbHandle.
const MAX_COMMAND_BYTES: usize = 16 * 1024 * 1024 + 4 * 1024;
const DB_ADMISSION_LIMITS: executor::AdmissionLimits = executor::AdmissionLimits {
    max_commands: 128,
    max_bytes: 64 * 1024 * 1024,
    max_command_bytes: MAX_COMMAND_BYTES,
    max_commands_per_source: 24,
    max_bytes_per_source: 20 * 1024 * 1024,
    max_sources: 64,
    reserved_interactive_commands: 16,
    reserved_interactive_bytes: 16 * 1024 * 1024,
};

async fn run_db_blocking<T: Send + 'static>(
    operation: impl FnOnce() -> T + Send + 'static,
) -> Result<T, DbError> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| {
            if error.is_panic() {
                std::panic::resume_unwind(error.into_panic());
            }
            DbError::Unavailable
        })
}

#[derive(Clone)]
pub(crate) struct DbHandle {
    connection: Arc<Mutex<Connection>>,
    admission: executor::AdmissionBudget,
}

impl DbHandle {
    pub(crate) fn from_shared_connection(connection: Arc<Mutex<Connection>>) -> Self {
        Self {
            connection,
            admission: executor::AdmissionBudget::new(DB_ADMISSION_LIMITS),
        }
    }

    /// Transitional access for command families not yet migrated. This always
    /// borrows the one connection owned by this handle; it never clones it.
    pub(crate) fn with_connection<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let guard = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        f(&guard)
    }

    /// KOS-51 restore needs mutable access to the existing connection until
    /// maintenance is represented by a command and lifecycle barrier.
    pub(crate) fn with_connection_mut<T>(&self, f: impl FnOnce(&mut Connection) -> T) -> T {
        let mut guard = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        f(&mut guard)
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
        if weight == 0 || weight > MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        let _admission = self
            .admission
            .try_acquire(context.class, &context.source, weight)
            .map_err(|error| match error {
                executor::AdmissionError::TooLarge => DbError::TooLarge,
                executor::AdmissionError::Saturated => DbError::Busy,
            })?;
        // Until the atomic executor cutover, preserve the current single
        // connection and mutex. The existing Tokio blocking pool keeps SQLite
        // work off async workers; this is not a second connection or writer.
        let handle = self.clone();
        run_db_blocking(move || {
            let _admission = _admission;
            handle.execute(context, command)
        })
        .await?
    }

    /// Execute a bounded read query off the async worker. It deliberately
    /// does not create a write transaction; until the reader-pool cutover it
    /// borrows the existing connection through this handle.
    pub(crate) async fn query<Q: DbQuery>(
        &self,
        context: CommandContext,
        query: Q,
    ) -> Result<Q::Output, DbError> {
        if context.class != query.class() {
            return Err(DbError::Invalid("database query class mismatch".into()));
        }
        let weight = query.weight();
        if weight == 0 || weight > MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        let handle = self.clone();
        run_db_blocking(move || {
            handle.with_connection(|connection| query.read(&context, connection))
        })
        .await?
    }

    fn execute<C: DbCommand>(
        &self,
        context: CommandContext,
        command: C,
    ) -> Result<C::Output, DbError> {
        self.with_connection(|connection| {
            let transaction =
                Transaction::new_unchecked(connection, TransactionBehavior::Immediate)
                    .map_err(|error| DbError::Storage(error.to_string()))?;
            let output = command.execute(&context, &transaction)?;
            transaction
                .commit()
                .map_err(|error| DbError::Storage(error.to_string()))?;
            Ok(output)
        })
    }

    #[cfg(test)]
    pub(crate) fn shared_connection_for_test(&self) -> Arc<Mutex<Connection>> {
        self.connection.clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CommandClass {
    Interactive,
    SyncLive,
    Backfill,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum CommandSource {
    Engine,
    System,
    AuthenticatedPeer(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandContext {
    class: CommandClass,
    source: CommandSource,
}

impl CommandContext {
    pub(crate) fn engine() -> Self {
        Self {
            class: CommandClass::Interactive,
            source: CommandSource::Engine,
        }
    }

    pub(crate) fn authenticated_peer(peer_id: String) -> Self {
        Self {
            class: CommandClass::SyncLive,
            source: CommandSource::AuthenticatedPeer(peer_id),
        }
    }

    pub(crate) fn sync_system(class: CommandClass) -> Self {
        Self {
            class,
            source: CommandSource::System,
        }
    }

    pub(crate) fn sync_peer(peer_id: String, class: CommandClass) -> Self {
        Self {
            class,
            source: CommandSource::AuthenticatedPeer(peer_id),
        }
    }

    fn authenticated_peer_id(&self) -> Result<&str, DbError> {
        match &self.source {
            CommandSource::AuthenticatedPeer(peer_id) => Ok(peer_id),
            CommandSource::Engine | CommandSource::System => Err(DbError::Permission(
                "authenticated sync principal required".into(),
            )),
        }
    }

    pub(crate) fn require_system_sync(&self) -> Result<(), DbError> {
        if self.class != CommandClass::SyncLive && self.class != CommandClass::Backfill {
            return Err(DbError::Invalid("sync query class required".into()));
        }
        match &self.source {
            CommandSource::System => Ok(()),
            _ => Err(DbError::Permission("system sync principal required".into())),
        }
    }

    pub(crate) fn require_sync_peer_or_system(&self) -> Result<(), DbError> {
        if self.class != CommandClass::SyncLive && self.class != CommandClass::Backfill {
            return Err(DbError::Invalid("sync query class required".into()));
        }
        match &self.source {
            CommandSource::System | CommandSource::AuthenticatedPeer(_) => Ok(()),
            CommandSource::Engine => Err(DbError::Permission("sync principal required".into())),
        }
    }

    fn require_engine(&self) -> Result<(), DbError> {
        match &self.source {
            CommandSource::Engine => Ok(()),
            CommandSource::AuthenticatedPeer(_) | CommandSource::System => {
                Err(DbError::Permission("engine principal required".into()))
            }
        }
    }
}

mod sealed {
    pub trait Sealed {}
}

pub(crate) trait DbCommand: sealed::Sealed + Send + 'static {
    type Output: Send + 'static;

    const CLASS: CommandClass;

    fn weight(&self) -> usize;

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError>;
}

/// Sealed read-only application surface. Query implementations receive a
/// connection without opening an IMMEDIATE transaction and inspect their
/// typed principal through `CommandContext`.
pub(crate) trait DbQuery: sealed::Sealed + Send + 'static {
    type Output: Send + 'static;

    fn class(&self) -> CommandClass;

    fn weight(&self) -> usize;

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError>;
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DbError {
    #[error("database command admission is saturated")]
    Busy,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Permission(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{0}")]
    Storage(String),
    #[error("database command exceeds admission size limits")]
    TooLarge,
    #[error("database command worker is unavailable")]
    Unavailable,
}

impl DbError {
    pub(crate) fn from_application_error(message: String) -> Self {
        if message.starts_with("object_conflict:") || message == "object_revision_payload_mismatch"
        {
            Self::Conflict(message)
        } else if message == "sync_peer_removed"
            || message == "authenticated sync principal required"
        {
            Self::Permission(message)
        } else if message.starts_with("object_write_slice_unsupported")
            || message.starts_with("object_write_slice_requires")
            || message.starts_with("unknown_type_or_version")
        {
            Self::Unsupported(message)
        } else if message.starts_with("invalid") || message == "object_write_origin_mismatch" {
            Self::Invalid(message)
        } else {
            Self::Storage(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ReadProbe;

    impl sealed::Sealed for ReadProbe {}

    struct WriteProbe;

    impl sealed::Sealed for WriteProbe {}

    impl DbCommand for WriteProbe {
        type Output = ();

        const CLASS: CommandClass = CommandClass::Interactive;

        fn weight(&self) -> usize {
            1
        }

        fn execute(
            self,
            context: &CommandContext,
            _transaction: &Transaction<'_>,
        ) -> Result<Self::Output, DbError> {
            context.require_engine()
        }
    }

    impl DbQuery for ReadProbe {
        type Output = bool;

        fn class(&self) -> CommandClass {
            CommandClass::Backfill
        }

        fn weight(&self) -> usize {
            1
        }

        fn read(
            self,
            context: &CommandContext,
            connection: &Connection,
        ) -> Result<Self::Output, DbError> {
            context.require_system_sync()?;
            Ok(connection.is_autocommit())
        }
    }

    #[tokio::test]
    async fn read_query_uses_backfill_context_without_opening_a_write_transaction() {
        let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        let handle = DbHandle::from_shared_connection(connection);
        let autocommit = handle
            .query(
                CommandContext::sync_system(CommandClass::Backfill),
                ReadProbe,
            )
            .await
            .unwrap();

        assert!(
            autocommit,
            "a read query must not start a write transaction"
        );
    }

    #[tokio::test]
    async fn submit_rejects_more_work_than_one_source_can_admit() {
        let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        let handle = DbHandle::from_shared_connection(connection);
        let reservations: Vec<_> = (0..DB_ADMISSION_LIMITS.max_commands_per_source)
            .map(|_| {
                handle
                    .admission
                    .try_acquire(CommandClass::Interactive, &CommandSource::Engine, 1)
                    .unwrap()
            })
            .collect();

        assert!(matches!(
            handle.submit(CommandContext::engine(), WriteProbe).await,
            Err(DbError::Busy)
        ));

        drop(reservations);
        assert!(handle
            .submit(CommandContext::engine(), WriteProbe)
            .await
            .is_ok());
    }
}
