#![allow(clippy::unwrap_used)]

use super::*;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn connected_server_session_routes_messages_and_reports_transport_end() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let handler = RecordingHandler(Arc::clone(&received));
    let mut session_io = SessionIo::start_transport(4, |driver| {
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
        })
    })
    .unwrap();

    let end = run_connected_server_session(&mut session_io, &handler).await;
    assert_eq!(end, DirectServerSessionEnd::Closed);
    session_io.finish().await.unwrap();
    assert_eq!(
        *received.lock().unwrap(),
        ["malformed", "message:Ping { ts: 7 }"]
    );
}

#[tokio::test]
async fn connected_server_session_reports_transport_failure() {
    let handler = RecordingHandler(Arc::new(Mutex::new(Vec::new())));
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
        run_connected_server_session(&mut session_io, &handler).await,
        DirectServerSessionEnd::Failed("read failed".to_owned())
    );
    session_io.finish().await.unwrap();
}

struct RecordingHandler(Arc<Mutex<Vec<String>>>);

#[async_trait::async_trait]
impl DirectServerSessionHandler for RecordingHandler {
    async fn on_message(&self, message: Box<LanSyncMessage>) {
        self.0.lock().unwrap().push(format!("message:{message:?}"));
    }

    async fn on_malformed_message(&self) {
        self.0.lock().unwrap().push("malformed".to_owned());
    }
}
