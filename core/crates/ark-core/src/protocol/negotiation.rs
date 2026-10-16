use std::collections::BTreeSet;

pub const SUPPORTED_PROTOCOL_VERSIONS: &[u32] = &[1];
pub const MAX_PROTOCOL_OFFER_VERSIONS: usize = 8;
pub const MAX_PROTOCOL_CAPABILITIES: usize = 32;
const MAX_CAPABILITY_ID_BYTES: usize = 64;

const DURABLE_RECEIPT_ACK_V1: &str = "durable_receipt_ack_v1";
const REGISTRY_HISTORY_V1: &str = "registry_history_v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProtocolCapability {
    DurableReceiptAckV1,
    RegistryHistoryV1,
}

impl ProtocolCapability {
    pub fn wire_id(self) -> &'static str {
        match self {
            Self::DurableReceiptAckV1 => DURABLE_RECEIPT_ACK_V1,
            Self::RegistryHistoryV1 => REGISTRY_HISTORY_V1,
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            DURABLE_RECEIPT_ACK_V1 => Some(Self::DurableReceiptAckV1),
            REGISTRY_HISTORY_V1 => Some(Self::RegistryHistoryV1),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolOffer {
    pub preferred_version: u32,
    pub supported_versions: BTreeSet<u32>,
    pub supported_capabilities: BTreeSet<String>,
}

impl ProtocolOffer {
    pub fn current(include_capabilities: bool) -> Self {
        Self {
            preferred_version: SUPPORTED_PROTOCOL_VERSIONS[0],
            supported_versions: SUPPORTED_PROTOCOL_VERSIONS.iter().copied().collect(),
            supported_capabilities: if include_capabilities {
                BTreeSet::from([DURABLE_RECEIPT_ACK_V1.to_owned()])
            } else {
                BTreeSet::new()
            },
        }
    }

    pub fn current_with_registry_history(include_capabilities: bool) -> Self {
        let mut offer = Self::current(include_capabilities);
        if include_capabilities {
            offer
                .supported_capabilities
                .insert(REGISTRY_HISTORY_V1.to_owned());
        }
        offer
    }

    pub fn new(
        preferred_version: u32,
        supported_versions: Option<&[u32]>,
        supported_capabilities: Option<&[String]>,
    ) -> Result<Self, ProtocolNegotiationError> {
        let versions = match supported_versions {
            Some(values) => {
                if values.is_empty() || values.len() > MAX_PROTOCOL_OFFER_VERSIONS {
                    return Err(ProtocolNegotiationError::InvalidVersionOffer);
                }
                let versions = values.iter().copied().collect::<BTreeSet<_>>();
                if versions.len() != values.len()
                    || versions.contains(&0)
                    || !versions.contains(&preferred_version)
                {
                    return Err(ProtocolNegotiationError::InvalidVersionOffer);
                }
                versions
            }
            None if preferred_version != 0 => BTreeSet::from([preferred_version]),
            None => return Err(ProtocolNegotiationError::InvalidVersionOffer),
        };

        let mut capabilities = BTreeSet::new();
        if let Some(values) = supported_capabilities {
            if values.len() > MAX_PROTOCOL_CAPABILITIES {
                return Err(ProtocolNegotiationError::InvalidCapabilityOffer);
            }
            for value in values {
                if value.is_empty()
                    || value.len() > MAX_CAPABILITY_ID_BYTES
                    || !value.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || byte == b'_'
                            || byte == b'.'
                            || byte == b'-'
                    })
                    || !capabilities.insert(value.clone())
                {
                    return Err(ProtocolNegotiationError::InvalidCapabilityOffer);
                }
            }
        }

        Ok(Self {
            preferred_version,
            supported_versions: versions,
            supported_capabilities: capabilities,
        })
    }

    fn canonical_versions(&self) -> Vec<u32> {
        self.supported_versions.iter().copied().collect()
    }

    pub fn version_ids(&self) -> Vec<u32> {
        self.canonical_versions()
    }

    fn canonical_capabilities(&self) -> Vec<String> {
        self.supported_capabilities.iter().cloned().collect()
    }

    pub fn capability_ids(&self) -> Vec<String> {
        self.canonical_capabilities()
    }

    pub fn selected_capability_ids(&self, other: &Self) -> Vec<String> {
        self.supported_capabilities
            .intersection(&other.supported_capabilities)
            .filter(|id| ProtocolCapability::parse(id).is_some())
            .cloned()
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NegotiatedProtocol {
    pub version: u32,
    pub capabilities: BTreeSet<ProtocolCapability>,
}

pub struct ProtocolSelectionTranscript<'a> {
    pub space_id: &'a str,
    pub initiator_device_id: &'a str,
    pub responder_device_id: &'a str,
    pub initiator_nonce: &'a str,
    pub responder_nonce: &'a str,
    pub initiator_offer: &'a ProtocolOffer,
    pub responder_offer: &'a ProtocolOffer,
    pub selection: &'a NegotiatedProtocol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolNegotiationError {
    InvalidVersionOffer,
    InvalidCapabilityOffer,
    NoCommonVersion,
    InvalidSelection,
    InvalidSelectionProof,
    MissingRegistryHistory,
}

impl ProtocolNegotiationError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidVersionOffer => "invalid_version_offer",
            Self::InvalidCapabilityOffer => "invalid_capability_offer",
            Self::NoCommonVersion => "upgrade_required",
            Self::InvalidSelection => "invalid_protocol_selection",
            Self::InvalidSelectionProof => "invalid_protocol_selection_proof",
            Self::MissingRegistryHistory => "upgrade_required_registry_history",
        }
    }
}

/// Validates the responder half of a Hello exchange. A response without any
/// negotiation fields is a legacy v1 peer and has no semantic capabilities.
#[allow(clippy::too_many_arguments)]
pub fn validate_protocol_selection(
    local_offer: &ProtocolOffer,
    selected_version: u32,
    remote_versions: Option<&[u32]>,
    remote_capabilities: Option<&[String]>,
    selected_capabilities: Option<&[String]>,
    secret: Option<&str>,
    space_id: &str,
    initiator_device_id: &str,
    responder_device_id: &str,
    initiator_nonce: Option<&str>,
    responder_nonce: Option<&str>,
    selection_hmac: Option<&str>,
) -> Result<NegotiatedProtocol, ProtocolNegotiationError> {
    if remote_versions.is_none()
        && remote_capabilities.is_none()
        && selected_capabilities.is_none()
        && selection_hmac.is_none()
    {
        if local_offer.supported_versions.contains(&selected_version)
            && SUPPORTED_PROTOCOL_VERSIONS.contains(&selected_version)
        {
            return Ok(NegotiatedProtocol {
                version: selected_version,
                capabilities: BTreeSet::new(),
            });
        }
        return Err(ProtocolNegotiationError::NoCommonVersion);
    }

    let remote_offer = ProtocolOffer::new(selected_version, remote_versions, remote_capabilities)?;
    let response_capabilities =
        selected_capabilities.ok_or(ProtocolNegotiationError::InvalidSelection)?;
    if response_capabilities.len() > MAX_PROTOCOL_CAPABILITIES
        || response_capabilities.iter().any(|id| {
            id.is_empty()
                || id.len() > MAX_CAPABILITY_ID_BYTES
                || !id.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'_'
                        || byte == b'.'
                        || byte == b'-'
                })
        })
    {
        return Err(ProtocolNegotiationError::InvalidCapabilityOffer);
    }
    let response_set = response_capabilities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if response_set.len() != response_capabilities.len() {
        return Err(ProtocolNegotiationError::InvalidSelection);
    }

    let selection = match (secret, selection_hmac) {
        (Some(secret), Some(proof)) => {
            let (Some(initiator_nonce), Some(responder_nonce)) = (initiator_nonce, responder_nonce)
            else {
                return Err(ProtocolNegotiationError::InvalidSelectionProof);
            };
            let selection = negotiate_protocol(local_offer, &remote_offer, true)?;
            let transcript = ProtocolSelectionTranscript {
                space_id,
                initiator_device_id,
                responder_device_id,
                initiator_nonce,
                responder_nonce,
                initiator_offer: local_offer,
                responder_offer: &remote_offer,
                selection: &selection,
            };
            if !verify_negotiation_selection_hmac(secret, &transcript, proof) {
                return Err(ProtocolNegotiationError::InvalidSelectionProof);
            }
            selection
        }
        (None, Some(_)) => return Err(ProtocolNegotiationError::InvalidSelectionProof),
        (_, None) => negotiate_protocol(local_offer, &remote_offer, false)?,
    };

    if selection.version != selected_version
        || response_set
            != selection
                .capabilities
                .iter()
                .map(|capability| capability.wire_id().to_owned())
                .collect::<BTreeSet<_>>()
    {
        return Err(ProtocolNegotiationError::InvalidSelection);
    }
    Ok(selection)
}

pub fn negotiate_protocol(
    local: &ProtocolOffer,
    remote: &ProtocolOffer,
    capabilities_authenticated: bool,
) -> Result<NegotiatedProtocol, ProtocolNegotiationError> {
    let version = local
        .supported_versions
        .intersection(&remote.supported_versions)
        .copied()
        .filter(|version| SUPPORTED_PROTOCOL_VERSIONS.contains(version))
        .max()
        .ok_or(ProtocolNegotiationError::NoCommonVersion)?;

    let common = local
        .supported_capabilities
        .intersection(&remote.supported_capabilities);
    // Registry support only selects the wire flow; data still stays closed
    // until signed history is verified. The durable receipt capability
    // changes persistence guarantees and remains HMAC-authenticated.
    let mut capabilities = common
        .filter_map(|id| ProtocolCapability::parse(id))
        .filter(|capability| {
            *capability == ProtocolCapability::RegistryHistoryV1 || capabilities_authenticated
        })
        .collect::<BTreeSet<_>>();
    if !capabilities_authenticated {
        capabilities.remove(&ProtocolCapability::DurableReceiptAckV1);
    }

    Ok(NegotiatedProtocol {
        version,
        capabilities,
    })
}

pub fn compute_negotiation_offer_hmac(
    secret: &str,
    space_id: &str,
    device_id: &str,
    nonce: &str,
    offer: &ProtocolOffer,
) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"ark-sync-v1:negotiation-offer\0");
    push_field(&mut bytes, space_id.as_bytes());
    push_field(&mut bytes, device_id.as_bytes());
    push_field(&mut bytes, nonce.as_bytes());
    push_offer(&mut bytes, offer);
    crate::protocol::auth::hmac_sha256_hex(secret.as_bytes(), &bytes)
}

pub fn verify_negotiation_offer_hmac(
    secret: &str,
    space_id: &str,
    device_id: &str,
    nonce: &str,
    offer: &ProtocolOffer,
    provided_hmac: &str,
) -> bool {
    let expected = compute_negotiation_offer_hmac(secret, space_id, device_id, nonce, offer);
    crate::protocol::auth::constant_time_eq(expected.as_bytes(), provided_hmac.as_bytes())
}

pub fn compute_negotiation_selection_hmac(
    secret: &str,
    transcript: &ProtocolSelectionTranscript<'_>,
) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"ark-sync-v1:negotiation-selection\0");
    push_field(&mut bytes, transcript.space_id.as_bytes());
    push_field(&mut bytes, transcript.initiator_device_id.as_bytes());
    push_field(&mut bytes, transcript.responder_device_id.as_bytes());
    push_field(&mut bytes, transcript.initiator_nonce.as_bytes());
    push_field(&mut bytes, transcript.responder_nonce.as_bytes());
    push_offer(&mut bytes, transcript.initiator_offer);
    push_offer(&mut bytes, transcript.responder_offer);
    bytes.extend_from_slice(&transcript.selection.version.to_be_bytes());
    let mut capabilities = transcript
        .selection
        .capabilities
        .iter()
        .map(|capability| capability.wire_id())
        .collect::<Vec<_>>();
    capabilities.sort_unstable();
    push_count(&mut bytes, capabilities.len());
    for capability in capabilities {
        push_field(&mut bytes, capability.as_bytes());
    }
    crate::protocol::auth::hmac_sha256_hex(secret.as_bytes(), &bytes)
}

pub fn verify_negotiation_selection_hmac(
    secret: &str,
    transcript: &ProtocolSelectionTranscript<'_>,
    provided_hmac: &str,
) -> bool {
    let expected = compute_negotiation_selection_hmac(secret, transcript);
    crate::protocol::auth::constant_time_eq(expected.as_bytes(), provided_hmac.as_bytes())
}

fn push_offer(bytes: &mut Vec<u8>, offer: &ProtocolOffer) {
    bytes.extend_from_slice(&offer.preferred_version.to_be_bytes());
    let versions = offer.canonical_versions();
    push_count(bytes, versions.len());
    for version in versions {
        bytes.extend_from_slice(&version.to_be_bytes());
    }
    let capabilities = offer.canonical_capabilities();
    push_count(bytes, capabilities.len());
    for capability in capabilities {
        push_field(bytes, capability.as_bytes());
    }
}

fn push_count(bytes: &mut Vec<u8>, count: usize) {
    bytes.extend_from_slice(&(count as u32).to_be_bytes());
}

fn push_field(bytes: &mut Vec<u8>, value: &[u8]) {
    push_count(bytes, value.len());
    bytes.extend_from_slice(value);
}

#[cfg(test)]
#[path = "negotiation/tests.rs"]
mod tests;
