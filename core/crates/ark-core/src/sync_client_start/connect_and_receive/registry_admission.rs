use super::super::*;

pub(super) struct PendingRegistryAdmission {
    admission: ark_sync::client_session::DirectClientAdmission,
    peer_record: PeerRecord,
}

pub(super) struct Context<'a> {
    pub(super) local_device_id: &'a str,
    pub(super) storage: &'a Arc<dyn SyncRuntimeStorage>,
    pub(super) tx: &'a mpsc::UnboundedSender<Message>,
    pub(super) authenticated_tx: &'a crate::sync_client::AuthenticatedTx,
    pub(super) peer: &'a Arc<RwLock<PeerRecord>>,
    pub(super) on_registry_events: &'a Arc<Mutex<Option<ClientOnRegistryEventsCallback>>>,
    pub(super) on_connected: &'a Arc<Mutex<Option<ClientOnConnectedCallback>>>,
}

impl PendingRegistryAdmission {
    pub(super) fn begin(
        local_device_id: &str,
        peer_device_id: &str,
        space_id: &str,
        protocol: NegotiatedProtocol,
        peer_record: PeerRecord,
    ) -> Result<(Self, LanSyncMessage), String> {
        let (admission, start) = ark_sync::client_session::DirectClientAdmission::begin(
            local_device_id,
            peer_device_id,
            space_id,
            protocol,
        )?;
        Ok((
            Self {
                admission,
                peer_record,
            },
            start,
        ))
    }

    pub(super) async fn handle(
        &mut self,
        message: LanSyncMessage,
        context: Context<'_>,
    ) -> Result<bool, String> {
        let Context {
            local_device_id,
            storage,
            tx,
            authenticated_tx,
            peer,
            on_registry_events,
            on_connected,
        } = context;
        let progress = self.admission.handle(storage.as_ref(), message).await?;
        if !progress.applied_events.is_empty() {
            if let Some(handler) = on_registry_events.lock().await.as_ref() {
                handler(progress.applied_events.clone());
            }
        }
        for outgoing in progress.outgoing {
            send_msg(tx, &outgoing);
        }
        let ready = match progress.status {
            ark_sync::client_session::AdmissionStatus::Pending => return Ok(false),
            ark_sync::client_session::AdmissionStatus::CloseSession => {
                return Err("registry_admission_session_revoked".into());
            }
            ark_sync::client_session::AdmissionStatus::Ready(ready) => ready,
        };

        if let Some(policy_message) = self
            .admission
            .sync_policy_message(storage.as_ref(), local_device_id, &ready)
            .await?
        {
            send_msg(tx, &policy_message);
        }
        let policy_bound_delivery = ready
            .protocol()
            .capabilities
            .contains(&ark_sync::protocol::ProtocolCapability::PolicyBoundDeliveryV1);
        *authenticated_tx.lock().await = Some(crate::sync_client::AuthenticatedPeerSession {
            peer_id: ready.peer_device_id().to_owned(),
            registry_session_id: ready.registry_session_id().to_owned(),
            tx: tx.clone(),
            sync_started: !policy_bound_delivery,
            sync_complete: false,
            protocol: ready.protocol().clone(),
        });

        let mut connected_record = self.peer_record.clone();
        connected_record.last_address = peer.read().await.last_address.clone();
        *peer.write().await = connected_record.clone();
        eprintln!(
            "{TAG} Registry admission ready with {} ({})",
            connected_record.device_name, connected_record.device_id
        );
        if let Some(handler) = on_connected.lock().await.as_ref() {
            handler(connected_record);
        }
        if let Some(vector) = self
            .admission
            .begin_initial_sync(storage.as_ref(), &ready)
            .await?
        {
            if let Some(session) = authenticated_tx.lock().await.as_mut() {
                session.sync_started = true;
            }
            send_msg(
                tx,
                &LanSyncMessage::VersionVector {
                    vector,
                    origin_device_id: None,
                },
            );
        }
        Ok(true)
    }
}
