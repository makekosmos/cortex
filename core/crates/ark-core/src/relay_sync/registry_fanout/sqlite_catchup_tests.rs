use super::super::*;
use super::sqlite_test_support::*;
use crate::db::{AppendLocalRegistryEventCommand, CommandContext};
use crate::replication_storage::{PeerIdentityBackend, SyncRuntimeStorage};
use ark_domain::device_registry::{DeviceId, RegistryCommand};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::test(flavor = "multi_thread")]
async fn disabled_offline_device_reopens_sqlite_and_catches_up_before_data_admission() {
    let directory = crate::test_support::temp_dir();
    let path_a = directory.path().join("device-a.sqlite");
    let path_b = directory.path().join("device-b.sqlite");
    let mut peer_a = open_peer(&path_a, "device-A").await;
    let mut peer_b = open_peer(&path_b, "device-B").await;
    let key_a = peer_a.secret.clone();
    let key_b = peer_b.secret.clone();
    let key_b_bytes = key_b.to_bytes();
    let endpoint_a = key_a.public();
    let endpoint_b = key_b.public();

    peer_a
        .storage
        .enroll_paired_device("device-A", "device-B", &endpoint_b.to_string())
        .await
        .expect("enroll device B on A");
    peer_b
        .storage
        .enroll_paired_device("device-B", "device-A", &endpoint_a.to_string())
        .await
        .expect("enroll device A on B");

    let transport_a = Arc::new(crate::iroh_transport::IrohTransport::new(iroh_config(
        "device-A", key_a, None,
    )));
    let relay_a = RelaySync::with_iroh_transport(
        peer_a.storage.clone() as Arc<dyn SyncRuntimeStorage>,
        relay_config("device-A"),
        transport_a.clone(),
    )
    .expect("Iroh RelaySync construction");
    relay_a.start().await.expect("start A transport");

    let transport_b = Arc::new(crate::iroh_transport::IrohTransport::new(iroh_config(
        "device-B",
        key_b,
        transport_a.endpoint_addr(),
    )));
    let relay_b = RelaySync::with_iroh_transport(
        peer_b.storage.clone() as Arc<dyn SyncRuntimeStorage>,
        relay_config("device-B"),
        transport_b.clone(),
    )
    .expect("Iroh RelaySync construction");
    relay_b.start().await.expect("start B transport");
    wait_until("initial registry admission", || async {
        transport_a.registry().is_data_ready_endpoint(&endpoint_b)
            && transport_b.registry().is_data_ready_endpoint(&endpoint_a)
    })
    .await;

    relay_b.stop().await;
    peer_b
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down device B database actor");
    drop(relay_b);
    drop(transport_b);
    drop(peer_b);
    wait_until("A observes B disconnect", || async {
        !transport_a.registry().is_data_ready_endpoint(&endpoint_b)
    })
    .await;

    let disable = peer_a
        .db
        .append_local_registry_event(
            CommandContext::engine(),
            AppendLocalRegistryEventCommand::new(
                "device-A".into(),
                RegistryCommand::Disable {
                    target_device_id: DeviceId::parse("device-B").expect("valid device id"),
                },
            ),
        )
        .await
        .expect("append signed disable event")
        .expect("disable produces registry event");
    relay_a
        .publish_registry_event(disable.clone())
        .await
        .expect("a durable offline disable remains valid without a live recipient");

    peer_b = open_peer(&path_b, "device-B").await;
    assert_eq!(
        peer_b.secret.to_bytes(),
        key_b_bytes,
        "reopened database keeps the device transport identity"
    );

    let transport_b = Arc::new(crate::iroh_transport::IrohTransport::new(iroh_config(
        "device-B",
        peer_b.secret.clone(),
        transport_a.endpoint_addr(),
    )));
    let relay_b = RelaySync::with_iroh_transport(
        peer_b.storage.clone() as Arc<dyn SyncRuntimeStorage>,
        relay_config("device-B"),
        transport_b.clone(),
    )
    .expect("Iroh RelaySync construction");
    let tombstone_seen = Arc::new(AtomicBool::new(false));
    let tombstone_seen_callback = tombstone_seen.clone();
    let ready_when_tombstone_seen = Arc::new(AtomicBool::new(true));
    let ready_when_tombstone_seen_callback = ready_when_tombstone_seen.clone();
    let event_id = disable.event_id().to_owned();
    let transport_b_callback = transport_b.clone();
    let endpoint_a_callback = endpoint_a;
    relay_b
        .set_on_registry_events(Arc::new(move |events| {
            if events.iter().any(|event| event.event_id() == event_id) {
                ready_when_tombstone_seen_callback.store(
                    transport_b_callback
                        .registry()
                        .is_data_ready_endpoint(&endpoint_a_callback),
                    Ordering::SeqCst,
                );
                tombstone_seen_callback.store(true, Ordering::SeqCst);
            }
        }))
        .await;
    relay_b.start().await.expect("restart B transport");

    wait_until(
        "persisted disable tombstone on restarted device B",
        || async {
            peer_b
                .storage
                .load_device_registry_events_page(0, None, 64)
                .await
                .is_ok_and(|(_, _, events)| events.contains(&disable))
        },
    )
    .await;
    wait_until("disable callback", || async {
        tombstone_seen.load(Ordering::SeqCst)
    })
    .await;
    assert!(
        !ready_when_tombstone_seen.load(Ordering::SeqCst),
        "the disable tombstone must be applied before callback and before user-data admission"
    );
    assert!(
        !transport_b.registry().is_data_ready_endpoint(&endpoint_a),
        "disabled device B must never become data-ready after catch-up"
    );
    assert!(
        !transport_a.registry().is_data_ready_endpoint(&endpoint_b),
        "device A must not admit the disabled peer after reconnect"
    );

    relay_b.stop().await;
    relay_a.stop().await;
    peer_b
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down restarted B database actor");
    peer_a
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down A database actor");
}
