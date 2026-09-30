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

use ark_core::db::{init_schema, SqliteStorageBackend};
use ark_core::protocol::{LanSyncMessage, PROTOCOL_VERSION};
use ark_core::relay_sync::{RelaySync, RelaySyncConfig};
use ark_core::sync_server::StorageBackend;
use ark_core::sync_transport::{SyncTransport, TransportEvent};

struct StubTransport {
    event_tx: StdMutex<Option<mpsc::UnboundedSender<TransportEvent>>>,
}

#[async_trait::async_trait]
impl SyncTransport for StubTransport {
    async fn start(&self, event_tx: mpsc::UnboundedSender<TransportEvent>) -> Result<(), String> {
        *self.event_tx.lock().unwrap() = Some(event_tx);
        Ok(())
    }
    fn send(&self, _msg: LanSyncMessage) -> Result<(), String> {
        Ok(())
    }
    fn stop(&self) {}
}

async fn relay_with_stub() -> (Arc<RelaySync>, mpsc::UnboundedSender<TransportEvent>) {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    let backend = Arc::new(SqliteStorageBackend::new(Arc::new(StdMutex::new(conn))));
    backend.set_device_id("me").unwrap();

    let transport = Arc::new(StubTransport {
        event_tx: StdMutex::new(None),
    });
    let relay = RelaySync::with_transport(
        backend as Arc<dyn StorageBackend>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "s".to_string(),
            device_id: "me".to_string(),
            device_name: "Me".to_string(),
            auth_secret: None,
        },
        transport.clone(),
    );
    relay.start().await.unwrap();

    let tx = transport
        .event_tx
        .lock()
        .unwrap()
        .clone()
        .expect("event_tx installed");
    (relay, tx)
}

async fn wait_for_peer(relay: &RelaySync) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while relay.get_connected_peer_entries().await.is_empty() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("peer never registered within 1 s");
}

fn send_hello(tx: &mpsc::UnboundedSender<TransportEvent>) {
    tx.send(TransportEvent::MessageReceived {
        from_device_id: "peer-x".to_string(),
        msg: LanSyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "peer-x".to_string(),
            device_name: "Peer X".to_string(),
            space_id: "s".to_string(),
            addresses: None,
            auth_nonce: None,
            auth_hmac: None,
        },
    })
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_dropped_evicts_all_peers() {
    let (relay, tx) = relay_with_stub().await;
    send_hello(&tx);
    wait_for_peer(&relay).await;
    assert_eq!(
        relay.get_connected_peer_entries().await,
        vec![("peer-x".to_string(), "Peer X".to_string())]
    );

    // RelayTransport emits TransportDropped when the multiplexed relay
    // socket drops (see relay_transport.rs reconnect loop). Before KOS-226
    // it emitted Disconnected{our own id}, which evicted nothing and left
    // stale authenticated peers — reconnects never re-fired on_peer_connect.
    tx.send(TransportEvent::TransportDropped).unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        relay.get_connected_peer_entries().await.is_empty(),
        "peers must be evicted on transport-level disconnect; got {:?}",
        relay.get_connected_peer_entries().await
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnect_with_own_device_id_is_ignored() {
    let (relay, tx) = relay_with_stub().await;
    send_hello(&tx);
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
        vec![("peer-x".to_string(), "Peer X".to_string())]
    );
}
