use rusqlite::{Connection, Transaction};

use crate::writer::{store_error_from_sqlite, StoreError};

use super::{ActorLogEntry, ActorLogOrder, ActorTestLog, ProductionActorFailureProbe};

pub struct ActorTestWrites<'scope, 'connection> {
    transaction: &'scope Transaction<'connection>,
}

impl<'scope, 'connection> ActorTestWrites<'scope, 'connection> {
    pub(crate) fn new(transaction: &'scope Transaction<'connection>) -> Self {
        Self { transaction }
    }

    pub fn write_actor_log_row(&mut self, sequence: i64, source: &str) -> Result<(), StoreError> {
        ActorTestLog::append(self.transaction, sequence, source).map_err(store_error_from_sqlite)
    }

    pub fn write_panic_probe(&mut self) -> Result<(), StoreError> {
        ActorTestLog::insert_panic_probe(self.transaction).map_err(store_error_from_sqlite)
    }

    pub fn write_failure_mutation(&mut self, payload_bytes: i64) -> Result<(), StoreError> {
        ProductionActorFailureProbe::insert_mutation(self.transaction, payload_bytes)
            .map_err(store_error_from_sqlite)
    }

    pub fn trigger_deferred_commit_failure(&mut self) -> Result<(), StoreError> {
        ProductionActorFailureProbe::trigger_deferred_commit_failure(self.transaction)
            .map_err(store_error_from_sqlite)
    }
}

pub struct ActorTestReads<'snapshot> {
    connection: &'snapshot Connection,
}

impl<'snapshot> ActorTestReads<'snapshot> {
    pub(crate) fn new(connection: &'snapshot Connection) -> Self {
        Self { connection }
    }

    pub fn actor_log_count(&self) -> Result<i64, StoreError> {
        ActorTestLog::count(self.connection).map_err(store_error_from_sqlite)
    }

    pub fn actor_log_entries(
        &self,
        order: ActorLogOrder,
    ) -> Result<Vec<ActorLogEntry>, StoreError> {
        ActorTestLog::entries(self.connection, order).map_err(store_error_from_sqlite)
    }
}

pub struct ActorTestWriterMaintenance<'writer> {
    connection: &'writer mut Connection,
}

impl<'writer> ActorTestWriterMaintenance<'writer> {
    pub(crate) fn new(connection: &'writer mut Connection) -> Self {
        Self { connection }
    }

    pub fn set_max_page_count(&mut self, page_count: u32) -> Result<u32, StoreError> {
        ProductionActorFailureProbe::set_max_page_count(self.connection, page_count)
            .map_err(store_error_from_sqlite)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_snapshot::SqliteReader;
    use crate::test_support::TestDatabase;
    use crate::writer::{SqliteWriter, StoreTransactionFailure};

    fn database_path(database: &TestDatabase) -> &str {
        database
            .path()
            .to_str()
            .expect("test database path is UTF-8")
    }

    #[test]
    fn actor_capabilities_write_read_and_maintain_without_exposing_sqlite() {
        let database = TestDatabase::create().expect("test database initializes");
        let connection = database.open_connection().expect("database opens");
        ProductionActorFailureProbe::prepare(&connection).expect("failure probe initializes");
        let mut writer = SqliteWriter::from_connection(connection);

        writer
            .write_with_capabilities(|capabilities| {
                let mut actor = capabilities.test_support();
                actor.write_actor_log_row(7, "interactive")?;
                actor.write_actor_log_row(3, "backfill")?;
                actor.write_panic_probe()?;
                actor.write_failure_mutation(4)?;
                Ok::<_, StoreError>(())
            })
            .expect("actor test writes commit");

        let actual_page_count = writer
            .test_support()
            .set_max_page_count(4096)
            .expect("page count limit updates");
        assert!(actual_page_count >= 4096);

        let mut reader = SqliteReader::open_read_only(database_path(&database))
            .expect("read-only connection opens");
        let entries = reader
            .with_read_snapshot(|capabilities| {
                let actor = capabilities.test_support();
                assert_eq!(actor.actor_log_count()?, 2);
                actor.actor_log_entries(ActorLogOrder::Sequence)
            })
            .expect("actor test rows read in one snapshot");
        assert_eq!(
            entries,
            vec![
                ActorLogEntry {
                    sequence: 3,
                    source: "backfill".into(),
                },
                ActorLogEntry {
                    sequence: 7,
                    source: "interactive".into(),
                },
            ]
        );
    }

    #[test]
    fn deferred_commit_probe_fails_at_the_store_transaction_boundary() {
        let database = TestDatabase::create().expect("test database initializes");
        let connection = database.open_connection().expect("database opens");
        ProductionActorFailureProbe::prepare(&connection).expect("failure probe initializes");
        let mut writer = SqliteWriter::from_connection(connection);

        let result = writer.write_with_capabilities(|capabilities| {
            capabilities
                .test_support()
                .trigger_deferred_commit_failure()?;
            Ok::<_, StoreError>(())
        });

        assert!(matches!(result, Err(StoreTransactionFailure::Commit(_))));
        let state =
            ProductionActorFailureProbe::inspect_path(database.path(), "no-operation-receipt")
                .expect("failure probe state reads");
        assert_eq!(state.mutations, 0);
        assert_eq!(state.deferred_children, 0);
    }
}
