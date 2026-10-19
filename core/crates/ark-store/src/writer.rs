use rusqlite::{Connection, Transaction, TransactionBehavior};

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
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TransactionFailure::Begin)?;
        let output = operation(&transaction).map_err(TransactionFailure::Operation)?;
        transaction.commit().map_err(TransactionFailure::Commit)?;
        Ok(output)
    }

    pub fn with_connection_mut<T>(&mut self, operation: impl FnOnce(&mut Connection) -> T) -> T {
        operation(&mut self.connection)
    }

    pub fn apply_local_object_delete(
        &mut self,
        write: ark_application::object_mutation::LocalObjectDelete,
        principal: &str,
        operation_id: Option<&str>,
    ) -> Result<
        (ark_application::object_mutation::ObjectDeleteCommit, bool),
        TransactionFailure<ark_application::object_mutation::ObjectDeleteError>,
    > {
        self.with_connection_mut(|connection| {
            crate::object_mutation::apply_local_object_delete(
                connection,
                write,
                principal,
                operation_id,
            )
        })
    }

    pub fn apply_local_object_upsert(
        &mut self,
        write: ark_application::object_mutation::LocalObjectUpsert,
        principal: &str,
        operation_id: Option<&str>,
    ) -> Result<
        ark_application::object_mutation::ObjectUpsertOutcome,
        TransactionFailure<ark_application::object_mutation::ObjectUpsertError>,
    > {
        self.with_connection_mut(|connection| {
            crate::object_write::apply_local_object_upsert(
                connection,
                write,
                principal,
                operation_id,
            )
        })
    }

    pub fn apply_local_book_cover(
        &mut self,
        write: ark_application::book_cover::LocalBookCoverWrite,
    ) -> Result<bool, TransactionFailure<crate::book_cover::BookCoverOperationError>> {
        self.with_connection_mut(|connection| {
            crate::book_cover::apply_local_book_cover(connection, write)
        })
    }

    pub fn apply_canonical_game_upsert(
        &mut self,
        command: ark_application::canonical_game::GameUpsertCommand,
        fallback_device_id: &str,
    ) -> Result<
        ark_application::canonical_game::GameMutationResult,
        TransactionFailure<crate::canonical_game::CanonicalGameOperationError>,
    > {
        self.with_connection_mut(|connection| {
            crate::canonical_game::apply_upsert(connection, command, fallback_device_id)
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
