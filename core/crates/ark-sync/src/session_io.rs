use std::num::NonZeroUsize;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::protocol::LanSyncMessage;

pub enum SessionIoEvent {
    Message(Box<LanSyncMessage>),
    MalformedMessage,
    Closed,
    Failed(String),
}

#[derive(Debug, Eq, PartialEq)]
pub enum SessionIoError {
    InvalidCapacity,
    OutboundClosed,
    TransportTask(String),
}

pub struct SessionIo {
    pub(crate) incoming: mpsc::Receiver<SessionIoEvent>,
    outgoing: SessionIoSender,
    shutdown: Option<watch::Sender<bool>>,
    worker: Option<JoinHandle<()>>,
}

pub struct SessionIoDriver {
    pub incoming: mpsc::Sender<SessionIoEvent>,
    pub outgoing: mpsc::Receiver<LanSyncMessage>,
    pub shutdown: watch::Receiver<bool>,
    pub stop: watch::Sender<bool>,
}

#[derive(Clone)]
pub struct SessionIoSender {
    outgoing: mpsc::Sender<LanSyncMessage>,
}

impl SessionIo {
    pub fn start_transport(
        capacity: usize,
        start_worker: impl FnOnce(SessionIoDriver) -> JoinHandle<()>,
    ) -> Result<Self, SessionIoError> {
        let capacity = NonZeroUsize::new(capacity)
            .ok_or(SessionIoError::InvalidCapacity)?
            .get();
        let (incoming_tx, incoming) = mpsc::channel(capacity);
        let (outgoing, outgoing_rx) = mpsc::channel(capacity);
        let (shutdown, shutdown_rx) = watch::channel(false);
        let worker = start_worker(SessionIoDriver {
            incoming: incoming_tx,
            outgoing: outgoing_rx,
            shutdown: shutdown_rx,
            stop: shutdown.clone(),
        });

        Ok(Self {
            incoming,
            outgoing: SessionIoSender { outgoing },
            shutdown: Some(shutdown),
            worker: Some(worker),
        })
    }

    pub fn sender(&self) -> SessionIoSender {
        self.outgoing.clone()
    }

    pub async fn send(&self, message: LanSyncMessage) -> Result<(), SessionIoError> {
        self.outgoing.send(message).await
    }

    pub async fn receive(&mut self) -> Option<SessionIoEvent> {
        self.incoming.recv().await
    }

    pub async fn finish(mut self) -> Result<(), SessionIoError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(true);
        }
        if let Some(worker) = self.worker.take() {
            worker
                .await
                .map_err(|error| SessionIoError::TransportTask(error.to_string()))?;
        }
        Ok(())
    }
}

impl SessionIoSender {
    pub async fn send(&self, message: LanSyncMessage) -> Result<(), SessionIoError> {
        self.outgoing
            .send(message)
            .await
            .map_err(|_| SessionIoError::OutboundClosed)
    }
}

impl Drop for SessionIo {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(true);
        }
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}

#[cfg(test)]
mod tests {
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
                let _ = driver.outgoing.recv().await;
                let _ = driver.outgoing.recv().await;
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
}
