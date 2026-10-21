#![allow(clippy::unwrap_used)]

use super::*;
use tokio::sync::oneshot;

#[test]
fn transport_requires_a_nonzero_queue_capacity() {
    let mut worker_started = false;
    let result = SessionIo::start_transport(0, |_| {
        worker_started = true;
        tokio::spawn(async {})
    });

    assert_eq!(result.err(), Some(SessionIoError::InvalidCapacity));
    assert!(!worker_started);
}

#[tokio::test]
async fn outbound_queue_backpressures_senders_until_transport_drains() {
    let (ready_tx, ready_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let session = SessionIo::start_transport(1, move |mut driver| {
        tokio::spawn(async move {
            let _ = ready_tx.send(());
            let _ = release_rx.await;
            let _ = driver.control_outgoing.recv().await;
            let _ = driver.control_outgoing.recv().await;
        })
    })
    .unwrap();
    ready_rx.await.unwrap();

    session.send(LanSyncMessage::Ping { ts: 1 }).await.unwrap();
    let sender = session.sender();
    let blocked_send =
        tokio::spawn(async move { sender.send(LanSyncMessage::Ping { ts: 2 }).await });
    tokio::task::yield_now().await;
    assert!(!blocked_send.is_finished());

    let _ = release_tx.send(());
    blocked_send.await.unwrap().unwrap();
    session.finish().await.unwrap();
}

#[tokio::test]
async fn finish_waits_for_a_sender_close_after_flush_request() {
    let (joined_tx, joined_rx) = oneshot::channel();
    let session = SessionIo::start_transport(1, move |mut driver| {
        tokio::spawn(async move {
            assert_eq!(driver.close_after_flush.recv().await, Some(()));
            driver.close_result.send(Some(Ok(()))).unwrap();
            joined_tx.send(()).unwrap();
        })
    })
    .unwrap();

    session.sender().close_after_flush();
    session.finish().await.unwrap();
    joined_rx.await.unwrap();
}

#[tokio::test]
async fn finish_waits_for_a_transport_task_owned_by_the_server() {
    let session = SessionIo::start_transport(1, |driver| {
        let mut shutdown = driver.shutdown;
        let completion = driver.worker_completion;
        tokio::spawn(async move {
            shutdown.changed().await.unwrap();
            completion.complete();
        })
    })
    .unwrap();
    let mut session = session;
    let transport = session.take_transport_task().unwrap();

    session.finish().await.unwrap();
    transport.await.unwrap();
}

#[tokio::test]
async fn bulk_sender_waits_for_capacity_without_dropping_the_frame() {
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let session = SessionIo::start_transport(1, move |mut driver| {
        tokio::spawn(async move {
            let _ = started_tx.send(());
            let _ = release_rx.await;
            let first = driver.outgoing.recv().await;
            let second = driver.outgoing.recv().await;
            assert!(matches!(
                first,
                Some(LanSyncMessage::SyncChanges { batch_id, .. }) if batch_id == "first"
            ));
            assert!(matches!(
                second,
                Some(LanSyncMessage::SyncChanges { batch_id, .. }) if batch_id == "second"
            ));
        })
    })
    .unwrap();
    started_rx.await.unwrap();

    let sender = session.sender();
    sender
        .send(LanSyncMessage::SyncChanges {
            policy_identity: None,
            batch_id: "first".into(),
            entities: Vec::new(),
            is_last: false,
            origin_device_id: None,
            usage_complete_through: None,
        })
        .await
        .unwrap();
    let blocked_send = tokio::spawn(async move {
        sender
            .send_bulk_wait(LanSyncMessage::SyncChanges {
                policy_identity: None,
                batch_id: "second".into(),
                entities: Vec::new(),
                is_last: false,
                origin_device_id: None,
                usage_complete_through: None,
            })
            .await
    });
    tokio::task::yield_now().await;
    assert!(!blocked_send.is_finished());

    release_tx.send(()).unwrap();
    blocked_send.await.unwrap().unwrap();
    session.finish().await.unwrap();
}

#[tokio::test]
async fn control_messages_keep_a_reserved_queue_when_bulk_queue_is_full() {
    let (release_tx, release_rx) = oneshot::channel();
    let session = SessionIo::start_transport(1, move |mut driver| {
        tokio::spawn(async move {
            let _ = release_rx.await;
            let control = driver.control_outgoing.recv().await;
            let bulk = driver.outgoing.recv().await;
            assert!(matches!(control, Some(LanSyncMessage::SyncAck { .. })));
            assert!(matches!(bulk, Some(LanSyncMessage::SyncChanges { .. })));
        })
    })
    .unwrap();

    session
        .sender()
        .send(LanSyncMessage::SyncChanges {
            policy_identity: None,
            batch_id: "batch-1".into(),
            entities: Vec::new(),
            is_last: false,
            origin_device_id: None,
            usage_complete_through: None,
        })
        .await
        .unwrap();
    assert_eq!(
        session
            .sender()
            .send(LanSyncMessage::SyncChanges {
                policy_identity: None,
                batch_id: "batch-2".into(),
                entities: Vec::new(),
                is_last: false,
                origin_device_id: None,
                usage_complete_through: None,
            })
            .await,
        Err(SessionIoError::OutboundFull)
    );
    session
        .sender()
        .send(LanSyncMessage::SyncAck {
            batch_id: "batch-0".into(),
            accepted: 1,
            policy_identity: None,
        })
        .await
        .unwrap();

    let _ = release_tx.send(());
    session.finish().await.unwrap();
}
