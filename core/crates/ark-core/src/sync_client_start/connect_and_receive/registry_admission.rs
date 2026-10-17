use super::super::*;

pub(super) struct PendingRegistryAdmission {
    admission: crate::sync_registry_admission::DirectRegistryAdmission,
    local_device_id: String,
    protocol: NegotiatedProtocol,
    peer_record: PeerRecord,
    on_registry_events: Arc<Mutex<Option<ClientOnRegistryEventsCallback>>>,
}

impl PendingRegistryAdmission {
    pub(super) fn begin(
        local_device_id: &str,
        peer_device_id: &str,
        space_id: &str,
        protocol: NegotiatedProtocol,
        peer_record: PeerRecord,
        on_registry_events: Arc<Mutex<Option<ClientOnRegistryEventsCallback>>>,
    ) -> Result<(Self, LanSyncMessage), String> {
        let (admission, start) =
            crate::sync_registry_admission::DirectRegistryAdmission::initiator(
                local_device_id,
                peer_device_id,
                space_id,
                protocol.clone(),
            )?;
        Ok((
            Self {
                admission,
                local_device_id: local_device_id.to_owned(),
                protocol,
                peer_record,
                on_registry_events,
            },
            start,
        ))
    }

    pub(super) async fn handle(
        &mut self,
        message: LanSyncMessage,
        storage: &Arc<dyn SyncRuntimeStorage>,
        tx: &mpsc::UnboundedSender<Message>,
        authenticated_tx: &crate::sync_client::AuthenticatedTx,
        peer: &Arc<RwLock<PeerRecord>>,
        on_connected: &Arc<Mutex<Option<ClientOnConnectedCallback>>>,
    ) -> Result<bool, String> {
        let update = self.admission.handle(storage.as_ref(), message).await?;
        if !update.applied_events.is_empty() {
            if let Some(handler) = self.on_registry_events.lock().await.as_ref() {
                handler(update.applied_events.clone());
            }
        }
        for outgoing in update.outgoing {
            send_msg(tx, &outgoing);
        }
        if update.close_session && update.exchange_complete {
            return Err("registry_admission_session_revoked".into());
        }
        if !update.became_ready {
            return Ok(false);
        }

        let selective_policy_capability = self
            .protocol
            .capabilities
            .contains(&crate::protocol::ProtocolCapability::SelectiveSyncPolicyV1);
        let policy_bound_delivery = self
            .protocol
            .capabilities
            .contains(&crate::protocol::ProtocolCapability::PolicyBoundDeliveryV1);
        if selective_policy_capability {
            let policy_message = storage
                .load_local_sync_profile(&self.local_device_id)
                .await?
                .as_ref()
                .map(crate::protocol::sync_policy_message)
                .unwrap_or_else(crate::protocol::default_sync_policy_message);
            send_msg(tx, &policy_message);
        }
        *authenticated_tx.lock().await = Some(crate::sync_client::AuthenticatedPeerSession {
            peer_id: self.peer_record.device_id.clone(),
            registry_session_id: self
                .admission
                .session_id()
                .ok_or_else(|| "registry_admission_session_missing".to_owned())?
                .to_owned(),
            tx: tx.clone(),
            sync_started: !policy_bound_delivery,
            sync_complete: false,
            protocol: self.protocol.clone(),
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
        if !policy_bound_delivery {
            let generation = storage
                .selective_sync_policy_generation_for_peer(&self.peer_record.device_id)
                .await?;
            storage
                .begin_outbound_policy(&self.peer_record.device_id, generation)
                .await?;
            let vector = load_version_vector(storage)
                .await
                .map_err(|error| format!("failed to load sync version vector: {error}"))?;
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
