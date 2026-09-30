#![allow(clippy::unwrap_used)]

//! Regression test for tokio::select! cancellation desync bug.
//!
//! BUG: In the original `handle_connection`, a single `select!` loop owned
//! BOTH the read half (via `read_frame`) AND the write half. `read_frame` is
//! NOT cancellation-safe: it does `recv.read_exact(len_buf)` then
//! `recv.read_exact(payload)`. When the `out_rx.recv()` branch fires WHILE
//! `read_frame` has already consumed the 4-byte length prefix, `select!` drops
//! the `read_frame` future — the prefix bytes are lost. The next iteration
//! interprets payload bytes as a length prefix → desync → bogus huge length →
//! error/MAX_FRAME_LEN exceeded → connection dies silently.
//!
//! This test forces a READ/WRITE OVERLAP by firing 50 multi-KB messages in
//! BOTH directions simultaneously, so a cancellation is near-certain. It
//! asserts that ALL messages sent by A arrive at B AND all sent by B arrive at
//! A (matched by unique change_id). On the buggy code this will fail (missing
//! messages or a desync error in stderr / timeout).
//!
//! Run with:
//!   cargo test --test iroh_bidirectional_burst

use std::collections::HashSet;
use std::time::Duration;

use tokio::sync::mpsc;

use ark_core::iroh_transport::{IrohConfig, IrohTransport};
use ark_core::protocol::{generate_id, LanSyncMessage};
use ark_core::sync_bind::SyncBind;
use ark_core::sync_transport::{SyncTransport, TransportEvent};
use ark_core::types::SyncEntity;
use iroh::RelayMode;

/// Number of messages each side sends. Must be large enough to reliably
/// overlap reads and writes in the select! loop.
const BURST_COUNT: usize = 200;

/// Payload size (bytes of entity data). Larger payloads make `read_exact` span
/// multiple polls, widening the cancellation window. ~32 KB per message.
const PAYLOAD_CHARS: usize = 32_768;

fn make_burst_message(prefix: &str, i: usize) -> (String, LanSyncMessage) {
    let change_id = format!("{prefix}-{i}-{}", generate_id());
    // Pad data field to force multi-poll reads.
    let padding = "x".repeat(PAYLOAD_CHARS);
    let entity = SyncEntity {
        entity_type: "burst-test".to_string(),
        id: format!("{prefix}-entity-{i}"),
        data: {
            let mut m = serde_json::Map::new();
            m.insert("payload".to_string(), serde_json::Value::String(padding));
            m.insert(
                "change_id".to_string(),
                serde_json::Value::String(change_id.clone()),
            );
            m
        },
        hlc: format!("2026-06-17T00:00:00.000Z:{i:06}:{prefix}"),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    };
    let msg = LanSyncMessage::LiveChange {
        change_id: change_id.clone(),
        entity,
        origin_device_id: Some(prefix.to_string()),
    };
    (change_id, msg)
}

/// Drain events from `rx` collecting all LiveChange `change_id`s until we have
/// `expected` count OR the deadline passes.
async fn collect_change_ids(
    rx: &mut mpsc::UnboundedReceiver<TransportEvent>,
    expected: usize,
    timeout: Duration,
) -> HashSet<String> {
    let mut found = HashSet::new();
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        if found.len() >= expected {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(
                TransportEvent::MessageReceived {
                    msg: LanSyncMessage::LiveChange { change_id, .. },
                    ..
                }
                | TransportEvent::MessageReceivedFromTransport {
                    msg: LanSyncMessage::LiveChange { change_id, .. },
                    ..
                },
            )) => {
                found.insert(change_id);
            }
            Ok(Some(_)) => {} // Hello / Connected / Disconnected — skip
            Ok(None) => break,
            Err(_) => break, // timeout
        }
    }
    found
}

#[tokio::test(flavor = "multi_thread")]
async fn iroh_bidirectional_burst_no_desync() {
    // ── 1. B starts first as a pure listener. ─────────────────────────────────
    let (b_events_tx, mut b_events_rx) = mpsc::unbounded_channel::<TransportEvent>();

    let transport_b = IrohTransport::new(IrohConfig {
        device_id: "device-B-burst".to_string(),
        device_name: "Device B Burst".to_string(),
        space_id: "test-space-iroh-burst".to_string(),
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

    let ticket_b = transport_b
        .our_ticket()
        .await
        .expect("transport B should produce a ticket after start()");

    // ── 2. A starts with peer_ticket = B. ─────────────────────────────────────
    let (a_events_tx, mut a_events_rx) = mpsc::unbounded_channel::<TransportEvent>();

    let transport_a = IrohTransport::new(IrohConfig {
        device_id: "device-A-burst".to_string(),
        device_name: "Device A Burst".to_string(),
        space_id: "test-space-iroh-burst".to_string(),
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

    // ── 3. Wait for the mutual Hello handshake before bursting. ───────────────
    // We need both connections to be established so the broadcast subscriber is
    // active on both sides before we fire the burst.
    let hello_timeout = Duration::from_secs(10);

    // Wait for B to receive A's Hello.
    let b_got_hello = {
        let deadline = tokio::time::Instant::now() + hello_timeout;
        let mut found = false;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, b_events_rx.recv()).await {
                Ok(Some(
                    TransportEvent::MessageReceived {
                        msg: LanSyncMessage::Hello { .. },
                        ..
                    }
                    | TransportEvent::MessageReceivedFromTransport {
                        msg: LanSyncMessage::Hello { .. },
                        ..
                    },
                )) => {
                    found = true;
                    break;
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        found
    };
    assert!(
        b_got_hello,
        "B must receive Hello from A within {hello_timeout:?}"
    );

    // Wait for A to receive B's Hello.
    let a_got_hello = {
        let deadline = tokio::time::Instant::now() + hello_timeout;
        let mut found = false;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, a_events_rx.recv()).await {
                Ok(Some(
                    TransportEvent::MessageReceived {
                        msg: LanSyncMessage::Hello { .. },
                        ..
                    }
                    | TransportEvent::MessageReceivedFromTransport {
                        msg: LanSyncMessage::Hello { .. },
                        ..
                    },
                )) => {
                    found = true;
                    break;
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        found
    };
    assert!(
        a_got_hello,
        "A must receive Hello from B within {hello_timeout:?}"
    );

    // ── 4. Build burst payloads. ───────────────────────────────────────────────
    let mut a_sent_ids = HashSet::new();
    let mut b_sent_ids = HashSet::new();

    let mut a_msgs = Vec::new();
    let mut b_msgs = Vec::new();

    for i in 0..BURST_COUNT {
        let (id, msg) = make_burst_message("device-A-burst", i);
        a_sent_ids.insert(id);
        a_msgs.push(msg);

        let (id, msg) = make_burst_message("device-B-burst", i);
        b_sent_ids.insert(id);
        b_msgs.push(msg);
    }

    // ── 5. Fire bursts from BOTH sides simultaneously. ─────────────────────────
    // Send all A messages interleaved with B messages to maximize read/write
    // overlap — the select! cancellation window is widened when both sides
    // are actively sending at the same time.
    for (a_msg, b_msg) in a_msgs.into_iter().zip(b_msgs.into_iter()) {
        transport_a.send(a_msg).expect("A send burst msg");
        transport_b.send(b_msg).expect("B send burst msg");
    }

    // ── 6. Collect and assert ALL messages arrived. ────────────────────────────
    let collect_timeout = Duration::from_secs(20);

    // B should receive everything A sent.
    let b_received = collect_change_ids(&mut b_events_rx, BURST_COUNT, collect_timeout).await;
    // A should receive everything B sent.
    let a_received = collect_change_ids(&mut a_events_rx, BURST_COUNT, collect_timeout).await;

    // Report missing for diagnostics.
    let b_missing: Vec<_> = a_sent_ids.difference(&b_received).collect();
    let a_missing: Vec<_> = b_sent_ids.difference(&a_received).collect();

    transport_a.stop();
    transport_b.stop();

    assert!(
        b_missing.is_empty(),
        "B is missing {}/{} messages from A: {:?}",
        b_missing.len(),
        BURST_COUNT,
        &b_missing[..b_missing.len().min(5)],
    );
    assert!(
        a_missing.is_empty(),
        "A is missing {}/{} messages from B: {:?}",
        a_missing.len(),
        BURST_COUNT,
        &a_missing[..a_missing.len().min(5)],
    );
}
