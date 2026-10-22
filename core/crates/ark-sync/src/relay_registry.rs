mod exchange;
mod fanout;
mod state;

use std::{collections::HashMap, sync::Arc};

use ark_application::registry_admission::RegistryAdmissionStorage;
use ark_domain::device_registry::RegistryEvent;
use ark_sync_contracts::peer_control::{PeerControlSendOutcome, PeerSessionControl};
use tokio::sync::Mutex;

use ark_sync_contracts::wire::LanSyncMessage;
use state::{Admission, RelayRegistryPeer};

#[derive(Debug, Clone)]
pub enum RegistryFrame {
    SyncStart {
        session_id: String,
    },
    EventsPage {
        session_id: String,
        page_index: u32,
        page_count: u32,
        events: Vec<RegistryEvent>,
    },
    SyncReady {
        session_id: String,
        digest: String,
    },
    EventUpdate {
        session_id: String,
        event: Box<RegistryEvent>,
    },
}

/// Results Core must publish even when a later state or network operation fails.
#[derive(Debug, Default)]
pub struct RegistryOutcome {
    pub applied_events: Vec<RegistryEvent>,
    pub disabled_device_ids: Vec<String>,
    pub revoked_peer_ids: Vec<String>,
    pub sender_is_admitted: bool,
    pub local_device_is_admitted: bool,
    pub became_ready: bool,
    pub policy_negotiated: bool,
    pub operation_error: Option<String>,
}

pub struct RelayRegistryCoordinator<S: RegistryAdmissionStorage + ?Sized> {
    pub(super) local_device_id: String,
    pub(super) storage: Arc<S>,
    pub(super) transport: Arc<dyn PeerSessionControl>,
    pub(in crate::relay_registry) peers: Mutex<HashMap<String, RelayRegistryPeer>>,
}

impl<S: RegistryAdmissionStorage + ?Sized + 'static> RelayRegistryCoordinator<S> {
    pub fn new(
        local_device_id: String,
        storage: Arc<S>,
        transport: Arc<dyn PeerSessionControl>,
    ) -> Self {
        Self {
            local_device_id,
            storage,
            transport,
            peers: Mutex::new(HashMap::new()),
        }
    }

    /// Record the endpoint authenticated by Core before registry frames arrive.
    pub async fn begin_peer(&self, peer: &str, endpoint: &str, policy_negotiated: bool) {
        self.peers.lock().await.insert(
            peer.to_owned(),
            RelayRegistryPeer {
                endpoint: endpoint.to_owned(),
                policy_negotiated,
                admission: Admission::Waiting,
            },
        );
    }

    pub async fn start_exchange(&self, peer: &str, endpoint: &str) -> Result<(), String> {
        self.start_exchange_inner(peer, endpoint).await
    }

    pub async fn handle_registry_frame(
        &self,
        peer: &str,
        endpoint: &str,
        frame: RegistryFrame,
        connected_peers: &[(String, String)],
    ) -> Result<RegistryOutcome, String> {
        self.handle_frame_inner(peer, endpoint, frame, connected_peers)
            .await
    }

    /// `connected_peers` is Core's current authenticated inventory. It lets a
    /// local-device disable revoke peers that have not entered registry state.
    pub async fn publish_delta(
        &self,
        events: &[RegistryEvent],
        disabled: &[String],
        origin: Option<&str>,
        connected_peers: &[(String, String)],
    ) -> RegistryOutcome {
        self.publish_delta_inner(events, disabled, origin, connected_peers)
            .await
    }

    pub async fn is_ready(&self, peer: &str, endpoint: &str) -> bool {
        self.is_ready_transport(peer, endpoint).await
    }

    pub async fn disconnect_peer(&self, peer: &str) {
        self.peers.lock().await.remove(peer);
    }

    pub(super) async fn send_outcome(
        &self,
        endpoint: &str,
        message: LanSyncMessage,
    ) -> Result<PeerControlSendOutcome, String> {
        self.transport
            .send_control_to_endpoint(endpoint, message)
            .await
    }

    pub(super) async fn send(&self, endpoint: &str, message: LanSyncMessage) -> Result<(), String> {
        match self.send_outcome(endpoint, message).await? {
            PeerControlSendOutcome::Sent => Ok(()),
            PeerControlSendOutcome::PeerOffline => Err("transport peer is offline".into()),
        }
    }
}

impl RegistryOutcome {
    pub(super) fn applied(
        applied: &ark_application::replication_storage::RegistryPageApply,
    ) -> Self {
        Self {
            applied_events: applied.applied_events.clone(),
            disabled_device_ids: applied.disabled_device_ids.clone(),
            sender_is_admitted: applied.sender_is_admitted,
            local_device_is_admitted: applied.local_device_is_admitted,
            ..Self::default()
        }
    }
}
