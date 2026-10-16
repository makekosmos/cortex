mod negotiation;
pub use negotiation::{
    compute_negotiation_offer_hmac, compute_negotiation_selection_hmac, negotiate_protocol,
    validate_protocol_selection, verify_negotiation_offer_hmac, verify_negotiation_selection_hmac,
    NegotiatedProtocol, ProtocolCapability, ProtocolNegotiationError, ProtocolOffer,
    ProtocolSelectionTranscript, MAX_PROTOCOL_CAPABILITIES, MAX_PROTOCOL_OFFER_VERSIONS,
    SUPPORTED_PROTOCOL_VERSIONS,
};

include!("protocol/part01.rs");
include!("protocol/part02.rs");
include!("protocol/sync_policy.rs");
