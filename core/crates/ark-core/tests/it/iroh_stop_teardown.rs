#![allow(clippy::unwrap_used)]

//! KOS-369 regression: `IrohTransport::stop` must join every task it
//! spawned before returning. The authorization capability retains the store
//! while the transport is active; teardown must release it after joining
//! connection tasks so Windows can remove the temporary database directory.
//!
//! `current_thread` makes it deterministic: after a stop that never joins,
//! no background task has had a chance to release its authorizer clone, so
//! `Arc::strong_count` on the shared connection stays above one.

use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use rusqlite::Connection;
use tokio::sync::mpsc;

use ark_core::db::{init_schema, SqliteStorageBackend};
use ark_core::iroh_transport::{IrohConfig, IrohTransport};
use ark_core::sync_bind::SyncBind;
use ark_core::sync_transport::{SyncTransport, TransportEvent};
use ark_sync::protocol::LanSyncMessage;
use iroh::RelayMode;

fn loopback_transport(device_id: &str, peer_addr: Option<iroh::EndpointAddr>) -> IrohTransport {
    IrohTransport::new(IrohConfig {
        device_id: device_id.to_string(),
        device_name: device_id.to_string(),
        space_id: "test-iroh-stop-teardown".to_string(),
        secret_key: None,
        peer_addr,
        peer_ticket: None,
        relay_mode: Some(RelayMode::Disabled),
        auth_secret: None,
        bind: SyncBind::Loopback,
    })
}

fn open_conn(dir: &crate::support::TestTempDir) -> Arc<StdMutex<Connection>> {
    let conn = Arc::new(StdMutex::new(
        Connection::open(dir.path().join("ark.db")).unwrap(),
    ));
    init_schema(&conn.lock().unwrap()).unwrap();
    conn
}

/// A live connection between two transports; both sides' tasks hold the
/// storage. `stop()` must leave the test's own clone as the only reference.
#[tokio::test(flavor = "current_thread")]
async fn stop_joins_connection_tasks_and_releases_authorizer() {
    let dir_a = crate::support::temp_dir();
    let dir_b = crate::support::temp_dir();
    let conn_a = open_conn(&dir_a);
    let conn_b = open_conn(&dir_b);

    let (b_events_tx, mut b_events_rx) = mpsc::unbounded_channel::<TransportEvent>();
    let transport_b = loopback_transport("device-b", None);
    transport_b
        .set_outbound_authorizer(Arc::new(
            ark_sync::transport::StorageBackedOutboundIntegrationFrameAuthorizer::new(
                Arc::new(SqliteStorageBackend::for_test(conn_b.clone())),
                "test-iroh-stop-teardown",
                "device-b",
            ),
        ))
        .unwrap();
    transport_b.start(b_events_tx).await.unwrap();
    let addr_b = transport_b.endpoint_addr().unwrap();

    let (a_events_tx, _a_events_rx) = mpsc::unbounded_channel::<TransportEvent>();
    let transport_a = loopback_transport("device-a", Some(addr_b));
    transport_a
        .set_outbound_authorizer(Arc::new(
            ark_sync::transport::StorageBackedOutboundIntegrationFrameAuthorizer::new(
                Arc::new(SqliteStorageBackend::for_test(conn_a.clone())),
                "test-iroh-stop-teardown",
                "device-a",
            ),
        ))
        .unwrap();
    transport_a.start(a_events_tx).await.unwrap();

    // Wait for the handshake so both sides have live connection tasks.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, b_events_rx.recv()).await {
            Ok(Some(TransportEvent::MessageReceivedFromTransport {
                msg: LanSyncMessage::Hello { device_id, .. },
                ..
            })) if device_id == "device-a" => break,
            Ok(Some(_)) => {}
            other => panic!("B never saw A's Hello: {other:?}"),
        }
    }

    transport_a.stop().await;
    transport_b.stop().await;
    drop(transport_a);
    drop(transport_b);

    assert_eq!(
        Arc::strong_count(&conn_a),
        1,
        "a transport task still holds device A's ark.db connection"
    );
    assert_eq!(
        Arc::strong_count(&conn_b),
        1,
        "a transport task still holds device B's ark.db connection"
    );
}
