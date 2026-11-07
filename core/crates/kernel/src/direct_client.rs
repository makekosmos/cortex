use std::sync::Arc;

use ::domain::sync_storage::SyncRuntimeStorage;
use ::domain::types::PeerRecord;

pub use ::sync::{DirectClient, DirectClientConfig};

#[cfg(test)]
#[path = "sync_client_start.rs"]
mod sync_client_start;

pub(crate) fn create_direct_client(
    storage: Arc<dyn SyncRuntimeStorage>,
    config: DirectClientConfig,
) -> DirectClient {
    DirectClient::new(
        storage,
        config,
        Arc::new(::net::websocket::DirectClientConnector),
    )
}

pub fn new_direct_client(
    storage: Arc<dyn SyncRuntimeStorage>,
    peer: PeerRecord,
    device_id: String,
    device_name: String,
    space_id: String,
    own_addresses: Vec<String>,
    auth_secret: Option<String>,
) -> DirectClient {
    let config = DirectClientConfig::new(
        peer,
        device_id,
        device_name,
        space_id,
        own_addresses,
        auth_secret,
    )
    .with_platform(
        Some(::net::host::local_platform()),
        ::net::host::app_version(),
    );
    create_direct_client(storage, config)
}
