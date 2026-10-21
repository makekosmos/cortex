impl RelaySync {
    fn disconnect_iroh_transport_endpoint(&self, endpoint: &str) {
        if let Some(session) = self.transport.session() {
            let _ = session.disconnect_transport_peer(endpoint);
        }
    }

    fn disconnect_iroh_peer(&self, device_id: &str) {
        if let Some(session) = self.transport.session() {
            let _ = session.disconnect_peer(device_id);
        }
    }

    /// Evict a peer before fallible persistence so failed writes cannot keep it live.
    pub async fn disconnect_peer(
        &self,
        device_id: &str,
    ) -> Result<crate::sync_transport::PeerDisconnectOutcome, String> {
        if let Some(registry) = &self.registry {
            registry.disconnect_peer(device_id).await;
        }
        self.transport_keys.lock().await.remove(device_id);
        if self
            .pending_pairing
            .lock()
            .await
            .remove(device_id)
            .is_some()
        {
            self.notify_pairing_changed().await;
        }
        let was_connected = self.peers.lock().await.remove(device_id).is_some();
        let transport_result = match self.transport.session() {
            Some(session) => session
                .disconnect_peer(device_id)
                .map_err(|error| format!("failed to disconnect transport peer: {error}")),
            None => Ok(crate::sync_transport::PeerDisconnectOutcome::SharedRelayUnaffected),
        };
        let persistence_result = async {
            let mut keys = crate::sync_server::load_peer_transport_keys(&self.storage).await?;
            if keys.remove(device_id).is_some() {
                crate::sync_server::save_peer_transport_keys(&self.storage, &keys).await?;
            }
            Ok::<(), String>(())
        }
        .await;
        if was_connected {
            if let Some(handler) = self.on_peer_disconnect.lock().await.as_ref() {
                handler(device_id.to_string(), self.peers.lock().await.len());
            }
        }
        match (transport_result, persistence_result) {
            (Ok(outcome), Ok(())) => Ok(outcome),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(first), Err(second)) => Err(format!("{first}; {second}")),
        }
    }
}
