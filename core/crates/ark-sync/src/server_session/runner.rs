use ark_sync_contracts::session_io::{SessionIo, SessionIoEvent};
use ark_sync_contracts::wire::LanSyncMessage;

#[derive(Debug, PartialEq, Eq)]
pub enum DirectServerSessionEnd {
    Closed,
    Failed(String),
}

pub(super) async fn run_connected_server_session_controlled(
    session_io: &mut SessionIo,
    actor: &mut super::ServerSessionActor,
) -> DirectServerSessionEnd {
    loop {
        if actor.is_revoked() {
            return close_for_control(session_io, actor.control());
        }
        let peer_list_deadline = actor.peer_list_deadline();
        tokio::select! {
            biased;
            control = actor.control.changed() => match control {
                Ok(()) => match actor.control() {
                    super::ServerSessionControl::Pending | super::ServerSessionControl::Active => {}
                    control @ (super::ServerSessionControl::CloseAfterFlush
                    | super::ServerSessionControl::CloseNow) => {
                        return close_for_control(session_io, control);
                    }
                },
                Err(_) => return DirectServerSessionEnd::Closed,
            },
            intent = actor.intents.recv() => match intent {
                Some(super::ServerSessionIntent::BroadcastRegistryEvent(event)) => {
                    let Some(connected) = actor.connected.take() else {
                        return DirectServerSessionEnd::Failed(
                            "server session actor is not configured".to_owned(),
                        );
                    };
                    let keep_running = connected
                        .send_registry_event(actor, event, &session_io.sender())
                        .await;
                    actor.connected = Some(connected);
                    if !keep_running {
                        return DirectServerSessionEnd::Closed;
                    }
                }
                Some(super::ServerSessionIntent::SendSignedFrame(frame)) => {
                    if actor.is_revoked() {
                        return close_for_control(session_io, actor.control());
                    }
                    if session_io
                        .sender()
                        .send_bulk_wait(LanSyncMessage::SignedIntegrationFrame { frame })
                        .await
                        .is_err()
                        || actor.is_revoked()
                    {
                        return DirectServerSessionEnd::Closed;
                    }
                }
                Some(super::ServerSessionIntent::WakeCommittedFeed) => {
                    let Some(mut connected) = actor.connected.take() else {
                        return DirectServerSessionEnd::Failed(
                            "server session actor is not configured".to_owned(),
                        );
                    };
                    let keep_running = connected
                        .pump_committed_feed(actor, &session_io.sender())
                        .await;
                    actor.connected = Some(connected);
                    if !keep_running {
                        return DirectServerSessionEnd::Closed;
                    }
                }
                Some(super::ServerSessionIntent::WakeTaskMerge) => {
                    let Some(mut connected) = actor.connected.take() else {
                        return DirectServerSessionEnd::Failed(
                            "server session actor is not configured".to_owned(),
                        );
                    };
                    let keep_running = connected
                        .pump_task_merge(actor, &session_io.sender())
                        .await;
                    actor.connected = Some(connected);
                    if !keep_running {
                        return DirectServerSessionEnd::Closed;
                    }
                }
                None => return DirectServerSessionEnd::Closed,
            },
            _ = tokio::time::sleep_until(
                peer_list_deadline.unwrap_or_else(tokio::time::Instant::now)
            ), if peer_list_deadline.is_some() => {
                if actor.try_send_due_peer_list(&session_io.sender()).is_err() {
                    return DirectServerSessionEnd::Closed;
                }
            }
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
                    message => {
                        let Some(mut connected) = actor.connected.take() else {
                            return DirectServerSessionEnd::Failed(
                                "server session actor is not configured".to_owned(),
                            );
                        };
                        let sender = session_io.sender();
                        let keep_running = if matches!(message, LanSyncMessage::Hello { .. }) {
                            connected.handle_hello(actor, message, &sender).await
                        } else if let Some(message) =
                            connected.classify_message(message, &sender)
                        {
                            match message {
                                super::DirectServerMessage::RegistryAdmission(message) => {
                                    connected
                                        .handle_registry_admission(actor, message, &sender)
                                        .await
                                }
                                super::DirectServerMessage::SyncPolicy(update) => {
                                    connected.handle_sync_policy(actor, update, &sender).await
                                }
                                message => connected.handle_message(actor, message, &sender).await,
                            }
                        } else {
                            true
                        };
                        actor.connected = Some(connected);
                        if !keep_running {
                            return DirectServerSessionEnd::Closed;
                        }
                    }
                },
                Some(SessionIoEvent::MalformedMessage) => {
                    let Some(host) = actor.host.as_ref() else {
                        return DirectServerSessionEnd::Failed(
                            "server session host is not configured".to_owned(),
                        );
                    };
                    host.malformed_message().await;
                }
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
