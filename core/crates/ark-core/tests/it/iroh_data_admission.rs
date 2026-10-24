#![allow(clippy::unwrap_used)]

use ark_core::sync_transport::IrohSessionTransport;
use std::time::Duration;

use ark_core::iroh_transport::{IrohConfig, IrohTransport};
use ark_core::sync_bind::SyncBind;
use ark_core::sync_transport::{SyncTransport, TransportEvent};
use ark_core::types::SyncEntity;
use ark_sync_contracts::wire::LanSyncMessage;
use iroh::RelayMode;
use tokio::sync::mpsc;

async fn receive_change(
    events: &mut mpsc::UnboundedReceiver<TransportEvent>,
    id: &str,
    timeout: Duration,
) -> bool {
    tokio::time::timeout(timeout, async {
        while let Some(event) = events.recv().await {
            if matches!(
                event,
                TransportEvent::MessageReceivedFromTransport {
                    msg: LanSyncMessage::LiveChange { entity, .. },
                    ..
                } if entity.id == id
            ) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false)
}

fn live_change(id: &str) -> LanSyncMessage {
    LanSyncMessage::LiveChange {
        policy_identity: None,
        change_id: format!("change-{id}"),
        entity: SyncEntity {
            entity_type: "object".to_string(),
            id: id.to_string(),
            data: serde_json::Map::from_iter([("typeId".into(), serde_json::json!("core.note"))]),
            hlc: "2026-10-16T00:00:00.000Z:000001:device-A".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        },
        origin_device_id: Some("device-A".to_string()),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn transport_blocks_outbound_and_inbound_data_until_peer_is_ready() {
    let (b_events_tx, mut b_events_rx) = mpsc::unbounded_channel();
    let transport_b = IrohTransport::new(IrohConfig {
        device_id: "device-B".into(),
        device_name: "Device B".into(),
        space_id: "admission-test".into(),
        secret_key: None,
        peer_addr: None,
        peer_ticket: None,
        relay_mode: Some(RelayMode::Disabled),
        auth_secret: None,
        bind: SyncBind::Loopback,
    });
    transport_b.start(b_events_tx).await.expect("start B");

    let (a_events_tx, mut a_events_rx) = mpsc::unbounded_channel();
    let transport_a = IrohTransport::new(IrohConfig {
        device_id: "device-A".into(),
        device_name: "Device A".into(),
        space_id: "admission-test".into(),
        secret_key: None,
        peer_addr: transport_b.endpoint_addr(),
        peer_ticket: None,
        relay_mode: Some(RelayMode::Disabled),
        auth_secret: None,
        bind: SyncBind::Loopback,
    });
    transport_a.start(a_events_tx).await.expect("start A");

    let hello_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while transport_a.registry().endpoint_id_for("device-B").is_none()
        || transport_b.registry().endpoint_id_for("device-A").is_none()
    {
        assert!(
            tokio::time::Instant::now() < hello_deadline,
            "Hello timeout"
        );
        let _ = tokio::time::timeout(Duration::from_millis(20), a_events_rx.recv()).await;
        let _ = tokio::time::timeout(Duration::from_millis(20), b_events_rx.recv()).await;
    }

    let endpoint_a = transport_a.endpoint_id().expect("endpoint A");
    let endpoint_b = transport_b.endpoint_id().expect("endpoint B");
    transport_a
        .bind_peer_for_control("device-B", &endpoint_b.to_string())
        .expect("bind B on A");
    transport_b
        .bind_peer_for_control("device-A", &endpoint_a.to_string())
        .expect("bind A on B");

    transport_b
        .mark_peer_data_ready("device-A", &endpoint_a.to_string())
        .expect("admit A on B");
    transport_a
        .send(live_change("outbound-pending"))
        .expect("send");
    assert!(
        !receive_change(&mut b_events_rx, "outbound-pending", Duration::from_secs(1)).await,
        "a bound but not admitted sender must not emit user data"
    );

    transport_a
        .mark_peer_data_ready("device-B", &endpoint_b.to_string())
        .expect("admit B on A");
    transport_a
        .send(live_change("outbound-ready"))
        .expect("send");
    assert!(
        receive_change(&mut b_events_rx, "outbound-ready", Duration::from_secs(5)).await,
        "data should flow after the sender is admitted"
    );

    transport_b.unbind_peer("device-A").unwrap();
    transport_b
        .bind_peer_for_control("device-A", &endpoint_a.to_string())
        .expect("rebind A on B");
    transport_a
        .send(live_change("inbound-pending"))
        .expect("send");
    assert!(
        !receive_change(&mut b_events_rx, "inbound-pending", Duration::from_secs(1)).await,
        "an unadmitted receiver must not forward user data to RelaySync"
    );

    transport_b
        .mark_peer_data_ready("device-A", &endpoint_a.to_string())
        .expect("readmit A on B");
    transport_a
        .send(live_change("inbound-ready"))
        .expect("send");
    assert!(
        receive_change(&mut b_events_rx, "inbound-ready", Duration::from_secs(5)).await,
        "data should reach the receiver after admission"
    );

    transport_a.stop().await;
    transport_b.stop().await;
}
