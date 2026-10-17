use ark_domain::device_registry::{RegistryEvent, RegistryOperation};

use super::{
    state::{Admission, RelayRegistryPeer},
    RegistryOutcome, RelayRegistryCoordinator,
};
use crate::protocol::LanSyncMessage;
use ark_application::registry_admission::RegistryAdmissionStorage;

impl<S: RegistryAdmissionStorage + ?Sized + 'static> RelayRegistryCoordinator<S> {
    pub(super) async fn publish_delta_inner(
        &self,
        events: &[RegistryEvent],
        disabled: &[String],
        origin: Option<&str>,
        connected: &[(String, String)],
    ) -> RegistryOutcome {
        let mut outcome = RegistryOutcome::default();
        if events.is_empty() {
            return outcome;
        }
        let local_disabled = disabled.iter().any(|id| id == &self.local_device_id);
        let revoked = {
            let mut peers = self.peers.lock().await;
            let mut ids = if local_disabled {
                let mut ids = connected
                    .iter()
                    .map(|(id, _)| id.clone())
                    .collect::<Vec<_>>();
                ids.extend(peers.keys().cloned());
                ids
            } else {
                disabled
                    .iter()
                    .filter(|id| {
                        peers.contains_key(*id)
                            || connected
                                .iter()
                                .any(|(connected_id, _)| connected_id == *id)
                    })
                    .cloned()
                    .collect()
            };
            ids.sort();
            ids.dedup();
            ids.into_iter()
                .map(|id| {
                    let bound_endpoint = connected
                        .iter()
                        .find(|(connected_id, _)| connected_id == &id)
                        .map(|(_, endpoint)| endpoint.clone());
                    let (endpoint, session) = match peers.get_mut(&id) {
                        Some(peer) => {
                            let endpoint = peer.endpoint.clone();
                            let session = peer.session_id().map(str::to_owned);
                            peer.admission = Admission::Revoked;
                            (endpoint, session)
                        }
                        None => {
                            let endpoint = bound_endpoint.unwrap_or_default();
                            if !endpoint.is_empty() {
                                peers.insert(
                                    id.clone(),
                                    RelayRegistryPeer {
                                        endpoint: endpoint.clone(),
                                        policy_negotiated: false,
                                        admission: Admission::Revoked,
                                    },
                                );
                            }
                            (endpoint, None)
                        }
                    };
                    (id, endpoint, session)
                })
                .collect::<Vec<_>>()
        };
        outcome.revoked_peer_ids = revoked.iter().map(|(id, _, _)| id.clone()).collect();

        for (id, _, _) in &revoked {
            if let Err(error) = self.transport.unbind_peer(id) {
                outcome.set_error(format!(
                    "failed to revoke registry data gate for {id}: {error}"
                ));
            }
        }

        if !local_disabled {
            let recipients = {
                let peers = self.peers.lock().await;
                peers
                    .iter()
                    .filter_map(|(id, peer)| match &peer.admission {
                        Admission::Ready { session_id } if Some(id.as_str()) != origin => {
                            Some((id.clone(), peer.endpoint.clone(), session_id.clone()))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            for (id, endpoint, session) in recipients {
                for event in events {
                    if !self.session_is_ready(&id, &endpoint, &session).await {
                        break;
                    }
                    if let Err(error) = self
                        .send(
                            &endpoint,
                            LanSyncMessage::RegistryEventUpdate {
                                session_id: session.clone(),
                                event: event.clone(),
                            },
                        )
                        .await
                    {
                        outcome.set_error(format!(
                            "failed to fan out registry event to {id}: {error}"
                        ));
                    }
                }
            }
        }

        for (id, endpoint, session) in revoked {
            if !self.is_revocation_current(&id, &endpoint).await {
                continue;
            }
            if endpoint.is_empty() {
                outcome.set_error(format!("missing connected endpoint for revoked peer {id}"));
                self.peers.lock().await.remove(&id);
                continue;
            }
            if let Some(session) = session {
                for event in events.iter().filter(|event| {
                    Some(id.as_str()) != origin
                        && event.target_device_id().as_str()
                            == if local_disabled {
                                self.local_device_id.as_str()
                            } else {
                                id.as_str()
                            }
                        && matches!(event.operation(), RegistryOperation::Disable)
                }) {
                    if !self.is_revocation_current(&id, &endpoint).await {
                        break;
                    }
                    if let Err(error) = self
                        .send(
                            &endpoint,
                            LanSyncMessage::RegistryEventUpdate {
                                session_id: session.clone(),
                                event: event.clone(),
                            },
                        )
                        .await
                    {
                        outcome.set_error(format!("failed to send disable event to {id}: {error}"));
                    }
                }
            }
            if !self.is_revocation_current(&id, &endpoint).await {
                continue;
            }
            if let Err(error) = self.transport.disconnect_transport_peer(&endpoint) {
                outcome.set_error(format!("failed to close revoked peer {id}: {error}"));
            }
            self.remove_revoked_peer(&id, &endpoint).await;
        }
        outcome
    }

    async fn session_is_ready(&self, peer: &str, endpoint: &str, session: &str) -> bool {
        self.peers
            .lock()
            .await
            .get(peer)
            .is_some_and(|entry| entry.is_ready_on(endpoint, Some(session)))
    }

    pub(super) async fn is_ready_transport(&self, peer: &str, endpoint: &str) -> bool {
        self.peers
            .lock()
            .await
            .get(peer)
            .is_some_and(|entry| entry.is_ready_on(endpoint, None))
    }

    pub(super) async fn revoke_peer(&self, peer: &str, endpoint: &str) -> RegistryOutcome {
        let mut outcome = RegistryOutcome::default();
        {
            let mut peers = self.peers.lock().await;
            match peers.get_mut(peer) {
                Some(entry) if entry.endpoint == endpoint => {
                    entry.admission = Admission::Revoked;
                }
                Some(_) => return outcome,
                None => {
                    peers.insert(
                        peer.to_owned(),
                        RelayRegistryPeer {
                            endpoint: endpoint.to_owned(),
                            policy_negotiated: false,
                            admission: Admission::Revoked,
                        },
                    );
                }
            }
        }
        outcome.revoked_peer_ids.push(peer.to_owned());
        if let Err(error) = self.transport.unbind_peer(peer) {
            outcome.set_error(format!(
                "failed to revoke registry data gate for {peer}: {error}"
            ));
        }
        if self.is_revocation_current(peer, endpoint).await {
            if let Err(error) = self.transport.disconnect_transport_peer(endpoint) {
                outcome.set_error(format!("failed to close revoked peer {peer}: {error}"));
            }
            self.remove_revoked_peer(peer, endpoint).await;
        }
        outcome
    }

    async fn is_revocation_current(&self, peer: &str, endpoint: &str) -> bool {
        self.peers.lock().await.get(peer).is_some_and(|entry| {
            entry.endpoint == endpoint && matches!(entry.admission, Admission::Revoked)
        })
    }

    async fn remove_revoked_peer(&self, peer: &str, endpoint: &str) {
        let mut peers = self.peers.lock().await;
        if peers.get(peer).is_some_and(|entry| {
            entry.endpoint == endpoint && matches!(entry.admission, Admission::Revoked)
        }) {
            peers.remove(peer);
        }
    }
}

impl RegistryOutcome {
    pub(super) fn merge(&mut self, other: Self) {
        self.revoked_peer_ids.extend(other.revoked_peer_ids);
        self.set_error_opt(other.operation_error);
    }

    pub(super) fn set_error(&mut self, error: impl ToString) {
        self.operation_error
            .get_or_insert_with(|| error.to_string());
    }

    fn set_error_opt(&mut self, error: Option<String>) {
        if let Some(error) = error {
            self.set_error(error);
        }
    }
}
