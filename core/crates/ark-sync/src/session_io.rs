use std::num::NonZeroUsize;
use std::time::Duration;

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
    CloseFlushFailed(String),
    CloseFlushTimeout,
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
    close_after_flush: bool,
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

    fn close_after_flush(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.closing = true;
        state.close_after_flush = true;
        drop(state);
        self.changed.notify_one();
    }

    fn is_close_after_flush(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .close_after_flush
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
    close_result: watch::Receiver<Option<Result<(), String>>>,
    worker_result: watch::Receiver<Option<Result<(), String>>>,
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
    pub close_result: watch::Sender<Option<Result<(), String>>>,
    pub worker_completion: SessionIoWorkerCompletion,
    pub shutdown: watch::Receiver<bool>,
    pub stop: watch::Sender<bool>,
}

pub struct SessionIoWorkerCompletion {
    result: watch::Sender<Option<Result<(), String>>>,
    completed: bool,
}

impl SessionIoWorkerCompletion {
    pub fn complete(mut self) {
        self.completed = true;
        let _ = self.result.send(Some(Ok(())));
    }
}

impl Drop for SessionIoWorkerCompletion {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self
                .result
                .send(Some(Err("transport worker exited unexpectedly".into())));
        }
    }
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
        let (close_result, close_result_rx) = watch::channel(None);
        let (worker_result_tx, worker_result) = watch::channel(None);
        let (close_after_flush, close_after_flush_rx) = mpsc::channel(1);
        let pending_sends = SessionIoPendingSends::new();
        let worker = start_worker(SessionIoDriver {
            incoming: incoming_tx,
            outgoing: outgoing_rx,
            control_outgoing: control_outgoing_rx,
            outbound_guard,
            close_after_flush: close_after_flush_rx,
            pending_sends: pending_sends.clone(),
            close_result: close_result.clone(),
            worker_completion: SessionIoWorkerCompletion {
                result: worker_result_tx,
                completed: false,
            },
            shutdown: shutdown_rx,
            stop: shutdown.clone(),
        });

        Ok(Self {
            incoming,
            close_result: close_result_rx,
            worker_result,
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
        let flush_result = if self.outgoing.pending_sends.is_close_after_flush() {
            Some(self.wait_for_close_flush().await)
        } else {
            None
        };
        self.outgoing.pending_sends.close();
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(true);
        }
        let (join_result, finish_result) = if let Some(worker) = self.worker.take() {
            (
                worker
                    .await
                    .map_err(|error| SessionIoError::TransportTask(error.to_string())),
                Ok(()),
            )
        } else {
            (Ok(()), self.wait_for_worker_result().await)
        };
        if let Some(flush_result) = flush_result {
            flush_result?;
        }
        join_result?;
        finish_result?;
        Ok(())
    }

    pub fn take_transport_task(&mut self) -> Option<JoinHandle<()>> {
        self.worker.take()
    }

    async fn wait_for_worker_result(&mut self) -> Result<(), SessionIoError> {
        loop {
            if let Some(result) = self.worker_result.borrow().clone() {
                return result.map_err(SessionIoError::TransportTask);
            }
            self.worker_result.changed().await.map_err(|_| {
                SessionIoError::TransportTask("worker stopped without a result".into())
            })?;
        }
    }

    pub async fn finish_after_flush(self) -> Result<(), SessionIoError> {
        self.outgoing.close_after_flush();
        self.finish().await
    }

    async fn wait_for_close_flush(&mut self) -> Result<(), SessionIoError> {
        let mut close_result = self.close_result.clone();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(result) = close_result.borrow().clone() {
                    return result.map_err(SessionIoError::CloseFlushFailed);
                }
                close_result
                    .changed()
                    .await
                    .map_err(|_| SessionIoError::CloseFlushFailed("transport stopped".into()))?;
            }
        })
        .await
        .map_err(|_| SessionIoError::CloseFlushTimeout)?
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
        self.pending_sends.close_after_flush();
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
#[path = "session_io_tests.rs"]
mod tests;
