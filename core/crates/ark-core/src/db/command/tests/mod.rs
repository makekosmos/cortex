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

    let first = crate::iroh_transport::load_or_generate_secret_key_via_actor(&handle)
        .await
        .unwrap();
    let second = crate::iroh_transport::load_or_generate_secret_key_via_actor(&handle)
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
    let db_path = directory.path().join("object-write.db");
    let (handle, mut owner) = DbHandle::open(
        db_path.to_string_lossy().into_owned(),
        "transaction-port-device".into(),
    )
    .await
    .unwrap();
    ark_store::test_support::ObjectWriteRollbackFixture::reject_receipt_insert(&db_path).unwrap();

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
        .submit_local_core_task_write(
            CommandContext::engine(),
            None,
            LocalCoreTaskWrite::new(object.clone(), None, Some("transaction-port-device".into())),
        )
        .await
        .unwrap_err();
    assert!(
        format!("{failed:?}").contains("injected application port failure"),
        "expected the injected SQLite port failure, got {failed:?}"
    );

    let persisted_counts =
        ark_store::test_support::ObjectWriteRollbackFixture::inspect(&db_path, &object.id).unwrap();
    assert_eq!(
        persisted_counts,
        ark_store::test_support::ObjectWriteRollbackState {
            objects: 0,
            revisions: 0,
            receipts: 0,
        }
    );

    ark_store::test_support::ObjectWriteRollbackFixture::allow_receipt_insert(&db_path).unwrap();
    handle
        .submit_local_core_task_write(
            CommandContext::engine(),
            None,
            LocalCoreTaskWrite::new(object.clone(), None, Some("transaction-port-device".into())),
        )
        .await
        .unwrap();
    let persisted =
        ark_store::test_support::ObjectWriteRollbackFixture::inspect(&db_path, &object.id).unwrap();
    assert_eq!(persisted.objects, 1);

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
async fn actor_store_keeps_the_persisted_iroh_identity() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::for_test(connection);
    handle.with_connection(|connection| ark_store::open::init_schema(connection).unwrap());

    let first = crate::iroh_transport::load_or_generate_secret_key_via_actor(&handle)
        .await
        .unwrap();
    let second = crate::iroh_transport::load_or_generate_secret_key_via_actor(&handle)
        .await
        .unwrap();

    assert_eq!(first.to_bytes(), second.to_bytes());
}

#[tokio::test]
async fn usage_sequence_migration_runs_as_a_store_owned_transaction() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::for_test(connection);
    handle.with_connection(|connection| ark_store::open::init_schema(connection).unwrap());

    handle
        .ensure_usage_sequence_migrated(CommandContext::engine(), "device-under-test".into())
        .await
        .unwrap();

    let migration_complete = handle.with_connection(|connection| {
        ark_store::sync_kv::get_sync_kv(
            connection,
            ark_store::usage_sync_log::USAGE_SEQUENCE_MIGRATION_KEY,
        )
        .unwrap()
        .as_deref()
            == Some("1")
    });
    assert!(migration_complete);
}

#[tokio::test]
async fn usage_write_returns_typed_validation_error_and_rolls_back() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("usage-write.db");
    let (handle, mut owner) = DbHandle::open(
        db_path.to_string_lossy().into_owned(),
        "usage-command-device".into(),
    )
    .await
    .unwrap();

    let committed = handle
        .submit_local_usage_write(
            CommandContext::engine(),
            "usage-command-device".into(),
            crate::application::usage::LocalUsageWrite::UpsertSpan(crate::types::UsageSpanWrite {
                device_id: "usage-command-device".into(),
                started_at_unix: 1_777_939_199,
                ended_at_unix: 1_777_939_201,
                tracked_app_id: "usage-command-app".into(),
                window_title: Some("crosses midnight".into()),
                flags: 0,
                updated_at: "2026-05-01T00:00:00.000Z".into(),
            }),
        )
        .await
        .unwrap();
    assert_eq!(committed.len(), 2);

    let invalid = handle
        .submit_local_usage_write(
            CommandContext::engine(),
            "usage-command-invalid-device".into(),
            crate::application::usage::LocalUsageWrite::UpsertSpan(crate::types::UsageSpanWrite {
                device_id: "usage-command-invalid-device".into(),
                started_at_unix: i64::MAX - 1,
                ended_at_unix: i64::MAX,
                tracked_app_id: "usage-command-app".into(),
                window_title: None,
                flags: 0,
                updated_at: "2026-05-01T00:00:00.000Z".into(),
            }),
        )
        .await;
    assert!(
        matches!(
            &invalid,
            Err(DbError::Invalid(message)) if message.starts_with("invalid usage span timestamp:")
        ),
        "unexpected usage command result: {invalid:?}"
    );

    let failed_device_rows = ark_store::test_support::UsageRollbackFixture::inspect(
        &db_path,
        "usage-command-invalid-device",
    )
    .unwrap();
    assert_eq!(
        failed_device_rows,
        ark_store::test_support::UsageRollbackState {
            usage_days: 0,
            sync_heads: 0,
            sync_log: 0,
            sync_versions: 0,
        }
    );
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn clear_all_runs_inside_a_maintenance_database_command() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("clear-all.db")
        .to_string_lossy()
        .into_owned();
    let (handle, mut owner) = DbHandle::open(db_path, "clear-all-device".into())
        .await
        .unwrap();
    handle
        .set_local_sync_key_value(
            CommandContext::engine(),
            "test-key".into(),
            "test-value".into(),
        )
        .await
        .unwrap();

    handle
        .submit_clear_all(CommandContext::engine_maintenance())
        .await
        .unwrap();

    let value = handle
        .query_with_store(
            CommandContext::sync_system(CommandClass::SyncLive),
            GetSyncKeyValueQuery::new("test-key".into()),
        )
        .await
        .unwrap();
    assert_eq!(value, None);
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn usage_compaction_runs_as_a_bounded_maintenance_command() {
    let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
    let handle = DbHandle::for_test(connection);
    handle.with_connection(|connection| ark_store::open::init_schema(connection).unwrap());

    let deleted = handle
        .compact_usage_sync_log(
            CommandContext::engine_maintenance(),
            CompactUsageSyncLogCommand::new(None, "2026-09-01T00:00:00.000Z".into(), 10),
        )
        .await
        .unwrap();

    assert_eq!(deleted, 0);
}

#[tokio::test]
async fn delete_trashed_runs_through_an_interactive_database_command() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("delete-trashed.db");
    let (handle, mut owner) = DbHandle::open(
        db_path.to_string_lossy().into_owned(),
        "delete-trashed-device".into(),
    )
    .await
    .unwrap();
    ark_store::test_support::TrashedTodoFixture::insert(&db_path, "trashed").unwrap();

    let deleted = handle
        .submit_delete_trashed(CommandContext::engine(), "device-under-test".into())
        .await
        .unwrap();

    let remains = ark_store::test_support::TrashedTodoFixture::exists(&db_path, "trashed").unwrap();
    assert_eq!(deleted, 1);
    assert!(!remains);
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
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
mod production_actor;
mod sync_policy;
mod task_merge;
