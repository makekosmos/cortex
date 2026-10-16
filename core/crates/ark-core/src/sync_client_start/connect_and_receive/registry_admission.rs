use super::super::*;

pub(super) struct PendingRegistryAdmission {
    admission: crate::sync_registry_admission::DirectRegistryAdmission,
    protocol: NegotiatedProtocol,
    peer_record: PeerRecord,
}

impl PendingRegistryAdmission {
    pub(super) fn begin(
        local_device_id: &str,
        peer_device_id: &str,
        space_id: &str,
        protocol: NegotiatedProtocol,
        peer_record: PeerRecord,
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
                protocol,
                peer_record,
            },
            start,
        ))
    }

    pub(super) async fn handle(
        &mut self,
        message: LanSyncMessage,
        storage: &Arc<dyn SyncRuntimeStorage>,
        peer_device_id: &str,
        tx: &mpsc::UnboundedSender<Message>,
        authenticated_tx: &crate::sync_client::AuthenticatedTx,
        peer: &Arc<RwLock<PeerRecord>>,
        on_connected: &Arc<Mutex<Option<ClientOnConnectedCallback>>>,
    ) -> Result<bool, String> {
        let update = self.admission.handle(storage.as_ref(), message).await?;
        for outgoing in update.outgoing {
            send_msg(tx, &outgoing);
        }
        if !update.became_ready {
            return Ok(false);
        }

        let generation = storage.selective_sync_policy_generation();
        storage
            .begin_outbound_policy(peer_device_id, generation)
            .await?;
        *authenticated_tx.lock().await = Some(crate::sync_client::AuthenticatedPeerSession {
            peer_id: peer_device_id.to_owned(),
            tx: tx.clone(),
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
        let vector = load_version_vector(storage)
            .await
            .map_err(|error| format!("failed to load sync version vector: {error}"))?;
        send_msg(
            tx,
            &LanSyncMessage::VersionVector {
                vector,
                origin_device_id: None,
            },
        );
        Ok(true)
    }
}
