use std::num::NonZeroUsize;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::protocol::LanSyncMessage;

#[async_trait::async_trait]
pub trait SessionOutboundGuard: Send + Sync {
    async fn allow(&self, message: &LanSyncMessage) -> bool;
}

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

#[derive(Clone)]
pub struct SessionIoPendingSends {
    state: std::sync::Arc<std::sync::Mutex<OutboundState>>,
    changed: std::sync::Arc<tokio::sync::Notify>,
}

#[derive(Default)]
struct OutboundState {
    closing: bool,
    pending: usize,
}

struct PendingSend(SessionIoPendingSends);

impl SessionIoPendingSends {
    fn new() -> Self {
        Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(OutboundState::default())),
            changed: std::sync::Arc::new(tokio::sync::Notify::new()),
        }
    }

    fn begin(&self) -> Result<PendingSend, SessionIoError> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.closing {
            return Err(SessionIoError::OutboundClosed);
        }
        state.pending += 1;
        Ok(PendingSend(self.clone()))
    }

    fn close(&self) {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .closing = true;
        self.changed.notify_one();
    }

    pub fn has_pending(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pending
            > 0
    }

    pub async fn changed(&self) {
        self.changed.notified().await;
    }
}

impl Drop for PendingSend {
    fn drop(&mut self) {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.pending -= 1;
        drop(state);
        self.0.changed.notify_one();
    }
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
    pub outbound_guard: Option<std::sync::Arc<dyn SessionOutboundGuard>>,
    pub close_after_flush: mpsc::Receiver<()>,
    pub pending_sends: SessionIoPendingSends,
    pub shutdown: watch::Receiver<bool>,
    pub stop: watch::Sender<bool>,
}

#[derive(Clone)]
pub struct SessionIoSender {
    outgoing: mpsc::Sender<LanSyncMessage>,
    control_outgoing: mpsc::Sender<LanSyncMessage>,
    shutdown: watch::Sender<bool>,
    close_after_flush: mpsc::Sender<()>,
    pending_sends: SessionIoPendingSends,
}

impl SessionIo {
    pub fn start_transport(
        capacity: usize,
        start_worker: impl FnOnce(SessionIoDriver) -> JoinHandle<()>,
    ) -> Result<Self, SessionIoError> {
        Self::start_transport_with_guard(capacity, None, start_worker)
    }

    pub fn start_transport_with_guard(
        capacity: usize,
        outbound_guard: Option<std::sync::Arc<dyn SessionOutboundGuard>>,
        start_worker: impl FnOnce(SessionIoDriver) -> JoinHandle<()>,
    ) -> Result<Self, SessionIoError> {
        let capacity = NonZeroUsize::new(capacity)
            .ok_or(SessionIoError::InvalidCapacity)?
            .get();
        let (incoming_tx, incoming) = mpsc::channel(capacity);
        let (outgoing, outgoing_rx) = mpsc::channel(capacity);
        let (control_outgoing, control_outgoing_rx) = mpsc::channel(capacity);
        let (shutdown, shutdown_rx) = watch::channel(false);
        let (close_after_flush, close_after_flush_rx) = mpsc::channel(1);
        let pending_sends = SessionIoPendingSends::new();
        let worker = start_worker(SessionIoDriver {
            incoming: incoming_tx,
            outgoing: outgoing_rx,
            control_outgoing: control_outgoing_rx,
            outbound_guard,
            close_after_flush: close_after_flush_rx,
            pending_sends: pending_sends.clone(),
            shutdown: shutdown_rx,
            stop: shutdown.clone(),
        });

        Ok(Self {
            incoming,
            outgoing: SessionIoSender {
                outgoing,
                control_outgoing,
                shutdown: shutdown.clone(),
                close_after_flush,
                pending_sends,
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
        self.outgoing.pending_sends.close();
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
    pub fn try_send(&self, message: LanSyncMessage) -> Result<(), SessionIoError> {
        let _pending = self.pending_sends.begin()?;
        let sender = if is_bulk_message(&message) {
            &self.outgoing
        } else {
            &self.control_outgoing
        };
        sender.try_send(message).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => SessionIoError::OutboundFull,
            mpsc::error::TrySendError::Closed(_) => SessionIoError::OutboundClosed,
        })
    }

    pub async fn send(&self, message: LanSyncMessage) -> Result<(), SessionIoError> {
        let _pending = self.pending_sends.begin()?;
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
        let _pending = self.pending_sends.begin()?;
        self.outgoing
            .send(message)
            .await
            .map_err(|_| SessionIoError::OutboundClosed)
    }

    pub fn close(&self) {
        self.pending_sends.close();
        let _ = self.shutdown.send(true);
    }

    pub fn close_after_flush(&self) {
        self.pending_sends.close();
        let _ = self.close_after_flush.try_send(());
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
        self.outgoing.pending_sends.close();
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
    async fn bulk_sender_waits_for_capacity_without_dropping_the_frame() {
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let session = SessionIo::start_transport(1, move |mut driver| {
            tokio::spawn(async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
                let first = driver.outgoing.recv().await;
                let second = driver.outgoing.recv().await;
                assert!(matches!(
                    first,
                    Some(LanSyncMessage::SyncChanges { batch_id, .. }) if batch_id == "first"
                ));
                assert!(matches!(
                    second,
                    Some(LanSyncMessage::SyncChanges { batch_id, .. }) if batch_id == "second"
                ));
            })
        })
        .unwrap();
        started_rx.await.unwrap();

        let sender = session.sender();
        sender
            .send(LanSyncMessage::SyncChanges {
                policy_identity: None,
                batch_id: "first".into(),
                entities: Vec::new(),
                is_last: false,
                origin_device_id: None,
                usage_complete_through: None,
            })
            .await
            .unwrap();
        let blocked_send = tokio::spawn(async move {
            sender
                .send_bulk_wait(LanSyncMessage::SyncChanges {
                    policy_identity: None,
                    batch_id: "second".into(),
                    entities: Vec::new(),
                    is_last: false,
                    origin_device_id: None,
                    usage_complete_through: None,
                })
                .await
        });
        tokio::task::yield_now().await;
        assert!(!blocked_send.is_finished());

        release_tx.send(()).unwrap();
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
