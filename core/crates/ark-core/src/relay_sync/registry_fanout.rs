use super::*;

impl RelaySync {
    pub(super) async fn registry_ready_for_peer(
        &self,
        peer_device_id: &str,
        transport_public_key: Option<&str>,
    ) -> bool {
        let Some(endpoint) = transport_public_key else {
            return false;
        };
        self.peers
            .lock()
            .await
            .get(peer_device_id)
            .is_some_and(|peer| {
                matches!(
                    &peer.registry_admission,
                    RelayRegistryAdmission::Ready { transport_public_key, .. }
                        if transport_public_key == endpoint
                )
            })
    }

    pub(crate) async fn publish_registry_event(
        &self,
        event: crate::domain::device_registry::RegistryEvent,
    ) -> Result<(), String> {
        let disabled = matches!(
            event.operation(),
            crate::domain::device_registry::RegistryOperation::Disable
        )
        .then(|| event.target_device_id().as_str().to_owned())
        .into_iter()
        .collect::<Vec<_>>();
        self.publish_registry_delta(&[event], &disabled, None).await
    }

    pub(super) async fn handle_registry_event_update(
        &self,
        peer_device_id: &str,
        transport_public_key: &str,
        session_id: &str,
        event: crate::domain::device_registry::RegistryEvent,
    ) -> Result<(), String> {
        let disables_local_device = matches!(
            event.operation(),
            crate::domain::device_registry::RegistryOperation::Disable
        ) && event.target_device_id().as_str() == self.config.device_id;
        let session_is_current = self
            .peers
            .lock()
            .await
            .get(peer_device_id)
            .is_some_and(|peer| {
                matches!(
                    &peer.registry_admission,
                    RelayRegistryAdmission::Ready {
                        session_id: current_session,
                        transport_public_key: current_endpoint,
                    } if current_session == session_id && current_endpoint == transport_public_key
                ) || matches!(
                    &peer.registry_admission,
                    RelayRegistryAdmission::Exchanging(state)
                        if disables_local_device
                            && state.session_id == session_id
                            && state.transport_public_key == transport_public_key
                )
            });
        if !session_is_current
            || !self
                .transport_key_matches(peer_device_id, transport_public_key)
                .await
        {
            return Err("registry_sync_session_mismatch".into());
        }

        let applied = self
            .storage
            .apply_device_registry_events(peer_device_id, std::slice::from_ref(&event))
            .await?;
        self.publish_registry_delta(
            &applied.applied_events,
            &applied.disabled_device_ids,
            Some(peer_device_id),
        )
        .await?;
        if !applied.sender_is_admitted {
            return Err("registry_sync_peer_disabled".into());
        }
        if !applied.local_device_is_admitted {
            return Err("registry_sync_local_device_disabled".into());
        }
        Ok(())
    }

    pub(super) async fn publish_registry_delta(
        &self,
        events: &[crate::domain::device_registry::RegistryEvent],
        disabled_device_ids: &[String],
        origin_device_id: Option<&str>,
    ) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }
        let local_device_disabled = disabled_device_ids
            .iter()
            .any(|device_id| device_id == &self.config.device_id);
        let transport_keys = self.transport_keys.lock().await.clone();
        let revoked = {
            let mut peers = self.peers.lock().await;
            let mut revoked = Vec::new();
            let device_ids = if local_device_disabled {
                peers.keys().cloned().collect::<Vec<_>>()
            } else {
                disabled_device_ids.to_vec()
            };
            for device_id in &device_ids {
                let Some(peer) = peers.get_mut(device_id) else {
                    continue;
                };
                let admission = std::mem::replace(
                    &mut peer.registry_admission,
                    RelayRegistryAdmission::Revoked,
                );
                let (session_id, transport_public_key) = match admission {
                    RelayRegistryAdmission::Ready {
                        session_id,
                        transport_public_key,
                    } => (Some(session_id), transport_public_key),
                    RelayRegistryAdmission::Exchanging(state) => {
                        (Some(state.session_id), state.transport_public_key)
                    }
                    RelayRegistryAdmission::Waiting | RelayRegistryAdmission::Revoked => (
                        None,
                        transport_keys.get(device_id).cloned().unwrap_or_default(),
                    ),
                };
                let control_target = if local_device_disabled {
                    self.config.device_id.as_str()
                } else {
                    device_id.as_str()
                };
                let control_events: Vec<crate::domain::device_registry::RegistryEvent> = events
                    .iter()
                    .filter(|event| {
                        event.target_device_id().as_str() == control_target
                            && matches!(
                                event.operation(),
                                crate::domain::device_registry::RegistryOperation::Disable
                            )
                    })
                    .cloned()
                    .collect();
                revoked.push((
                    device_id.clone(),
                    session_id,
                    transport_public_key,
                    control_events,
                ));
            }
            revoked
        };
        // Remove Iroh's queued-user-data authority before the first network await.
        let mut failure = None;
        for (device_id, _, _, _) in &revoked {
            if let Err(error) = self.transport.unbind_peer(device_id) {
                failure.get_or_insert_with(|| {
                    format!("failed to revoke registry data gate for {device_id}: {error}")
                });
            }
        }

        let recipients = {
            let peers = self.peers.lock().await;
            if local_device_disabled {
                Vec::new()
            } else {
                peers
                    .iter()
                    .filter_map(|(device_id, peer)| {
                        if Some(device_id.as_str()) == origin_device_id {
                            return None;
                        }
                        match &peer.registry_admission {
                            RelayRegistryAdmission::Ready {
                                session_id,
                                transport_public_key,
                            } => Some((
                                device_id.clone(),
                                session_id.clone(),
                                transport_public_key.clone(),
                            )),
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>()
            }
        };
        for (device_id, session_id, endpoint) in recipients {
            for event in events {
                let still_current = self.peers.lock().await.get(&device_id).is_some_and(|peer| {
                    matches!(
                        &peer.registry_admission,
                        RelayRegistryAdmission::Ready {
                            session_id: current_session,
                            transport_public_key: current_endpoint,
                        } if current_session == &session_id && current_endpoint == &endpoint
                    )
                });
                if !still_current {
                    break;
                }
                if let Err(error) = self
                    .send_registry_control(
                        &endpoint,
                        LanSyncMessage::RegistryEventUpdate {
                            session_id: session_id.clone(),
                            event: event.clone(),
                        },
                    )
                    .await
                {
                    failure.get_or_insert_with(|| {
                        format!("failed to fan out registry event to {device_id}: {error}")
                    });
                }
            }
        }
        for (device_id, session_id, endpoint, control_events) in revoked {
            if let Some(session_id) = session_id {
                for event in control_events {
                    if let Err(error) = self
                        .send_registry_control(
                            &endpoint,
                            LanSyncMessage::RegistryEventUpdate {
                                session_id: session_id.clone(),
                                event: event.clone(),
                            },
                        )
                        .await
                    {
                        failure.get_or_insert_with(|| {
                            format!("failed to send disable event to {device_id}: {error}")
                        });
                    }
                }
                if let Err(error) = self.transport.disconnect_transport_peer(&endpoint) {
                    failure.get_or_insert_with(|| {
                        format!("failed to close disabled peer {device_id}: {error}")
                    });
                }
                self.peers.lock().await.remove(&device_id);
            } else if !endpoint.is_empty() {
                if let Err(error) = self.transport.disconnect_transport_peer(&endpoint) {
                    failure.get_or_insert_with(|| {
                        format!("failed to close revoked peer {device_id}: {error}")
                    });
                }
                self.peers.lock().await.remove(&device_id);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
#[path = "registry_fanout_tests.rs"]
mod tests;
