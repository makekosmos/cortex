use ark_sync::session_io::{SessionIo, SessionIoError};

pub(super) async fn finish_rejected_session(
    session_io: SessionIo,
    sender: &ark_sync::session_io::SessionIoSender,
) -> Result<(), SessionIoError> {
    sender.close();
    session_io.finish().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn rejected_session_closes_sender_and_joins_transport() {
        let (finished_tx, finished_rx) = oneshot::channel();
        let session_io = SessionIo::start_transport(1, move |mut driver| {
            tokio::spawn(async move {
                let _ = driver.shutdown.changed().await;
                driver.worker_completion.complete();
                let _ = finished_tx.send(());
            })
        })
        .unwrap();
        let sender = session_io.sender();

        finish_rejected_session(session_io, &sender)
            .await
            .expect("rejected session transport finishes cleanly");
        finished_rx
            .await
            .expect("finish waits for transport worker completion");
        assert!(matches!(
            sender.try_send(ark_sync::protocol::LanSyncMessage::Ping { ts: 1 }),
            Err(SessionIoError::OutboundClosed)
        ));
    }
}
