#![cfg(feature = "iroh-spike")]
#![allow(clippy::unwrap_used)]

//! Integration test (RED stage): iroh p2p round-trip.
//!
//! Зеркалит `relay_round_trip.rs`, но без in-process "сервера" — оба узла
//! равноправны (p2p). Поднимает два `IrohTransport` в одном процессе на
//! loopback, отправляет `LanSyncMessage::LiveChange` с одного, ждёт его на
//! другом. На RED-стадии `IrohTransport::start`/`send` — `todo!()`, поэтому
//! тест должен СКОМПИЛИРОВАТЬСЯ и УПАСТЬ (panic от todo!), не пройти.
//!
//! См. spec: `.agent/tasks/2026-06-16-iroh-transport/spec.md` §6.

use std::time::Duration;

use tokio::sync::mpsc;

use ark_core::iroh_transport::{IrohConfig, IrohTransport};
use iroh::RelayMode;
use ark_core::protocol::LanSyncMessage;
use ark_core::sync_transport::{SyncTransport, TransportEvent};

#[tokio::test(flavor = "multi_thread")]
async fn iroh_round_trip() {
    // 1. B стартует первым — в реализации (GREEN) тест узнает её endpoint
    // addr через `endpoint_addr()` после `start()`. На RED-стадии `start()`
    // паникует через `todo!()` раньше, чем мы дойдём до отправки.
    let (b_events_tx, mut b_events_rx) = mpsc::unbounded_channel::<TransportEvent>();

    let transport_b = IrohTransport::new(IrohConfig {
        device_id: "device-B".to_string(),
        device_name: "Device B".to_string(),
        space_id: "test-space-iroh-round-trip".to_string(),
        secret_key: None,
        peer_addr: None,
        peer_ticket: None,
        // Offline loopback на одной машине — CI не должен зависеть от
        // сетевого доступа к production relay (см. iroh_transport.rs doc).
        relay_mode: Some(RelayMode::Disabled),
    });

    transport_b
        .start(b_events_tx)
        .await
        .expect("transport B start");

    let peer_addr_b = transport_b
        .endpoint_addr()
        .expect("transport B should expose its endpoint addr after start()");

    // 2. A стартует, зная адрес B напрямую (без discovery/pairing UI).
    let (a_events_tx, _a_events_rx) = mpsc::unbounded_channel::<TransportEvent>();

    let transport_a = IrohTransport::new(IrohConfig {
        device_id: "device-A".to_string(),
        device_name: "Device A".to_string(),
        space_id: "test-space-iroh-round-trip".to_string(),
        secret_key: None,
        peer_addr: Some(peer_addr_b),
        peer_ticket: None,
        relay_mode: Some(RelayMode::Disabled),
    });

    transport_a
        .start(a_events_tx)
        .await
        .expect("transport A start");

    // 3. A отправляет live_change с тестовой entity (тот же паттерн, что в
    // relay_round_trip.rs).
    let test_entity = ark_core::types::SyncEntity {
        entity_type: "todo".to_string(),
        id: "test-entity-iroh-001".to_string(),
        data: {
            let mut m = serde_json::Map::new();
            m.insert(
                "title".to_string(),
                serde_json::Value::String("Iroh test task".to_string()),
            );
            m
        },
        hlc: "2026-06-16T00:00:00.000Z:000001:device-A".to_string(),
        deleted: None,
    };

    let msg = LanSyncMessage::LiveChange {
        change_id: "change-iroh-test-001".to_string(),
        entity: test_entity.clone(),
        origin_device_id: Some("device-A".to_string()),
    };

    transport_a.send(msg).expect("A send");

    // 4. Ждём, что B получит MessageReceived в течение 5 секунд.
    let received = tokio::time::timeout(Duration::from_secs(5), b_events_rx.recv())
        .await
        .expect("timed out waiting for iroh message")
        .expect("channel closed");

    match received {
        TransportEvent::MessageReceived { from_device_id, msg } => {
            assert_eq!(from_device_id, "device-A");
            match msg {
                LanSyncMessage::LiveChange { entity, .. } => {
                    assert_eq!(entity.id, "test-entity-iroh-001", "entity id mismatch");
                    assert_eq!(entity.entity_type, "todo", "entity type mismatch");
                }
                other => panic!("expected LiveChange, got {:?}", other),
            }
        }
        other => panic!("expected MessageReceived, got {:?}", other),
    }

    transport_a.stop();
    transport_b.stop();
}
