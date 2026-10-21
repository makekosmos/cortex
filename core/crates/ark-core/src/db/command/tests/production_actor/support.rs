use super::*;

pub(in crate::db::command::tests) struct SnapshotReadProbe {
    pub(in crate::db::command::tests) started: std::sync::mpsc::Sender<()>,
    pub(in crate::db::command::tests) resume: std::sync::mpsc::Receiver<()>,
}

impl sealed::Sealed for SnapshotReadProbe {}

impl sealed::Sealed for ReadProbe {}

pub(in crate::db::command::tests) struct WriteProbe;

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

#[derive(serde::Serialize)]
pub(in crate::db::command::tests) struct FailureMutation {
    pub(in crate::db::command::tests) payload_bytes: i64,
}

impl sealed::Sealed for FailureMutation {}

impl DbCommand for FailureMutation {
    type Output = String;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        1
    }

    fn execute(
        self,
        _context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        ark_store::test_support::ProductionActorFailureProbe::insert_mutation(
            transaction,
            self.payload_bytes,
        )
        .map_err(DbError::from_test_sqlite_error)?;
        Ok(format!("inserted:{}", self.payload_bytes))
    }
}

pub(in crate::db::command::tests) struct SetMaxPageCount {
    pub(in crate::db::command::tests) page_count: u32,
}

impl sealed::Sealed for SetMaxPageCount {}

impl DbMaintenanceCommand for SetMaxPageCount {
    type Output = u32;

    fn weight(&self) -> usize {
        1
    }

    fn execute(
        self,
        writer: &mut ark_store::writer::SqliteWriter,
    ) -> Result<Self::Output, DbError> {
        writer.with_connection_mut(|connection| {
            ark_store::test_support::ProductionActorFailureProbe::set_max_page_count(
                connection,
                self.page_count,
            )
            .map_err(|error| DbError::Storage(error.to_string()))
        })
    }

    fn execute_on_connection(self, connection: &mut Connection) -> Result<Self::Output, DbError> {
        ark_store::test_support::ProductionActorFailureProbe::set_max_page_count(
            connection,
            self.page_count,
        )
        .map_err(|error| DbError::Storage(error.to_string()))
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

impl DbQuery for SnapshotReadProbe {
    type Output = (i64, i64);

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
        let before = ark_store::test_support::ActorTestLog::count(connection)
            .map_err(|error| DbError::Storage(error.to_string()))?;
        let _ = self.started.send(());
        self.resume
            .recv()
            .map_err(|error| DbError::Storage(error.to_string()))?;
        let after = ark_store::test_support::ActorTestLog::count(connection)
            .map_err(|error| DbError::Storage(error.to_string()))?;
        Ok((before, after))
    }
}

pub(in crate::db::command::tests) struct QueueProbe<const CLASS_ID: u8> {
    pub(in crate::db::command::tests) sequence: i64,
    pub(in crate::db::command::tests) payload: Vec<u8>,
    pub(in crate::db::command::tests) gate:
        Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
}

impl<const CLASS_ID: u8> sealed::Sealed for QueueProbe<CLASS_ID> {}

impl<const CLASS_ID: u8> DbCommand for QueueProbe<CLASS_ID> {
    type Output = ();

    const CLASS: CommandClass = match CLASS_ID {
        0 => CommandClass::Interactive,
        1 => CommandClass::SyncLive,
        2 => CommandClass::Backfill,
        _ => panic!("unsupported actor test class"),
    };

    fn weight(&self) -> usize {
        self.payload.len().max(1)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        if let Some((entered, release)) = self.gate {
            let _ = entered.send(());
            release
                .recv()
                .map_err(|error| DbError::Storage(error.to_string()))?;
        }
        let source = format!("{:?}", context.source);
        ark_store::test_support::ActorTestLog::append(transaction, self.sequence, &source)
            .map_err(|error| DbError::Storage(error.to_string()))?;
        Ok(())
    }
}

pub(in crate::db::command::tests) struct PanicAfterWriteProbe;

impl sealed::Sealed for PanicAfterWriteProbe {}

impl DbCommand for PanicAfterWriteProbe {
    type Output = ();

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        1
    }

    fn execute(
        self,
        _context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        ark_store::test_support::ActorTestLog::insert_panic_probe(transaction)
            .map_err(|error| DbError::Storage(error.to_string()))?;
        panic!("actor panic probe after an uncommitted write");
    }
}

pub(in crate::db::command::tests) struct GateMaintenance {
    pub(in crate::db::command::tests) entered: std::sync::mpsc::Sender<()>,
    pub(in crate::db::command::tests) release: std::sync::mpsc::Receiver<()>,
}

impl sealed::Sealed for GateMaintenance {}

impl DbMaintenanceCommand for GateMaintenance {
    type Output = ();

    fn weight(&self) -> usize {
        1
    }

    fn execute(
        self,
        _writer: &mut ark_store::writer::SqliteWriter,
    ) -> Result<Self::Output, DbError> {
        let _ = self.entered.send(());
        self.release
            .recv()
            .map_err(|error| DbError::Storage(error.to_string()))
    }

    fn execute_on_connection(self, _connection: &mut Connection) -> Result<Self::Output, DbError> {
        let _ = self.entered.send(());
        self.release
            .recv()
            .map_err(|error| DbError::Storage(error.to_string()))
    }
}

pub(in crate::db::command::tests) fn actor_test_limits() -> executor::AdmissionLimits {
    executor::AdmissionLimits {
        max_commands: 4,
        max_bytes: 64,
        max_command_bytes: 40,
        max_commands_per_source: 4,
        max_bytes_per_source: 40,
        max_sources: 4,
        reserved_interactive_sources: 1,
        reserved_sync_live_sources: 1,
        reserved_interactive_commands: 1,
        reserved_interactive_bytes: 16,
        reserved_sync_live_commands: 1,
        reserved_sync_live_bytes: 16,
    }
}

pub(in crate::db::command::tests) async fn open_actor_fixture(
) -> (tempfile::TempDir, DbHandle, DbOwner) {
    open_actor_fixture_with_limits(actor_test_limits()).await
}

pub(in crate::db::command::tests) async fn open_actor_fixture_with_limits(
    limits: executor::AdmissionLimits,
) -> (tempfile::TempDir, DbHandle, DbOwner) {
    let directory = tempfile::tempdir().unwrap();
    let db_path = actor_database_path(&directory)
        .to_string_lossy()
        .into_owned();
    let (handle, owner) = DbHandle::open_with_limits(db_path, "actor-test-device".into(), limits)
        .await
        .unwrap();
    (directory, handle, owner)
}

pub(in crate::db::command::tests) fn actor_database_path(
    directory: &tempfile::TempDir,
) -> std::path::PathBuf {
    directory.path().join("actor.db")
}

pub(in crate::db::command::tests) async fn wait_for_admitted_commands(
    handle: &DbHandle,
    expected: usize,
) {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if handle.admitted_commands_for_test() == expected {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actor did not admit expected command count");
}
