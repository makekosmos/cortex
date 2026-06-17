#![cfg(feature = "iroh-spike")]
#![allow(clippy::unwrap_used)]

//! Integration test (RED → GREEN): fully bidirectional iroh transport.
//!
//! Проверяет:
//! 1. Транспорт инжектирует `Hello` автоматически при подключении (без явного
//!    вызова `send(Hello)`).
//! 2. Обратное направление: listener (B, без `peer_ticket`) может отправить
//!    сообщение dialer-у (A) через долгоживущий bi-стрим входящего соединения.
//!
//! Зеркалит заголовочные соглашения `tests/iroh_ticket_pairing.rs`:
//! `#![cfg(feature="iroh-spike")]`, `RelayMode::Disabled`, `#[tokio::test(flavor="multi_thread")]`.

use std::time::Duration;

use tokio::sync::mpsc;

use ark_core::iroh_transport::{IrohConfig, IrohTransport};
use ark_core::protocol::{generate_id, LanSyncMessage};
use ark_core::sync_transport::{SyncTransport, TransportEvent};
use ark_core::types::SyncEntity;
use iroh::RelayMode;

/// Вспомогательная функция: дренирует события из `rx` до тех пор, пока не
/// найдёт `MessageReceived` с нужным условием, либо не истечёт таймаут.
/// Пропускает `Connected`/`Disconnected` и `MessageReceived` других типов.
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
                // Не то сообщение — продолжаем дренировать.
            }
            Ok(Some(_)) => {
                // Connected/Disconnected — игнорируем.
            }
            Ok(None) => return None, // канал закрыт
            Err(_) => return None,   // таймаут
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn iroh_bidirectional_hello_and_reverse_send() {
    // ── 1. B стартует первым как чистый listener (без peer_ticket). ──────────
    let (b_events_tx, mut b_events_rx) = mpsc::unbounded_channel::<TransportEvent>();

    let transport_b = IrohTransport::new(IrohConfig {
        device_id: "device-B".to_string(),
        device_name: "Device B".to_string(),
        space_id: "test-space-iroh-bidirectional".to_string(),
        secret_key: None,
        peer_addr: None,
        peer_ticket: None,
        relay_mode: Some(RelayMode::Disabled),
        auth_secret: None,
    });

    transport_b
        .start(b_events_tx)
        .await
        .expect("transport B start");

    let ticket_b = transport_b
        .our_ticket()
        .await
        .expect("transport B should produce a ticket after start()");

    // ── 2. A стартует с peer_ticket = B. Сохраняем канал событий A. ──────────
    let (a_events_tx, mut a_events_rx) = mpsc::unbounded_channel::<TransportEvent>();

    let transport_a = IrohTransport::new(IrohConfig {
        device_id: "device-A".to_string(),
        device_name: "Device A".to_string(),
        space_id: "test-space-iroh-bidirectional".to_string(),
        secret_key: None,
        peer_addr: None,
        peer_ticket: Some(ticket_b),
        relay_mode: Some(RelayMode::Disabled),
        auth_secret: None,
    });

    transport_a
        .start(a_events_tx)
        .await
        .expect("transport A start");

    // ── Assertion 1: автоматический Hello без явного send(). ─────────────────
    //
    // Транспорт ДОЛЖЕН инжектировать Hello как первый фрейм на подключении.
    // B должна получить Hello от A, A должна получить Hello от B.

    let (b_got_hello_from, _b_hello_msg) =
        find_message(&mut b_events_rx, Duration::from_secs(5), |_from, msg| {
            matches!(msg, LanSyncMessage::Hello { .. })
        })
        .await
        .expect("B must receive a Hello from A (auto-injected by transport) within 5 s");

    assert_eq!(
        b_got_hello_from, "device-A",
        "B должна получить Hello с from_device_id == device-A"
    );

    let (a_got_hello_from, _a_hello_msg) =
        find_message(&mut a_events_rx, Duration::from_secs(5), |_from, msg| {
            matches!(msg, LanSyncMessage::Hello { .. })
        })
        .await
        .expect("A must receive a Hello from B (auto-injected by transport) within 5 s");

    assert_eq!(
        a_got_hello_from, "device-B",
        "A должна получить Hello с from_device_id == device-B"
    );

    // ── Assertion 2: обратное направление — B отправляет, A получает. ─────────
    //
    // B НЕ имеет peer_ticket/peer_addr — она НЕ может dial. Поэтому send() от B
    // должен работать только через долгоживущий bi-стрим ВХОДЯЩЕГО соединения A.
    // Именно это подтверждает истинную двунаправленность.

    let change_id = format!("reverse-change-{}", generate_id());
    let entity = SyncEntity {
        entity_type: "todo".to_string(),
        id: format!("reverse-entity-{}", generate_id()),
        data: {
            let mut m = serde_json::Map::new();
            m.insert(
                "title".to_string(),
                serde_json::Value::String("B→A reverse direction test".to_string()),
            );
            m
        },
        hlc: "2026-06-17T00:00:00.000Z:000001:device-B".to_string(),
        deleted: None,
    };

    let live_change = LanSyncMessage::LiveChange {
        change_id: change_id.clone(),
        entity: entity.clone(),
        origin_device_id: None, // намеренно None — registry должен резолвить
    };

    transport_b
        .send(live_change)
        .expect("B send LiveChange to A");

    // A должна получить LiveChange от B через обратный стрим.
    let (a_got_from, a_msg) =
        find_message(&mut a_events_rx, Duration::from_secs(5), |_from, msg| {
            matches!(msg, LanSyncMessage::LiveChange { .. })
        })
        .await
        .expect("A must receive LiveChange from B within 5 s (reverse direction)");

    assert_eq!(
        a_got_from, "device-B",
        "from_device_id должен быть device-B (резолвится через реестр или Hello)"
    );

    match a_msg {
        LanSyncMessage::LiveChange {
            change_id: cid,
            entity: ent,
            ..
        } => {
            assert_eq!(cid, change_id, "change_id mismatch");
            assert_eq!(ent.id, entity.id, "entity.id mismatch");
        }
        other => panic!("expected LiveChange, got {:?}", other),
    }

    transport_a.stop();
    transport_b.stop();
}
