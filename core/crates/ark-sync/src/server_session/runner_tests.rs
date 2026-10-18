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

struct RecordingHandler(Arc<Mutex<Vec<String>>>);

#[async_trait::async_trait]
impl DirectServerSessionHandler for RecordingHandler {
    async fn on_message(&mut self, message: Box<LanSyncMessage>) {
        self.0.lock().unwrap().push(format!("message:{message:?}"));
    }

    async fn on_malformed_message(&mut self) {
        self.0.lock().unwrap().push("malformed".to_owned());
    }
}
