use std::sync::Arc;

use ark_application::sync_storage::SyncRuntimeStorage;
use ark_domain::device_registry::RegistryEvent;
use ark_domain::types::{PeerRecord, SyncEntity};

#[path = "connected/feed.rs"]
mod feed;
#[path = "connected/hello.rs"]
mod hello;
#[path = "connected/inbound.rs"]
mod inbound;
#[path = "connected/messages.rs"]
mod messages;
#[path = "connected/snapshot.rs"]
mod snapshot;
#[path = "connected/task_merge.rs"]
mod task_merge;

use super::{
    AdmissionStatus, AdmittedServerPeer, DirectServerAdmission, DirectServerMessage,
    DirectServerSession, DirectServerSessionEffect, DirectServerSessionInput,
    DirectServerSyncStartPolicy, ResponderHelloRequest, ServerConnectionLease,
    ServerPeerProjection, ServerPeerReadiness, ServerSessionActor,
};
use crate::protocol::LanSyncMessage;

#[async_trait::async_trait]
pub trait ConnectedServerSessionHost: Send + Sync {
    async fn broadcast_registry_events(
        &self,
        source: ServerConnectionLease,
        events: Vec<RegistryEvent>,
    );

    async fn revoke_disabled_sessions(
        &self,
        disabled_device_ids: Vec<String>,
        preserve_pending: Option<ServerConnectionLease>,
        local_device_revoked: bool,
    ) -> bool;

    async fn evict_replaced_sessions(
        &self,
        device_id: &str,
        current: ServerConnectionLease,
    ) -> bool;

    async fn prepare_peer_connected(
        &self,
        lease: ServerConnectionLease,
        projection: ServerPeerProjection,
    );

    async fn notify_peer_connected(&self, device_id: String);

    async fn registry_events_applied(&self, events: Vec<RegistryEvent>);

    async fn peer_list(&self) -> Vec<PeerRecord>;

    async fn entity_applied(&self, source_device_id: &str, entity: SyncEntity);

    async fn discovered_peers(&self, peers: Vec<PeerRecord>);

    async fn malformed_message(&self);

    async fn task_merge_batch_applied(&self);
}

pub struct ConnectedServerSession {
    pub(crate) request: ResponderHelloRequest,
    pub(crate) storage: Arc<dyn SyncRuntimeStorage>,
    pub(crate) host: Arc<dyn ConnectedServerSessionHost>,
    pub(crate) direct: Option<DirectServerSession>,
    pub(crate) admission: Option<DirectServerAdmission>,
    pub(crate) peer: ServerPeerProjection,
    pub(crate) protocol: Option<crate::protocol::NegotiatedProtocol>,
    pub(crate) snapshot_progress: crate::outbound_snapshot::OutboundSnapshotProgress,
    pub(crate) feed_flight: Option<crate::committed_feed_delivery::FeedFlight>,
}

impl ConnectedServerSession {
    pub fn new(
        request: ResponderHelloRequest,
        storage: Arc<dyn SyncRuntimeStorage>,
        host: Arc<dyn ConnectedServerSessionHost>,
    ) -> Self {
        Self {
            request,
            storage,
            host,
            direct: None,
            admission: None,
            peer: ServerPeerProjection::default(),
            protocol: None,
            snapshot_progress: crate::outbound_snapshot::OutboundSnapshotProgress::not_sent(),
            feed_flight: None,
        }
    }

    pub(crate) fn classify_message(
        &mut self,
        message: LanSyncMessage,
        sender: &crate::session_io::SessionIoSender,
    ) -> Option<DirectServerMessage> {
        let Some(direct) = self.direct.as_mut() else {
            sender.close_after_flush();
            return None;
        };
        match direct
            .transition(DirectServerSessionInput::Message(Box::new(message)))
            .effect
        {
            DirectServerSessionEffect::Dispatch(message) => Some(*message),
            DirectServerSessionEffect::Ignore => None,
            DirectServerSessionEffect::Close(_) => {
                sender.close_after_flush();
                None
            }
            DirectServerSessionEffect::CheckRemovedPeer { .. }
            | DirectServerSessionEffect::Accepted(_)
            | DirectServerSessionEffect::Rejected(_) => None,
        }
    }

    pub(super) fn supports(&self, capability: crate::protocol::ProtocolCapability) -> bool {
        self.protocol
            .as_ref()
            .is_some_and(|protocol| protocol.capabilities.contains(&capability))
    }

    pub(super) fn snapshot_ready(&self) -> bool {
        self.peer.readiness == ServerPeerReadiness::Ready
            && self.snapshot_progress.has_been_acknowledged()
    }

    pub(crate) async fn handle_sync_policy(
        &mut self,
        actor: &ServerSessionActor,
        update: crate::client_session::SyncPolicyUpdate,
        sender: &crate::session_io::SessionIoSender,
    ) -> bool {
        let (Some(device_id), Some(protocol), Some(direct)) = (
            self.peer.device_id.as_deref(),
            self.protocol.as_ref(),
            self.direct.as_mut(),
        ) else {
            sender.close_after_flush();
            return false;
        };
        if crate::client_session::accept_sync_policy(
            self.storage.as_ref(),
            device_id,
            protocol,
            true,
            update,
        )
        .await
        .is_err()
            || actor.is_revoked()
        {
            sender.close_after_flush();
            return false;
        }
        if !direct.start_sync_after_policy_persisted() {
            return true;
        }
        let vector = match super::begin_policy_bound_sync(self.storage.as_ref(), device_id).await {
            Ok(vector) if !actor.is_revoked() => vector,
            _ => return false,
        };
        sender
            .send(LanSyncMessage::VersionVector {
                vector,
                origin_device_id: None,
            })
            .await
            .is_ok()
            && !actor.is_revoked()
    }

    pub(crate) async fn send_registry_event(
        &self,
        actor: &ServerSessionActor,
        event: RegistryEvent,
        sender: &crate::session_io::SessionIoSender,
    ) -> bool {
        let Some(session_id) = self
            .peer
            .registry_session_id
            .as_deref()
            .filter(|_| self.peer.readiness == ServerPeerReadiness::Ready)
        else {
            return true;
        };
        if actor.is_revoked() {
            return false;
        }
        sender
            .send(LanSyncMessage::RegistryEventUpdate {
                session_id: session_id.to_owned(),
                event,
            })
            .await
            .is_ok()
            && !actor.is_revoked()
    }

    pub(crate) async fn handle_registry_admission(
        &mut self,
        actor: &mut ServerSessionActor,
        message: LanSyncMessage,
        sender: &crate::session_io::SessionIoSender,
    ) -> bool {
        let Some(device_id) = self.peer.device_id.clone() else {
            sender.close_after_flush();
            return false;
        };
        let Some(mut admission) = self.admission.take() else {
            sender.close_after_flush();
            return false;
        };
        let progress = match admission.handle(self.storage.as_ref(), message).await {
            Ok(progress) => progress,
            Err(_) => {
                sender.close_after_flush();
                return false;
            }
        };
        if actor.is_revoked() {
            return false;
        }

        if !progress.applied_events.is_empty() {
            self.host
                .broadcast_registry_events(actor.lease(), progress.applied_events.clone())
                .await;
            if actor.is_revoked() {
                return false;
            }
        }
        let preserve_pending = progress.close_session.then_some(actor.lease());
        if self
            .host
            .revoke_disabled_sessions(
                progress.disabled_device_ids.clone(),
                preserve_pending,
                !progress.local_device_is_admitted,
            )
            .await
        {
            return false;
        }
        if actor.is_revoked() {
            return false;
        }
        if !progress.applied_events.is_empty() {
            self.host
                .registry_events_applied(progress.applied_events.clone())
                .await;
        }
        for outgoing in progress.outgoing {
            if sender.send(outgoing).await.is_err() || actor.is_revoked() {
                return false;
            }
        }

        let ready = match progress.status {
            AdmissionStatus::Ready(ready) => Some(ready),
            AdmissionStatus::Pending => None,
            AdmissionStatus::CloseSession => {
                sender.close_after_flush();
                return false;
            }
        };
        let Some(ready) = ready else {
            self.admission = Some(admission);
            return true;
        };
        self.activate_ready_peer(actor, sender, admission, ready, device_id)
            .await
    }

    async fn activate_ready_peer(
        &mut self,
        actor: &mut ServerSessionActor,
        sender: &crate::session_io::SessionIoSender,
        admission: DirectServerAdmission,
        ready: AdmittedServerPeer,
        device_id: String,
    ) -> bool {
        if admission
            .begin_legacy_baseline(self.storage.as_ref())
            .await
            .is_err()
            || actor.is_revoked()
        {
            sender.close_after_flush();
            return false;
        }
        if !self
            .storage
            .sync_session_is_admitted(&device_id)
            .await
            .unwrap_or(false)
            || actor.is_revoked()
        {
            sender.close_after_flush();
            return false;
        }
        let local_policy = match admission
            .local_sync_policy_message(self.storage.as_ref(), &self.request.device_id)
            .await
        {
            Ok(policy) if !actor.is_revoked() => policy,
            _ => {
                sender.close_after_flush();
                return false;
            }
        };
        let policy_bound = ready.policy_bound_delivery();
        let start_policy = if policy_bound {
            DirectServerSyncStartPolicy::AfterPolicyPersistence
        } else {
            DirectServerSyncStartPolicy::Immediate
        };
        let Some(direct) = self.direct.as_mut() else {
            sender.close_after_flush();
            return false;
        };
        if !direct.mark_ready(start_policy) {
            sender.close_after_flush();
            return false;
        }
        if !self
            .host
            .evict_replaced_sessions(&device_id, actor.lease())
            .await
            || actor.is_revoked()
        {
            return false;
        }
        self.admission = Some(admission);
        self.peer.registry_session_id = Some(ready.registry_session_id().to_owned());
        self.peer.readiness = ServerPeerReadiness::Ready;
        let projection = self.peer.clone();
        self.host
            .prepare_peer_connected(actor.lease(), projection.clone())
            .await;
        if actor.is_revoked() {
            return false;
        }
        actor.publish_projection(projection);
        self.host.notify_peer_connected(device_id.clone()).await;
        if actor.is_revoked() {
            return false;
        }
        if let Some(policy) = local_policy {
            if sender.send(policy).await.is_err() || actor.is_revoked() {
                return false;
            }
        }
        if !policy_bound {
            let vector = match self.storage.load_version_vector().await {
                Ok(vector) if !actor.is_revoked() => vector,
                _ => return false,
            };
            if sender
                .send(LanSyncMessage::VersionVector {
                    vector,
                    origin_device_id: None,
                })
                .await
                .is_err()
                || actor.is_revoked()
            {
                return false;
            }
        }
        let peer_list = self.host.peer_list().await;
        if actor.is_revoked() {
            return false;
        }
        if !peer_list.is_empty() {
            actor.schedule_peer_list(peer_list);
        }
        true
    }
}

pub(super) struct PendingPeerList {
    deadline: tokio::time::Instant,
    peers: Vec<ark_domain::types::PeerRecord>,
}

impl PendingPeerList {
    pub(super) fn new(peers: Vec<ark_domain::types::PeerRecord>) -> Self {
        Self::at(
            tokio::time::Instant::now() + std::time::Duration::from_millis(100),
            peers,
        )
    }

    pub(super) fn at(
        deadline: tokio::time::Instant,
        peers: Vec<ark_domain::types::PeerRecord>,
    ) -> Self {
        Self { deadline, peers }
    }

    pub(super) fn deadline(&self) -> tokio::time::Instant {
        self.deadline
    }

    pub(super) fn try_send_due(
        &mut self,
        now: tokio::time::Instant,
        sender: &crate::session_io::SessionIoSender,
    ) -> Result<bool, crate::session_io::SessionIoError> {
        if now < self.deadline {
            return Ok(false);
        }
        match sender.try_send(LanSyncMessage::PeerList {
            peers: self.peers.clone(),
        }) {
            Ok(()) => Ok(true),
            Err(crate::session_io::SessionIoError::OutboundFull) => {
                self.deadline = now + std::time::Duration::from_millis(25);
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }
}
