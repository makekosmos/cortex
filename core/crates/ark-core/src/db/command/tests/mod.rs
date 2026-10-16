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
async fn read_queries_share_bounded_source_admission() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::from_shared_connection(connection);
    let reservations: Vec<_> = (0..DB_ADMISSION_LIMITS.max_commands_per_source)
        .map(|_| {
            handle
                .admission
                .try_acquire(CommandClass::Backfill, &CommandSource::System, 1)
                .unwrap()
        })
        .collect();

    assert!(matches!(
        handle
            .query(
                CommandContext::sync_system(CommandClass::Backfill),
                ReadProbe,
            )
            .await,
        Err(DbError::Busy)
    ));

    drop(reservations);
    assert!(handle
        .query(
            CommandContext::sync_system(CommandClass::Backfill),
            ReadProbe,
        )
        .await
        .is_ok());
}

#[tokio::test]
async fn usage_sequence_migration_runs_as_an_interactive_database_command() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::from_shared_connection(connection);
    handle.with_connection(|connection| crate::db::init_schema(connection).unwrap());

    handle
        .submit(
            CommandContext::engine(),
            EnsureUsageSequenceMigratedCommand::new("device-under-test".into()),
        )
        .await
        .unwrap();

    let migration_complete = handle.with_connection(|connection| {
        crate::db::get_sync_kv(connection, crate::db::USAGE_SEQUENCE_MIGRATION_KEY)
            .unwrap()
            .as_deref()
            == Some("1")
    });
    assert!(migration_complete);
}

#[tokio::test]
async fn clear_all_runs_inside_a_maintenance_database_command() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::from_shared_connection(connection);
    handle.with_connection(|connection| crate::db::init_schema(connection).unwrap());
    handle
        .submit(
            CommandContext::engine(),
            SetLocalSyncKeyValueCommand::new("test-key".into(), "test-value".into()),
        )
        .await
        .unwrap();

    handle
        .submit(CommandContext::engine_maintenance(), ClearAllCommand)
        .await
        .unwrap();

    let value = handle
        .with_connection(|connection| crate::db::get_sync_kv(connection, "test-key").unwrap());
    assert_eq!(value, None);
}

#[tokio::test]
async fn usage_compaction_runs_as_a_bounded_maintenance_command() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::from_shared_connection(connection);
    handle.with_connection(|connection| crate::db::init_schema(connection).unwrap());

    let deleted = handle
        .submit(
            CommandContext::engine_maintenance(),
            CompactUsageSyncLogCommand::new(None, "2026-09-01T00:00:00.000Z".into(), 10),
        )
        .await
        .unwrap();

    assert_eq!(deleted, 0);
}

#[tokio::test]
async fn delete_trashed_runs_through_an_interactive_database_command() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::from_shared_connection(connection);
    handle.with_connection(|connection| {
        crate::db::init_schema(connection).unwrap();
        connection
            .execute(
                "INSERT INTO todos (id, title, created_at, is_trashed) \
                     VALUES ('trashed', 'Trashed', '2026-01-01T00:00:00Z', 1)",
                [],
            )
            .unwrap();
    });

    let deleted = handle
        .submit(
            CommandContext::engine(),
            DeleteTrashedCommand::new("device-under-test".into()),
        )
        .await
        .unwrap();

    let remains = handle.with_connection(|connection| {
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM todos WHERE id = 'trashed')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap()
    });
    assert_eq!(deleted, 1);
    assert!(!remains);
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
