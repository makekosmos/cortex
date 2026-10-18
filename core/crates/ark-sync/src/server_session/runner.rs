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
    async fn on_message(&mut self, message: Box<LanSyncMessage>);
    async fn on_malformed_message(&mut self);
}

pub async fn run_connected_server_session(
    session_io: &mut SessionIo,
    handler: &mut dyn DirectServerSessionHandler,
) -> DirectServerSessionEnd {
    loop {
        match session_io.receive().await {
            Some(SessionIoEvent::Message(message)) => match *message {
                LanSyncMessage::Ping { ts } => {
                    if let Err(error) = session_io.send(LanSyncMessage::Pong { ts }).await {
                        return DirectServerSessionEnd::Failed(format!("{error:?}"));
                    }
                }
                LanSyncMessage::Pong { .. } => {}
                message => handler.on_message(Box::new(message)).await,
            },
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
