#![cfg(feature = "iroh-spike")]
#![allow(clippy::unwrap_used)]

//! Integration test (RED stage, шаг 3): ticket-based pairing.
//!
//! Зеркалит `iroh_round_trip.rs`, но вместо передачи `EndpointAddr` напрямую
//! (как в фазе 0) device B стартует, печатает свой `our_ticket()` — единую
//! строку для pairing — и device A конфигурируется ТОЛЬКО этой строкой
//! (`peer_ticket`, без `peer_addr`). Дополнительно проверяем, что после
//! получения сообщения от A реестр device_id ↔ EndpointId на стороне B
//! содержит правильный CRDT device_id (через `from_device_id` в событии),
//! а не fallback/пустую строку — см. spec шага 3 в системном промпте задачи.
//!
//! См. также: `.agent/tasks/2026-06-16-iroh-transport/spec.md`.

use std::time::Duration;

use tokio::sync::mpsc;

use ark_core::iroh_transport::{IrohConfig, IrohTransport};
use iroh::RelayMode;
use ark_core::protocol::LanSyncMessage;
use ark_core::sync_transport::{SyncTransport, TransportEvent};

#[tokio::test(flavor = "multi_thread")]
async fn iroh_ticket_pairing_round_trip() {
    // 1. B стартует первым, как точка приёма.
    let (b_events_tx, mut b_events_rx) = mpsc::unbounded_channel::<TransportEvent>();

    let transport_b = IrohTransport::new(IrohConfig {
        device_id: "device-B".to_string(),
        device_name: "Device B".to_string(),
        space_id: "test-space-iroh-ticket-pairing".to_string(),
        secret_key: None,
        peer_addr: None,
        peer_ticket: None,
        relay_mode: Some(RelayMode::Disabled),
    });

    transport_b
        .start(b_events_tx)
        .await
        .expect("transport B start");

    // 2. B производит ticket-строку для самой себя — единственное, что нужно
    // передать пиру для pairing (вместо сырого EndpointAddr).
    let ticket_b = transport_b
        .our_ticket()
        .await
        .expect("transport B should produce a ticket after start()");
    assert!(
        !ticket_b.is_empty(),
        "ticket string must not be empty"
    );

    // 3. A стартует, зная B ТОЛЬКО через ticket-строку (без peer_addr).
    let (a_events_tx, _a_events_rx) = mpsc::unbounded_channel::<TransportEvent>();

    let transport_a = IrohTransport::new(IrohConfig {
        device_id: "device-A".to_string(),
        device_name: "Device A".to_string(),
        space_id: "test-space-iroh-ticket-pairing".to_string(),
        secret_key: None,
        peer_addr: None,
        peer_ticket: Some(ticket_b),
        relay_mode: Some(RelayMode::Disabled),
    });

    transport_a
        .start(a_events_tx)
        .await
        .expect("transport A start");

    // 4. A отправляет Hello сначала (так реестр device_id ↔ EndpointId на B
    // успевает заполниться до live_change — в реальном протоколе Hello идёт
    // первым сообщением сессии).
    let hello = LanSyncMessage::Hello {
        protocol_version: 1,
        device_id: "device-A".to_string(),
        device_name: "Device A".to_string(),
        space_id: "test-space-iroh-ticket-pairing".to_string(),
        addresses: None,
        auth_nonce: None,
        auth_hmac: None,
    };
    transport_a.send(hello).expect("A send hello");

    let received_hello = tokio::time::timeout(Duration::from_secs(5), b_events_rx.recv())
        .await
        .expect("timed out waiting for hello")
        .expect("channel closed");

    match received_hello {
        TransportEvent::MessageReceived { from_device_id, msg } => {
            assert_eq!(from_device_id, "device-A", "hello must carry real device_id");
            assert!(matches!(msg, LanSyncMessage::Hello { .. }));
        }
        other => panic!("expected MessageReceived(Hello), got {:?}", other),
    }

    // 5. A отправляет live_change БЕЗ origin_device_id — на этом сообщении
    // from_device_id обязан прийти из реестра (EndpointId пира → device_id из
    // Hello), а не из самого сообщения и не быть пустым/fallback.
    let test_entity = ark_core::types::SyncEntity {
        entity_type: "todo".to_string(),
        id: "test-entity-iroh-ticket-001".to_string(),
        data: {
            let mut m = serde_json::Map::new();
            m.insert(
                "title".to_string(),
                serde_json::Value::String("Iroh ticket pairing test task".to_string()),
            );
            m
        },
        hlc: "2026-06-16T00:00:00.000Z:000001:device-A".to_string(),
        deleted: None,
    };

    let msg = LanSyncMessage::LiveChange {
        change_id: "change-iroh-ticket-test-001".to_string(),
        entity: test_entity,
        origin_device_id: None,
    };

    transport_a.send(msg).expect("A send live_change");

    let received = tokio::time::timeout(Duration::from_secs(5), b_events_rx.recv())
        .await
        .expect("timed out waiting for iroh message")
        .expect("channel closed");

    match received {
        TransportEvent::MessageReceived { from_device_id, msg } => {
            assert_eq!(
                from_device_id, "device-A",
                "registry must resolve real CRDT device_id, not empty/fallback"
            );
            match msg {
                LanSyncMessage::LiveChange { entity, .. } => {
                    assert_eq!(entity.id, "test-entity-iroh-ticket-001", "entity id mismatch");
                }
                other => panic!("expected LiveChange, got {:?}", other),
            }
        }
        other => panic!("expected MessageReceived, got {:?}", other),
    }

    transport_a.stop();
    transport_b.stop();
}
