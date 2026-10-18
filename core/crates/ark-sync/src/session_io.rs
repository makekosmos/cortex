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
    OutboundFull,
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
    pub control_outgoing: mpsc::Receiver<LanSyncMessage>,
    pub shutdown: watch::Receiver<bool>,
    pub stop: watch::Sender<bool>,
}

#[derive(Clone)]
pub struct SessionIoSender {
    outgoing: mpsc::Sender<LanSyncMessage>,
    control_outgoing: mpsc::Sender<LanSyncMessage>,
    shutdown: watch::Sender<bool>,
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
        let (control_outgoing, control_outgoing_rx) = mpsc::channel(capacity);
        let (shutdown, shutdown_rx) = watch::channel(false);
        let worker = start_worker(SessionIoDriver {
            incoming: incoming_tx,
            outgoing: outgoing_rx,
            control_outgoing: control_outgoing_rx,
            shutdown: shutdown_rx,
            stop: shutdown.clone(),
        });

        Ok(Self {
            incoming,
            outgoing: SessionIoSender {
                outgoing,
                control_outgoing,
                shutdown: shutdown.clone(),
            },
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
        if is_bulk_message(&message) {
            return self
                .outgoing
                .try_send(message)
                .map_err(|error| match error {
                    mpsc::error::TrySendError::Full(_) => SessionIoError::OutboundFull,
                    mpsc::error::TrySendError::Closed(_) => SessionIoError::OutboundClosed,
                });
        }
        self.control_outgoing
            .send(message)
            .await
            .map_err(|_| SessionIoError::OutboundClosed)
    }

    pub async fn send_bulk_wait(&self, message: LanSyncMessage) -> Result<(), SessionIoError> {
        if !is_bulk_message(&message) {
            return self.send(message).await;
        }
        self.outgoing
            .send(message)
            .await
            .map_err(|_| SessionIoError::OutboundClosed)
    }

    pub fn close(&self) {
        let _ = self.shutdown.send(true);
    }
}

fn is_bulk_message(message: &LanSyncMessage) -> bool {
    matches!(
        message,
        LanSyncMessage::SyncChanges { .. }
            | LanSyncMessage::LiveChange { .. }
            | LanSyncMessage::TaskMergeBatch { .. }
            | LanSyncMessage::SignedIntegrationFrame { .. }
    )
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
}
