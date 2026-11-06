mod auth;
mod negotiation;
pub use auth::{
    compute_hello_auth_hmac, generate_auth_nonce, normalize_auth_secret, verify_hello_auth_hmac,
};
pub use negotiation::{
    compute_negotiation_offer_hmac, compute_negotiation_selection_hmac, negotiate_protocol,
    validate_protocol_selection, verify_negotiation_offer_hmac, verify_negotiation_selection_hmac,
    NegotiatedProtocol, ProtocolCapability, ProtocolNegotiationError, ProtocolOffer,
    ProtocolSelectionTranscript, MAX_PROTOCOL_CAPABILITIES, MAX_PROTOCOL_OFFER_VERSIONS,
    SUPPORTED_PROTOCOL_VERSIONS,
};
