#![allow(clippy::unwrap_used)]
#![cfg(feature = "iroh-spike")]
#![allow(clippy::unwrap_used)]

//! Integration test (GREEN stage): iroh p2p round-trip.
//!
//! Зеркалит `relay_round_trip.rs`, но без in-process "сервера" — оба узла
//! равноправны (p2p). Поднимает два `IrohTransport` в одном процессе на
//! loopback, отправляет `LanSyncMessage::LiveChange` с одного, ждёт его на
//! другом.
//!
//! Примечание: транспорт теперь автоматически инжектирует `Hello` при
//! подключении. После `start()` диалер асинхронно устанавливает соединение и
//! только тогда подписывается на broadcast-канал. Поэтому тест сначала ждёт
//! подтверждения рукопожатия (Hello от A на стороне B), затем отправляет
//! LiveChange — это гарантирует, что broadcast-подписчик уже активен.
//!
//! См. spec: `.agent/tasks/2026-06-16-iroh-transport/spec.md` §6.

use std::time::Duration;

use tokio::sync::mpsc;

use ark_core::iroh_transport::{IrohConfig, IrohTransport};
use ark_core::protocol::LanSyncMessage;
use ark_core::sync_transport::{SyncTransport, TransportEvent};
use iroh::RelayMode;

/// Дренирует события до нужного типа или таймаута.
async fn find_message<F>(
    rx: &mut mpsc::UnboundedReceiver<TransportEvent>,
    timeout: Duration,
    predicate: F,
) -> Option<(String, LanSyncMessage)>
where
    F: Fn(&str, &LanSyncMessage) -> bool,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TransportEvent::MessageReceived {
                from_device_id,
                msg,
            })) => {
                if predicate(&from_device_id, &msg) {
                    return Some((from_device_id, msg));
                }
            }
            Ok(Some(_)) => {} // Connected / Disconnected — пропускаем
            Ok(None) => return None,
            Err(_) => return None,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn iroh_round_trip() {
    // 1. B стартует первым — в реализации (GREEN) тест узнает её endpoint
    // addr через `endpoint_addr()` после `start()`.
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
        auth_secret: None,
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
        auth_secret: None,
    });

    transport_a
        .start(a_events_tx)
        .await
        .expect("transport A start");

    // 3. Ждём подтверждения рукопожатия: B должна получить auto-Hello от A.
    // Это гарантирует, что dial-соединение A установлено и broadcast-подписчик
    // (handle_connection задача A) уже активен и готов принимать send().
    find_message(&mut b_events_rx, Duration::from_secs(5), |from, msg| {
        from == "device-A" && matches!(msg, LanSyncMessage::Hello { .. })
    })
    .await
    .expect("B must receive Hello from A (auto-injected) within 5s — connection not established");

    // 4. A отправляет live_change с тестовой entity.
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
        origin_device_id: None,
        origin_seq: None,
    };

    let msg = LanSyncMessage::LiveChange {
        change_id: "change-iroh-test-001".to_string(),
        entity: test_entity.clone(),
        origin_device_id: Some("device-A".to_string()),
    };

    transport_a.send(msg).expect("A send");

    // 5. Дренируем события B до нужного LiveChange.
    let (from_device_id, received) = find_message(
        &mut b_events_rx,
        Duration::from_secs(5),
        |from, msg| {
            from == "device-A"
                && matches!(msg, LanSyncMessage::LiveChange { entity, .. } if entity.id == "test-entity-iroh-001")
        },
    )
    .await
    .expect("timed out waiting for iroh LiveChange on B");

    assert_eq!(from_device_id, "device-A");
    match received {
        LanSyncMessage::LiveChange { entity, .. } => {
            assert_eq!(entity.id, "test-entity-iroh-001", "entity id mismatch");
            assert_eq!(entity.entity_type, "todo", "entity type mismatch");
        }
        other => panic!("expected LiveChange, got {:?}", other),
    }

    transport_a.stop();
    transport_b.stop();
}
