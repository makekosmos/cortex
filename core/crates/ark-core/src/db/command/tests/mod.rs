use super::*;
use production_actor::WriteProbe;

struct ReadProbe;

#[tokio::test]
async fn read_query_uses_backfill_context_without_opening_a_write_transaction() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::for_test(connection);
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
async fn open_publishes_only_a_database_ready_for_commands() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("ark.db")
        .to_string_lossy()
        .into_owned();
    let (handle, mut owner) = DbHandle::open(db_path, "device-under-test".into())
        .await
        .unwrap();

    let first = handle
        .submit(CommandContext::engine(), LoadOrGenerateIrohSecretKeyCommand)
        .await
        .unwrap();
    let second = handle
        .submit(CommandContext::engine(), LoadOrGenerateIrohSecretKeyCommand)
        .await
        .unwrap();

    assert_eq!(first.to_bytes(), second.to_bytes());
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn application_object_write_rolls_back_sqlite_transaction_on_port_error() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("object-write.db")
        .to_string_lossy()
        .into_owned();
    let (handle, mut owner) = DbHandle::open(db_path, "transaction-port-device".into())
        .await
        .unwrap();

    handle.with_connection(|connection| {
        connection
            .execute_batch(
                "CREATE TRIGGER reject_object_write_receipt
                 BEFORE INSERT ON entity_write_receipts
                 BEGIN
                     SELECT RAISE(ABORT, 'injected application port failure');
                 END;",
            )
            .unwrap();
    });

    let object = crate::types::ArkObjectWrite {
        id: "transaction-port-rollback".into(),
        type_id: "core.task".into(),
        type_version: Some("0.1.0".into()),
        title: "Must roll back".into(),
        content_json: serde_json::json!({
            "type": "doc",
            "content": [{ "type": "paragraph" }]
        }),
        props_json: serde_json::json!({
            "state": {"kind": "open", "at": null},
            "planning": {"kind": "inbox", "period": "any", "date": null},
            "deadline": null,
            "reminders": [],
            "recurrence": null,
            "priority": "none",
            "checklist": [],
            "extensions": {}
        }),
        created_at: "2026-10-16T00:00:00.000Z".into(),
        updated_at: "2026-10-16T00:00:00.000Z".into(),
        deleted_at: None,
    };

    let failed = handle
        .submit(
            CommandContext::engine(),
            LocalCoreTaskWrite::new(object.clone(), None, Some("transaction-port-device".into())),
        )
        .await
        .unwrap_err();
    assert!(
        format!("{failed:?}").contains("injected application port failure"),
        "expected the injected SQLite port failure, got {failed:?}"
    );

    let persisted_counts = handle.with_connection(|connection| {
        let objects: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE id = ?1",
                [&object.id],
                |row| row.get(0),
            )
            .unwrap();
        let revisions: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM object_sync_versions WHERE object_id = ?1",
                [&object.id],
                |row| row.get(0),
            )
            .unwrap();
        let receipts: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                [&object.id],
                |row| row.get(0),
            )
            .unwrap();
        (objects, revisions, receipts)
    });
    assert_eq!(persisted_counts, (0, 0, 0));

    handle.with_connection(|connection| {
        connection
            .execute_batch("DROP TRIGGER reject_object_write_receipt;")
            .unwrap();
    });
    handle
        .submit(
            CommandContext::engine(),
            LocalCoreTaskWrite::new(object.clone(), None, Some("transaction-port-device".into())),
        )
        .await
        .unwrap();
    let persisted = handle.with_connection(|connection| {
        connection
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE id = ?1",
                [&object.id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    });
    assert_eq!(persisted, 1);

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn open_rejects_an_unreadable_database_before_returning_a_handle() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("not-a-database.db");
    std::fs::write(&db_path, b"not a SQLite database").unwrap();

    let result = DbHandle::open(
        db_path.to_string_lossy().into_owned(),
        "device-under-test".into(),
    )
    .await;

    assert!(matches!(result, Err(DbError::Storage(_))));
}

#[tokio::test]
async fn read_queries_share_bounded_source_admission() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::for_test(connection);
    let reservations: Vec<_> = (0..DB_ADMISSION_LIMITS.max_commands_per_source)
        .map(|_| handle.hold_admission_for_test(CommandClass::Backfill, &CommandSource::System))
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
async fn iroh_secret_key_command_keeps_the_persisted_identity() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::for_test(connection);
    handle.with_connection(|connection| crate::db::init_schema(connection).unwrap());

    let first = handle
        .submit(CommandContext::engine(), LoadOrGenerateIrohSecretKeyCommand)
        .await
        .unwrap();
    let second = handle
        .submit(CommandContext::engine(), LoadOrGenerateIrohSecretKeyCommand)
        .await
        .unwrap();

    assert_eq!(first.to_bytes(), second.to_bytes());
}

#[tokio::test]
async fn usage_sequence_migration_runs_as_an_interactive_database_command() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::for_test(connection);
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
async fn usage_write_returns_typed_validation_error_and_rolls_back() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::for_test(connection);
    handle.with_connection(|connection| crate::db::init_schema(connection).unwrap());

    let committed = handle
        .submit(
            CommandContext::engine(),
            LocalUsageWriteCommand::new(
                "usage-command-device".into(),
                crate::application::usage::LocalUsageWrite::UpsertSpan(
                    crate::types::UsageSpanWrite {
                        device_id: "usage-command-device".into(),
                        started_at_unix: 1_777_939_199,
                        ended_at_unix: 1_777_939_201,
                        tracked_app_id: "usage-command-app".into(),
                        window_title: Some("crosses midnight".into()),
                        flags: 0,
                        updated_at: "2026-05-01T00:00:00.000Z".into(),
                    },
                ),
            ),
        )
        .await
        .unwrap();
    assert_eq!(committed.len(), 2);

    let invalid = handle
        .submit(
            CommandContext::engine(),
            LocalUsageWriteCommand::new(
                "usage-command-invalid-device".into(),
                crate::application::usage::LocalUsageWrite::UpsertSpan(
                    crate::types::UsageSpanWrite {
                        device_id: "usage-command-invalid-device".into(),
                        started_at_unix: i64::MAX - 1,
                        ended_at_unix: i64::MAX,
                        tracked_app_id: "usage-command-app".into(),
                        window_title: None,
                        flags: 0,
                        updated_at: "2026-05-01T00:00:00.000Z".into(),
                    },
                ),
            ),
        )
        .await;
    assert!(
        matches!(
            &invalid,
            Err(DbError::Invalid(message)) if message.starts_with("invalid usage span timestamp:")
        ),
        "unexpected usage command result: {invalid:?}"
    );

    let failed_device_rows = handle.with_connection(|connection| {
        let usage_days: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM usage_days WHERE device_id = ?1",
                ["usage-command-invalid-device"],
                |row| row.get(0),
            )
            .unwrap();
        let sync_heads: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM usage_sync_heads WHERE device_id = ?1",
                ["usage-command-invalid-device"],
                |row| row.get(0),
            )
            .unwrap();
        let sync_log: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM usage_sync_log WHERE device_id = ?1",
                ["usage-command-invalid-device"],
                |row| row.get(0),
            )
            .unwrap();
        let sync_versions: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM usage_sync_versions WHERE entity_id LIKE ?1",
                ["usage-day:usage-command-invalid-device:%"],
                |row| row.get(0),
            )
            .unwrap();
        (usage_days, sync_heads, sync_log, sync_versions)
    });
    assert_eq!(failed_device_rows, (0, 0, 0, 0));
}

#[tokio::test]
async fn clear_all_runs_inside_a_maintenance_database_command() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::for_test(connection);
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
    let handle = DbHandle::for_test(connection);
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
    let handle = DbHandle::for_test(connection);
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
    let handle = DbHandle::for_test(connection);
    let reservations: Vec<_> = (0..DB_ADMISSION_LIMITS.max_commands_per_source)
        .map(|_| handle.hold_admission_for_test(CommandClass::Interactive, &CommandSource::Engine))
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

mod device_registry;
mod operation_receipts;
mod production_actor;
mod sync_policy;
mod task_merge;
