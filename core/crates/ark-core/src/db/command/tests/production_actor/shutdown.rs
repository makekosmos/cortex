use super::*;

#[tokio::test]
async fn production_actor_shutdown_respects_deadline_and_finishes_started_work() {
    let (_directory, handle, mut owner) = open_actor_fixture().await;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let command_handle = handle.clone();
    let command = tokio::spawn(async move {
        command_handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 26,
                    payload: vec![0],
                    gate: Some((entered_tx, release_rx)),
                },
            )
            .await
    });
    tokio::task::spawn_blocking(move || entered_rx.recv().unwrap())
        .await
        .unwrap();

    let started = std::time::Instant::now();
    let shutdown = owner
        .shutdown(started + std::time::Duration::from_millis(20))
        .await;
    assert!(matches!(shutdown, Err(DbError::OutcomeUnknown)));
    assert!(
        started.elapsed() < std::time::Duration::from_millis(250),
        "shutdown exceeded its bounded deadline: {:?}",
        started.elapsed()
    );
    assert!(matches!(
        handle
            .submit(
                CommandContext::engine(),
                QueueProbe::<0> {
                    sequence: 27,
                    payload: vec![0],
                    gate: None,
                },
            )
            .await,
        Err(DbError::Unavailable)
    ));

    release_tx.send(()).unwrap();
    command.await.unwrap().unwrap();
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();

    let committed = ark_store::test_support::ActorTestLog::entries_at(
        &actor_database_path(&_directory),
        ark_store::test_support::ActorLogOrder::Insertion,
    )
    .unwrap()
    .iter()
    .filter(|entry| entry.sequence == 26)
    .count();
    assert_eq!(committed, 1, "started work must finish before actor closes");
}
