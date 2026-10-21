use std::sync::Arc;

use ark_core::iroh_transport::IrohTransport;
use ark_core::relay_transport::RelayTransport;
use ark_core::sync_transport::{
    IrohSessionTransport, LanSyncMessage, PeerSessionControl, SyncTransport, SyncTransportHandle,
};

#[test]
fn transport_handle_and_adapters_share_core_contracts() {
    let _: fn(&SyncTransportHandle, LanSyncMessage) -> Result<(), String> =
        SyncTransportHandle::send;
    let _: fn(&SyncTransportHandle) -> Option<Arc<dyn PeerSessionControl>> =
        SyncTransportHandle::peer_control;
    let _: fn(Arc<dyn IrohSessionTransport>, Arc<dyn PeerSessionControl>) -> SyncTransportHandle =
        SyncTransportHandle::iroh_with_peer_control;

    fn assert_iroh<T: IrohSessionTransport + PeerSessionControl>() {}
    fn assert_transport<T: SyncTransport>() {}
    assert_iroh::<IrohTransport>();
    assert_transport::<RelayTransport>();
}
