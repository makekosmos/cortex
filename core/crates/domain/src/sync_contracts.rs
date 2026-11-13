// Test fixtures use unwrap to fail immediately when setup is invalid.
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod client_transport;
pub mod peer_control;
pub mod protocol_negotiation;
pub mod session_io;
pub mod transport;
pub mod wire;
