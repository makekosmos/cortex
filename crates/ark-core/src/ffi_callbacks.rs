use super::*;

impl ArkCore {
    pub(super) async fn install_server_callbacks(&self, server: &Arc<SyncServer>) {
        let listener = self.listener.read().await.clone();
        let listener_for_change = listener.clone();
        server
            .set_on_change(Arc::new(move |entity| {
                if let Some(l) = listener_for_change.as_ref() {
                    if let Ok(json) = serde_json::to_string(&entity) {
                        l.on_entity_changed(json);
                    }
                }
            }))
            .await;
        let listener_for_connect = listener.clone();
        let server_for_lookup = server.clone();
        server
            .set_on_peer_connect(Arc::new(move |device_id| {
                let listener = listener_for_connect.clone();
                let server = server_for_lookup.clone();
                tokio::spawn(async move {
                    if let Some(l) = listener.as_ref() {
                        let entries = server.get_connected_peer_entries().await;
                        let name = entries
                            .into_iter()
                            .find(|(id, _)| id == &device_id)
                            .map(|(_, n)| n)
                            .unwrap_or_default();
                        l.on_peer_connected(device_id, name);
                    }
                });
            }))
            .await;
        let listener_for_disconnect = listener;
        server
            .set_on_peer_disconnect(Arc::new(move |device_id, remaining| {
                if let Some(l) = listener_for_disconnect.as_ref() {
                    l.on_peer_disconnected(device_id, remaining as u32);
                }
            }))
            .await;
    }

    pub(super) async fn install_relay_callbacks(&self, relay: &Arc<RelaySync>) {
        let listener = self.listener.read().await.clone();
        let listener_for_change = listener.clone();
        relay
            .set_on_change(Arc::new(move |entity| {
                if let Some(l) = listener_for_change.as_ref() {
                    if let Ok(json) = serde_json::to_string(&entity) {
                        l.on_entity_changed(json);
                    }
                }
            }))
            .await;

        let listener_for_connect = listener.clone();
        relay
            .set_on_peer_connect(Arc::new(move |device_id| {
                if let Some(l) = listener_for_connect.as_ref() {
                    l.on_peer_connected(device_id, String::new());
                }
            }))
            .await;

        let listener_for_disconnect = listener;
        relay
            .set_on_peer_disconnect(Arc::new(move |device_id, remaining| {
                if let Some(l) = listener_for_disconnect.as_ref() {
                    l.on_peer_disconnected(device_id, remaining as u32);
                }
            }))
            .await;
    }
}
