use super::*;

#[uniffi::export]
impl ArkCore {
    pub fn broadcast_change_json(&self, entity_json: String) -> Result<bool> {
        let mut entity: SyncEntity =
            serde_json::from_str(&entity_json).map_err(|_| malformed_json_error("/entity"))?;
        if entity.entity_type != "object" {
            return Err(err(
                "canonical_ingress:invalid_request:canonical_entity_type",
            ));
        }
        if entity.deleted != Some(true) {
            self.with_conn(|conn| {
                let mut data = entity.data.clone();
                data.insert("id".into(), Value::String(entity.id.clone()));
                let input: crate::types::ArkObjectWrite =
                    serde_json::from_value(Value::Object(data)).map_err(|e| err(e.to_string()))?;
                crate::canonical_types::ingress::prepare_object(conn, input)
                    .map_err(ingress_error)?;
                Ok(())
            })?;
        }
        self.runtime.block_on(async {
            let guard = self.sync.lock().await;
            let runtime = guard
                .as_ref()
                .ok_or_else(|| err("sync not running"))?
                .clone_refs();
            drop(guard);

            let hlc = runtime.server.update_entity_hlc(&entity.id).await;
            entity.hlc = hlc;
            runtime
                .storage
                .apply_entity(&entity)
                .await
                .map_err(ArkCoreError::from)?;
            runtime
                .server
                .broadcast_live_change(entity.clone(), None)
                .await;
            let clients = runtime.clients.lock().await;
            for client in clients.values() {
                client.broadcast_live_change(entity.clone()).await;
            }
            drop(clients);
            if let Some(relay) = runtime.relay.as_ref() {
                relay
                    .broadcast_live_change(entity.clone())
                    .map_err(ArkCoreError::from)?;
            }
            Ok(true)
        })
    }

    pub fn get_connected_peers(&self) -> Result<Vec<FfiConnectedPeer>> {
        self.runtime.block_on(async {
            let guard = self.sync.lock().await;
            let runtime = match guard.as_ref() {
                Some(r) => r.clone_refs(),
                None => return Ok(Vec::new()),
            };
            drop(guard);
            let entries = runtime.server.get_connected_peer_entries().await;
            let mut out: Vec<FfiConnectedPeer> = entries
                .into_iter()
                .map(|(id, name)| FfiConnectedPeer {
                    device_id: id,
                    device_name: name,
                })
                .collect();

            // Merge outbound-only connections.
            let clients = runtime.clients.lock().await;
            for client in clients.values() {
                let peer = client.current_peer().await;
                if peer.device_id.is_empty() {
                    continue;
                }
                if !out.iter().any(|e| e.device_id == peer.device_id) {
                    out.push(FfiConnectedPeer {
                        device_id: peer.device_id,
                        device_name: peer.device_name,
                    });
                }
            }
            drop(clients);

            if let Some(relay) = runtime.relay.as_ref() {
                for (device_id, device_name) in relay.get_connected_peer_entries().await {
                    if !out.iter().any(|e| e.device_id == device_id) {
                        out.push(FfiConnectedPeer {
                            device_id,
                            device_name,
                        });
                    }
                }
            }
            Ok(out)
        })
    }

    pub fn add_seed_peer(&self, addresses: Vec<String>) -> Result<bool> {
        self.runtime.block_on(async {
            let guard = self.sync.lock().await;
            let runtime = match guard.as_ref() {
                Some(r) => r.clone_refs(),
                None => return Err(err("sync not running")),
            };
            drop(guard);
            self.spawn_seed_client(
                &runtime.server,
                &runtime.storage,
                &runtime.clients,
                addresses,
                runtime.device_id,
                runtime.device_name,
                runtime.space_id,
                runtime.own_addresses,
                runtime.auth_secret,
                runtime.bind,
            )
            .await;
            Ok(true)
        })
    }
}
