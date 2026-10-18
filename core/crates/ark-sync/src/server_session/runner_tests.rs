#![allow(clippy::unwrap_used)]

use super::*;
use crate::session_io::SessionIoError;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

#[tokio::test]
async fn connected_server_session_routes_messages_and_reports_transport_end() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let mut handler = RecordingHandler(Arc::clone(&received));
    let (pong_tx, pong_rx) = oneshot::channel();
    let mut session_io = SessionIo::start_transport(4, |mut driver| {
        tokio::spawn(async move {
            for event in [
                SessionIoEvent::MalformedMessage,
                SessionIoEvent::Message(Box::new(LanSyncMessage::Ping { ts: 7 })),
                SessionIoEvent::Closed,
            ] {
                if driver.incoming.send(event).await.is_err() {
                    return;
                }
            }
            let pong = driver.control_outgoing.recv().await;
            let _ = pong_tx.send(pong);
        })
    })
    .unwrap();

    let end = run_connected_server_session(&mut session_io, &mut handler).await;
    assert_eq!(end, DirectServerSessionEnd::Closed);
    session_io.finish().await.unwrap();
    assert_eq!(*received.lock().unwrap(), ["malformed"]);
    assert!(matches!(
        pong_rx.await.unwrap(),
        Some(LanSyncMessage::Pong { ts: 7 })
    ));
}

#[tokio::test]
async fn connected_server_session_reports_transport_failure() {
    let mut handler = RecordingHandler(Arc::new(Mutex::new(Vec::new())));
    let mut session_io = SessionIo::start_transport(1, |driver| {
        tokio::spawn(async move {
            let _ = driver
                .incoming
                .send(SessionIoEvent::Failed("read failed".to_owned()))
                .await;
        })
    })
    .unwrap();

    assert_eq!(
        run_connected_server_session(&mut session_io, &mut handler).await,
        DirectServerSessionEnd::Failed("read failed".to_owned())
    );
    session_io.finish().await.unwrap();
}

#[tokio::test]
async fn connected_server_session_fails_closed_when_control_queue_is_full() {
    let mut handler = RecordingHandler(Arc::new(Mutex::new(Vec::new())));
    let (started_tx, started_rx) = oneshot::channel();
    let mut session_io = SessionIo::start_transport(1, |mut driver| {
        tokio::spawn(async move {
            let _control_outgoing = driver.control_outgoing;
            let _ = driver
                .incoming
                .send(SessionIoEvent::Message(Box::new(LanSyncMessage::Ping {
                    ts: 7,
                })))
                .await;
            let _ = started_tx.send(());
            while !*driver.shutdown.borrow_and_update() {
                if driver.shutdown.changed().await.is_err() {
                    break;
                }
            }
        })
    })
    .unwrap();
    started_rx.await.unwrap();
    session_io
        .sender()
        .try_send(LanSyncMessage::Pong { ts: 0 })
        .unwrap();

    assert_eq!(
        tokio::time::timeout(
            std::time::Duration::from_millis(50),
            run_connected_server_session(&mut session_io, &mut handler),
        )
        .await,
        Ok(DirectServerSessionEnd::Failed(format!(
            "{:?}",
            SessionIoError::OutboundFull
        )))
    );
    session_io.finish().await.unwrap();
}

#[tokio::test]
async fn connected_server_session_observes_revoke_when_intent_mailbox_is_full() {
    use crate::server_session::{ServerSessionHandle, ServerSessionIntent};
    use std::num::NonZeroUsize;

    let (handle, mut actor, _lease, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).unwrap()).unwrap();
    activation.activate().unwrap();
    handle
        .try_send(ServerSessionIntent::WakeCommittedFeed)
        .unwrap();

    let (close_seen_tx, close_seen_rx) = oneshot::channel();
    let mut session_io = SessionIo::start_transport(1, |mut driver| {
        tokio::spawn(async move {
            let _ = driver.close_after_flush.recv().await;
            let _ = close_seen_tx.send(());
            let _ = driver.close_result.send(Some(Ok(())));
            driver.worker_completion.complete();
        })
    })
    .unwrap();
    handle.close_after_flush();

    let mut handler = RecordingHandler(Arc::new(Mutex::new(Vec::new())));
    assert_eq!(
        run_connected_server_session_controlled(&mut session_io, &mut actor, &mut handler).await,
        DirectServerSessionEnd::Closed
    );
    close_seen_rx.await.unwrap();
    session_io.finish().await.unwrap();
}

#[tokio::test]
async fn delayed_peer_list_waits_until_due_and_sends_once() {
    use crate::server_session::ServerSessionHandle;
    use std::num::NonZeroUsize;
    use std::time::Duration;

    let (handle, mut actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).unwrap()).unwrap();
    activation.activate().unwrap();
    actor.schedule_peer_list_at(
        tokio::time::Instant::now() + Duration::from_millis(50),
        vec![test_peer_record()],
    );

    let (peer_list_tx, mut peer_list_rx) = oneshot::channel();
    let mut session_io = SessionIo::start_transport(1, |mut driver| {
        tokio::spawn(async move {
            let first = driver.control_outgoing.recv().await;
            let _ = driver.incoming.send(SessionIoEvent::Closed).await;
            let second =
                tokio::time::timeout(Duration::from_millis(40), driver.control_outgoing.recv())
                    .await
                    .ok()
                    .flatten();
            let _ = peer_list_tx.send((first, second));
            driver.worker_completion.complete();
        })
    })
    .unwrap();

    let mut handler = RecordingHandler(Arc::new(Mutex::new(Vec::new())));
    let run = tokio::spawn(async move {
        let end =
            run_connected_server_session_controlled(&mut session_io, &mut actor, &mut handler)
                .await;
        session_io.finish().await.unwrap();
        end
    });

    assert!(
        tokio::time::timeout(Duration::from_millis(15), &mut peer_list_rx)
            .await
            .is_err()
    );
    let (first, second) = peer_list_rx.await.unwrap();
    assert!(matches!(first, Some(LanSyncMessage::PeerList { .. })));
    assert!(second.is_none(), "peer list must be sent exactly once");
    assert_eq!(run.await.unwrap(), DirectServerSessionEnd::Closed);
    drop(handle);
}

#[tokio::test]
async fn full_peer_list_queue_retries_without_blocking_inbound_receive() {
    use crate::server_session::ServerSessionHandle;
    use std::num::NonZeroUsize;
    use std::time::Duration;

    let (handle, mut actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).unwrap()).unwrap();
    activation.activate().unwrap();
    actor.schedule_peer_list_at(tokio::time::Instant::now(), vec![test_peer_record()]);
    let (malformed_tx, malformed_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let (frames_tx, frames_rx) = oneshot::channel();
    let mut session_io = SessionIo::start_transport(1, |mut driver| {
        tokio::spawn(async move {
            let _ = driver.incoming.send(SessionIoEvent::MalformedMessage).await;
            let _ = release_rx.await;
            let first = driver.control_outgoing.recv().await;
            let second = driver.control_outgoing.recv().await;
            let duplicate =
                tokio::time::timeout(Duration::from_millis(40), driver.control_outgoing.recv())
                    .await
                    .ok()
                    .flatten();
            let _ = frames_tx.send((first, second, duplicate));
            let _ = driver.incoming.send(SessionIoEvent::Closed).await;
            driver.worker_completion.complete();
        })
    })
    .unwrap();
    session_io
        .sender()
        .try_send(LanSyncMessage::Pong { ts: 1 })
        .unwrap();

    let run = tokio::spawn(async move {
        let mut handler = MalformedSignal(Some(malformed_tx));
        let end =
            run_connected_server_session_controlled(&mut session_io, &mut actor, &mut handler)
                .await;
        session_io.finish().await.unwrap();
        end
    });

    tokio::time::timeout(Duration::from_millis(100), malformed_rx)
        .await
        .expect("inbound work should be selected while peer-list retry is pending")
        .unwrap();
    release_tx.send(()).unwrap();
    let (first, second, duplicate) = frames_rx.await.unwrap();
    assert!(matches!(first, Some(LanSyncMessage::Pong { ts: 1 })));
    assert!(matches!(second, Some(LanSyncMessage::PeerList { .. })));
    assert!(duplicate.is_none(), "peer list must be sent exactly once");
    assert_eq!(run.await.unwrap(), DirectServerSessionEnd::Closed);
    handle.revoke();
}

fn test_peer_record() -> ark_domain::types::PeerRecord {
    ark_domain::types::PeerRecord {
        device_id: "peer".to_owned(),
        device_name: "Peer".to_owned(),
        addresses: vec!["127.0.0.1:1".to_owned()],
        last_seen: "2026-10-19T00:00:00.000Z".to_owned(),
        last_address: None,
        platform: None,
        app_version: None,
    }
}

struct MalformedSignal(Option<oneshot::Sender<()>>);

#[async_trait::async_trait]
impl DirectServerSessionHandler for MalformedSignal {
    async fn on_message(&mut self, _message: super::super::DirectServerMessage) {}

    async fn on_malformed_message(&mut self) {
        if let Some(signal) = self.0.take() {
            let _ = signal.send(());
        }
    }
}

struct RecordingHandler(Arc<Mutex<Vec<String>>>);

#[async_trait::async_trait]
impl DirectServerSessionHandler for RecordingHandler {
    async fn on_message(&mut self, _: super::super::DirectServerMessage) {
        self.0.lock().unwrap().push("message".to_owned());
    }

    async fn on_malformed_message(&mut self) {
        self.0.lock().unwrap().push("malformed".to_owned());
    }
}
