use super::super::{
    AuthenticatedTx, ClientMessageSender, ClientOnConnectedCallback,
    ClientOnRegistryEventsCallback, TAG,
};
use super::registry_admission;
use ark_application::sync_storage::SyncRuntimeStorage;
use ark_domain::sync_contracts::wire::LanSyncMessage;
use ark_domain::types::PeerRecord;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub(super) struct Context<'a> {
    pub(super) pending_registry_admission:
        &'a mut Option<registry_admission::PendingRegistryAdmission>,
    pub(super) device_id: &'a str,
    pub(super) storage: &'a Arc<dyn SyncRuntimeStorage>,
    pub(super) tx: &'a ClientMessageSender,
    pub(super) authenticated_tx: &'a AuthenticatedTx,
    pub(super) peer: &'a Arc<RwLock<PeerRecord>>,
    pub(super) on_registry_events: &'a Arc<Mutex<Option<ClientOnRegistryEventsCallback>>>,
    pub(super) on_connected: &'a Arc<Mutex<Option<ClientOnConnectedCallback>>>,
    pub(super) client_session: &'a mut crate::client_session::DirectClientSession,
    pub(super) stopped: &'a std::sync::atomic::AtomicBool,
}

pub(super) async fn handle(sync_msg: LanSyncMessage, context: Context<'_>) -> bool {
    let Context {
        pending_registry_admission,
        device_id,
        storage,
        tx,
        authenticated_tx,
        peer,
        on_registry_events,
        on_connected,
        client_session,
        stopped,
    } = context;
    let Some(admission) = pending_registry_admission.as_mut() else {
        eprintln!("{TAG} registry control arrived before Hello");
        return false;
    };
    let became_ready = match admission
        .handle(
            sync_msg,
            registry_admission::Context {
                local_device_id: device_id,
                storage,
                tx,
                authenticated_tx,
                peer,
                on_registry_events,
                on_connected,
            },
        )
        .await
    {
        Ok(became_ready) => became_ready,
        Err(error) => {
            if matches!(
                &error,
                registry_admission::PendingRegistryAdmissionError::SessionRevoked
            ) {
                stopped.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            eprintln!("{TAG} direct registry admission failed: {error}");
            return false;
        }
    };
    if became_ready && !client_session.mark_ready() {
        eprintln!("{TAG} registry admission completed outside the client session phase");
        return false;
    }
    true
}
