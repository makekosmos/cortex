use super::*;

#[tokio::test]
async fn production_actor_reserves_source_slots_for_interactive_and_live_work() {
    let (_directory, handle, mut owner) = open_actor_fixture().await;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .submit(
                CommandContext::sync_peer("device-a".into(), CommandClass::Backfill),
                QueueProbe::<2> {
                    sequence: 70,
                    payload: vec![0],
                    gate: Some((entered_tx, release_rx)),
                },
            )
            .await
    });
    tokio::task::spawn_blocking(move || entered_rx.recv().unwrap())
        .await
        .unwrap();

    let backfill_handle = handle.clone();
    let backfill = tokio::spawn(async move {
        backfill_handle
            .submit(
                CommandContext::sync_peer("device-b".into(), CommandClass::Backfill),
                QueueProbe::<2> {
                    sequence: 71,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await
    });
    wait_for_admitted_commands(&handle, 2).await;
    assert!(matches!(
        handle
            .submit(
                CommandContext::sync_peer("device-c".into(), CommandClass::Backfill),
                QueueProbe::<2> {
                    sequence: 72,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await,
        Err(DbError::Busy)
    ));

    let live_handle = handle.clone();
    let live = tokio::spawn(async move {
        live_handle
            .submit(
                CommandContext::sync_peer("device-d".into(), CommandClass::SyncLive),
                QueueProbe::<1> {
                    sequence: 73,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await
    });
    wait_for_admitted_commands(&handle, 3).await;
    let interactive_handle = handle.clone();
    let interactive = tokio::spawn(async move {
        interactive_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 74,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await
    });
    wait_for_admitted_commands(&handle, 4).await;

    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    backfill.await.unwrap().unwrap();
    live.await.unwrap().unwrap();
    interactive.await.unwrap().unwrap();
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_rotates_between_sources_within_a_class() {
    let mut limits = actor_test_limits();
    limits.max_commands = 8;
    limits.max_commands_per_source = 8;
    let (_directory, handle, mut owner) = open_actor_fixture_with_limits(limits).await;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .submit(
                CommandContext::sync_peer("device-a".into(), CommandClass::SyncLive),
                QueueProbe::<1> {
                    sequence: 80,
                    payload: vec![0],
                    gate: Some((entered_tx, release_rx)),
                },
            )
            .await
    });
    tokio::task::spawn_blocking(move || entered_rx.recv().unwrap())
        .await
        .unwrap();

    let mut queued = Vec::new();
    for sequence in [81, 82] {
        let task_handle = handle.clone();
        queued.push(tokio::spawn(async move {
            task_handle
                .submit(
                    CommandContext::sync_peer("device-a".into(), CommandClass::SyncLive),
                    QueueProbe::<1> {
                        sequence,
                        payload: vec![0],
                        gate: None,
                    },
                )
                .await
        }));
    }
    for sequence in [90, 91, 92] {
        let task_handle = handle.clone();
        queued.push(tokio::spawn(async move {
            task_handle
                .submit(
                    CommandContext::sync_peer("device-b".into(), CommandClass::SyncLive),
                    QueueProbe::<1> {
                        sequence,
                        payload: vec![0],
                        gate: None,
                    },
                )
                .await
        }));
    }
    wait_for_admitted_commands(&handle, 6).await;
    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    for task in queued {
        task.await.unwrap().unwrap();
    }

    let sources = handle.with_connection(|connection| {
        let mut statement = connection
            .prepare("SELECT source FROM actor_test_log ORDER BY rowid")
            .unwrap();
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    });
    assert_eq!(sources.len(), 6);
    assert!(sources[1].contains("device-a"));
    assert!(sources[2].contains("device-b"));
    assert!(sources[3].contains("device-a"));
    assert!(sources[4].contains("device-b"));
    assert!(sources[5].contains("device-b"));
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
