use crate::{
    protocol::LanSyncMessage,
    session_io::{SessionIo, SessionIoEvent},
};

#[derive(Debug, PartialEq, Eq)]
pub enum DirectServerSessionEnd {
    Closed,
    Failed(String),
}

#[async_trait::async_trait]
pub trait DirectServerSessionHandler: Send + Sync {
    async fn on_message(&self, message: Box<LanSyncMessage>);
    async fn on_malformed_message(&self);
}

pub async fn run_connected_server_session(
    session_io: &mut SessionIo,
    handler: &dyn DirectServerSessionHandler,
) -> DirectServerSessionEnd {
    loop {
        match session_io.receive().await {
            Some(SessionIoEvent::Message(message)) => handler.on_message(message).await,
            Some(SessionIoEvent::MalformedMessage) => handler.on_malformed_message().await,
            Some(SessionIoEvent::Closed) | None => return DirectServerSessionEnd::Closed,
            Some(SessionIoEvent::Failed(error)) => {
                return DirectServerSessionEnd::Failed(error);
            }
        }
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
