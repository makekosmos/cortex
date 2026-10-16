use crate::domain::device_registry::{
    DeviceId, RegistryHandshakeProof, RegistryHandshakeTranscript, RegistryHandshakeTranscriptDraft,
};
use crate::protocol::{
    generate_auth_nonce, generate_id, LanSyncMessage, NegotiatedProtocol, ProtocolCapability,
};
use crate::replication_storage::SyncRuntimeStorage;

const PAGE_SIZE: usize = 32;
const MAX_PAGE_COUNT: u32 = 4096;

#[cfg(test)]
#[path = "sync_registry_admission_tests.rs"]
mod tests;

#[derive(Clone)]
enum AdmissionPhase {
    WaitingForStart,
    WaitingForProof {
        initiator_nonce: String,
    },
    WaitingForConfirm {
        transcript: RegistryHandshakeTranscript,
    },
    Exchanging {
        incoming_page_count: Option<u32>,
        next_page_index: u32,
        local_digest: Option<String>,
        remote_digest: Option<String>,
        start_received: bool,
    },
    Ready,
}

pub(crate) struct DirectRegistryAdmission {
    local_device_id: DeviceId,
    peer_device_id: DeviceId,
    space_id: String,
    protocol: NegotiatedProtocol,
    session_id: Option<String>,
    phase: AdmissionPhase,
    session_revoked: bool,
}

pub(crate) struct AdmissionUpdate {
    pub(crate) outgoing: Vec<LanSyncMessage>,
    pub(crate) became_ready: bool,
    pub(crate) exchange_complete: bool,
    pub(crate) close_session: bool,
    pub(crate) local_device_is_admitted: bool,
    pub(crate) disabled_device_ids: Vec<String>,
    pub(crate) applied_events: Vec<crate::domain::device_registry::RegistryEvent>,
}

impl DirectRegistryAdmission {
    pub(crate) fn responder(
        local_device_id: &str,
        peer_device_id: &str,
        space_id: &str,
        protocol: NegotiatedProtocol,
    ) -> Result<Self, String> {
        Self::new(
            local_device_id,
            peer_device_id,
            space_id,
            protocol,
            AdmissionPhase::WaitingForStart,
        )
    }

    pub(crate) fn initiator(
        local_device_id: &str,
        peer_device_id: &str,
        space_id: &str,
        protocol: NegotiatedProtocol,
    ) -> Result<(Self, LanSyncMessage), String> {
        let mut admission = Self::new(
            local_device_id,
            peer_device_id,
            space_id,
            protocol,
            AdmissionPhase::WaitingForStart,
        )?;
        let session_id = generate_id();
        let initiator_nonce = generate_auth_nonce();
        let message = LanSyncMessage::RegistryAdmissionStart {
            session_id: session_id.clone(),
            initiator_device_id: admission.local_device_id.as_str().to_owned(),
            responder_device_id: admission.peer_device_id.as_str().to_owned(),
            initiator_nonce: initiator_nonce.clone(),
            protocol_version: admission.protocol.version,
            negotiated_capabilities: admission.capability_ids(),
        };
        admission.session_id = Some(session_id);
        admission.phase = AdmissionPhase::WaitingForProof { initiator_nonce };
        Ok((admission, message))
    }

    pub(crate) fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    fn new(
        local_device_id: &str,
        peer_device_id: &str,
        space_id: &str,
        protocol: NegotiatedProtocol,
        phase: AdmissionPhase,
    ) -> Result<Self, String> {
        if !protocol
            .capabilities
            .contains(&ProtocolCapability::RegistryHistoryV1)
        {
            return Err("registry_admission_capability_missing".into());
        }
        let local_device_id =
            DeviceId::parse(local_device_id.to_owned()).map_err(|error| error.to_string())?;
        let peer_device_id =
            DeviceId::parse(peer_device_id.to_owned()).map_err(|error| error.to_string())?;
        if local_device_id == peer_device_id || space_id.is_empty() {
            return Err("registry_admission_identity_invalid".into());
        }
        Ok(Self {
            local_device_id,
            peer_device_id,
            space_id: space_id.to_owned(),
            protocol,
            session_id: None,
            phase,
            session_revoked: false,
        })
    }

    pub(crate) async fn handle(
        &mut self,
        storage: &dyn SyncRuntimeStorage,
        message: LanSyncMessage,
    ) -> Result<AdmissionUpdate, String> {
        let was_ready = matches!(self.phase, AdmissionPhase::Ready);
        let mut disabled_device_ids = Vec::new();
        let mut applied_events = Vec::new();
        let mut close_session = false;
        let mut local_device_is_admitted = true;
        let outgoing = match (&self.phase, message) {
            (
                AdmissionPhase::WaitingForStart,
                LanSyncMessage::RegistryAdmissionStart {
                    session_id,
                    initiator_device_id,
                    responder_device_id,
                    initiator_nonce,
                    protocol_version,
                    negotiated_capabilities,
                },
            ) if initiator_device_id == self.peer_device_id.as_str()
                && responder_device_id == self.local_device_id.as_str()
                && protocol_version == self.protocol.version
                && negotiated_capabilities == self.capability_ids() =>
            {
                let responder_nonce = generate_auth_nonce();
                let transcript =
                    self.transcript(&session_id, &initiator_nonce, &responder_nonce)?;
                let proof = storage
                    .sign_registry_handshake(self.local_device_id.as_str(), &transcript)
                    .await?;
                self.session_id = Some(session_id.clone());
                self.phase = AdmissionPhase::WaitingForConfirm { transcript };
                vec![LanSyncMessage::RegistryAdmissionProof {
                    session_id,
                    responder_nonce,
                    signer_device_id: proof.signer_device_id().as_str().to_owned(),
                    signature: proof.signature().to_owned(),
                }]
            }
            (
                AdmissionPhase::WaitingForProof { initiator_nonce },
                LanSyncMessage::RegistryAdmissionProof {
                    session_id,
                    responder_nonce,
                    signer_device_id,
                    signature,
                },
            ) if self.session_id.as_deref() == Some(session_id.as_str())
                && signer_device_id == self.peer_device_id.as_str() =>
            {
                let transcript = self.transcript(&session_id, initiator_nonce, &responder_nonce)?;
                let proof = RegistryHandshakeProof::from_wire(signer_device_id, signature)
                    .map_err(|error| error.to_string())?;
                storage
                    .verify_registry_handshake(self.peer_device_id.as_str(), &transcript, &proof)
                    .await?;
                let local_proof = storage
                    .sign_registry_handshake(self.local_device_id.as_str(), &transcript)
                    .await?;
                self.phase = AdmissionPhase::Exchanging {
                    incoming_page_count: None,
                    next_page_index: 0,
                    local_digest: None,
                    remote_digest: None,
                    start_received: false,
                };
                let mut messages = vec![LanSyncMessage::RegistryAdmissionConfirm {
                    session_id: session_id.clone(),
                    signer_device_id: local_proof.signer_device_id().as_str().to_owned(),
                    signature: local_proof.signature().to_owned(),
                }];
                messages.extend(self.begin_exchange(storage, &session_id).await?);
                messages
            }
            (
                AdmissionPhase::WaitingForConfirm { transcript },
                LanSyncMessage::RegistryAdmissionConfirm {
                    session_id,
                    signer_device_id,
                    signature,
                },
            ) if self.session_id.as_deref() == Some(session_id.as_str())
                && signer_device_id == self.peer_device_id.as_str() =>
            {
                let proof = RegistryHandshakeProof::from_wire(signer_device_id, signature)
                    .map_err(|error| error.to_string())?;
                storage
                    .verify_registry_handshake(self.peer_device_id.as_str(), transcript, &proof)
                    .await?;
                self.phase = AdmissionPhase::Exchanging {
                    incoming_page_count: None,
                    next_page_index: 0,
                    local_digest: None,
                    remote_digest: None,
                    start_received: false,
                };
                self.begin_exchange(storage, &session_id).await?
            }
            (
                AdmissionPhase::Exchanging { start_received, .. },
                LanSyncMessage::RegistrySyncStart { session_id },
            ) if self.session_id.as_deref() == Some(session_id.as_str())
                && self.local_device_id > self.peer_device_id =>
            {
                if *start_received {
                    return Err("registry_sync_start_replayed".into());
                }
                if let AdmissionPhase::Exchanging { start_received, .. } = &mut self.phase {
                    *start_received = true;
                }
                self.load_pages(storage, &session_id).await?
            }
            (
                AdmissionPhase::Exchanging {
                    incoming_page_count,
                    next_page_index,
                    start_received,
                    ..
                },
                LanSyncMessage::RegistryEventsPage {
                    session_id,
                    page_index,
                    page_count,
                    events,
                },
            ) if self.session_id.as_deref() == Some(session_id.as_str()) => {
                if self.local_device_id > self.peer_device_id && !start_received {
                    return Err("registry_sync_page_without_start".into());
                }
                if page_count == 0
                    || page_count > MAX_PAGE_COUNT
                    || events.len() > PAGE_SIZE
                    || page_index >= page_count
                    || page_index != *next_page_index
                    || incoming_page_count.is_some_and(|count| count != page_count)
                    || (page_index + 1 < page_count && events.len() != PAGE_SIZE)
                {
                    return Err("registry_sync_page_bounds_or_order".into());
                }
                let result = storage
                    .apply_device_registry_events(self.peer_device_id.as_str(), &events)
                    .await?;
                applied_events.extend(result.applied_events);
                close_session |= result
                    .disabled_device_ids
                    .iter()
                    .any(|device_id| device_id == self.local_device_id.as_str());
                close_session |= !result.sender_is_admitted;
                close_session |= !result.local_device_is_admitted;
                local_device_is_admitted = result.local_device_is_admitted;
                disabled_device_ids.extend(result.disabled_device_ids);
                let is_last = page_index + 1 == page_count;
                if let AdmissionPhase::Exchanging {
                    incoming_page_count,
                    next_page_index,
                    local_digest,
                    ..
                } = &mut self.phase
                {
                    *incoming_page_count = Some(page_count);
                    *next_page_index += 1;
                    if is_last {
                        *local_digest = Some(result.digest.clone());
                    }
                }
                if is_last {
                    vec![LanSyncMessage::RegistrySyncReady {
                        session_id,
                        digest: result.digest,
                    }]
                } else {
                    Vec::new()
                }
            }
            (
                AdmissionPhase::Exchanging { remote_digest, .. },
                LanSyncMessage::RegistrySyncReady { session_id, digest },
            ) if self.session_id.as_deref() == Some(session_id.as_str()) && is_digest(&digest) => {
                if remote_digest.is_some() {
                    return Err("registry_sync_ready_replayed".into());
                }
                if let AdmissionPhase::Exchanging { remote_digest, .. } = &mut self.phase {
                    *remote_digest = Some(digest);
                }
                Vec::new()
            }
            (AdmissionPhase::Ready, LanSyncMessage::RegistryEventUpdate { session_id, event })
                if self.session_id.as_deref() == Some(session_id.as_str()) =>
            {
                let result = storage
                    .apply_device_registry_events(
                        self.peer_device_id.as_str(),
                        std::slice::from_ref(&event),
                    )
                    .await?;
                applied_events.extend(result.applied_events);
                close_session |= !result.sender_is_admitted;
                close_session |= !result.local_device_is_admitted;
                local_device_is_admitted = result.local_device_is_admitted;
                close_session |= result.disabled_device_ids.iter().any(|device_id| {
                    device_id == self.local_device_id.as_str()
                        || device_id == self.peer_device_id.as_str()
                });
                disabled_device_ids.extend(result.disabled_device_ids);
                Vec::new()
            }
            _ => return Err("registry_admission_unexpected_control".into()),
        };
        self.promote_if_ready()?;
        let exchange_complete = matches!(self.phase, AdmissionPhase::Ready);
        self.session_revoked |= close_session;
        Ok(AdmissionUpdate {
            outgoing,
            became_ready: exchange_complete && !self.session_revoked && !was_ready,
            exchange_complete,
            close_session: self.session_revoked,
            local_device_is_admitted,
            disabled_device_ids,
            applied_events,
        })
    }

    fn transcript(
        &self,
        session_id: &str,
        initiator_nonce: &str,
        responder_nonce: &str,
    ) -> Result<RegistryHandshakeTranscript, String> {
        let (initiator_device_id, responder_device_id) =
            if matches!(self.phase, AdmissionPhase::WaitingForProof { .. }) {
                (self.local_device_id.clone(), self.peer_device_id.clone())
            } else {
                (self.peer_device_id.clone(), self.local_device_id.clone())
            };
        RegistryHandshakeTranscript::new(RegistryHandshakeTranscriptDraft {
            session_id: session_id.to_owned(),
            space_id: self.space_id.clone(),
            initiator_device_id,
            responder_device_id,
            initiator_nonce: initiator_nonce.to_owned(),
            responder_nonce: responder_nonce.to_owned(),
            protocol_version: self.protocol.version,
            capabilities: self.capability_ids(),
        })
        .map_err(|error| error.to_string())
    }

    async fn begin_exchange(
        &mut self,
        storage: &dyn SyncRuntimeStorage,
        session_id: &str,
    ) -> Result<Vec<LanSyncMessage>, String> {
        if self.local_device_id < self.peer_device_id {
            let mut messages = vec![LanSyncMessage::RegistrySyncStart {
                session_id: session_id.to_owned(),
            }];
            messages.extend(self.load_pages(storage, session_id).await?);
            Ok(messages)
        } else {
            Ok(Vec::new())
        }
    }

    async fn load_pages(
        &self,
        storage: &dyn SyncRuntimeStorage,
        session_id: &str,
    ) -> Result<Vec<LanSyncMessage>, String> {
        let (high_watermark, total_events, first_page) = storage
            .load_device_registry_events_page(0, None, PAGE_SIZE)
            .await?;
        let page_count = total_events
            .saturating_add(PAGE_SIZE - 1)
            .checked_div(PAGE_SIZE)
            .unwrap_or_default()
            .max(1);
        let page_count =
            u32::try_from(page_count).map_err(|_| "registry_sync_page_count_overflow")?;
        if page_count > MAX_PAGE_COUNT {
            return Err("registry_sync_page_count_too_large".into());
        }
        let mut after_position = 0_i64;
        let mut messages = Vec::with_capacity(page_count as usize);
        for page_index in 0..page_count {
            let events = if page_index == 0 {
                first_page.clone()
            } else {
                storage
                    .load_device_registry_events_page(
                        after_position,
                        Some(high_watermark),
                        PAGE_SIZE,
                    )
                    .await?
                    .2
            };
            if events.len() > PAGE_SIZE
                || (page_index + 1 < page_count && events.len() != PAGE_SIZE)
            {
                return Err("registry_sync_snapshot_changed_or_invalid".into());
            }
            after_position = after_position
                .checked_add(
                    i64::try_from(events.len()).map_err(|_| "registry_sync_page_size_overflow")?,
                )
                .ok_or("registry_sync_cursor_overflow")?;
            messages.push(LanSyncMessage::RegistryEventsPage {
                session_id: session_id.to_owned(),
                page_index,
                page_count,
                events,
            });
        }
        Ok(messages)
    }

    fn capability_ids(&self) -> Vec<String> {
        self.protocol
            .capabilities
            .iter()
            .map(|capability| capability.wire_id().to_owned())
            .collect()
    }

    fn promote_if_ready(&mut self) -> Result<(), String> {
        let AdmissionPhase::Exchanging {
            local_digest: Some(local),
            remote_digest: Some(remote),
            ..
        } = &self.phase
        else {
            return Ok(());
        };
        if local != remote {
            return Err("registry_sync_digest_mismatch".into());
        }
        self.phase = AdmissionPhase::Ready;
        Ok(())
    }
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
