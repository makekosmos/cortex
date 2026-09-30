#![allow(clippy::unwrap_used)]
#![cfg(feature = "iroh-spike")]
#![allow(clippy::unwrap_used)]

//! Integration test (GREEN stage, шаг 3): ticket-based pairing.
//!
//! Зеркалит `iroh_round_trip.rs`, но вместо передачи `EndpointAddr` напрямую
//! (как в фазе 0) device B стартует, печатает свой `our_ticket()` — единую
//! строку для pairing — и device A конфигурируется ТОЛЬКО этой строкой
//! (`peer_ticket`, без `peer_addr`). Дополнительно проверяем, что после
//! получения сообщения от A реестр device_id ↔ EndpointId на стороне B
//! содержит правильный CRDT device_id (через `from_device_id` в событии),
//! а не fallback/пустую строку — см. spec шага 3 в системном промпте задачи.
//!
//! Примечание: транспорт теперь автоматически инжектирует `Hello` при
//! подключении. Поэтому B может получить несколько событий до LiveChange.
//! Тест дренирует события до нужного типа вместо предположения о строгом
//! порядке. При этом ключевое требование СОХРАНЕНО: `from_device_id` у
//! `LiveChange` с `origin_device_id: None` должен резолвиться из реестра как
//! `"device-A"`, а не быть пустым/fallback — реестр заполняется auto-Hello.
//!
//! См. также: `.agent/tasks/2026-06-16-iroh-transport/spec.md`.

use std::time::Duration;

use tokio::sync::mpsc;

use ark_core::iroh_transport::{IrohConfig, IrohTransport};
use ark_core::protocol::LanSyncMessage;
use ark_core::sync_bind::SyncBind;
use ark_core::sync_transport::{SyncTransport, TransportEvent};
use iroh::RelayMode;

/// Дренирует события пока не найдёт `MessageReceived` с нужным `from_device_id`
/// и типом сообщения (определяется предикатом), либо не истечёт таймаут.
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
            Ok(Some(
                TransportEvent::MessageReceived {
                    from_device_id,
                    msg,
                }
                | TransportEvent::MessageReceivedFromTransport {
                    from_device_id,
                    msg,
                    ..
                },
            )) => {
                if predicate(&from_device_id, &msg) {
                    return Some((from_device_id, msg));
                }
                // Не тот тип/from — продолжаем дренировать.
            }
            Ok(Some(_)) => {} // Connected / Disconnected — игнорируем
            Ok(None) => return None,
            Err(_) => return None,
        }
    }
}

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
        auth_secret: None,
        bind: SyncBind::Loopback,
    });

    transport_b
        .start(b_events_tx)
        .await
        .expect("transport B start");

    // 2. B производит ticket-строку для самой себя.
    let ticket_b = transport_b
        .our_ticket()
        .await
        .expect("transport B should produce a ticket after start()");
    assert!(!ticket_b.is_empty(), "ticket string must not be empty");

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
        auth_secret: None,
        bind: SyncBind::Loopback,
    });

    transport_a
        .start(a_events_tx)
        .await
        .expect("transport A start");

    // 4. Ждём auto-инжектированного Hello от A (транспорт инжектирует его
    // при подключении, до любого явного send()). Это гарантирует, что реестр
    // device_id ↔ EndpointId заполнен до LiveChange.
    let (hello_from, _hello_msg) =
        find_message(&mut b_events_rx, Duration::from_secs(5), |from, msg| {
            from == "device-A" && matches!(msg, LanSyncMessage::Hello { .. })
        })
        .await
        .expect("B must receive Hello from A (auto-injected by transport) within 5s");

    assert_eq!(hello_from, "device-A", "hello must carry real device_id");

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
        origin_device_id: None,
        origin_seq: None,
    };

    let msg = LanSyncMessage::LiveChange {
        change_id: "change-iroh-ticket-test-001".to_string(),
        entity: test_entity,
        origin_device_id: None, // намеренно None — registry должен резолвить
    };

    transport_a.send(msg).expect("A send live_change");

    let (live_change_from, received) =
        find_message(&mut b_events_rx, Duration::from_secs(5), |from, msg| {
            from == "device-A"
                && matches!(msg, LanSyncMessage::LiveChange {
                    entity,
                    ..
                } if entity.id == "test-entity-iroh-ticket-001")
        })
        .await
        .expect("timed out waiting for iroh message");

    assert_eq!(
        live_change_from, "device-A",
        "registry must resolve real CRDT device_id, not empty/fallback"
    );
    match received {
        LanSyncMessage::LiveChange { entity, .. } => {
            assert_eq!(
                entity.id, "test-entity-iroh-ticket-001",
                "entity id mismatch"
            );
        }
        other => panic!("expected LiveChange, got {:?}", other),
    }

    transport_a.stop();
    transport_b.stop();
}
