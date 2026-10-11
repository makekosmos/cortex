// RelaySync pairing consent (KOS-369): unknown endpoints park their Hello
// here until a human presses «Принять»/«Отклонить», and the initiator's own
// attempt resolves through `outgoing_pairing` in the snapshot.
impl RelaySync {
    /// Mark `endpoint` as the device whose code this side entered. Its Hello
    /// is pre-consented (entering the code is the consent), and a
    /// `PairingRejected` from it resolves the attempt as declined.
    pub async fn begin_outgoing_pairing(&self, endpoint: String) {
        *self.outgoing_pairing.lock().await = Some(OutgoingPairing {
            endpoint,
            started_at: Instant::now(),
            declined: false,
            accepted: false,
        });
        self.notify_pairing_changed().await;
    }

    /// The responder consented — a post-accept Hello or a data frame from
    /// the endpoint we dialed.
    async fn mark_outgoing_accepted(&self, transport_public_key: &str) {
        let mut guard = self.outgoing_pairing.lock().await;
        match guard.as_mut() {
            Some(o) if o.endpoint == transport_public_key && !o.accepted => {
                o.accepted = true;
                drop(guard);
                self.notify_pairing_changed().await;
            }
            _ => {}
        }
    }

    /// The responder declined: roll back the trust we pre-granted when the
    /// code was entered (in-memory peer state, the persisted endpoint key,
    /// the peer record — none of it outlives a «Отклонить»), then close the
    /// connection and stop the dial loop from resurrecting it.
    async fn handle_pairing_rejected(&self, responder_device_id: &str, endpoint: &str) {
        if let Some(outgoing) = self.outgoing_pairing.lock().await.as_mut() {
            outgoing.declined = true;
        }
        self.peers.lock().await.remove(responder_device_id);
        self.transport_keys.lock().await.remove(responder_device_id);
        let mut keys = crate::sync_server::load_peer_transport_keys(&self.storage).await;
        if keys.remove(responder_device_id).is_some() {
            crate::sync_server::save_peer_transport_keys(&self.storage, &keys).await;
        }
        crate::sync_server::remove_peer_record(&self.storage, responder_device_id).await;
        let _ = self.transport.disconnect_transport_peer(endpoint);
        if let Some(handler) = self.on_peer_disconnect.lock().await.as_ref() {
            handler(
                responder_device_id.to_string(),
                self.peers.lock().await.len(),
            );
        }
        self.notify_pairing_changed().await;
    }

    /// Park an unknown endpoint's Hello for explicit consent. A device id
    /// declined recently is dropped silently so a retrying old-version peer
    /// cannot resurrect the prompt on every backoff tick.
    async fn record_pairing_request(
        &self,
        from_device_id: String,
        transport_public_key: String,
        hello: LanSyncMessage,
    ) {
        {
            let mut declined = self.declined_pairing.lock().await;
            match declined.get(&from_device_id) {
                Some(since) if since.elapsed() < PAIRING_DECLINE_SUPPRESS => {
                    eprintln!("{TAG} dropping Hello from {from_device_id}: pairing was declined");
                    return;
                }
                Some(_) => {
                    declined.remove(&from_device_id);
                }
                None => {}
            }
        }
        let LanSyncMessage::Hello {
            device_name,
            platform,
            app_version,
            ..
        } = &hello
        else {
            return;
        };
        eprintln!("{TAG} pairing request from {from_device_id} ({device_name})");
        self.pending_pairing.lock().await.insert(
            from_device_id,
            PendingPairingRequest {
                device_name: device_name.clone(),
                platform: platform.clone(),
                app_version: app_version.clone(),
                transport_public_key,
                hello,
            },
        );
        self.notify_pairing_changed().await;
    }

    /// Apply a consented Hello: bind the endpoint so the frames we send from
    /// `handle_message` (our Hello, the version vector) actually leave, then
    /// persist the binding on success. On a failed handshake (HMAC,
    /// protocol mismatch, removed list) the binding is rolled back —
    /// transport trust must not outlive authentication.
    async fn admit_transport_hello(
        &self,
        from_device_id: String,
        transport_public_key: String,
        msg: LanSyncMessage,
        mut paired_keys: HashMap<String, String>,
    ) {
        let _ = self
            .transport
            .bind_authenticated_peer(&from_device_id, &transport_public_key);
        self.transport_keys
            .lock()
            .await
            .insert(from_device_id.clone(), transport_public_key.clone());
        self.handle_message(from_device_id.clone(), msg, None).await;
        if self.is_authenticated_peer(&from_device_id).await {
            if paired_keys.get(&from_device_id) != Some(&transport_public_key) {
                paired_keys.insert(from_device_id, transport_public_key);
                crate::sync_server::save_peer_transport_keys(&self.storage, &paired_keys).await;
            }
        } else {
            let _ = self.transport.unbind_authenticated_peer(&from_device_id);
            self.transport_keys.lock().await.remove(&from_device_id);
        }
    }

    /// Consent requests currently awaiting a human decision — shown in the
    /// snapshot so the Manager can render «Принять / Отклонить».
    pub async fn pending_pairing_requests(&self) -> Vec<PeerEntry> {
        self.pending_pairing
            .lock()
            .await
            .iter()
            .map(|(device_id, request)| PeerEntry {
                device_id: device_id.clone(),
                device_name: request.device_name.clone(),
                platform: request.platform.clone(),
                app_version: request.app_version.clone(),
            })
            .collect()
    }

    /// Outbound pairing attempt for the snapshot: `pending` while waiting on
    /// the other side's human, `declined` on an explicit `PairingRejected`,
    /// `connected` once the ticketed endpoint authenticated.
    pub async fn outgoing_pairing_status(&self) -> Option<serde_json::Value> {
        let outgoing = self.outgoing_pairing.lock().await.clone()?;
        let endpoint = outgoing.endpoint.clone();
        let connected = self.authenticated_peer_for_transport_key(&endpoint).await;
        let status = if outgoing.accepted {
            "connected"
        } else if outgoing.declined {
            "declined"
        } else {
            "pending"
        };
        Some(serde_json::json!({
            "status": status,
            "endpoint": endpoint,
            "device_id": connected.as_ref().map(|peer| peer.device_id.clone()),
            "device_name": connected.map(|peer| peer.device_name),
            "expires_in_ms": PAIRING_DECISION_TIMEOUT
                .checked_sub(outgoing.started_at.elapsed())
                .unwrap_or_default()
                .as_millis() as u64,
        }))
    }

    /// «Принять»: replay the parked Hello through the normal handshake and
    /// persist the pairing exactly like a live accept.
    pub async fn accept_pairing(&self, device_id: &str) -> Result<PeerEntry, String> {
        let request = self
            .pending_pairing
            .lock()
            .await
            .remove(device_id)
            .ok_or_else(|| format!("no pending pairing request for {device_id}"))?;
        // Saying «Принять» to a previously removed device is explicit
        // re-consent — clear the removed flag before handle_message drops it.
        crate::sync_server::unblock_peer_id(&self.storage, device_id).await;
        let paired_keys = crate::sync_server::load_peer_transport_keys(&self.storage).await;
        self.admit_transport_hello(
            device_id.to_string(),
            request.transport_public_key,
            request.hello,
            paired_keys,
        )
        .await;
        self.notify_pairing_changed().await;
        let peers = self.peers.lock().await;
        peers
            .get(device_id)
            .filter(|peer| peer.authenticated)
            .map(|peer| PeerEntry {
                device_id: device_id.to_string(),
                device_name: peer.device_name.clone(),
                platform: peer.platform.clone(),
                app_version: peer.app_version.clone(),
            })
            .ok_or_else(|| format!("pairing handshake with {device_id} did not authenticate"))
    }

    /// «Отклонить»: answer `PairingRejected` over the wire, close the
    /// connection, and persist nothing.
    pub async fn decline_pairing(&self, device_id: &str) -> Result<(), String> {
        let request = self
            .pending_pairing
            .lock()
            .await
            .remove(device_id)
            .ok_or_else(|| format!("no pending pairing request for {device_id}"))?;
        self.declined_pairing
            .lock()
            .await
            .insert(device_id.to_string(), Instant::now());
        let _ = self
            .transport
            .send_to_transport_peer(
                &request.transport_public_key,
                LanSyncMessage::PairingRejected {
                    device_id: self.config.device_id.clone(),
                },
            )
            .await;
        // `conn.close` aborts unacknowledged stream data — give the
        // rejection frame a beat to flush before severing, or an old peer
        // that can't parse it still gets its connection dropped.
        let transport = self.transport.clone();
        let endpoint = request.transport_public_key.clone();
        self.tasks.lock().await.push(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let _ = transport.disconnect_transport_peer(&endpoint);
        }));
        self.notify_pairing_changed().await;
        Ok(())
    }

    /// «Отмена»/dismiss on the initiator: drop the attempt. While pending it
    /// also cuts the connection and stops the dial loop, which removes the
    /// responder's prompt via its Disconnected event; once connected or
    /// declined the entry is just cleared.
    pub async fn cancel_outgoing_pairing(&self) {
        let Some(outgoing) = self.outgoing_pairing.lock().await.take() else {
            return;
        };
        if !outgoing.accepted {
            let _ = self.transport.disconnect_transport_peer(&outgoing.endpoint);
        }
        self.notify_pairing_changed().await;
    }
}
