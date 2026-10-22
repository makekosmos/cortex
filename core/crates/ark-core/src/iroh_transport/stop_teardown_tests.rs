#![allow(clippy::unwrap_used)]

//! KOS-369 regression: `IrohTransport::stop` must join every task it
//! spawned before returning. The authorization capability retains the store
//! while the transport is active; teardown must release it after joining
//! connection tasks so Windows can remove the temporary database directory.
//!
//! `current_thread` makes it deterministic: after a stop that never joins,
//! no background task has had a chance to release its authorizer clone, so
//! `Arc::strong_count` on the storage backend stays above one.

use crate::replication_storage::IntegrationReplicationBackend;
use crate::sync_transport::IrohSessionTransport;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::db::SqliteStorageBackend;
use crate::iroh_transport::{IrohConfig, IrohTransport};
use crate::sync_bind::SyncBind;
use crate::sync_transport::{SyncTransport, TransportEvent};
use ark_sync_contracts::wire::LanSyncMessage;
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

async fn open_storage(
    device_id: &str,
) -> (
    ark_store::test_support::TestDatabase,
    Arc<SqliteStorageBackend>,
) {
    let database = ark_store::test_support::TestDatabase::create().unwrap();
    let backend = Arc::new(
        SqliteStorageBackend::open_for_test(
            database.path().to_str().unwrap().to_owned(),
            device_id,
        )
        .await
        .unwrap(),
    );
    backend.set_device_id(device_id).unwrap();
    (database, backend)
}

/// A live backend makes each transport retain its authorizer. `stop()` must
/// leave the test's own backend clone as the only reference.
#[tokio::test(flavor = "current_thread")]
async fn stop_joins_connection_tasks_and_releases_authorizer() {
    let (database_a, backend_a) = open_storage("device-a").await;
    let (database_b, backend_b) = open_storage("device-b").await;

    let (b_events_tx, mut b_events_rx) = mpsc::unbounded_channel::<TransportEvent>();
    let transport_b = loopback_transport("device-b", None);
    transport_b
        .set_outbound_authorizer(Arc::new(
            ark_sync::transport::StorageBackedOutboundIntegrationFrameAuthorizer::new(
                Arc::clone(&backend_b) as Arc<dyn IntegrationReplicationBackend>,
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
                Arc::clone(&backend_a) as Arc<dyn IntegrationReplicationBackend>,
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
        Arc::strong_count(&backend_a),
        1,
        "a transport task still holds device A's storage backend"
    );
    assert_eq!(
        Arc::strong_count(&backend_b),
        1,
        "a transport task still holds device B's storage backend"
    );

    backend_a
        .shutdown_for_test(std::time::Instant::now() + Duration::from_secs(5))
        .await
        .unwrap();
    backend_b
        .shutdown_for_test(std::time::Instant::now() + Duration::from_secs(5))
        .await
        .unwrap();

    drop(backend_a);
    drop(backend_b);
    drop(database_a);
    drop(database_b);
}
