use super::*;

#[tokio::test]
async fn production_actor_skips_a_queued_command_after_caller_cancellation() {
    let (_directory, handle, mut owner) = open_actor_fixture().await;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 20,
                    payload: vec![0],
                    gate: Some((entered_tx, release_rx)),
                },
            )
            .await
    });
    tokio::task::spawn_blocking(move || entered_rx.recv().unwrap())
        .await
        .unwrap();

    let canceled_handle = handle.clone();
    let canceled = tokio::spawn(async move {
        canceled_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 21,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await
    });
    wait_for_admitted_commands(&handle, 2).await;
    canceled.abort();
    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    assert!(canceled.await.unwrap_err().is_cancelled());

    let follow_up = handle
        .submit(
            CommandContext::engine(),
            QueueProbe::<0> {
                sequence: 22,
                payload: vec![0],
                gate: None,
            },
        )
        .await;
    assert!(
        follow_up.is_ok(),
        "canceled queued work must release its admission slot"
    );
    let rows = handle.with_connection(|connection| {
        let mut statement = connection
            .prepare("SELECT sequence FROM actor_test_log ORDER BY sequence")
            .unwrap();
        statement
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    });
    assert_eq!(rows, vec![20, 22]);
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_finishes_started_work_after_caller_cancellation() {
    let (_directory, handle, mut owner) = open_actor_fixture().await;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let command_handle = handle.clone();
    let command = tokio::spawn(async move {
        command_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 23,
                    payload: vec![0],
                    gate: Some((entered_tx, release_rx)),
                },
            )
            .await
    });
    tokio::task::spawn_blocking(move || entered_rx.recv().unwrap())
        .await
        .unwrap();
    command.abort();
    assert!(command.await.unwrap_err().is_cancelled());
    release_tx.send(()).unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let wrote = handle.with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM actor_test_log WHERE sequence = 23)",
                        [],
                        |row| row.get::<_, bool>(0),
                    )
                    .unwrap()
            });
            if wrote {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_panic_rolls_back_and_recovers_on_the_same_owner() {
    let (_directory, handle, mut owner) = open_actor_fixture().await;
    let generation = handle.generation_for_test();
    let result = handle
        .submit(CommandContext::engine(), PanicAfterWriteProbe)
        .await;
    assert!(matches!(result, Err(DbError::OutcomeUnknown)));

    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while handle.generation_for_test() == generation || !handle.actor_ready_for_test() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actor did not recover after a command panic");
    let table_exists = handle.with_connection(|connection| {
        connection
            .query_row(
                concat!(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master ",
                    "WHERE type = 'table' AND name = 'panic_probe')"
                ),
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap()
    });
    assert!(!table_exists, "uncommitted panic write must be rolled back");
    handle
        .submit(
            CommandContext::engine(),
            QueueProbe::<0> {
                sequence: 24,
                payload: vec![0],
                gate: None,
            },
        )
        .await
        .unwrap();
    wait_for_admitted_commands(&handle, 0).await;
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_maintenance_is_a_barrier_for_later_writes_and_reads() {
    let (_directory, handle, mut owner) = open_actor_fixture().await;
    let (first_entered_tx, first_entered_rx) = std::sync::mpsc::channel();
    let (first_release_tx, first_release_rx) = std::sync::mpsc::channel();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 30,
                    payload: vec![0],
                    gate: Some((first_entered_tx, first_release_rx)),
                },
            )
            .await
    });
    tokio::task::spawn_blocking(move || first_entered_rx.recv().unwrap())
        .await
        .unwrap();

    let (maintenance_entered_tx, maintenance_entered_rx) = std::sync::mpsc::channel();
    let (maintenance_release_tx, maintenance_release_rx) = std::sync::mpsc::channel();
    let maintenance_handle = handle.clone();
    let maintenance = tokio::spawn(async move {
        maintenance_handle
            .submit_maintenance(
                CommandContext::engine_maintenance(),
                GateMaintenance {
                    entered: maintenance_entered_tx,
                    release: maintenance_release_rx,
                },
            )
            .await
    });
    wait_for_admitted_commands(&handle, 2).await;
    let later_handle = handle.clone();
    let later = tokio::spawn(async move {
        later_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 31,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await
    });
    wait_for_admitted_commands(&handle, 3).await;
    assert!(matches!(
        handle
            .query(
                CommandContext::sync_system(CommandClass::Backfill),
                ReadProbe,
            )
            .await,
        Err(DbError::Busy)
    ));

    first_release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    tokio::task::spawn_blocking(move || maintenance_entered_rx.recv().unwrap())
        .await
        .unwrap();
    assert!(
        !later.is_finished(),
        "later writes must wait behind maintenance"
    );
    maintenance_release_tx.send(()).unwrap();
    maintenance.await.unwrap().unwrap();
    later.await.unwrap().unwrap();
    let rows = handle.with_connection(|connection| {
        let mut statement = connection
            .prepare("SELECT sequence FROM actor_test_log ORDER BY rowid")
            .unwrap();
        statement
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    });
    assert_eq!(rows, vec![30, 31]);
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_shutdown_drains_accepted_work_and_allows_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("restart.db")
        .to_string_lossy()
        .into_owned();
    let (handle, mut owner) = DbHandle::open_with_limits(
        db_path.clone(),
        "actor-test-device".into(),
        actor_test_limits(),
    )
    .await
    .unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 40,
                    payload: vec![0],
                    gate: Some((entered_tx, release_rx)),
                },
            )
            .await
    });
    tokio::task::spawn_blocking(move || entered_rx.recv().unwrap())
        .await
        .unwrap();
    let second_handle = handle.clone();
    let second = tokio::spawn(async move {
        second_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 41,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await
    });
    wait_for_admitted_commands(&handle, 2).await;
    let release = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        release_tx.send(()).unwrap();
    });
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
    release.await.unwrap();
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    assert!(matches!(
        handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 42,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await,
        Err(DbError::Unavailable)
    ));

    let (reopened, mut reopened_owner) = DbHandle::open(db_path, "actor-test-device".into())
        .await
        .unwrap();
    let rows = reopened.with_connection(|connection| {
        let mut statement = connection
            .prepare("SELECT sequence FROM actor_test_log ORDER BY rowid")
            .unwrap();
        statement
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    });
    assert_eq!(rows, vec![40, 41]);
    reopened_owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
