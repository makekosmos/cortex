#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod client_session;
pub mod committed_feed_delivery;
pub mod inbound_live_change;
pub mod inbound_snapshot;
pub mod outbound_snapshot;
pub mod protocol;
pub mod protocol_negotiation;
pub mod registry_admission;
pub mod relay_registry;
pub mod server_session;
pub mod session_io;
mod sync_client;
pub mod task_merge_delivery;
mod task_set;
pub mod transport;
pub mod transport_select;

pub use ark_application::sync_storage::SyncRuntimeStorage;
pub use ark_domain::integration_replication::SignedSyncEnvelope;
#[doc(hidden)]
pub use sync_client::{
    acknowledge_client_feed, pump_client_committed_feed, ClientFeedInFlight, ClientMessageSender,
};
pub use sync_client::{
    ClientCallbackTaskRegistry, ClientOnChangeCallback, ClientOnConnectedCallback,
    ClientOnDisconnectedCallback, ClientOnPeerListCallback, ClientOnRegistryEventsCallback,
    DirectClient, DirectClientConfig,
};
