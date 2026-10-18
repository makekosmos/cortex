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
                    if let Err(error) = session_io.sender().try_send(LanSyncMessage::Pong { ts }) {
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

pub async fn run_connected_server_session_controlled(
    session_io: &mut SessionIo,
    actor: &mut super::ServerSessionActor,
    handler: &mut dyn DirectServerSessionHandler,
) -> DirectServerSessionEnd {
    loop {
        if actor.is_revoked() {
            return close_for_control(session_io, actor.control());
        }
        tokio::select! {
            biased;
            control = actor.control_changed() => match control {
                Ok(super::ServerSessionControl::Pending | super::ServerSessionControl::Active) => {}
                Ok(
                    control @ (super::ServerSessionControl::CloseAfterFlush
                    | super::ServerSessionControl::CloseNow),
                ) => {
                    return close_for_control(session_io, control);
                }
                Err(_) => return DirectServerSessionEnd::Closed,
            },
            event = session_io.receive() => {
                if actor.is_revoked() {
                    return close_for_control(session_io, actor.control());
                }
                match event {
                Some(SessionIoEvent::Message(message)) => match *message {
                    LanSyncMessage::Ping { ts } => {
                        if let Err(error) =
                            session_io.sender().try_send(LanSyncMessage::Pong { ts })
                        {
                            return DirectServerSessionEnd::Failed(format!("{error:?}"));
                        }
                    }
                    LanSyncMessage::Pong { .. } => {}
                    message => handler.on_message(Box::new(message)).await,
                },
                Some(SessionIoEvent::MalformedMessage) => handler.on_malformed_message().await,
                Some(SessionIoEvent::Closed) | None => return DirectServerSessionEnd::Closed,
                Some(SessionIoEvent::Failed(error)) => return DirectServerSessionEnd::Failed(error),
                }
            }
        }
    }
}

fn close_for_control(
    session_io: &SessionIo,
    control: super::ServerSessionControl,
) -> DirectServerSessionEnd {
    match control {
        super::ServerSessionControl::CloseAfterFlush => session_io.sender().close_after_flush(),
        super::ServerSessionControl::CloseNow => session_io.sender().close(),
        super::ServerSessionControl::Pending | super::ServerSessionControl::Active => {}
    }
    DirectServerSessionEnd::Closed
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
