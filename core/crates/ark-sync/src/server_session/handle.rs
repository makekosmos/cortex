use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

use ark_domain::device_registry::RegistryEvent;
use tokio::sync::{mpsc, watch};

use crate::SignedSyncEnvelope;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

const PENDING: u8 = 0;
const ACTIVE: u8 = 1;
const CLOSE_AFTER_FLUSH: u8 = 2;
const CLOSE_NOW: u8 = 3;

#[derive(Clone, Debug, PartialEq)]
pub enum ServerSessionIntent {
    WakeCommittedFeed,
    WakeTaskMerge,
    BroadcastRegistryEvent(RegistryEvent),
    SendSignedFrame(SignedSyncEnvelope),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerSessionIntentError {
    Full,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerSessionControl {
    Pending,
    Active,
    CloseAfterFlush,
    CloseNow,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ServerPeerReadiness {
    #[default]
    Pending,
    Ready,
    Closed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerPeerProjection {
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub addresses: Vec<String>,
    pub platform: Option<String>,
    pub app_version: Option<String>,
    pub registry_session_id: Option<String>,
    pub readiness: ServerPeerReadiness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ServerConnectionLease(u64);

impl ServerConnectionLease {
    pub fn matches(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ServerSessionCreateError {
    IdentityExhausted,
}

pub struct ServerSessionHandle {
    lease: ServerConnectionLease,
    intents: mpsc::Sender<ServerSessionIntent>,
    projection: watch::Receiver<ServerPeerProjection>,
    lifecycle: Arc<AtomicU8>,
    control_tx: watch::Sender<ServerSessionControl>,
}

pub struct ServerSessionActor {
    lease: ServerConnectionLease,
    pub(super) intents: mpsc::Receiver<ServerSessionIntent>,
    pub(super) control: watch::Receiver<ServerSessionControl>,
    projection: watch::Sender<ServerPeerProjection>,
    lifecycle: Arc<AtomicU8>,
    pub(super) connected: Option<super::ConnectedServerSession>,
    pending_peer_list: Option<super::connected::PendingPeerList>,
}

pub struct ServerSessionActivation {
    lease: ServerConnectionLease,
    lifecycle: Arc<AtomicU8>,
    control_tx: watch::Sender<ServerSessionControl>,
    used: bool,
}

impl ServerSessionHandle {
    pub fn channel(
        intent_capacity: NonZeroUsize,
    ) -> Result<
        (
            Self,
            ServerSessionActor,
            ServerConnectionLease,
            ServerSessionActivation,
        ),
        ServerSessionCreateError,
    > {
        let id = NEXT_SESSION_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .map_err(|_| ServerSessionCreateError::IdentityExhausted)?;
        let lease = ServerConnectionLease(id);
        let lifecycle = Arc::new(AtomicU8::new(PENDING));
        let (intent_tx, intent_rx) = mpsc::channel(intent_capacity.get());
        let (control_tx, control_rx) = watch::channel(ServerSessionControl::Pending);
        let (projection_tx, projection_rx) = watch::channel(ServerPeerProjection::default());

        Ok((
            Self {
                lease,
                intents: intent_tx,
                projection: projection_rx,
                lifecycle: lifecycle.clone(),
                control_tx: control_tx.clone(),
            },
            ServerSessionActor {
                lease,
                intents: intent_rx,
                control: control_rx,
                projection: projection_tx,
                lifecycle: lifecycle.clone(),
                connected: None,
                pending_peer_list: None,
            },
            lease,
            ServerSessionActivation {
                lease,
                lifecycle,
                control_tx,
                used: false,
            },
        ))
    }

    pub fn lease(&self) -> ServerConnectionLease {
        self.lease
    }

    pub fn matches_lease(&self, lease: ServerConnectionLease) -> bool {
        self.lease == lease
    }

    pub fn try_send(&self, intent: ServerSessionIntent) -> Result<(), ServerSessionIntentError> {
        if matches!(
            self.lifecycle.load(Ordering::Acquire),
            CLOSE_AFTER_FLUSH | CLOSE_NOW
        ) {
            return Err(ServerSessionIntentError::Closed);
        }
        self.intents.try_send(intent).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => ServerSessionIntentError::Full,
            mpsc::error::TrySendError::Closed(_) => ServerSessionIntentError::Closed,
        })
    }

    pub fn revoke(&self) -> bool {
        self.transition_to_terminal(CLOSE_NOW)
    }

    pub fn close_after_flush(&self) -> bool {
        self.transition_to_terminal(CLOSE_AFTER_FLUSH)
    }

    fn transition_to_terminal(&self, requested: u8) -> bool {
        loop {
            let current = self.lifecycle.load(Ordering::Acquire);
            let next = match (current, requested) {
                (CLOSE_NOW, _) | (CLOSE_AFTER_FLUSH, CLOSE_AFTER_FLUSH) => return false,
                (CLOSE_AFTER_FLUSH, CLOSE_NOW) => CLOSE_NOW,
                (PENDING | ACTIVE, terminal) => terminal,
                _ => return false,
            };
            if self
                .lifecycle
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.control_tx.send_replace(control_for(next));
                return true;
            }
        }
    }

    pub fn control(&self) -> ServerSessionControl {
        control_for(self.lifecycle.load(Ordering::Acquire))
    }

    pub fn projection(&self) -> ServerPeerProjection {
        self.projection.borrow().clone()
    }

    pub fn projection_updates(&self) -> watch::Receiver<ServerPeerProjection> {
        self.projection.clone()
    }
}

impl Clone for ServerSessionHandle {
    fn clone(&self) -> Self {
        Self {
            lease: self.lease,
            intents: self.intents.clone(),
            projection: self.projection.clone(),
            lifecycle: self.lifecycle.clone(),
            control_tx: self.control_tx.clone(),
        }
    }
}

impl ServerSessionActivation {
    pub fn lease(&self) -> ServerConnectionLease {
        self.lease
    }

    pub fn activate(&mut self) -> Result<(), ServerSessionActivationError> {
        if self.used {
            return Err(ServerSessionActivationError::AlreadyUsed);
        }
        self.used = true;
        self.lifecycle
            .compare_exchange(PENDING, ACTIVE, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|actual| {
                if actual == CLOSE_AFTER_FLUSH || actual == CLOSE_NOW {
                    ServerSessionActivationError::Revoked
                } else {
                    ServerSessionActivationError::AlreadyActive
                }
            })?;
        self.control_tx.send_replace(ServerSessionControl::Active);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerSessionActivationError {
    AlreadyUsed,
    AlreadyActive,
    Revoked,
}

impl ServerSessionActor {
    pub fn configure_connected(&mut self, session: super::ConnectedServerSession) {
        self.connected = Some(session);
    }

    pub fn lease(&self) -> ServerConnectionLease {
        self.lease
    }

    pub fn is_revoked(&self) -> bool {
        matches!(
            self.lifecycle.load(Ordering::Acquire),
            CLOSE_AFTER_FLUSH | CLOSE_NOW
        )
    }

    pub fn control(&self) -> ServerSessionControl {
        control_for(self.lifecycle.load(Ordering::Acquire))
    }

    #[cfg(test)]
    pub(super) async fn control_changed(
        &mut self,
    ) -> Result<ServerSessionControl, watch::error::RecvError> {
        self.control.changed().await?;
        Ok(self.control())
    }

    pub fn publish_projection(&self, projection: ServerPeerProjection) {
        self.projection.send_replace(projection);
    }

    pub(super) fn schedule_peer_list(&mut self, peers: Vec<ark_domain::types::PeerRecord>) {
        if !peers.is_empty() && !self.is_revoked() {
            self.pending_peer_list = Some(super::connected::PendingPeerList::new(peers));
        }
    }

    #[cfg(test)]
    pub(super) fn schedule_peer_list_at(
        &mut self,
        deadline: tokio::time::Instant,
        peers: Vec<ark_domain::types::PeerRecord>,
    ) {
        self.pending_peer_list = Some(super::connected::PendingPeerList::at(deadline, peers));
    }

    pub(super) fn peer_list_deadline(&self) -> Option<tokio::time::Instant> {
        self.pending_peer_list
            .as_ref()
            .map(super::connected::PendingPeerList::deadline)
    }

    pub(super) fn try_send_due_peer_list(
        &mut self,
        sender: &crate::session_io::SessionIoSender,
    ) -> Result<(), crate::session_io::SessionIoError> {
        if self.is_revoked() {
            self.pending_peer_list = None;
            return Err(crate::session_io::SessionIoError::OutboundClosed);
        }
        let Some(pending) = self.pending_peer_list.as_mut() else {
            return Ok(());
        };
        let sent = pending.try_send_due(tokio::time::Instant::now(), sender)?;
        if sent {
            self.pending_peer_list = None;
        }
        Ok(())
    }
}

fn control_for(lifecycle: u8) -> ServerSessionControl {
    match lifecycle {
        PENDING => ServerSessionControl::Pending,
        ACTIVE => ServerSessionControl::Active,
        CLOSE_AFTER_FLUSH => ServerSessionControl::CloseAfterFlush,
        _ => ServerSessionControl::CloseNow,
    }
}

#[cfg(test)]
#[path = "handle_tests.rs"]
mod tests;
