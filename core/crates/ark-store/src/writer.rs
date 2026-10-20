use rusqlite::{Connection, Transaction, TransactionBehavior};

mod authenticated_task;
mod sync_key_value;
mod task_objects;
mod usage_sequence_migration;
use task_objects::apply_local_core_task_write_in;
pub use task_objects::{TaskObjectWriteError, TaskObjectWrites};
/// SQLite failure detail that can cross the Store boundary without exposing
/// rusqlite types to callers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreError {
    pub kind: StoreErrorKind,
    message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreErrorKind {
    DiskFull,
    Io,
    Corrupt,
    Other,
}

impl StoreError {
    fn from_sqlite(error: rusqlite::Error) -> Self {
        let kind = match &error {
            rusqlite::Error::SqliteFailure(code, _) => match code.code {
                rusqlite::ErrorCode::DiskFull => StoreErrorKind::DiskFull,
                rusqlite::ErrorCode::SystemIoFailure => StoreErrorKind::Io,
                rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                    StoreErrorKind::Corrupt
                }
                _ => StoreErrorKind::Other,
            },
            _ => StoreErrorKind::Other,
        };
        Self {
            kind,
            message: error.to_string(),
        }
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Debug)]
pub enum StoreTransactionFailure<E> {
    Begin(StoreError),
    Operation(E),
    Commit(StoreError),
}

/// Short-lived operations available inside one Store-owned write transaction.
/// The transaction and all SQLite details remain private to ark-store.
pub struct WriteCapabilities<'scope, 'connection> {
    transaction: &'scope Transaction<'connection>,
}

pub struct UsageWrites<'scope, 'connection> {
    transaction: &'scope Transaction<'connection>,
}

pub struct ObjectWrites<'scope, 'connection> {
    transaction: &'scope Transaction<'connection>,
}

pub struct BookCoverWrites<'scope, 'connection> {
    transaction: &'scope Transaction<'connection>,
}

pub struct CanonicalGameWrites<'scope, 'connection> {
    transaction: &'scope Transaction<'connection>,
}

impl WriteCapabilities<'_, '_> {
    pub fn usage(&mut self) -> UsageWrites<'_, '_> {
        UsageWrites {
            transaction: self.transaction,
        }
    }

    pub fn objects(&mut self) -> ObjectWrites<'_, '_> {
        ObjectWrites {
            transaction: self.transaction,
        }
    }

    pub fn book_cover(&mut self) -> BookCoverWrites<'_, '_> {
        BookCoverWrites {
            transaction: self.transaction,
        }
    }

    pub fn canonical_game(&mut self) -> CanonicalGameWrites<'_, '_> {
        CanonicalGameWrites {
            transaction: self.transaction,
        }
    }

    pub fn task_objects(&mut self) -> TaskObjectWrites<'_, '_> {
        TaskObjectWrites {
            transaction: self.transaction,
        }
    }
}

impl UsageWrites<'_, '_> {
    pub fn apply_local_usage_write(
        &mut self,
        device_id: &str,
        write: ark_application::usage::LocalUsageWrite,
    ) -> Result<Vec<ark_application::usage::UsageMutation>, String> {
        let session = crate::usage_mutation::SqliteUsageWriteSession::new(self.transaction);
        ark_application::usage::apply_local_usage_write(&session, device_id, write)
    }
}

impl ObjectWrites<'_, '_> {
    pub fn apply_local_object_upsert(
        &mut self,
        write: ark_application::object_mutation::LocalObjectUpsert,
        principal: &str,
        operation_id: Option<&str>,
    ) -> Result<
        ark_application::object_mutation::ObjectUpsertOutcome,
        ark_application::object_mutation::ObjectUpsertError,
    > {
        crate::object_write::apply_local_object_upsert_in(
            self.transaction,
            write,
            principal,
            operation_id,
        )
    }

    pub fn apply_local_object_delete(
        &mut self,
        write: ark_application::object_mutation::LocalObjectDelete,
        principal: &str,
        operation_id: Option<&str>,
    ) -> Result<
        (ark_application::object_mutation::ObjectDeleteCommit, bool),
        ark_application::object_mutation::ObjectDeleteError,
    > {
        crate::object_mutation::apply_local_object_delete_in(
            self.transaction,
            write,
            principal,
            operation_id,
        )
    }
}

impl BookCoverWrites<'_, '_> {
    pub fn apply_local_book_cover(
        &mut self,
        write: ark_application::book_cover::LocalBookCoverWrite,
    ) -> Result<bool, crate::book_cover::BookCoverOperationError> {
        crate::book_cover::apply_local_book_cover_in(self.transaction, write)
    }
}

impl CanonicalGameWrites<'_, '_> {
    pub fn apply_upsert(
        &mut self,
        command: ark_application::canonical_game::GameUpsertCommand,
        fallback_device_id: &str,
    ) -> Result<
        ark_application::canonical_game::GameMutationResult,
        crate::canonical_game::CanonicalGameOperationError,
    > {
        crate::canonical_game::upsert_game_in(self.transaction, command, fallback_device_id)
    }
}

/// The single SQLite connection used by ARK's production writer.
///
/// The caller owns scheduling and must ensure only one writer task runs at a
/// time. This type owns SQLite transaction boundaries and synchronous
/// maintenance access; callbacks cannot await while the connection is held.
pub struct SqliteWriter {
    connection: Connection,
}

#[derive(Debug)]
pub enum TransactionFailure<E> {
    Begin(rusqlite::Error),
    Operation(E),
    Commit(rusqlite::Error),
}

impl SqliteWriter {
    pub fn from_connection(connection: Connection) -> Self {
        Self { connection }
    }

    pub fn open(path: &str) -> Result<Self, String> {
        crate::open::open_db(path).map(Self::from_connection)
    }

    pub fn immediate<T, E>(
        &mut self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, E>,
    ) -> Result<T, TransactionFailure<E>> {
        Self::immediate_on_connection(&mut self.connection, operation)
    }

    pub fn write_with_capabilities<T, E>(
        &mut self,
        operation: impl for<'scope, 'connection> FnOnce(
            &mut WriteCapabilities<'scope, 'connection>,
        ) -> Result<T, E>,
    ) -> Result<T, StoreTransactionFailure<E>> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| StoreTransactionFailure::Begin(StoreError::from_sqlite(error)))?;
        let output = {
            let mut capabilities = WriteCapabilities {
                transaction: &transaction,
            };
            operation(&mut capabilities).map_err(StoreTransactionFailure::Operation)?
        };
        transaction
            .commit()
            .map_err(|error| StoreTransactionFailure::Commit(StoreError::from_sqlite(error)))?;
        Ok(output)
    }

    pub fn apply_local_core_task_write(
        &mut self,
        write: ark_application::task_object_write::LocalCoreTaskWrite,
        principal: &str,
        operation_id: Option<&str>,
    ) -> Result<
        ark_application::task_object_write::TaskWriteOperationResult,
        StoreTransactionFailure<TaskObjectWriteError>,
    > {
        self.write_with_capabilities(|capabilities| {
            capabilities
                .task_objects()
                .apply_local_core_task_write(write, principal, operation_id)
        })
    }

    pub fn apply_authenticated_core_task_write(
        &mut self,
        peer_id: &str,
        write: ark_application::task_object_write::AuthenticatedCoreTaskWrite,
    ) -> Result<
        ark_application::task_object_write::RemoteCoreTaskOutcome,
        StoreTransactionFailure<
            ark_application::task_object_write::AuthenticatedCoreTaskWriteError,
        >,
    > {
        self.write_with_capabilities(|capabilities| {
            authenticated_task::apply_in(capabilities.transaction, peer_id, write)
        })
    }

    fn immediate_on_connection<T, E>(
        connection: &mut Connection,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, E>,
    ) -> Result<T, TransactionFailure<E>> {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TransactionFailure::Begin)?;
        let output = operation(&transaction).map_err(TransactionFailure::Operation)?;
        transaction.commit().map_err(TransactionFailure::Commit)?;
        Ok(output)
    }

    pub fn with_connection_mut<T>(&mut self, operation: impl FnOnce(&mut Connection) -> T) -> T {
        operation(&mut self.connection)
    }

    pub fn apply_local_usage_write_on_connection(
        connection: &mut Connection,
        device_id: &str,
        write: ark_application::usage::LocalUsageWrite,
    ) -> Result<Vec<ark_application::usage::UsageMutation>, TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            let session = crate::usage_mutation::SqliteUsageWriteSession::new(transaction);
            ark_application::usage::apply_local_usage_write(&session, device_id, write)
        })
    }

    pub fn apply_local_core_task_write_on_connection(
        connection: &mut Connection,
        write: ark_application::task_object_write::LocalCoreTaskWrite,
        principal: &str,
        operation_id: Option<&str>,
    ) -> Result<
        ark_application::task_object_write::TaskWriteOperationResult,
        TransactionFailure<TaskObjectWriteError>,
    > {
        Self::immediate_on_connection(connection, |transaction| {
            apply_local_core_task_write_in(transaction, write, principal, operation_id)
        })
    }

    pub fn apply_authenticated_core_task_write_on_connection(
        connection: &mut Connection,
        peer_id: &str,
        write: ark_application::task_object_write::AuthenticatedCoreTaskWrite,
    ) -> Result<
        ark_application::task_object_write::RemoteCoreTaskOutcome,
        TransactionFailure<ark_application::task_object_write::AuthenticatedCoreTaskWriteError>,
    > {
        Self::immediate_on_connection(connection, |transaction| {
            authenticated_task::apply_in(transaction, peer_id, write)
        })
    }

    pub fn clear_all(&mut self) -> Result<(), TransactionFailure<String>> {
        self.immediate(|transaction| crate::load_all::clear_all(transaction))
    }

    pub fn clear_all_on_connection(
        connection: &mut Connection,
    ) -> Result<(), TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::load_all::clear_all(transaction)
        })
    }

    pub fn delete_trashed(&mut self, device_id: &str) -> Result<usize, TransactionFailure<String>> {
        self.immediate(|transaction| crate::load_all::delete_trashed(transaction, device_id))
    }

    pub fn delete_trashed_on_connection(
        connection: &mut Connection,
        device_id: &str,
    ) -> Result<usize, TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::load_all::delete_trashed(transaction, device_id)
        })
    }

    pub fn upsert_object_link(
        &mut self,
        object_link: ark_domain::types::ObjectLink,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, TransactionFailure<String>>
    {
        self.immediate(|transaction| {
            let session =
                crate::object_link_mutation::SqliteObjectLinkMutationSession::new(transaction);
            ark_application::type_mutation::upsert_object_link(
                &session,
                object_link,
                device_id.as_deref(),
            )
        })
    }

    pub fn delete_object_link(
        &mut self,
        id: String,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, TransactionFailure<String>>
    {
        self.immediate(|transaction| {
            let session =
                crate::object_link_mutation::SqliteObjectLinkMutationSession::new(transaction);
            ark_application::type_mutation::delete_object_link(&session, id, device_id.as_deref())
        })
    }

    pub fn replace_object_link(
        &mut self,
        object_link: ark_domain::types::ObjectLink,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::ReplaceObjectLinkCommit, TransactionFailure<String>>
    {
        self.immediate(|transaction| {
            let session =
                crate::object_link_mutation::SqliteObjectLinkMutationSession::new(transaction);
            ark_application::type_mutation::replace_object_link(
                &session,
                object_link,
                device_id.as_deref(),
            )
        })
    }

    pub fn upsert_object_link_on_connection(
        connection: &mut Connection,
        object_link: ark_domain::types::ObjectLink,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, TransactionFailure<String>>
    {
        Self::immediate_on_connection(connection, |transaction| {
            let session =
                crate::object_link_mutation::SqliteObjectLinkMutationSession::new(transaction);
            ark_application::type_mutation::upsert_object_link(
                &session,
                object_link,
                device_id.as_deref(),
            )
        })
    }

    pub fn delete_object_link_on_connection(
        connection: &mut Connection,
        id: String,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, TransactionFailure<String>>
    {
        Self::immediate_on_connection(connection, |transaction| {
            let session =
                crate::object_link_mutation::SqliteObjectLinkMutationSession::new(transaction);
            ark_application::type_mutation::delete_object_link(&session, id, device_id.as_deref())
        })
    }

    pub fn replace_object_link_on_connection(
        connection: &mut Connection,
        object_link: ark_domain::types::ObjectLink,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::ReplaceObjectLinkCommit, TransactionFailure<String>>
    {
        Self::immediate_on_connection(connection, |transaction| {
            let session =
                crate::object_link_mutation::SqliteObjectLinkMutationSession::new(transaction);
            ark_application::type_mutation::replace_object_link(
                &session,
                object_link,
                device_id.as_deref(),
            )
        })
    }

    pub fn upsert_external_ref(
        &mut self,
        request: ark_application::external_ref::ExternalRefUpsert,
    ) -> Result<(), TransactionFailure<String>> {
        self.immediate(|transaction| {
            crate::data_platform::ensure_schema(transaction)?;
            crate::data_platform::upsert_external_ref(transaction, &request)
        })
    }

    pub fn upsert_external_ref_on_connection(
        connection: &mut Connection,
        request: ark_application::external_ref::ExternalRefUpsert,
    ) -> Result<(), TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::data_platform::ensure_schema(transaction)?;
            crate::data_platform::upsert_external_ref(transaction, &request)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SqliteWriter, TransactionFailure};
    use rusqlite::Connection;

    #[test]
    fn immediate_transaction_commits_success_and_rolls_back_operation_error() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch("CREATE TABLE values_table(value INTEGER NOT NULL)")
            .expect("test schema creates");
        let mut writer = SqliteWriter::from_connection(connection);

        writer
            .immediate(|transaction| {
                transaction.execute("INSERT INTO values_table VALUES (1)", [])?;
                Ok::<_, rusqlite::Error>(())
            })
            .expect("successful transaction commits");

        let failure = writer.immediate(|transaction| {
            transaction
                .execute("INSERT INTO values_table VALUES (2)", [])
                .map_err(|_| "test write fails")?;
            Err::<(), _>("injected operation failure")
        });
        assert!(matches!(
            failure,
            Err(TransactionFailure::Operation("injected operation failure"))
        ));

        let count = writer.with_connection_mut(|connection| {
            connection
                .query_row("SELECT COUNT(*) FROM values_table", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("committed row count reads")
        });
        assert_eq!(count, 1, "failed transaction leaves no partial write");
    }
}
