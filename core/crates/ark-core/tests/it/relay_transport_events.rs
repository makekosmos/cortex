#![allow(clippy::unwrap_used)]

//! Integration test: `RelaySync` peer-state handling for transport lifecycle
//! events.
//!
//! Regression coverage for KOS-226: a dropped transport-level socket (e.g.
//! the relay WebSocket) must evict all authenticated peers, and a per-peer
//! `Disconnected` carrying our own device id must evict nothing.

use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use rusqlite::Connection;
use tokio::sync::mpsc;

use ark_core::db::SqliteStorageBackend;
use ark_core::relay_sync::{RelaySync, RelaySyncConfig};
use ark_core::replication_storage::PeerIdentityBackend;
use ark_core::sync_server::SyncRuntimeStorage;
use ark_core::sync_transport::{IrohSessionTransport, SyncTransport, TransportEvent};
use ark_sync::protocol::{LanSyncMessage, PROTOCOL_VERSION};
use ark_sync::relay_registry::{PeerControlSendOutcome, PeerSessionControl};

struct StubTransport {
    event_tx: StdMutex<Option<mpsc::UnboundedSender<TransportEvent>>>,
}

// These tests start from an already-paired Hello before injecting lifecycle
// events, so the stub supplies the complete peer-session contract.
#[async_trait::async_trait]
impl SyncTransport for StubTransport {
    async fn start(&self, event_tx: mpsc::UnboundedSender<TransportEvent>) -> Result<(), String> {
        *self.event_tx.lock().unwrap() = Some(event_tx);
        Ok(())
    }
    fn send(&self, _msg: LanSyncMessage) -> Result<(), String> {
        Ok(())
    }

    async fn stop(&self) {}
}

#[async_trait::async_trait]
impl IrohSessionTransport for StubTransport {
    async fn send_to(&self, _: &str, _: LanSyncMessage) -> Result<(), String> {
        Ok(())
    }
    fn set_outbound_authorizer(
        &self,
        _: Arc<dyn ark_core::sync_transport::OutboundIntegrationFrameAuthorizer>,
    ) -> Result<(), String> {
        Ok(())
    }
    fn bind_peer_for_control(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn mark_peer_data_ready(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn unbind_peer(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    async fn send_to_transport_peer(
        &self,
        _: &str,
        _: LanSyncMessage,
    ) -> Result<ark_core::sync_transport::AddressedSendOutcome, String> {
        Ok(ark_core::sync_transport::AddressedSendOutcome::Sent)
    }
    fn disconnect_peer(
        &self,
        _: &str,
    ) -> Result<ark_sync::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync::transport::PeerDisconnectOutcome::PeerClosed)
    }
    fn disconnect_transport_peer(
        &self,
        _: &str,
    ) -> Result<ark_sync::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync::transport::PeerDisconnectOutcome::PeerClosed)
    }
}

#[async_trait::async_trait]
impl PeerSessionControl for StubTransport {
    async fn send_control_to_endpoint(
        &self,
        _: &str,
        _: LanSyncMessage,
    ) -> Result<PeerControlSendOutcome, String> {
        Ok(PeerControlSendOutcome::Sent)
    }
    fn mark_peer_data_ready(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn unbind_peer(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn disconnect_peer(
        &self,
        _: &str,
    ) -> Result<ark_sync::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync::transport::PeerDisconnectOutcome::PeerClosed)
    }
    fn disconnect_transport_endpoint(
        &self,
        _: &str,
    ) -> Result<ark_sync::transport::PeerDisconnectOutcome, String> {
        Ok(ark_sync::transport::PeerDisconnectOutcome::PeerClosed)
    }
}

async fn relay_with_stub() -> (
    Arc<RelaySync>,
    mpsc::UnboundedSender<TransportEvent>,
    String,
) {
    let conn = Connection::open_in_memory().unwrap();
    ark_store::open::init_schema(&conn).unwrap();
    let backend = Arc::new(SqliteStorageBackend::for_test(Arc::new(StdMutex::new(
        conn,
    ))));
    backend.set_device_id("me").unwrap();
    let peer_endpoint = iroh::SecretKey::generate().public().to_string();
    backend
        .enroll_paired_device("me", "peer-x", &peer_endpoint)
        .await
        .unwrap();

    let transport = Arc::new(StubTransport {
        event_tx: StdMutex::new(None),
    });
    let relay = RelaySync::with_iroh_transport(
        backend as Arc<dyn SyncRuntimeStorage>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "s".to_string(),
            device_id: "me".to_string(),
            device_name: "Me".to_string(),
            auth_secret: None,
        },
        transport.clone(),
    )
    .unwrap();
    relay.start().await.unwrap();

    let tx = transport
        .event_tx
        .lock()
        .unwrap()
        .clone()
        .expect("event_tx installed");
    (relay, tx, peer_endpoint)
}

/// Generous hang guard for event waits — the peer list state is the real
/// condition; the cap only bounds a wedged relay under load (KOS-308).
const EVENT_GUARD: Duration = Duration::from_secs(60);

async fn wait_for_peer(relay: &RelaySync) {
    tokio::time::timeout(EVENT_GUARD, async {
        while relay.get_connected_peer_entries().await.is_empty() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("peer never registered");
}

async fn wait_for_no_peers(relay: &RelaySync) {
    tokio::time::timeout(EVENT_GUARD, async {
        while !relay.get_connected_peer_entries().await.is_empty() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("peers never evicted");
}

fn send_hello(tx: &mpsc::UnboundedSender<TransportEvent>, peer_endpoint: &str) {
    let offer = ark_sync::protocol::ProtocolOffer::current_with_registry_history(true);
    tx.send(TransportEvent::MessageReceivedFromTransport {
        from_device_id: "peer-x".to_string(),
        transport_public_key: peer_endpoint.to_owned(),
        msg: LanSyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "peer-x".to_string(),
            device_name: "Peer X".to_string(),
            space_id: "s".to_string(),
            addresses: None,
            auth_nonce: None,
            auth_hmac: None,
            platform: None,
            app_version: None,
            supported_protocol_versions: Some(offer.version_ids()),
            supported_capabilities: Some(offer.capability_ids()),
            negotiated_capabilities: None,
            negotiation_hmac: None,
        },
    })
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_dropped_evicts_all_peers() {
    let (relay, tx, peer_endpoint) = relay_with_stub().await;
    send_hello(&tx, &peer_endpoint);
    wait_for_peer(&relay).await;
    assert_eq!(
        relay.get_connected_peer_entries().await,
        vec![ark_core::sync_server::PeerEntry {
            device_id: "peer-x".to_string(),
            device_name: "Peer X".to_string(),
            platform: None,
            app_version: None,
        }]
    );

    // RelayTransport emits TransportDropped when the multiplexed relay
    // socket drops (see relay_transport.rs reconnect loop). Before KOS-226
    // it emitted Disconnected{our own id}, which evicted nothing and left
    // stale authenticated peers — reconnects never re-fired on_peer_connect.
    tx.send(TransportEvent::TransportDropped).unwrap();
    // Wait on the eviction itself — a fixed sleep both flakes under load and
    // can pass before the event is even processed (KOS-308).
    wait_for_no_peers(&relay).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnect_with_own_device_id_is_ignored() {
    let (relay, tx, peer_endpoint) = relay_with_stub().await;
    send_hello(&tx, &peer_endpoint);
    wait_for_peer(&relay).await;

    // A Disconnected carrying our own device id (unresolvable remote, e.g.
    // iroh registry miss) must not evict the real peer.
    tx.send(TransportEvent::Disconnected {
        device_id: "me".to_string(),
    })
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        relay.get_connected_peer_entries().await,
        vec![ark_core::sync_server::PeerEntry {
            device_id: "peer-x".to_string(),
            device_name: "Peer X".to_string(),
            platform: None,
            app_version: None,
        }]
    );
}
