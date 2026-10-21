use super::*;

#[tokio::test]
async fn production_read_keeps_one_snapshot_while_writer_commits() {
    let (_directory, handle, mut owner) = open_actor_fixture().await;
    handle
        .submit(
            CommandContext::engine(),
            QueueProbe::<0> {
                sequence: 49,
                payload: vec![0],
                gate: None,
            },
        )
        .await
        .unwrap();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let read_handle = handle.clone();
    let read = tokio::spawn(async move {
        read_handle
            .query(
                CommandContext::sync_system(CommandClass::Backfill),
                SnapshotReadProbe {
                    started: started_tx,
                    resume: resume_rx,
                },
            )
            .await
    });
    tokio::task::spawn_blocking(move || started_rx.recv().unwrap())
        .await
        .unwrap();
    handle
        .submit(
            CommandContext::engine(),
            QueueProbe::<0> {
                sequence: 50,
                payload: vec![0],
                gate: None,
            },
        )
        .await
        .unwrap();
    resume_tx.send(()).unwrap();
    assert_eq!(read.await.unwrap().unwrap(), (1, 1));
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_reads_and_writes_share_global_admission_capacity() {
    let (_directory, handle, mut owner) = open_actor_fixture().await;
    handle
        .submit(
            CommandContext::engine(),
            QueueProbe::<0> {
                sequence: 59,
                payload: vec![0],
                gate: None,
            },
        )
        .await
        .unwrap();

    let (read_started_tx, read_started_rx) = std::sync::mpsc::channel();
    let (read_resume_tx, read_resume_rx) = std::sync::mpsc::channel();
    let read_handle = handle.clone();
    let read = tokio::spawn(async move {
        read_handle
            .query(
                CommandContext::sync_system(CommandClass::Backfill),
                SnapshotReadProbe {
                    started: read_started_tx,
                    resume: read_resume_rx,
                },
            )
            .await
    });
    tokio::task::spawn_blocking(move || read_started_rx.recv().unwrap())
        .await
        .unwrap();

    let (write_started_tx, write_started_rx) = std::sync::mpsc::channel();
    let (write_resume_tx, write_resume_rx) = std::sync::mpsc::channel();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 60,
                    payload: vec![0],
                    gate: Some((write_started_tx, write_resume_rx)),
                },
            )
            .await
    });
    tokio::task::spawn_blocking(move || write_started_rx.recv().unwrap())
        .await
        .unwrap();
    let mut queued = Vec::new();
    for sequence in [61, 62] {
        let queued_handle = handle.clone();
        queued.push(tokio::spawn(async move {
            queued_handle
                .submit(
                    CommandContext::engine(),
                    QueueProbe::<0> {
                        sequence,
                        payload: vec![0],
                        gate: None,
                    },
                )
                .await
        }));
    }
    wait_for_admitted_commands(&handle, 4).await;
    assert!(matches!(
        handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 63,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await,
        Err(DbError::Busy)
    ));

    read_resume_tx.send(()).unwrap();
    assert_eq!(read.await.unwrap().unwrap(), (1, 1));
    write_resume_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    for task in queued {
        task.await.unwrap().unwrap();
    }
    let row_count =
        ark_store::test_support::ActorTestLog::count_at(&actor_database_path(&_directory)).unwrap();
    assert_eq!(row_count, 4);
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_bounds_count_and_keeps_interactive_and_live_reserves() {
    let (_directory, handle, mut owner) = open_actor_fixture().await;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .submit(
                CommandContext::sync_peer("device-a".into(), CommandClass::Backfill),
                QueueProbe::<2> {
                    sequence: 0,
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
                CommandContext::sync_peer("device-a".into(), CommandClass::Backfill),
                QueueProbe::<2> {
                    sequence: 2,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await
    });
    tokio::task::yield_now().await;
    let saturated = handle
        .submit(
            CommandContext::sync_peer("device-b".into(), CommandClass::Backfill),
            QueueProbe::<2> {
                sequence: 4,
                payload: vec![0],
                gate: None,
            },
        )
        .await;
    assert!(matches!(saturated, Err(DbError::Busy)));

    let live_handle = handle.clone();
    let live = tokio::spawn(async move {
        live_handle
            .submit(
                CommandContext::sync_peer("device-c".into(), CommandClass::SyncLive),
                QueueProbe::<1> {
                    sequence: 3,
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
                    sequence: 1,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await
    });
    wait_for_admitted_commands(&handle, 4).await;
    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    interactive.await.unwrap().unwrap();
    live.await.unwrap().unwrap();

    let order = ark_store::test_support::ActorTestLog::entries_at(
        &actor_database_path(&_directory),
        ark_store::test_support::ActorLogOrder::Insertion,
    )
    .unwrap()
    .into_iter()
    .map(|entry| entry.sequence)
    .collect::<Vec<_>>();
    assert_eq!(order.len(), 4);
    assert!(
        order.contains(&1),
        "reserved Interactive admission must remain available"
    );
    assert!(
        order.contains(&3),
        "reserved SyncLive admission must remain available"
    );
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_bounds_payload_bytes_and_preserves_source_fifo_across_classes() {
    let mut limits = actor_test_limits();
    limits.max_commands = 8;
    limits.max_commands_per_source = 8;
    limits.max_bytes_per_source = 40;
    let (_directory, handle, mut owner) = open_actor_fixture_with_limits(limits).await;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .submit(
                CommandContext::sync_peer("device-a".into(), CommandClass::Backfill),
                QueueProbe::<2> {
                    sequence: 10,
                    payload: vec![0; 16],
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
                CommandContext::sync_peer("device-a".into(), CommandClass::Backfill),
                QueueProbe::<2> {
                    sequence: 12,
                    payload: vec![0; 16],
                    gate: None,
                },
            )
            .await
    });
    tokio::task::yield_now().await;
    let over_bytes = handle
        .submit(
            CommandContext::sync_peer("device-b".into(), CommandClass::Backfill),
            QueueProbe::<2> {
                sequence: 14,
                payload: vec![0],
                gate: None,
            },
        )
        .await;
    assert!(matches!(over_bytes, Err(DbError::Busy)));

    let live_handle = handle.clone();
    let live = tokio::spawn(async move {
        live_handle
            .submit(
                CommandContext::sync_peer("device-c".into(), CommandClass::SyncLive),
                QueueProbe::<1> {
                    sequence: 13,
                    payload: vec![0; 16],
                    gate: None,
                },
            )
            .await
    });
    let interactive_handle = handle.clone();
    let interactive = tokio::spawn(async move {
        interactive_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 11,
                    payload: vec![0; 16],
                    gate: None,
                },
            )
            .await
    });
    tokio::task::yield_now().await;
    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    backfill.await.unwrap().unwrap();
    live.await.unwrap().unwrap();
    interactive.await.unwrap().unwrap();

    let sequence = ark_store::test_support::ActorTestLog::entries_at(
        &actor_database_path(&_directory),
        ark_store::test_support::ActorLogOrder::Insertion,
    )
    .unwrap()
    .into_iter()
    .map(|entry| entry.sequence)
    .collect::<Vec<_>>();
    let backfill_positions =
        [10, 12].map(|value| sequence.iter().position(|item| *item == value).unwrap());
    assert!(
        backfill_positions[0] < backfill_positions[1],
        "same-source FIFO must survive class scheduling"
    );
    assert!(
        sequence.contains(&11),
        "Interactive class should be served during mixed backlog"
    );
    assert!(
        sequence.contains(&13),
        "SyncLive class should be served during mixed backlog"
    );
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_services_live_and_backfill_in_the_documented_weighted_share() {
    let mut limits = actor_test_limits();
    limits.max_commands = 16;
    limits.max_commands_per_source = 16;
    limits.max_sources = 8;
    let (_directory, handle, mut owner) = open_actor_fixture_with_limits(limits).await;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .submit(
                CommandContext::sync_peer("device-a".into(), CommandClass::Backfill),
                QueueProbe::<2> {
                    sequence: 0,
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
    for sequence in 10..14 {
        let task_handle = handle.clone();
        queued.push(tokio::spawn(async move {
            task_handle
                .submit(
                    CommandContext::engine(),
                    QueueProbe::<0> {
                        sequence,
                        payload: vec![0],
                        gate: None,
                    },
                )
                .await
        }));
    }
    for (sequence, device) in [(20, "device-c"), (21, "device-e")] {
        let task_handle = handle.clone();
        queued.push(tokio::spawn(async move {
            task_handle
                .submit(
                    CommandContext::sync_peer(device.into(), CommandClass::SyncLive),
                    QueueProbe::<1> {
                        sequence,
                        payload: vec![0],
                        gate: None,
                    },
                )
                .await
        }));
    }
    for (sequence, device) in [(30, "device-b"), (31, "device-d")] {
        let task_handle = handle.clone();
        queued.push(tokio::spawn(async move {
            task_handle
                .submit(
                    CommandContext::sync_peer(device.into(), CommandClass::Backfill),
                    QueueProbe::<2> {
                        sequence,
                        payload: vec![0],
                        gate: None,
                    },
                )
                .await
        }));
    }
    wait_for_admitted_commands(&handle, 9).await;
    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    for task in queued {
        task.await.unwrap().unwrap();
    }
    let sequence = ark_store::test_support::ActorTestLog::entries_at(
        &actor_database_path(&_directory),
        ark_store::test_support::ActorLogOrder::Insertion,
    )
    .unwrap()
    .into_iter()
    .map(|entry| entry.sequence)
    .collect::<Vec<_>>();
    let classes: Vec<_> = sequence
        .iter()
        .skip(1)
        .take(7)
        .map(|sequence| match sequence {
            10..=13 => CommandClass::Interactive,
            20..=21 => CommandClass::SyncLive,
            30..=31 => CommandClass::Backfill,
            _ => panic!("unexpected test sequence {sequence}"),
        })
        .collect();
    assert_eq!(
        classes,
        vec![
            CommandClass::Interactive,
            CommandClass::Interactive,
            CommandClass::Interactive,
            CommandClass::Interactive,
            CommandClass::SyncLive,
            CommandClass::SyncLive,
            CommandClass::Backfill,
        ]
    );
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
