use super::*;

impl RelaySync {
    pub(super) async fn handle_event(&self, event: TransportEvent) {
        match event {
            TransportEvent::MessageReceived {
                from_device_id,
                msg,
            } => {
                if from_device_id.is_empty() || from_device_id == self.config.device_id {
                    return;
                }
                self.handle_message(from_device_id, msg, None, false).await;
            }
            TransportEvent::MessageReceivedFromTransport {
                from_device_id,
                transport_public_key,
                msg,
            } => {
                if let LanSyncMessage::PairingAccepted { device_id } = &msg {
                    let active_attempt = self
                        .outgoing_pairing
                        .lock()
                        .await
                        .as_ref()
                        .is_some_and(|outgoing| outgoing.endpoint == transport_public_key);
                    if !active_attempt || from_device_id != *device_id {
                        eprintln!(
                            "{TAG} ignoring PairingAccepted from an unexpected endpoint or device"
                        );
                        return;
                    }
                    let hello = {
                        let mut pending = self.pending_responder_hellos.lock().await;
                        match pending.get(&transport_public_key) {
                            Some(hello)
                                if crate::protocol::message_origin_device_id(hello)
                                    == Some(device_id.clone()) =>
                            {
                                pending.remove(&transport_public_key)
                            }
                            Some(_) => {
                                eprintln!(
                                    "{TAG} ignoring PairingAccepted for a different device id"
                                );
                                return;
                            }
                            None => {
                                eprintln!("{TAG} ignoring PairingAccepted without a pending Hello");
                                return;
                            }
                        }
                    };
                    let Some(hello) = hello else {
                        return;
                    };
                    let paired_keys =
                        match crate::sync_server::load_peer_transport_keys(&self.storage).await {
                            Ok(keys) => keys,
                            Err(error) => {
                                eprintln!("{TAG} failed to load pairing keys: {error}");
                                return;
                            }
                        };
                    match self
                        .admit_transport_hello(
                            from_device_id.clone(),
                            transport_public_key.clone(),
                            hello,
                            paired_keys,
                            false,
                        )
                        .await
                    {
                        Ok(()) => self.mark_outgoing_accepted(&transport_public_key).await,
                        Err(error) => eprintln!("{TAG} pairing admission failed: {error}"),
                    }
                    return;
                }
                if matches!(&msg, LanSyncMessage::Hello { .. }) {
                    let authorized = match self
                        .storage
                        .authorized_transport_public_key(&from_device_id)
                        .await
                    {
                        Ok(key) => key,
                        Err(error) => {
                            eprintln!(
                                "{TAG} failed to load authorized transport identity: {error}"
                            );
                            return;
                        }
                    };
                    let paired_keys =
                        match crate::sync_server::load_peer_transport_keys(&self.storage).await {
                            Ok(keys) => keys,
                            Err(error) => {
                                eprintln!("{TAG} failed to load pairing keys: {error}");
                                return;
                            }
                        };
                    let known_key = authorized
                        .as_deref()
                        .or_else(|| paired_keys.get(&from_device_id).map(String::as_str));
                    let outgoing_match = self
                        .outgoing_pairing
                        .lock()
                        .await
                        .as_ref()
                        .is_some_and(|o| o.endpoint == transport_public_key);
                    // A stored integration-replication key never yields to a
                    // pairing consent — that trust domain owns the binding.
                    // A stored *pairing* key may rotate under explicit
                    // re-consent: entering that endpoint's code rebinds it.
                    let known_key_matches = known_key == Some(transport_public_key.as_str());
                    let accept = match known_key {
                        Some(key) if key == transport_public_key => true,
                        Some(_) => outgoing_match && authorized.is_none(),
                        None => outgoing_match,
                    };
                    if accept {
                        // Entering a previously removed device's code is
                        // explicit re-consent — unblock it before
                        // handle_message's removed-peer check drops the Hello.
                        if outgoing_match {
                            if let Err(error) =
                                crate::sync_server::unblock_peer_id(&self.storage, &from_device_id)
                                    .await
                            {
                                eprintln!("{TAG} failed to clear peer block: {error}");
                                return;
                            }
                        }
                        let already_authenticated =
                            self.is_authenticated_peer(&from_device_id).await;
                        if outgoing_match && !known_key_matches && !already_authenticated {
                            self.pending_responder_hellos
                                .lock()
                                .await
                                .insert(transport_public_key, msg);
                        } else {
                            if let Err(error) = self
                                .admit_transport_hello(
                                    from_device_id.clone(),
                                    transport_public_key.clone(),
                                    msg,
                                    paired_keys,
                                    false,
                                )
                                .await
                            {
                                eprintln!("{TAG} peer admission failed: {error}");
                                return;
                            }
                            if outgoing_match && already_authenticated {
                                self.mark_outgoing_accepted(&transport_public_key).await;
                            }
                        }
                    } else if known_key.is_some() {
                        eprintln!(
                            "{TAG} rejecting Hello from {from_device_id}: transport key \
                             does not match the stored record"
                        );
                    } else {
                        self.record_pairing_request(from_device_id, transport_public_key, msg)
                            .await;
                    }
                } else if let LanSyncMessage::PairingRejected { device_id } = &msg {
                    // Only the endpoint we are pairing with may decline us —
                    // and its transport identity must match the payload. A
                    // forged id must not let that endpoint erase another
                    // device's pairing state.
                    let matches_outgoing = self
                        .outgoing_pairing
                        .lock()
                        .await
                        .as_ref()
                        .is_some_and(|o| o.endpoint == transport_public_key);
                    if matches_outgoing && from_device_id == *device_id {
                        self.handle_pairing_rejected(device_id, &transport_public_key)
                            .await;
                    } else if matches_outgoing {
                        eprintln!("{TAG} ignoring PairingRejected with unexpected device id");
                    }
                } else if self
                    .transport_keys
                    .lock()
                    .await
                    .get(&from_device_id)
                    .is_some_and(|key| key == &transport_public_key)
                {
                    // Any data frame from the endpoint we dialed means the
                    // responder processed our Hello — it consented.
                    self.mark_outgoing_accepted(&transport_public_key).await;
                    self.handle_message(from_device_id, msg, Some(transport_public_key), false)
                        .await;
                }
            }
            TransportEvent::Connected { .. } => {}
            TransportEvent::Disconnected { device_id } => {
                // Transports fall back to our own id when a remote's id can't
                // be resolved (iroh registry miss) — never evict that.
                if device_id != self.config.device_id {
                    self.peers.lock().await.remove(&device_id);
                    self.transport_keys.lock().await.remove(&device_id);
                    // An initiator that hung up (cancel, Engine stop) takes
                    // its unanswered consent request down with it.
                    if self
                        .pending_pairing
                        .lock()
                        .await
                        .remove(&device_id)
                        .is_some()
                    {
                        self.notify_pairing_changed().await;
                    }
                    let had_pending_responder_hello = {
                        let mut pending = self.pending_responder_hellos.lock().await;
                        let before = pending.len();
                        pending.retain(|_, hello| {
                            crate::protocol::message_origin_device_id(hello).as_deref()
                                != Some(device_id.as_str())
                        });
                        pending.len() != before
                    };
                    if had_pending_responder_hello {
                        self.notify_pairing_changed().await;
                    }
                    if let Some(handler) = self.on_peer_disconnect.lock().await.as_ref() {
                        handler(device_id, self.peers.lock().await.len());
                    }
                }
            }
            TransportEvent::TransportDropped => {
                // Whole transport went down — every peer reached through it is
                // gone. Evict all peer state so reconnect re-announces them.
                let evicted: Vec<String> = {
                    let mut peers = self.peers.lock().await;
                    let evicted = peers.keys().cloned().collect();
                    peers.clear();
                    evicted
                };
                self.transport_keys.lock().await.clear();
                if !self.pending_pairing.lock().await.is_empty() {
                    self.pending_pairing.lock().await.clear();
                    self.notify_pairing_changed().await;
                }
                if !self.pending_responder_hellos.lock().await.is_empty() {
                    self.pending_responder_hellos.lock().await.clear();
                    self.notify_pairing_changed().await;
                }
                if let Some(handler) = self.on_peer_disconnect.lock().await.as_ref() {
                    let mut remaining = evicted.len();
                    for device_id in evicted {
                        remaining -= 1;
                        handler(device_id, remaining);
                    }
                }
            }
        }
        trim_process_heap();
    }
}
